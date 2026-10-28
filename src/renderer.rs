//! egui renders into an owned GLES texture, then joins native surface render elements.

use anyhow::Result;
use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            Bind, Frame, Offscreen, Renderer,
            element::{
                Kind,
                texture::{TextureRenderBuffer, TextureRenderElement},
            },
            gles::{GlesError, GlesTexture},
            glow::GlowRenderer,
        },
    },
    utils::{Physical, Rectangle, Size, Transform},
};
use std::time::{Duration, Instant};
use tarot_core::{Command, Layout, ShellModel};

pub struct ShellFrame<'a> {
    pub model: &'a ShellModel,
    pub layout: Layout,
    pub presentation: &'a tarot_core::motion::Presentation,
    pub fullscreen: bool,
    pub physical: Size<i32, Physical>,
    pub scale: i32,
    pub events: Vec<egui::Event>,
}

pub fn dmabuf_capabilities(
    renderer: &GlowRenderer,
) -> (Vec<smithay::backend::allocator::Format>, Option<u64>) {
    use smithay::backend::{egl::EGLDevice, renderer::ImportDma};
    use std::os::unix::fs::MetadataExt;
    let formats = renderer.dmabuf_formats().into_iter().collect();
    let device = EGLDevice::device_for_display(renderer.egl_context().display())
        .ok()
        .and_then(|device| {
            device
                .render_device_path()
                .or_else(|_| device.drm_device_path())
                .ok()
        })
        .and_then(|path| std::fs::metadata(path).ok())
        .filter(|metadata| metadata.rdev() != 0)
        .map(|metadata| metadata.rdev());
    (formats, device)
}

pub struct ShellRenderer {
    pub cards: crate::card_renderer::Cards,
    context: egui::Context,
    painter: Option<egui_glow::Painter>,
    texture: Option<TextureRenderBuffer<GlesTexture>>,
    wallpaper: Option<TextureRenderBuffer<GlesTexture>>,
    extent: Option<(Size<i32, Physical>, i32)>,
    start: Instant,
    recovery: Option<crate::recovery::Renderer>,
    cancelled_generation: Option<u64>,
}

impl ShellRenderer {
    pub fn new() -> Self {
        let context = egui::Context::default();
        context.set_visuals(egui::Visuals::dark());
        Self {
            cards: Default::default(),
            context,
            painter: None,
            texture: None,
            wallpaper: None,
            extent: None,
            start: Instant::now(),
            recovery: None,
            cancelled_generation: None,
        }
    }

    pub fn initialize(&mut self, renderer: &mut GlowRenderer) -> Result<()> {
        if self.painter.is_none() {
            self.painter = Some(renderer.with_context(|gl| {
                use glow::HasContext;
                // SAFETY: with_context makes this compositor-owned
                // context current on the rendering thread.
                unsafe {
                    tracing::info!(
                        vendor = %gl.get_parameter_string(glow::VENDOR),
                        renderer = %gl.get_parameter_string(glow::RENDERER),
                        version = %gl.get_parameter_string(glow::VERSION),
                        "shell graphics initialized"
                    );
                }
                egui_glow::Painter::new(gl.clone(), "", None, false)
            })??);
        }
        Ok(())
    }

    pub fn update(
        &mut self,
        renderer: &mut GlowRenderer,
        frame: ShellFrame<'_>,
    ) -> Result<(Vec<Command>, Duration)> {
        let ShellFrame {
            model,
            layout,
            presentation,
            fullscreen,
            physical,
            scale,
            events,
        } = frame;
        self.initialize(renderer)?;
        let painter = self.painter.as_mut().expect("painter initialized");
        let resized = self.extent != Some((physical, scale));
        if resized {
            let texture =
                renderer.create_buffer(Fourcc::Abgr8888, (physical.w, physical.h).into())?;
            self.texture = Some(TextureRenderBuffer::from_texture(
                renderer,
                texture,
                scale,
                Transform::Flipped180,
                None,
            ));
            let background =
                renderer.create_buffer(Fourcc::Abgr8888, (physical.w, physical.h).into())?;
            self.wallpaper = Some(TextureRenderBuffer::from_texture(
                renderer,
                background,
                scale,
                Transform::Flipped180,
                None,
            ));
            self.extent = Some((physical, scale));
        }
        tarot_ui::prepare_pointer_input(&self.context, &events);
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(layout.width, layout.height),
            )),
            time: Some(self.start.elapsed().as_secs_f64()),
            events,
            focused: true,
            max_texture_side: Some(painter.max_texture_side()),
            ..Default::default()
        };
        input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(scale as f32);
        let mut commands = Vec::new();
        let output = self.context.run(input, |ctx| {
            // Only retain the final pass. Effects are applied after Context::run.
            commands =
                tarot_ui::show_with_native_fullscreen(ctx, model, layout, presentation, fullscreen);
        });
        let repaint = output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .map(|view| view.repaint_delay)
            .unwrap_or(Duration::MAX);
        let primitives = self
            .context
            .tessellate(output.shapes, output.pixels_per_point);
        paint_layer(
            renderer,
            painter,
            self.texture.as_mut().expect("texture allocated"),
            physical,
            output.pixels_per_point,
            &primitives,
            &output.textures_delta,
        )?;
        if resized {
            let shapes = vec![egui::epaint::ClippedShape {
                clip_rect: egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(layout.width, layout.height),
                ),
                shape: tarot_ui::wallpaper(&self.context, layout),
            }];
            let primitives = self.context.tessellate(shapes, output.pixels_per_point);
            paint_layer(
                renderer,
                painter,
                self.wallpaper.as_mut().expect("wallpaper allocated"),
                physical,
                output.pixels_per_point,
                &primitives,
                &egui::TexturesDelta::default(),
            )?;
        }
        Ok((commands, repaint))
    }

    pub fn element(&self) -> Option<TextureRenderElement<GlesTexture>> {
        self.texture.as_ref().map(|texture| {
            TextureRenderElement::from_texture_render_buffer(
                (0.0, 0.0),
                texture,
                None,
                None,
                None,
                Kind::Unspecified,
            )
        })
    }

    pub fn wallpaper_element(&self) -> Option<TextureRenderElement<GlesTexture>> {
        self.wallpaper.as_ref().map(|texture| {
            TextureRenderElement::from_texture_render_buffer(
                (0.0, 0.0),
                texture,
                None,
                None,
                None,
                Kind::Unspecified,
            )
        })
    }

    pub fn cancel_normal_input(&mut self, generation: u64) {
        if self.cancelled_generation != Some(generation) {
            // Drop normal widget presses/focus without evaluating the normal UI.
            self.context = egui::Context::default();
            self.context.set_visuals(egui::Visuals::dark());
            self.extent = None;
            self.cancelled_generation = Some(generation);
        }
    }
    pub fn update_recovery(
        &mut self,
        renderer: &mut GlowRenderer,
        physical: Size<i32, Physical>,
        scale: i32,
        generation: u64,
        events: Vec<egui::Event>,
        capture: Option<&std::path::Path>,
    ) -> Result<(bool, Duration, bool)> {
        self.recovery
            .get_or_insert_with(crate::recovery::Renderer::new)
            .update(renderer, physical, scale, generation, events, capture)
    }
    pub fn recovery_element(&self) -> Option<TextureRenderElement<GlesTexture>> {
        self.recovery.as_ref().and_then(|r| r.element())
    }
    pub fn capture_current_framebuffer(&self, size: Size<i32, Physical>) -> Result<Vec<u8>> {
        let painter = self
            .painter
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("shell renderer is not initialized"))?;
        capture_bound(painter, size)
    }
    pub fn destroy(&mut self, renderer: &mut GlowRenderer) {
        self.cards = Default::default();
        if let Some(recovery) = &mut self.recovery {
            recovery.destroy(renderer);
        }
        self.recovery = None;
        if let Some(mut painter) = self.painter.take()
            && let Err(error) = renderer.with_context(|_| painter.destroy())
        {
            tracing::warn!(%error, "destroying egui graphics resources");
        }
        self.texture = None;
        self.wallpaper = None;
    }
}

pub(crate) fn capture_bound(
    painter: &egui_glow::Painter,
    size: Size<i32, Physical>,
) -> Result<Vec<u8>> {
    use glow::HasContext;
    let gl = painter.gl();
    let mut rgba = vec![0; size.w as usize * size.h as usize * 4];
    // SAFETY: Called only on the compositor thread immediately after drawing
    // the bound window target. The context is current, the destination is
    // sized for RGBA8, and readback is confined to the requested framebuffer.
    unsafe {
        gl.bind_buffer(glow::PIXEL_PACK_BUFFER, None);
        gl.pixel_store_i32(glow::PACK_ALIGNMENT, 1);
        let target = if gl.get_parameter_i32(glow::FRAMEBUFFER_BINDING) == 0 {
            glow::BACK
        } else {
            glow::COLOR_ATTACHMENT0
        };
        gl.read_buffer(target);
        gl.read_pixels(
            0,
            0,
            size.w,
            size.h,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelPackData::Slice(Some(&mut rgba)),
        );
        let error = gl.get_error();
        anyhow::ensure!(
            error == glow::NO_ERROR,
            "framebuffer readback GL error {error:#x}"
        );
    }
    anyhow::ensure!(
        rgba.iter().any(|byte| *byte != 0),
        "empty framebuffer readback; see the documented NVIDIA rendering issue"
    );
    Ok(rgba)
}

pub(crate) fn paint_layer(
    renderer: &mut GlowRenderer,
    painter: &mut egui_glow::Painter,
    texture: &mut TextureRenderBuffer<GlesTexture>,
    physical: Size<i32, Physical>,
    pixels_per_point: f32,
    primitives: &[egui::ClippedPrimitive],
    delta: &egui::TexturesDelta,
) -> Result<()> {
    texture.render().draw(|texture| {
        let mut framebuffer = renderer.bind(texture)?;
        let mut frame = renderer.render(&mut framebuffer, physical, Transform::Normal)?;
        frame.clear(
            [0.0, 0.0, 0.0, 0.0].into(),
            &[Rectangle::from_size(physical)],
        )?;
        frame.with_context(|_| {
            painter.paint_and_update_textures(
                [physical.w as u32, physical.h as u32],
                pixels_per_point,
                primitives,
                delta,
            )
        })?;
        let _completion = frame.finish()?;
        Ok::<_, GlesError>(vec![Rectangle::from_size((physical.w, physical.h).into())])
    })?;
    Ok(())
}
