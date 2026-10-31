use crate::{CalloopData, Compositor, Options, renderer::ShellRenderer};
use anyhow::Result;
use smithay::{
    backend::{
        renderer::{
            ImportDma,
            damage::OutputDamageTracker,
            element::{
                AsRenderElements, surface::WaylandSurfaceRenderElement,
                texture::TextureRenderElement, utils::CropRenderElement,
            },
            gles::{GlesTexture, element::PixelShaderElement},
            glow::GlowRenderer,
        },
        winit::{self, WinitEvent, WinitGraphicsBackend},
    },
    output::{Mode, Output, PhysicalProperties, Scale, Subpixel},
    reexports::{
        calloop::EventLoop,
        wayland_server::Resource,
        winit::{
            dpi::PhysicalSize,
            window::{Fullscreen, Window},
        },
    },
    utils::{Physical, Rectangle, Transform},
    wayland::compositor::SurfaceAttributes,
};
use std::{cell::RefCell, rc::Rc, time::Duration};
use tarot_core::WindowId;

type TransformedCardElement =
    crate::focus::FittedElement<WaylandSurfaceRenderElement<GlowRenderer>>;
type CardElement = CropRenderElement<crate::card_renderer::Masked<TransformedCardElement>>;
smithay::backend::renderer::element::render_elements! {
    pub RecoveryElement<=GlowRenderer>;
    Texture=TextureRenderElement<GlesTexture>,
}
smithay::backend::renderer::element::render_elements! {
    pub SceneElement<=GlowRenderer>;
    Card=CardElement,
    Fill=CropRenderElement<crate::card_renderer::FilledCard>,
    Shadow=CropRenderElement<PixelShaderElement>,
    Chrome=TextureRenderElement<GlesTexture>,
    Keyboard=CropRenderElement<WaylandSurfaceRenderElement<GlowRenderer>>,
    Lock=WaylandSurfaceRenderElement<GlowRenderer>,
    Recovery=RecoveryElement,
}

pub struct NestedSession {
    backend: Rc<RefCell<WinitGraphicsBackend<GlowRenderer>>>,
    shell: Rc<RefCell<ShellRenderer>>,
    output: Output,
}

impl NestedSession {
    pub fn request_redraw(&self, state: &mut Compositor) {
        if let Some(orientation) = state.output_geometry.requested {
            let size = self.backend.borrow().window_size();
            if size.w > 0 && size.h > 0 {
                self.output.change_current_state(
                    None,
                    Some(nested_transform(orientation)),
                    None,
                    None,
                );
                state.configure_output_geometry(
                    size.w as u32,
                    size.h as u32,
                    state.scale as u32,
                    orientation,
                );
            }
        }
        self.backend.borrow().window().request_redraw();
    }
    pub fn shutdown(&self) {
        self.shell
            .borrow_mut()
            .destroy(self.backend.borrow_mut().renderer());
    }
}

pub fn init(
    event_loop: &mut EventLoop<CalloopData>,
    data: &mut CalloopData,
    options: &Options,
) -> Result<NestedSession> {
    let attributes = Window::default_attributes()
        .with_title("Tarot")
        .with_inner_size(PhysicalSize::new(options.width, options.height))
        .with_fullscreen(options.fullscreen.then_some(Fullscreen::Borderless(None)));
    let (backend, events) = winit::init_from_attributes::<GlowRenderer>(attributes)
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let backend = Rc::new(RefCell::new(backend));
    let (formats, device) = crate::renderer::dmabuf_capabilities(backend.borrow_mut().renderer());
    let import_backend = backend.clone();
    data.state.enable_dmabuf(
        formats,
        device,
        Box::new(move |dmabuf| {
            match import_backend
                .borrow_mut()
                .renderer()
                .import_dmabuf(dmabuf, None)
            {
                Ok(_) => true,
                Err(error) => {
                    tracing::warn!(%error, "client DMA-BUF import rejected");
                    false
                }
            }
        }),
    )?;
    let mode = Mode {
        size: backend.borrow().window_size(),
        refresh: 60_000,
    };
    let scale = options.scale as i32;
    data.state.frame_clock =
        tarot_core::frame::FrameClock::from_refresh_millihz(mode.refresh.max(1) as u32);
    let output = Output::new(
        "Tarot-nested".into(),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: Subpixel::Unknown,
            make: "Tarot".into(),
            model: "Nested phone".into(),
        },
    );
    output.create_global::<Compositor>(&data.display_handle);
    output.change_current_state(
        Some(mode),
        Some(nested_transform(options.orientation.unwrap_or_default())),
        Some(Scale::Integer(scale)),
        Some((0, 0).into()),
    );
    output.set_preferred(mode);
    data.state.space.map_output(&output, (0, 0));
    data.state.configure_output_geometry(
        mode.size.w as u32,
        mode.size.h as u32,
        options.scale,
        options.orientation.unwrap_or_default(),
    );
    let session_output = output.clone();
    let mut damage_tracker = OutputDamageTracker::from_output(&output);
    let shell = Rc::new(RefCell::new(ShellRenderer::new()));
    let render_shell = shell.clone();
    let render_backend = backend.clone();
    event_loop
        .handle()
        .insert_source(events, move |event, _, data| {
            let state = &mut data.state;
            match event {
                WinitEvent::Resized { size, .. } if size.w > 0 && size.h > 0 => {
                    let orientation = state
                        .output_geometry
                        .requested
                        .unwrap_or(state.output_geometry.orientation);
                    output.change_current_state(
                        Some(Mode {
                            size,
                            refresh: 60_000,
                        }),
                        Some(nested_transform(orientation)),
                        None,
                        None,
                    );
                    state.configure_output_geometry(
                        size.w as u32,
                        size.h as u32,
                        scale as u32,
                        orientation,
                    );
                    state.ui_dirty = true;
                    state.redraw = true;
                }
                WinitEvent::Input(event) => {
                    state.process_input_event(event);
                    state.redraw = true;
                }
                WinitEvent::Redraw => {
                    let mut backend = render_backend.borrow_mut();
                    let result = {
                        let mut shell = render_shell.borrow_mut();
                        draw(
                            &mut backend,
                            state,
                            &output,
                            &mut damage_tracker,
                            &mut shell,
                        )
                    };
                    if let Err(error) = result {
                        tracing::error!(%error, "rendering failed");
                        state.fatal_error = Some(error.to_string());
                        render_shell.borrow_mut().destroy(backend.renderer());
                        state.loop_signal.stop();
                    }
                }
                WinitEvent::CloseRequested => {
                    render_shell
                        .borrow_mut()
                        .destroy(render_backend.borrow_mut().renderer());
                    state.loop_signal.stop();
                }
                _ => {}
            }
        })
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    Ok(NestedSession {
        backend,
        shell,
        output: session_output,
    })
}

fn nested_transform(orientation: tarot_core::orientation::Orientation) -> Transform {
    // The window framebuffer's vertical flip reverses quarter-turn handedness.
    Transform::Flipped180 + crate::output::transform(orientation).invert()
}

fn draw(
    backend: &mut WinitGraphicsBackend<GlowRenderer>,
    state: &mut Compositor,
    output: &Output,
    damage: &mut OutputDamageTracker,
    shell: &mut ShellRenderer,
) -> Result<()> {
    let started = std::time::Instant::now();
    let size = backend.window_size();
    if size.w <= 0 || size.h <= 0 {
        return Ok(());
    }
    let scene_size = output.current_transform().transform_size(size);
    let (scene, commands) = build_scene(backend.renderer(), state, shell, scene_size)?;
    let menu_retirement = state.menu_retirement_token();
    let snapshot_due =
        state.snapshot.is_some() && state.start_time.elapsed() >= state.snapshot_after;
    if menu_retirement.is_some()
        || state.sleep.gate.barrier.is_some()
        || state.idle.barrier().is_some()
        || snapshot_due
        || matches!(
            state.protection.gate.phase,
            tarot_core::session::LockPhase::Preparing | tarot_core::session::LockPhase::Awaiting
        )
    {
        *damage = OutputDamageTracker::from_output(output);
    }
    let (renderer, mut framebuffer) = backend.bind()?;
    let result =
        damage.render_output(renderer, &mut framebuffer, 0, &scene, state.clear_color())?;
    state.idle.render_states = Some(result.states);
    let rendered = result.damage.is_some();
    if rendered {
        state.metrics.record(started.elapsed());
    }
    audit_lock_blank(state, shell, size)?;
    capture_framebuffer(state, shell, size, true);
    drop(framebuffer);
    if rendered {
        let sleep_barrier = state.sleep.gate.barrier;
        let idle_barrier = state.idle.barrier();
        let generation = state.lock_generation();
        backend.submit(Some(&[Rectangle::from_size(size)]))?;
        state
            .output_geometry
            .presented(state.output_geometry.revision);
        state.record_presentation(state.protected());
        if let Some(generation) = state.lock_generation() {
            state.lock_frame_presented(
                generation,
                state.active_lock_surface().is_some(),
                state.recovery_visible(),
            );
        }
        state.sleep_frame_presented(sleep_barrier, generation);
        state.idle_frame_presented(idle_barrier, generation);
    }
    complete_frame(state, output, commands);
    if rendered && let Some(revision) = menu_retirement {
        state.menu_frame_presented(revision);
    }
    Ok(())
}

pub fn build_scene(
    renderer: &mut GlowRenderer,
    state: &mut Compositor,
    shell: &mut ShellRenderer,
    size: smithay::utils::Size<i32, Physical>,
) -> Result<(Vec<SceneElement>, Vec<tarot_core::Command>)> {
    let mut commands = Vec::new();
    state.check_locker();
    if state.protected() {
        state.configure_lock_surface();
        state.refresh_lock_focus();
        state.ui_events.clear();
        shell.cancel_normal_input(state.protection.gate.generation);
        state.presented_windows.clear();
        state.presented_surfaces.clear();
        if state.protection.verify_blank {
            shell.initialize(renderer)?;
        }
        if state.recovery_visible() {
            let capture = state.protection.recovery_capture.take();
            let (retry, repaint, capture_error) = shell.update_recovery(
                renderer,
                size,
                state.scale,
                state.protection.gate.generation,
                std::mem::take(&mut state.protection.recovery_events),
                capture.as_deref(),
            )?;
            if capture_error {
                state.protection.recovery_capture_error = Some("Recovery capture failed".into());
            }
            if retry {
                state.retry_recovery();
            }
            if repaint < Duration::from_secs(60)
                && let Some(deadline) = std::time::Instant::now().checked_add(repaint)
            {
                state.repaint_at = Some(state.repaint_at.map_or(deadline, |old| old.min(deadline)));
            }
            // Retry may reserve a fresh generation; render black until its owner
            // supplies a real lock surface. Never show stale recovery controls.
            let scene = if state.recovery_visible() {
                shell
                    .recovery_element()
                    .into_iter()
                    .map(RecoveryElement::Texture)
                    .map(SceneElement::Recovery)
                    .collect()
            } else {
                Vec::new()
            };
            return Ok((scene, commands));
        }
        let scene = state
            .active_lock_surface()
            .map(|surface| {
                crate::focus::cache_surface_tree(
                    &surface,
                    (0, 0).into(),
                    WindowId(0),
                    true,
                    &mut state.presented_surfaces,
                );
                smithay::backend::renderer::element::surface::render_elements_from_surface_tree(
                    renderer,
                    &surface,
                    (0, 0),
                    state.scale as f64,
                    1.0,
                    smithay::backend::renderer::element::Kind::Unspecified,
                )
                .into_iter()
                .map(SceneElement::Lock)
                .collect()
            })
            .unwrap_or_default();
        return Ok((scene, commands));
    }
    state.popups.cleanup();
    state.refresh_keyboard();
    state.advance_presentation();
    if state.ui_dirty {
        state.ui_dirty = false;
        let (next_commands, repaint) = shell.update(
            renderer,
            crate::renderer::ShellFrame {
                model: &state.shell,
                layout: state.layout,
                presentation: &state.presentation,
                fullscreen: state.fullscreen_chrome(),
                physical: size,
                scale: state.scale,
                events: std::mem::take(&mut state.ui_events),
            },
        )?;
        commands = next_commands;
        if repaint < Duration::from_secs(60)
            && let Some(deadline) = std::time::Instant::now().checked_add(repaint)
        {
            state.repaint_at = Some(state.repaint_at.map_or(deadline, |old| old.min(deadline)));
        }
    }
    let mut scene: Vec<SceneElement> = Vec::new();
    state.presented_windows.clear();
    state.presented_surfaces.clear();
    // A strip-owned wave overlays the native keyboard without changing its
    // reservation or the application's geometry/focus during the gesture.
    let wave = state.presentation.quick_launch.is_some();
    if wave && let Some(chrome) = shell.element() {
        scene.push(SceneElement::Chrome(chrome));
    }
    if state.native_keyboard_visible() {
        let keyboard = state.keyboard.as_ref().unwrap();
        let rect = state.native_keyboard_rect();
        let origin = (rect.x as i32, rect.y as i32).into();
        crate::focus::cache_surface_tree(
            keyboard.layer.wl_surface(),
            origin,
            keyboard.owner.unwrap(),
            true,
            &mut state.presented_surfaces,
        );
        use smithay::backend::renderer::element::AsRenderElements;
        let elements: Vec<WaylandSurfaceRenderElement<GlowRenderer>> =
            keyboard.layer.render_elements(
                renderer,
                (
                    (rect.x * state.scale as f32).round() as i32,
                    (rect.y * state.scale as f32).round() as i32,
                )
                    .into(),
                f64::from(state.scale).into(),
                1.0,
            );
        let clip = Rectangle::<i32, smithay::utils::Logical>::new(
            origin,
            (rect.width as i32, rect.height as i32).into(),
        )
        .to_physical_precise_round(f64::from(state.scale));
        scene.extend(
            elements
                .into_iter()
                .filter_map(|element| {
                    CropRenderElement::from_element(element, f64::from(state.scale), clip)
                })
                .map(SceneElement::Keyboard),
        );
    }
    if !wave && let Some(chrome) = shell.element() {
        scene.push(SceneElement::Chrome(chrome));
    }
    let content = state.layout.content();
    let output_scale = f64::from(state.scale);
    let content_clip: Rectangle<i32, Physical> = Rectangle::new(
        (
            (f64::from(content.x) * output_scale).round() as i32,
            (f64::from(content.y) * output_scale).round() as i32,
        )
            .into(),
        (
            (f64::from(content.width) * output_scale).round() as i32,
            (f64::from(content.height) * output_scale).round() as i32,
        )
            .into(),
    );
    let mut visible_windows: Vec<_> = state
        .space
        .elements()
        .filter(|window| {
            window
                .user_data()
                .get::<WindowId>()
                .is_some_and(|id| state.families.mapped(*id))
        })
        .cloned()
        .collect();
    // Native elements use the reverse of the chrome's back-to-front order.
    visible_windows.sort_by_key(|window| {
        let id = window.user_data().get::<WindowId>().copied().unwrap();
        let owner = state.families.owner(id).unwrap();
        (
            std::cmp::Reverse(
                state
                    .presentation
                    .windows
                    .iter()
                    .position(|visual| visual.id == owner)
                    .unwrap_or(0),
            ),
            std::cmp::Reverse(
                state
                    .families
                    .members(owner)
                    .iter()
                    .position(|member| *member == id)
                    .unwrap_or(0),
            ),
        )
    });
    shell.cards.begin(renderer)?;
    for window in &visible_windows {
        let Some(id) = window.user_data().get::<WindowId>().copied() else {
            continue;
        };
        let Some(owner) = state.families.owner(id) else {
            continue;
        };
        if !state.presentation.visible(owner, state.layout) {
            continue;
        }
        let Some(visual) = state.presentation.window(owner) else {
            continue;
        };
        let scale = state.scale as f64;
        let visual = *visual;
        let fullscreen = state.fullscreen_window(owner);
        let content_clip = if fullscreen {
            Rectangle::from_size(size)
        } else {
            content_clip
        };
        let Some(card_geometry) = crate::focus::RenderGeometry::new(
            window.geometry(),
            visual.rect,
            visual.opacity,
            scale,
        ) else {
            continue;
        };
        let card_target = card_geometry.target;
        let native_rect = state.native_rect(id, visual.rect);
        if let Some(mut geometry) =
            crate::focus::RenderGeometry::new(window.geometry(), native_rect, visual.opacity, scale)
        {
            geometry.rotation = visual.rotation;
            geometry.rotation_center = Some(
                (
                    f64::from(card_target.loc.x) + f64::from(card_target.size.w) * 0.5,
                    f64::from(card_target.loc.y) + f64::from(card_target.size.h) * 0.5,
                )
                    .into(),
            );
            state.presented_windows.insert(id, geometry);
            crate::focus::cache_surface_placements(window, id, &mut state.presented_surfaces);
            let native: Vec<WaylandSurfaceRenderElement<GlowRenderer>> = window.render_elements(
                renderer,
                geometry.render_location(),
                scale.into(),
                geometry.opacity,
            );
            for element in geometry.fit_elements(native, card_target) {
                if let Some(element) = shell
                    .cards
                    .mask(element, card_target, visual, size, scale)
                    .and_then(|element| {
                        CropRenderElement::from_element(element, scale, content_clip)
                    })
                {
                    scene.push(SceneElement::Card(element));
                }
            }
            if id == owner
                && fullscreen
                && let Some(fill) = shell
                    .cards
                    .fill(renderer, visual, card_target, size, scale)?
                    .and_then(|fill| CropRenderElement::from_element(fill, scale, content_clip))
            {
                scene.push(SceneElement::Fill(fill));
            }
            if id == owner
                && let Some(shadow) =
                    shell
                        .cards
                        .shadow(visual, card_target, scale)
                        .and_then(|element| {
                            CropRenderElement::from_element(element, scale, content_clip)
                        })
            {
                scene.push(SceneElement::Shadow(shadow));
            }
        }
    }
    shell.cards.end();
    if let Some(wallpaper) = shell.wallpaper_element() {
        scene.push(SceneElement::Chrome(wallpaper));
    }
    Ok((scene, commands))
}

pub fn complete_frame(state: &mut Compositor, output: &Output, commands: Vec<tarot_core::Command>) {
    if state.protected() {
        if let Some(surface) = state.lock_callback_surface() {
            smithay::desktop::utils::send_frames_surface_tree(
                &surface,
                output,
                state.start_time.elapsed(),
                Some(Duration::ZERO),
                |_, _| Some(output.clone()),
            );
        }
        return;
    }

    if let Some(deadline) = state.frame_clock.request(state.start_time.elapsed()) {
        state.frame_callback_at = state.start_time.checked_add(deadline);
    } else {
        let windows: Vec<_> = state.space.elements().cloned().collect();
        let visible = |window: &smithay::desktop::Window| {
            window
                .user_data()
                .get::<WindowId>()
                .and_then(|id| state.families.owner(*id))
                .is_some_and(|owner| state.presentation.visible(owner, state.layout))
        };
        let visible_surfaces: Vec<_> = windows
            .iter()
            .filter(|window| visible(window))
            .filter_map(|window| window.toplevel().map(|top| top.wl_surface().id()))
            .collect();
        let mut background = Vec::new();
        for window in &windows {
            // Several toolkits render multiple windows on one thread and can
            // block in EGL swap for a hidden sibling. Keep a visible client's
            // entire connection responsive; app IDs do not establish identity.
            let foreground = visible(window)
                || window.toplevel().is_some_and(|top| {
                    visible_surfaces
                        .iter()
                        .any(|id| id.same_client_as(&top.wl_surface().id()))
                });
            if foreground {
                window.send_frame(
                    output,
                    state.start_time.elapsed(),
                    Some(Duration::ZERO),
                    |_, _| Some(output.clone()),
                );
            } else {
                let mut pending = false;
                window.with_surfaces(|_, states| {
                    pending |= !states
                        .cached_state
                        .get::<SurfaceAttributes>()
                        .current()
                        .frame_callbacks
                        .is_empty();
                });
                if pending {
                    background.push(window);
                }
            }
        }
        if !background.is_empty() {
            let now = state.start_time.elapsed();
            if let Some(deadline) = state.hidden_frame_clock.request(now) {
                state.hidden_frame_callback_at = state.start_time.checked_add(deadline);
            } else {
                for window in background {
                    window.send_frame(output, now, Some(Duration::ZERO), |_, _| {
                        Some(output.clone())
                    });
                }
            }
        }
        if state.native_keyboard_visible() {
            state.keyboard.as_ref().unwrap().layer.send_frame(
                output,
                state.start_time.elapsed(),
                Some(Duration::ZERO),
                |_, _| Some(output.clone()),
            );
        }
    }
    state.space.refresh();
    state.refresh_popup_grab();
    state.refresh_pointer_focus();
    for command in commands {
        state.apply_command(command);
    }
}

pub fn audit_lock_blank(
    state: &mut Compositor,
    shell: &ShellRenderer,
    size: smithay::utils::Size<i32, Physical>,
) -> Result<()> {
    if state.protection.verify_blank && state.protected() {
        let preparing = matches!(
            state.protection.gate.phase,
            tarot_core::session::LockPhase::Preparing | tarot_core::session::LockPhase::Awaiting
        );
        let surface = state.active_lock_surface().is_some();
        if state.recovery_visible() {
            let pixels = shell.capture_current_framebuffer(size)?;
            let (pixels, remainder) = pixels.as_chunks::<4>();
            let opaque = remainder.is_empty() && pixels.iter().all(|p| p[3] == 255);
            let text = pixels
                .iter()
                .filter(|p| p[0] > 150 && p[1] > 150 && p[2] > 150)
                .count()
                > 100;
            let valid = opaque && text;
            state.protection.recovery_verified = Some(valid);
            anyhow::ensure!(valid, "Protected recovery frame audit failed");
            return Ok(());
        }
        let expected = if surface {
            state.protection.verify_surface_rgba
        } else {
            Some([0, 0, 0, 255])
        };
        if let Some(expected) = expected {
            let pixels = shell.capture_current_framebuffer(size)?;
            let (pixels, remainder) = pixels.as_chunks::<4>();
            let valid = remainder.is_empty() && pixels.iter().all(|p| *p == expected);
            if preparing {
                state.protection.blank_verified = Some(valid);
                state.protection.blank_verified_generation = Some(state.protection.gate.generation);
            } else if surface {
                state.protection.surface_verified = Some(valid);
            } else {
                state.protection.fallback_verified = Some(valid);
            }
            anyhow::ensure!(valid, "Protected-frame audit failed");
        }
    }
    Ok(())
}

pub fn capture_framebuffer(
    state: &mut Compositor,
    shell: &ShellRenderer,
    size: smithay::utils::Size<i32, Physical>,
    flip_y: bool,
) {
    if state.protected() {
        state.snapshot = None;
        return;
    }
    if state.snapshot.is_some() && state.start_time.elapsed() >= state.snapshot_after {
        let path = state.snapshot.take().expect("pending snapshot");
        if let Err(error) = write_capture(shell, size, flip_y, &path) {
            tracing::warn!(%error, path = %path.display(), "diagnostic capture failed; session continues");
            state.snapshot_error = Some(error.to_string());
        }
    }
}

fn write_capture(
    shell: &ShellRenderer,
    size: smithay::utils::Size<i32, Physical>,
    flip_y: bool,
    path: &std::path::Path,
) -> Result<()> {
    let data = shell.capture_current_framebuffer(size)?;
    let stride = size.w as usize * 4;
    let mut rgba = Vec::with_capacity(data.len());
    if flip_y {
        for row in data.chunks_exact(stride).rev() {
            rgba.extend_from_slice(row);
        }
    } else {
        rgba = data;
    }
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    let mut encoder = png::Encoder::new(
        std::io::BufWriter::new(std::fs::File::create(path)?),
        size.w as u32,
        size.h as u32,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&rgba)?;
    tracing::info!(path = %path.display(), "captured Tarot framebuffer");
    Ok(())
}
