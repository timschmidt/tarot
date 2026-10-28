//! EGL/GLES egui rendering shared by the trusted keyboard and lock clients.
use anyhow::{Context, Result};
use glutin::{
    config::{Api, ConfigTemplateBuilder},
    context::{ContextApi, ContextAttributesBuilder, PossiblyCurrentContext, Version},
    display::{Display, DisplayApiPreference},
    prelude::*,
    surface::{Surface, SurfaceAttributesBuilder, WindowSurface},
};
use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
};
use std::{ffi::CString, num::NonZeroU32, ptr::NonNull, sync::Arc};
use wayland_client::{Connection, Proxy, protocol::wl_surface::WlSurface};

pub struct Graphics {
    pub painter: egui_glow::Painter,
    context: PossiblyCurrentContext,
    surface: Surface<WindowSurface>,
    gl: Arc<glow::Context>,
    pub width: u32,
    pub height: u32,
}

impl Graphics {
    pub fn new(
        connection: &Connection,
        surface: &WlSurface,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        let raw_display = RawDisplayHandle::Wayland(WaylandDisplayHandle::new(
            NonNull::new(connection.backend().display_ptr().cast())
                .context("Wayland display pointer")?,
        ));
        let raw_window = RawWindowHandle::Wayland(WaylandWindowHandle::new(
            NonNull::new(surface.id().as_ptr().cast()).context("Wayland surface pointer")?,
        ));
        // SAFETY: handles come from the system Wayland backend and remain alive
        // until Graphics is destroyed, before its wl_surface and Connection.
        let display = unsafe { Display::new(raw_display, DisplayApiPreference::Egl)? };
        let template = ConfigTemplateBuilder::new()
            .with_alpha_size(8)
            .with_api(Api::GLES2)
            .build();
        // SAFETY: the template contains no borrowed native window handles.
        let config = unsafe { display.find_configs(template)? }
            .next()
            .context("no EGL window configuration")?;
        let attributes = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::Gles(Some(Version::new(3, 0))))
            .build(Some(raw_window));
        let attributes_surface = SurfaceAttributesBuilder::<WindowSurface>::new().build(
            raw_window,
            NonZeroU32::new(width).context("zero width")?,
            NonZeroU32::new(height).context("zero height")?,
        );
        // SAFETY: this thread owns the live wl_surface, and dimensions are nonzero.
        let (context, surface) = unsafe {
            (
                display.create_context(&config, &attributes)?,
                display.create_window_surface(&config, &attributes_surface)?,
            )
        };
        let context = context.make_current(&surface)?;
        // SAFETY: the owning context is current on this thread. Procedure lookup
        // is through the display that created it; no cross-context resources exist.
        let gl = Arc::new(unsafe {
            glow::Context::from_loader_function(|name| {
                display.get_proc_address(&CString::new(name).unwrap())
            })
        });
        use glow::HasContext;
        // SAFETY: context is current; this is diagnostic metadata, never text content.
        eprintln!("Tarot Wayland client GLES renderer: {}", unsafe {
            gl.get_parameter_string(glow::RENDERER)
        });
        let painter = egui_glow::Painter::new(gl.clone(), "", None, false)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        Ok(Self {
            painter,
            context,
            surface,
            gl,
            width,
            height,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if (self.width, self.height) != (width, height) {
            self.surface.resize(
                &self.context,
                NonZeroU32::new(width).unwrap(),
                NonZeroU32::new(height).unwrap(),
            );
            self.width = width;
            self.height = height;
        }
    }

    pub fn paint(&mut self, context: &egui::Context, output: egui::FullOutput) -> Result<()> {
        self.paint_capture(context, output, None)
    }

    /// Explicit development capture of this client's own EGL scene. Never reads
    /// another client or compositor target; normal clients leave it disabled.
    pub fn paint_capture(
        &mut self,
        context: &egui::Context,
        output: egui::FullOutput,
        capture: Option<&std::path::Path>,
    ) -> Result<()> {
        use glow::HasContext;
        // SAFETY: context is current for the entire Graphics lifetime; the
        // framebuffer is this client's EGL window, never a compositor target.
        unsafe {
            self.gl
                .viewport(0, 0, self.width as i32, self.height as i32);
            self.gl.clear_color(0.12, 0.13, 0.15, 1.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT);
        }
        let primitives = context.tessellate(output.shapes, output.pixels_per_point);
        self.painter.paint_and_update_textures(
            [self.width, self.height],
            output.pixels_per_point,
            &primitives,
            &output.textures_delta,
        );
        if let Some(path) = capture {
            use std::os::unix::fs::OpenOptionsExt;
            anyhow::ensure!(
                self.width <= 4096 && self.height <= 4096,
                "capture size exceeds diagnostic bound"
            );
            let mut pixels = vec![0; self.width as usize * self.height as usize * 4];
            // SAFETY: current client's RGBA framebuffer, fully sized byte slice.
            unsafe {
                self.gl.bind_buffer(glow::PIXEL_PACK_BUFFER, None);
                self.gl.pixel_store_i32(glow::PACK_ALIGNMENT, 1);
                self.gl.read_pixels(
                    0,
                    0,
                    self.width as i32,
                    self.height as i32,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelPackData::Slice(Some(&mut pixels)),
                );
            }
            // SAFETY: query the error flag of this client's current context.
            let error = unsafe { self.gl.get_error() };
            anyhow::ensure!(
                error == glow::NO_ERROR,
                "client capture GL error {error:#x}"
            );
            let stride = self.width as usize * 4;
            for y in 0..self.height as usize / 2 {
                let other = self.height as usize - 1 - y;
                let (top, bottom) = pixels.split_at_mut(other * stride);
                top[y * stride..(y + 1) * stride].swap_with_slice(&mut bottom[..stride]);
            }
            let file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(path)?;
            let mut encoder = png::Encoder::new(file, self.width, self.height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.write_header()?.write_image_data(&pixels)?;
        }
        self.surface.swap_buffers(&self.context)?;
        Ok(())
    }
}

impl Drop for Graphics {
    fn drop(&mut self) {
        self.painter.destroy();
    }
}
