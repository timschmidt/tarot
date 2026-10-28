//! Compositor-owned recovery. This renderer never receives a ShellModel, client
//! scene, credential, clipboard or ordinary-shell egui context.
use crate::Compositor;
use anyhow::{Context, Result};
use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            Bind, Frame, Offscreen, Renderer as _,
            element::{
                Kind,
                texture::{TextureRenderBuffer, TextureRenderElement},
            },
            gles::{GlesError, GlesTexture},
            glow::GlowRenderer,
        },
    },
    utils::{Logical, Physical, Point, SERIAL_COUNTER, Size, Transform},
};
use std::time::{Duration, Instant};

pub struct Renderer {
    context: egui::Context,
    painter: Option<egui_glow::Painter>,
    texture: Option<TextureRenderBuffer<GlesTexture>>,
    extent: Option<(Size<i32, Physical>, i32)>,
    generation: Option<u64>,
    repaint_at: Option<Instant>,
    warmup: u8,
    start: Instant,
}
impl Renderer {
    pub fn new() -> Self {
        Self {
            context: egui::Context::default(),
            painter: None,
            texture: None,
            extent: None,
            generation: None,
            repaint_at: None,
            warmup: 0,
            start: Instant::now(),
        }
    }
    pub fn update(
        &mut self,
        renderer: &mut GlowRenderer,
        physical: Size<i32, Physical>,
        scale: i32,
        generation: u64,
        events: Vec<egui::Event>,
        capture: Option<&std::path::Path>,
    ) -> Result<(bool, Duration, bool)> {
        let changed = self.extent != Some((physical, scale)) || self.generation != Some(generation);
        if !changed
            && events.is_empty()
            && self.repaint_at.is_none_or(|d| d > Instant::now())
            && capture.is_none()
        {
            return Ok((
                false,
                self.repaint_at.map_or(Duration::MAX, |d| {
                    d.saturating_duration_since(Instant::now())
                }),
                false,
            ));
        }
        if self.painter.is_none() {
            self.painter = Some(
                renderer
                    .with_context(|gl| egui_glow::Painter::new(gl.clone(), "", None, false))??,
            );
        }
        if changed {
            self.context = egui::Context::default();
            self.warmup = 0;
            let texture =
                renderer.create_buffer(Fourcc::Abgr8888, (physical.w, physical.h).into())?;
            self.texture = Some(TextureRenderBuffer::from_texture(
                renderer,
                texture,
                scale,
                Transform::Flipped180,
                None,
            ));
            self.extent = Some((physical, scale));
            self.generation = Some(generation);
        }
        let width = physical.w as f32 / scale as f32;
        let height = physical.h as f32 / scale as f32;
        tarot_ui::prepare_pointer_input(&self.context, &events);
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width, height),
            )),
            events,
            focused: true,
            time: Some(self.start.elapsed().as_secs_f64()),
            max_texture_side: Some(self.painter.as_ref().unwrap().max_texture_side()),
            ..Default::default()
        };
        input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(scale as f32);
        let mut retry = false;
        let output = self.context.run(input, |ctx| {
            retry = tarot_ui::lock_recovery::show(ctx, width, height);
        });
        let mut repaint = output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .map_or(Duration::MAX, |o| o.repaint_delay);
        if self.warmup < 2 {
            self.warmup += 1;
            repaint = Duration::ZERO;
        }
        self.repaint_at = if repaint < Duration::from_secs(60) {
            Instant::now().checked_add(repaint)
        } else {
            None
        };
        let primitives = self
            .context
            .tessellate(output.shapes, output.pixels_per_point);
        crate::renderer::paint_layer(
            renderer,
            self.painter.as_mut().unwrap(),
            self.texture.as_mut().unwrap(),
            physical,
            output.pixels_per_point,
            &primitives,
            &output.textures_delta,
        )?;
        let mut capture_error = false;
        if let Some(path) = capture
            && let Err(error) = self.capture(renderer, physical, path)
        {
            tracing::warn!(%error,"own recovery-view diagnostic capture failed");
            capture_error = true;
        }
        Ok((retry, repaint, capture_error))
    }
    fn capture(
        &mut self,
        renderer: &mut GlowRenderer,
        size: Size<i32, Physical>,
        path: &std::path::Path,
    ) -> Result<()> {
        use std::os::unix::fs::OpenOptionsExt;
        anyhow::ensure!(
            size.w <= 4096 && size.h <= 4096,
            "recovery capture size exceeds diagnostic bound"
        );
        let mut pixels = None;
        self.texture.as_mut().unwrap().render().draw(|texture| {
            let mut framebuffer = renderer.bind(texture)?;
            let mut frame = renderer.render(&mut framebuffer, size, Transform::Normal)?;
            frame.with_context(|_| {
                pixels = Some(crate::renderer::capture_bound(
                    self.painter.as_ref().unwrap(),
                    size,
                ));
            })?;
            let _completion = frame.finish()?;
            Ok::<_, GlesError>(Vec::new())
        })?;
        let mut pixels = pixels.context("no own-view readback")??;
        let stride = size.w as usize * 4;
        for y in 0..size.h as usize / 2 {
            let (top, bottom) = pixels.split_at_mut((size.h as usize - 1 - y) * stride);
            top[y * stride..(y + 1) * stride].swap_with_slice(&mut bottom[..stride]);
        }
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?;
        let mut encoder = png::Encoder::new(file, size.w as u32, size.h as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&pixels)?;
        Ok(())
    }
    pub fn element(&self) -> Option<TextureRenderElement<GlesTexture>> {
        self.texture.as_ref().map(|t| {
            TextureRenderElement::from_texture_render_buffer(
                (0.0, 0.0),
                t,
                None,
                None,
                None,
                Kind::Unspecified,
            )
        })
    }
    pub fn destroy(&mut self, renderer: &mut GlowRenderer) {
        if let Some(mut painter) = self.painter.take() {
            let _ = renderer.with_context(|_| painter.destroy());
        }
        self.texture = None;
    }
}

impl Compositor {
    fn recovery_pointer_motion(&mut self, pos: Point<f64, Logical>, time: u32) {
        if let Some(pointer) = self.seat.get_pointer() {
            pointer.motion(
                self,
                None,
                &smithay::input::pointer::MotionEvent {
                    location: pos,
                    serial: SERIAL_COUNTER.next_serial(),
                    time,
                },
            );
            pointer.frame(self);
        }
        if self.recovery_ready() {
            self.protection
                .recovery_events
                .push(egui::Event::PointerMoved(egui::pos2(
                    pos.x as f32,
                    pos.y as f32,
                )));
            self.redraw = true;
        }
    }
    fn recovery_button(&mut self, pos: Point<f64, Logical>, pressed: bool) {
        self.protection
            .recovery_events
            .push(egui::Event::PointerButton {
                pos: egui::pos2(pos.x as f32, pos.y as f32),
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        self.redraw = true;
    }
    /// Separate routing; ordinary shell gestures and client input never run here.
    pub fn process_recovery_pointer_touch<I: smithay::backend::input::InputBackend>(
        &mut self,
        event: &smithay::backend::input::InputEvent<I>,
    ) -> bool {
        use smithay::backend::input::{
            AbsolutePositionEvent, ButtonState, Event, InputEvent, PointerButtonEvent,
            PointerMotionEvent, TouchEvent,
        };
        use tarot_core::touch::Target;
        match event {
            InputEvent::PointerMotionAbsolute { event } => {
                self.recovery_pointer_motion(
                    self.oriented_position(event.position_transformed((1, 1).into())),
                    event.time_msec(),
                );
            }
            InputEvent::PointerMotion { event } => {
                if let Some(pointer) = self.seat.get_pointer() {
                    let delta = event.delta();
                    let (x, y) = self.output_geometry.orientation.map_delta(delta.x, delta.y);
                    let p = pointer.current_location() + Point::from((x, y));
                    let pos = (
                        p.x.clamp(0.0, self.layout.width as f64),
                        p.y.clamp(0.0, self.layout.height as f64),
                    )
                        .into();
                    self.recovery_pointer_motion(pos, event.time_msec());
                }
            }
            InputEvent::PointerButton { event } => {
                let code = event.button_code();
                let pressed = event.state() == ButtonState::Pressed;
                if pressed {
                    self.pointer_buttons.insert(code);
                } else {
                    self.pointer_buttons.remove(&code);
                }
                if self.suppressed_pointer_buttons.contains(&code) {
                    if !pressed {
                        self.suppressed_pointer_buttons.remove(&code);
                    }
                    return true;
                }
                if self.recovery_ready()
                    && code == 0x110
                    && let Some(pointer) = self.seat.get_pointer()
                {
                    self.recovery_button(pointer.current_location(), pressed);
                }
            }
            InputEvent::PointerAxis { .. } => {}
            InputEvent::TouchDown { event } => {
                let ready = self.recovery_ready();
                let route = self.touches.begin(i32::from(event.slot()), ready);
                if route.cancel_shell {
                    tarot_ui::cancel_pointer_input(&mut self.protection.recovery_events);
                    self.redraw = true;
                }
                if ready && route.target == Target::Shell {
                    let pos = self.oriented_position(event.position_transformed((1, 1).into()));
                    self.protection
                        .recovery_events
                        .push(egui::Event::PointerMoved(egui::pos2(
                            pos.x as f32,
                            pos.y as f32,
                        )));
                    self.protection.recovery_touch_position =
                        egui::pos2(pos.x as f32, pos.y as f32);
                    self.recovery_button(pos, true);
                }
            }
            InputEvent::TouchMotion { event } => {
                if self.recovery_ready()
                    && self.touches.target(i32::from(event.slot())) == Target::Shell
                {
                    let pos = self.oriented_position(event.position_transformed((1, 1).into()));
                    self.protection.recovery_touch_position =
                        egui::pos2(pos.x as f32, pos.y as f32);
                    self.protection
                        .recovery_events
                        .push(egui::Event::PointerMoved(egui::pos2(
                            pos.x as f32,
                            pos.y as f32,
                        )));
                    self.redraw = true;
                }
            }
            InputEvent::TouchUp { event } => {
                let target = self.touches.end(i32::from(event.slot()));
                if self.recovery_ready() && target == Target::Shell {
                    self.protection
                        .recovery_events
                        .push(egui::Event::PointerButton {
                            pos: self.protection.recovery_touch_position,
                            button: egui::PointerButton::Primary,
                            pressed: false,
                            modifiers: egui::Modifiers::NONE,
                        });
                    self.protection
                        .recovery_events
                        .push(egui::Event::PointerGone);
                    self.redraw = true;
                }
            }
            InputEvent::TouchCancel { .. } => {
                self.cancel_input_sequences();
                self.touches.reset();
            }
            _ => return false,
        }
        true
    }
}
