//! Native surface clipping. No app snapshots or wallpaper-painted corner covers.
use smithay::{
    backend::renderer::{
        Frame,
        element::{Element, Id, Kind, RenderElement, UnderlyingStorage},
        gles::{
            GlesError, GlesFrame, GlesPixelProgram, GlesRenderer, GlesTexProgram, Uniform,
            UniformName, UniformType, element::PixelShaderElement,
        },
        glow::{GlowFrame, GlowRenderer},
        utils::{CommitCounter, DamageSet, OpaqueRegions},
    },
    utils::{Buffer, Logical, Physical, Rectangle, Scale, Size, Transform},
};
use std::{
    borrow::BorrowMut,
    collections::{HashMap, VecDeque},
};
use tarot_core::{WindowId, motion::WindowVisual};

#[derive(Clone, Copy, Debug, PartialEq)]
struct Clip {
    rect: Rectangle<i32, Physical>,
    radius: f32,
    scene: Size<i32, Physical>,
}
struct Commit {
    native: CommitCounter,
    clip: Clip,
    counter: CommitCounter,
    used: bool,
    history: VecDeque<History>,
}
#[derive(Clone, Copy, Debug)]
struct History {
    counter: CommitCounter,
    native: CommitCounter,
    clip: Clip,
}
struct Shadow {
    element: PixelShaderElement,
    area: Rectangle<i32, Logical>,
    uniforms: Vec<Uniform<'static>>,
    alpha: f32,
    used: bool,
}
#[derive(Default)]
pub struct Cards {
    mask: Option<GlesTexProgram>,
    shadow: Option<GlesPixelProgram>,
    commits: HashMap<Id, Commit>,
    shadows: HashMap<WindowId, Shadow>,
}
impl Cards {
    pub fn begin(&mut self, renderer: &mut GlowRenderer) -> Result<(), GlesError> {
        if self.mask.is_none() {
            let gles: &mut GlesRenderer = renderer.borrow_mut();
            let mask = gles.compile_custom_texture_shader(
                include_str!("shaders/card-mask.frag"),
                &[
                    UniformName::new("tarot_card", UniformType::_4f),
                    UniformName::new("tarot_radius", UniformType::_1f),
                    UniformName::new("tarot_from_fb_x", UniformType::_3f),
                    UniformName::new("tarot_from_fb_y", UniformType::_3f),
                ],
            )?;
            let shadow = gles.compile_custom_pixel_shader(
                include_str!("shaders/card-shadow.frag"),
                &[
                    UniformName::new("tarot_card", UniformType::_4f),
                    UniformName::new("tarot_radius", UniformType::_1f),
                    UniformName::new("tarot_sigma", UniformType::_1f),
                    UniformName::new("tarot_offset", UniformType::_1f),
                    UniformName::new("tarot_pixel", UniformType::_1f),
                ],
            )?;
            self.mask = Some(mask);
            self.shadow = Some(shadow);
        }
        for entry in self.commits.values_mut() {
            entry.used = false;
        }
        for entry in self.shadows.values_mut() {
            entry.used = false;
        }
        Ok(())
    }
    pub fn end(&mut self) {
        self.commits.retain(|_, v| v.used);
        self.shadows.retain(|_, v| v.used);
    }
    pub fn mask<E: RenderElement<GlowRenderer>>(
        &mut self,
        inner: E,
        rect: Rectangle<i32, Physical>,
        radius: f32,
        scene: Size<i32, Physical>,
        scale: f64,
    ) -> Option<Masked<E>> {
        let radius = radius
            .max(0.0)
            .min(rect.size.w.min(rect.size.h) as f32 * 0.5);
        let clip = Clip {
            rect,
            radius,
            scene,
        };
        if radius > 0.001 && !rounded_intersects(rect, radius, inner.geometry(scale.into())) {
            return None;
        }
        let native = inner.current_commit();
        let entry = self.commits.entry(inner.id().clone()).or_insert(Commit {
            native,
            clip,
            counter: CommitCounter::default(),
            used: true,
            history: VecDeque::new(),
        });
        if entry.native != native || entry.clip != clip {
            entry.history.push_back(History {
                counter: entry.counter,
                native: entry.native,
                clip: entry.clip,
            });
            if entry.history.len() > 8 {
                entry.history.pop_front();
            }
            entry.counter.increment();
            entry.native = native;
            entry.clip = clip;
        }
        entry.used = true;
        Some(Masked {
            inner,
            clip,
            program: self.mask.as_ref().unwrap().clone(),
            commit: entry.counter,
            history: entry.history.clone(),
        })
    }
    pub fn shadow(
        &mut self,
        visual: WindowVisual,
        target: Rectangle<i32, Physical>,
        scale: f64,
    ) -> Option<PixelShaderElement> {
        if visual.corner_radius <= 0.001 {
            return None;
        }
        // Packet SVG's dy=6/stdDeviation=6 before card scale; alpha=.55.
        let material_scale = visual.corner_radius / tarot_core::design::CARD_RADIUS;
        let margin = (24.0 * material_scale).ceil().max(1.0);
        let left = (visual.rect.x - margin).floor() as i32;
        let top = (visual.rect.y - margin).floor() as i32;
        let right = (visual.rect.x + visual.rect.width + margin).ceil() as i32;
        let bottom = (visual.rect.y + visual.rect.height + margin).ceil() as i32;
        let area = Rectangle::new((left, top).into(), (right - left, bottom - top).into());
        let physical_area: Rectangle<i32, Physical> = area.to_physical_precise_round(scale);
        let origin = physical_area.loc;
        // PixelShaderElement's built-in `size` uses its logical area, even
        // when the destination is scaled. Keep material coordinates logical.
        let radius = visual
            .corner_radius
            .min(target.size.w.min(target.size.h) as f32 / scale as f32 * 0.5);
        let uniforms = vec![
            Uniform::new(
                "tarot_card",
                (
                    (f64::from(target.loc.x - origin.x) / scale) as f32,
                    (f64::from(target.loc.y - origin.y) / scale) as f32,
                    (f64::from(target.size.w) / scale) as f32,
                    (f64::from(target.size.h) / scale) as f32,
                ),
            ),
            Uniform::new("tarot_radius", radius),
            Uniform::new("tarot_sigma", 6.0 * material_scale),
            Uniform::new("tarot_offset", 6.0 * material_scale),
            Uniform::new("tarot_pixel", 1.0 / scale as f32),
        ];
        let entry = self.shadows.entry(visual.id).or_insert_with(|| Shadow {
            element: PixelShaderElement::new(
                self.shadow.as_ref().unwrap().clone(),
                area,
                None,
                visual.opacity,
                uniforms.clone(),
                Kind::Unspecified,
            ),
            area,
            uniforms: uniforms.clone(),
            alpha: visual.opacity,
            used: true,
        });
        // PixelShaderElement has no alpha setter; rebuild only for opacity.
        if entry.alpha != visual.opacity {
            entry.element = PixelShaderElement::new(
                self.shadow.as_ref().unwrap().clone(),
                area,
                None,
                visual.opacity,
                uniforms.clone(),
                Kind::Unspecified,
            );
        } else {
            if entry.area != area {
                entry.element.resize(area, None);
            }
            if entry.uniforms != uniforms {
                entry.element.update_uniforms(uniforms.clone());
            }
        }
        entry.area = area;
        entry.uniforms = uniforms;
        entry.alpha = visual.opacity;
        entry.used = true;
        Some(entry.element.clone())
    }
}

fn rounded_intersects(
    card: Rectangle<i32, Physical>,
    radius: f32,
    element: Rectangle<i32, Physical>,
) -> bool {
    // Test raster sample centers against the rounded rectangle expanded by
    // half a pixel for antialiasing. Fully clipped children must not count as
    // visible clients for idle-inhibitor accounting.
    if element.is_empty() {
        return false;
    }
    let x0 = element.loc.x as f32 + 0.5;
    let x1 = (element.loc.x + element.size.w) as f32 - 0.5;
    let y0 = element.loc.y as f32 + 0.5;
    let y1 = (element.loc.y + element.size.h) as f32 - 0.5;
    let dx = (card.loc.x as f32 + radius - x1)
        .max(x0 - (card.loc.x + card.size.w) as f32 + radius)
        .max(0.0);
    let dy = (card.loc.y as f32 + radius - y1)
        .max(y0 - (card.loc.y + card.size.h) as f32 + radius)
        .max(0.0);
    dx.hypot(dy) < radius + 0.5
}

#[derive(Clone, Debug)]
pub struct Masked<E> {
    inner: E,
    clip: Clip,
    program: GlesTexProgram,
    commit: CommitCounter,
    history: VecDeque<History>,
}
impl<E: Element> Element for Masked<E> {
    fn id(&self) -> &Id {
        self.inner.id()
    }
    fn current_commit(&self) -> CommitCounter {
        self.commit
    }
    fn src(&self) -> Rectangle<f64, Buffer> {
        self.inner.src()
    }
    fn geometry(&self, scale: Scale<f64>) -> Rectangle<i32, Physical> {
        self.inner.geometry(scale)
    }
    fn transform(&self) -> Transform {
        self.inner.transform()
    }
    fn alpha(&self) -> f32 {
        self.inner.alpha()
    }
    fn kind(&self) -> Kind {
        self.inner.kind()
    }
    fn damage_since(
        &self,
        scale: Scale<f64>,
        commit: Option<CommitCounter>,
    ) -> DamageSet<i32, Physical> {
        if commit == Some(self.commit) {
            DamageSet::default()
        } else if let Some(old) = self
            .history
            .iter()
            .find(|old| Some(old.counter) == commit && old.clip == self.clip)
        {
            self.inner.damage_since(scale, Some(old.native))
        } else {
            DamageSet::from_slice(&[Rectangle::from_size(self.geometry(scale).size)])
        }
    }
    fn opaque_regions(&self, scale: Scale<f64>) -> OpaqueRegions<i32, Physical> {
        let inner = self.inner.opaque_regions(scale);
        if self.clip.radius <= 0.001 {
            return inner;
        }
        // Conservative opaque cross: rounded/antialiased corners must blend.
        let card = self.clip.rect;
        let inset = (self.clip.radius.ceil() as i32 + 1).min(card.size.w.min(card.size.h) / 2);
        let bands = [
            Rectangle::new(
                (card.loc.x, card.loc.y + inset).into(),
                (card.size.w, card.size.h - 2 * inset).into(),
            ),
            Rectangle::new(
                (card.loc.x + inset, card.loc.y).into(),
                (card.size.w - 2 * inset, inset).into(),
            ),
            Rectangle::new(
                (card.loc.x + inset, card.loc.y + card.size.h - inset).into(),
                (card.size.w - 2 * inset, inset).into(),
            ),
        ];
        let origin = self.geometry(scale).loc;
        inner
            .iter()
            .flat_map(|r| {
                bands.iter().filter_map(move |b| {
                    let absolute = Rectangle::new(r.loc + origin, r.size);
                    absolute
                        .intersection(*b)
                        .filter(|r| !r.is_empty())
                        .map(|r| Rectangle::new(r.loc - origin, r.size))
                })
            })
            .collect()
    }
}
impl<E: RenderElement<GlowRenderer>> RenderElement<GlowRenderer> for Masked<E> {
    fn draw(
        &self,
        frame: &mut GlowFrame<'_, '_>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        opaque: &[Rectangle<i32, Physical>],
    ) -> Result<(), GlesError> {
        if self.clip.radius <= 0.001 {
            return self.inner.draw(frame, src, dst, damage, opaque);
        }
        {
            let gles: &mut GlesFrame<'_, '_> = frame.borrow_mut();
            let fb = gles.transformation().transform_size(self.clip.scene);
            let (x, y) = from_framebuffer(gles.projection(), fb);
            let r = self.clip.rect;
            gles.override_default_tex_program(
                self.program.clone(),
                vec![
                    Uniform::new(
                        "tarot_card",
                        (
                            r.loc.x as f32,
                            r.loc.y as f32,
                            r.size.w as f32,
                            r.size.h as f32,
                        ),
                    ),
                    Uniform::new("tarot_radius", self.clip.radius),
                    Uniform::new("tarot_from_fb_x", x),
                    Uniform::new("tarot_from_fb_y", y),
                ],
            );
        }
        let result = self.inner.draw(frame, src, dst, damage, opaque);
        let gles: &mut GlesFrame<'_, '_> = frame.borrow_mut();
        gles.clear_tex_program_override();
        result
    }
    fn underlying_storage(&self, renderer: &mut GlowRenderer) -> Option<UnderlyingStorage<'_>> {
        (self.clip.radius <= 0.001)
            .then(|| self.inner.underlying_storage(renderer))
            .flatten()
    }
}

fn from_framebuffer(m: &[f32; 9], fb: Size<i32, Physical>) -> ([f32; 3], [f32; 3]) {
    // Inverse of the actual frame projection, including its output rotation and
    // framebuffer flip. Fragment coordinates use GL's lower-left viewport.
    let (a, b, c, d, tx, ty) = (
        f64::from(m[0]),
        f64::from(m[3]),
        f64::from(m[1]),
        f64::from(m[4]),
        f64::from(m[6]),
        f64::from(m[7]),
    );
    let det = a * d - b * c;
    assert!(
        det.is_finite() && det != 0.0 && fb.w > 0 && fb.h > 0,
        "non-singular compositor projection"
    );
    (
        [
            (2.0 * d / (f64::from(fb.w) * det)) as f32,
            (-2.0 * b / (f64::from(fb.h) * det)) as f32,
            ((d * (-1.0 - tx) + b * (1.0 + ty)) / det) as f32,
        ],
        [
            (-2.0 * c / (f64::from(fb.w) * det)) as f32,
            (2.0 * a / (f64::from(fb.h) * det)) as f32,
            ((c * (1.0 + tx) - a * (1.0 + ty)) / det) as f32,
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fully_clipped_children_have_no_visible_render_element() {
        let card = Rectangle::new((0, 0).into(), (100, 100).into());
        assert!(!rounded_intersects(
            card,
            11.0,
            Rectangle::new((0, 0).into(), (2, 2).into())
        ));
        assert!(rounded_intersects(
            card,
            11.0,
            Rectangle::new((10, 0).into(), (2, 2).into())
        ));
        assert!(rounded_intersects(
            card,
            11.0,
            Rectangle::new((40, 40).into(), (20, 20).into())
        ));
        assert!(!rounded_intersects(
            card,
            11.0,
            Rectangle::new((-20, -20).into(), (2, 2).into())
        ));
    }
}
