//! Shared native rendering and final-target input placement, including held grabs.
//! Smithay keeps the original target during a grab; its origin remains zero here.

#[cfg(test)]
#[path = "focus_keyboard_tests.rs"]
mod keyboard_tests;

use crate::Compositor;
use smithay::{
    backend::renderer::element::{
        Element,
        utils::{CropRenderElement, Relocate, RelocateRenderElement, RescaleRenderElement},
    },
    backend::renderer::utils::RendererSurfaceStateUserData,
    desktop::{PopupManager, Window},
    input::{
        Seat,
        pointer::{self, PointerTarget},
        touch::{self, TouchTarget},
    },
    reexports::wayland_server::{Resource, protocol::wl_surface::WlSurface},
    utils::{IsAlive, Logical, Physical, Point, Rectangle, Serial},
    wayland::{
        compositor::{TraversalAction, with_surface_tree_downward},
        seat::WaylandFocus,
    },
};
use std::borrow::Cow;
use tarot_core::{Rect, WindowId};

pub type FittedElement<E> = CropRenderElement<RelocateRenderElement<RescaleRenderElement<E>>>;

/// Exact physical reference and target rectangles for shared Fit/CENTER placement.
#[derive(Clone, Copy, Debug)]
pub struct RenderGeometry {
    pub target: Rectangle<i32, Physical>,
    pub reference: Rectangle<i32, Physical>,
    pub opacity: f32,
    pub rotation: f32,
    pub rotation_center: Option<Point<f64, Physical>>,
    pub output_scale: f64,
}

impl RenderGeometry {
    pub fn new(
        reference: Rectangle<i32, Logical>,
        target: Rect,
        opacity: f32,
        output_scale: f64,
    ) -> Option<Self> {
        let target = Rectangle::new(
            (
                (f64::from(target.x) * output_scale).round() as i32,
                (f64::from(target.y) * output_scale).round() as i32,
            )
                .into(),
            (
                (f64::from(target.width) * output_scale).round() as i32,
                (f64::from(target.height) * output_scale).round() as i32,
            )
                .into(),
        );
        let reference = reference.to_physical_precise_round(output_scale);
        if target.is_empty() || reference.is_empty() {
            return None;
        }
        Some(Self {
            target,
            reference,
            opacity,
            rotation: 0.0,
            rotation_center: None,
            output_scale,
        })
    }

    fn factor(self) -> f64 {
        (f64::from(self.target.size.w) / f64::from(self.reference.size.w))
            .min(f64::from(self.target.size.h) / f64::from(self.reference.size.h))
    }

    pub fn render_location(self) -> Point<i32, Physical> {
        self.target.loc - self.reference.loc
    }

    fn fit_offset(self) -> Point<i32, Physical> {
        // Match pinned Smithay's Fit/CENTER operation order and single round.
        let reference = self.reference.to_f64().upscale(self.factor());
        let offset = self.reference.loc.to_f64() - reference.loc;
        let align: Point<f64, Physical> = (
            (f64::from(self.target.size.w) - reference.size.w) / 2.0,
            (f64::from(self.target.size.h) - reference.size.h) / 2.0,
        )
            .into();
        (offset + align).to_i32_round()
    }

    /// Fit the native tree using this window's geometry, then clip to the
    /// family card. Popup and decoration pixels may extend beyond the window.
    pub fn fit_elements<E: Element>(
        self,
        elements: impl IntoIterator<Item = E>,
        clip: Rectangle<i32, Physical>,
    ) -> impl Iterator<Item = FittedElement<E>> {
        let origin = self.render_location();
        let factor = self.factor();
        let offset = self.fit_offset();
        elements
            .into_iter()
            .map(move |element| RescaleRenderElement::from_element(element, origin, factor))
            .map(move |element| {
                RelocateRenderElement::from_element(element, offset, Relocate::Relative)
            })
            .filter_map(move |element| {
                CropRenderElement::from_element(element, self.output_scale, clip)
            })
    }

    /// Inverse of constrain_render_elements: scale around location, then round
    /// the alignment/reference offset once, exactly as pinned Smithay does.
    pub fn to_window(self, position: Point<f64, Logical>) -> Point<f64, Logical> {
        let (sin, cos) = f64::from(self.rotation).to_radians().sin_cos();
        let center: Point<f64, Physical> = self.rotation_center.unwrap_or_else(|| {
            (
                f64::from(self.target.loc.x) + f64::from(self.target.size.w) * 0.5,
                f64::from(self.target.loc.y) + f64::from(self.target.size.h) * 0.5,
            )
                .into()
        });
        let dx = position.x * self.output_scale - center.x;
        let dy = position.y * self.output_scale - center.y;
        let position: Point<f64, Logical> = (
            (center.x + cos * dx + sin * dy) / self.output_scale,
            (center.y - sin * dx + cos * dy) / self.output_scale,
        )
            .into();
        let factor = self.factor();
        let origin = self.render_location() + self.fit_offset();
        (
            (position.x * self.output_scale - f64::from(origin.x)) / (factor * self.output_scale),
            (position.y * self.output_scale - f64::from(origin.y)) / (factor * self.output_scale),
        )
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fitted_roots_match_pinned_geometry_source_and_opacity_at_both_scales() {
        use smithay::backend::renderer::{
            element::{
                Id, Kind,
                solid::SolidColorRenderElement,
                utils::{ConstrainAlign, ConstrainScaleBehavior, constrain_render_elements},
            },
            utils::CommitCounter,
        };
        for (scale, alpha) in [1.0, 2.0]
            .into_iter()
            .flat_map(|s| [1.0_f32, 0.5].into_iter().map(move |a| (s, a)))
        {
            for (loc, size) in [
                ((20, 30), (80, 120)),
                ((19, 37), (601, 997)),
                ((0, 0), (360, 652)),
            ] {
                let reference = Rectangle::new(loc.into(), size.into());
                let geometry = RenderGeometry::new(
                    reference,
                    Rect {
                        x: 61.25,
                        y: 139.25,
                        width: 237.5,
                        height: 430.5,
                    },
                    1.0,
                    scale,
                )
                .unwrap();
                let origin = geometry.render_location();
                let elements = [
                    Rectangle::new(origin + geometry.reference.loc, geometry.reference.size),
                    Rectangle::new(
                        origin
                            + Point::<i32, Logical>::from((loc.0 + 71, loc.1 + 61))
                                .to_physical_precise_round(scale),
                        (280, 200).into(),
                    ),
                ]
                .map(|rect| {
                    SolidColorRenderElement::new(
                        Id::new(),
                        rect,
                        CommitCounter::default(),
                        [0.2 * alpha, 0.4 * alpha, 0.6 * alpha, alpha],
                        Kind::Unspecified,
                    )
                });
                let expected: Vec<_> = constrain_render_elements(
                    elements.clone(),
                    origin,
                    geometry.target,
                    geometry.reference,
                    ConstrainScaleBehavior::Fit,
                    ConstrainAlign::CENTER,
                    scale,
                )
                .collect();
                let actual: Vec<_> = geometry.fit_elements(elements, geometry.target).collect();
                assert_eq!(expected.len(), actual.len());
                for (a, b) in actual.iter().zip(&expected) {
                    assert_eq!(a.geometry(scale.into()), b.geometry(scale.into()));
                    assert_eq!(a.src(), b.src());
                    assert_eq!(a.transform(), b.transform());
                    assert_eq!(a.alpha(), b.alpha());
                    assert_eq!(
                        a.opaque_regions(scale.into())
                            .into_iter()
                            .collect::<Vec<_>>(),
                        b.opaque_regions(scale.into())
                            .into_iter()
                            .collect::<Vec<_>>()
                    );
                    assert_eq!(
                        a.damage_since(scale.into(), None)
                            .into_iter()
                            .collect::<Vec<_>>(),
                        b.damage_since(scale.into(), None)
                            .into_iter()
                            .collect::<Vec<_>>()
                    );
                }
            }
        }
    }
    #[test]
    fn held_native_input_inverts_rotation_before_fit_and_buffer_origin() {
        let reference = Rectangle::new((20, 30).into(), (600, 1000).into());
        let target = Rect {
            x: 61.25,
            y: 139.25,
            width: 237.5,
            height: 430.5,
        };
        for scale in [1.0, 2.0] {
            let base = RenderGeometry::new(reference, target, 1.0, scale).unwrap();
            for (rotation, rotation_center) in
                [-5.0_f32, 0.0, 5.0].into_iter().flat_map(|rotation| {
                    [None, Some((217.5, 419.0).into())]
                        .into_iter()
                        .map(move |center| (rotation, center))
                })
            {
                let posed = RenderGeometry {
                    rotation,
                    rotation_center,
                    ..base
                };
                let original: Point<f64, Logical> = (100.0, 230.0).into();
                let center = rotation_center.unwrap_or_else(|| {
                    (
                        f64::from(base.target.loc.x) + f64::from(base.target.size.w) * 0.5,
                        f64::from(base.target.loc.y) + f64::from(base.target.size.h) * 0.5,
                    )
                        .into()
                });
                let cx = center.x / scale;
                let cy = center.y / scale;
                let (sin, cos) = f64::from(rotation).to_radians().sin_cos();
                let output = (
                    cx + cos * (original.x - cx) - sin * (original.y - cy),
                    cy + sin * (original.x - cx) + cos * (original.y - cy),
                )
                    .into();
                let actual = posed.to_window(output);
                let expected = base.to_window(original);
                assert!(
                    (actual.x - expected.x).abs() < 1e-8 && (actual.y - expected.y).abs() < 1e-8
                );
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceFocus {
    pub surface: WlSurface,
}

impl From<WlSurface> for SurfaceFocus {
    fn from(surface: WlSurface) -> Self {
        Self { surface }
    }
}

#[derive(Clone, Debug)]
pub struct SurfacePlacement {
    pub window: WindowId,
    pub direct: bool,
    pub origin: Point<f64, Logical>,
    pub tree_root: WlSurface,
}

impl SurfaceFocus {
    fn position(
        &self,
        data: &Compositor,
        global: Point<f64, Logical>,
    ) -> Option<Point<f64, Logical>> {
        let mut keyboard_placements = std::collections::HashMap::new();
        let placement = if let Some(placement) = data.presented_surfaces.get(&self.surface) {
            placement
        } else {
            // A newly committed keyboard can receive input before the next
            // render populates presented_surfaces. Its direct layer uses the
            // same committed tree and origin for hit-testing and rendering.
            // App/card transforms must still come from the presented scene.
            if !data.native_keyboard_visible() {
                return None;
            }
            let keyboard = data.keyboard.as_ref()?;
            if !self
                .surface
                .id()
                .same_client_as(&keyboard.layer.wl_surface().id())
            {
                return None;
            }
            let rect = data.layout.keyboard();
            cache_surface_tree(
                keyboard.layer.wl_surface(),
                (rect.x as i32, rect.y as i32).into(),
                keyboard.owner?,
                true,
                &mut keyboard_placements,
            );
            keyboard_placements.get(&self.surface)?
        };
        if placement.direct {
            return Some(global - placement.origin);
        }
        let geometry = data.presented_windows.get(&placement.window)?;
        Some(geometry.to_window(global) - placement.origin)
    }
}

/// Snapshot mapped surface origins with the rendered window transform. Popup and
/// subsurface movement can then update held input without changing grab ownership.
pub fn cache_surface_placements(
    window: &Window,
    id: WindowId,
    placements: &mut std::collections::HashMap<WlSurface, SurfacePlacement>,
) {
    if let Some(toplevel) = window.toplevel() {
        cache_surface_tree(toplevel.wl_surface(), (0, 0).into(), id, false, placements);
        for (popup, offset) in PopupManager::popups_for_surface(toplevel.wl_surface()) {
            cache_surface_tree(
                popup.wl_surface(),
                window.geometry().loc + offset - popup.geometry().loc,
                id,
                false,
                placements,
            );
        }
    }
}

pub fn cache_surface_tree(
    root: &WlSurface,
    origin: Point<i32, Logical>,
    id: WindowId,
    direct: bool,
    placements: &mut std::collections::HashMap<WlSurface, SurfacePlacement>,
) {
    with_surface_tree_downward(
        root,
        origin,
        |_, states, location| {
            // The traversal already holds the surface state lock. Use the
            // supplied states rather than re-entering with_states here.
            states
                .data_map
                .get::<RendererSurfaceStateUserData>()
                .and_then(|state| state.lock().unwrap().view())
                .map_or(TraversalAction::SkipChildren, |view| {
                    TraversalAction::DoChildren(*location + view.offset)
                })
        },
        |surface, states, location| {
            if let Some(view) = states
                .data_map
                .get::<RendererSurfaceStateUserData>()
                .and_then(|state| state.lock().unwrap().view())
            {
                placements.insert(
                    surface.clone(),
                    SurfacePlacement {
                        window: id,
                        direct,
                        origin: (*location + view.offset).to_f64(),
                        tree_root: root.clone(),
                    },
                );
            }
        },
        |_, _, _| true,
    );
}

impl IsAlive for SurfaceFocus {
    fn alive(&self) -> bool {
        self.surface.is_alive()
    }
}

impl WaylandFocus for SurfaceFocus {
    fn wl_surface(&self) -> Option<Cow<'_, WlSurface>> {
        Some(Cow::Borrowed(&self.surface))
    }
}

macro_rules! forward_pointer {
    ($($method:ident($event:ident)),* $(,)?) => {$(
        fn $method(&self, seat: &Seat<Compositor>, data: &mut Compositor, event: &pointer::$event) {
            PointerTarget::<Compositor>::$method(&self.surface, seat, data, event);
        }
    )*};
}

impl PointerTarget<Compositor> for SurfaceFocus {
    fn enter(&self, seat: &Seat<Compositor>, data: &mut Compositor, event: &pointer::MotionEvent) {
        if let Some(location) = self.position(data, event.location) {
            let event = pointer::MotionEvent {
                location,
                ..event.clone()
            };
            PointerTarget::<Compositor>::enter(&self.surface, seat, data, &event);
        }
    }

    fn motion(&self, seat: &Seat<Compositor>, data: &mut Compositor, event: &pointer::MotionEvent) {
        if let Some(location) = self.position(data, event.location) {
            let event = pointer::MotionEvent {
                location,
                ..event.clone()
            };
            PointerTarget::<Compositor>::motion(&self.surface, seat, data, &event);
        }
    }

    fn axis(&self, seat: &Seat<Compositor>, data: &mut Compositor, frame: pointer::AxisFrame) {
        PointerTarget::<Compositor>::axis(&self.surface, seat, data, frame);
    }

    fn frame(&self, seat: &Seat<Compositor>, data: &mut Compositor) {
        PointerTarget::<Compositor>::frame(&self.surface, seat, data);
    }

    fn leave(&self, seat: &Seat<Compositor>, data: &mut Compositor, serial: Serial, time: u32) {
        PointerTarget::<Compositor>::leave(&self.surface, seat, data, serial, time);
    }

    forward_pointer! {
        relative_motion(RelativeMotionEvent), button(ButtonEvent),
        gesture_swipe_begin(GestureSwipeBeginEvent), gesture_swipe_update(GestureSwipeUpdateEvent),
        gesture_swipe_end(GestureSwipeEndEvent), gesture_pinch_begin(GesturePinchBeginEvent),
        gesture_pinch_update(GesturePinchUpdateEvent), gesture_pinch_end(GesturePinchEndEvent),
        gesture_hold_begin(GestureHoldBeginEvent), gesture_hold_end(GestureHoldEndEvent),
    }
}

impl TouchTarget<Compositor> for SurfaceFocus {
    fn down(
        &self,
        seat: &Seat<Compositor>,
        data: &mut Compositor,
        event: &touch::DownEvent,
        seq: Serial,
    ) {
        if let Some(location) = self.position(data, event.location) {
            let event = touch::DownEvent { location, ..*event };
            TouchTarget::<Compositor>::down(&self.surface, seat, data, &event, seq);
        } else {
            // Never log the location: keyboard positions can reveal text.
            tracing::warn!("touch down omitted: target has no presented placement");
        }
    }

    fn motion(
        &self,
        seat: &Seat<Compositor>,
        data: &mut Compositor,
        event: &touch::MotionEvent,
        seq: Serial,
    ) {
        if let Some(location) = self.position(data, event.location) {
            let event = touch::MotionEvent { location, ..*event };
            TouchTarget::<Compositor>::motion(&self.surface, seat, data, &event, seq);
        }
    }

    fn up(
        &self,
        seat: &Seat<Compositor>,
        data: &mut Compositor,
        event: &touch::UpEvent,
        seq: Serial,
    ) {
        TouchTarget::<Compositor>::up(&self.surface, seat, data, event, seq);
    }

    fn frame(&self, seat: &Seat<Compositor>, data: &mut Compositor, seq: Serial) {
        TouchTarget::<Compositor>::frame(&self.surface, seat, data, seq);
    }

    fn cancel(&self, seat: &Seat<Compositor>, data: &mut Compositor, seq: Serial) {
        TouchTarget::<Compositor>::cancel(&self.surface, seat, data, seq);
    }

    fn shape(
        &self,
        seat: &Seat<Compositor>,
        data: &mut Compositor,
        event: &touch::ShapeEvent,
        seq: Serial,
    ) {
        TouchTarget::<Compositor>::shape(&self.surface, seat, data, event, seq);
    }

    fn orientation(
        &self,
        seat: &Seat<Compositor>,
        data: &mut Compositor,
        event: &touch::OrientationEvent,
        seq: Serial,
    ) {
        TouchTarget::<Compositor>::orientation(&self.surface, seat, data, event, seq);
    }
}
