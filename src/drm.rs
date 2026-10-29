//! Initial single-panel DRM backend. Session ownership is delegated to libseat.
//! Uses Smithay's DRM/GBM primitives; see the pinned Smithay Anvil example.

use crate::{CalloopData, Compositor, Options, renderer::ShellRenderer, winit};
use anyhow::{Context, Result};
use smithay::{
    backend::{
        allocator::{
            Fourcc,
            gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
        },
        drm::{DrmDevice, DrmDeviceFd, DrmError, DrmEvent, GbmBufferedSurface},
        egl::{EGLContext, EGLDisplay},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::{Bind, ImportDma, damage::OutputDamageTracker, glow::GlowRenderer},
        session::{Event as SessionEvent, Session, libseat::LibSeatSession},
    },
    output::{Mode, Output, PhysicalProperties, Scale, Subpixel},
    reexports::{
        calloop::EventLoop,
        drm::control::{Device as _, ModeTypeFlags, connector},
        input::Libinput,
        rustix::fs::OFlags,
    },
    utils::DeviceFd,
};
use std::{cell::RefCell, rc::Rc};

type Surface = GbmBufferedSurface<GbmAllocator<DrmDeviceFd>, ()>;

pub struct DrmSession {
    device: Rc<RefCell<Device>>,
}

struct Device {
    // Keep all graphics resources alive until the shell painter is destroyed.
    shell: ShellRenderer,
    renderer: GlowRenderer,
    surface: Surface,
    drm: DrmDevice,
    _session: LibSeatSession,
    output: Output,
    damage: OutputDamageTracker,
    active: bool,
    pending: bool,
    queued_lock_generation: Option<u64>,
    queued_lock_surface: bool,
    queued_recovery: bool,
    queued_sleep: Option<tarot_core::sleep::Barrier>,
    queued_idle: Option<u64>,
    queued_output_revision: u64,
    queued_menu_retirement: Option<u64>,
    powered_off: bool,
    requested: bool,
}

impl DrmSession {
    pub fn request_redraw(&self, state: &mut Compositor) -> Result<()> {
        let mut device = self.device.borrow_mut();
        device.requested = true;
        if !device.active || device.pending {
            return Ok(());
        }
        let result = (|| {
            if let Some(orientation) = state.output_geometry.requested {
                let size = device
                    .output
                    .current_mode()
                    .context("DRM mode disappeared")?
                    .size;
                device.output.change_current_state(
                    None,
                    Some(crate::output::transform(orientation)),
                    None,
                    None,
                );
                state.configure_output_geometry(
                    size.w as u32,
                    size.h as u32,
                    state.scale as u32,
                    orientation,
                );
                device.damage = OutputDamageTracker::from_output(&device.output);
            }
            if !device.reconcile_power(state)? {
                return Ok(());
            }
            device.draw(state)
        })();
        match result {
            Err(error) if seat_interruption(&error) => {
                // The kernel may revoke master before libseat's pause callback
                // reaches calloop. Keep the process alive until Enable arrives.
                device.active = false;
                device.requested = true;
                device.drm.pause();
                tracing::info!(%error, "DRM access revoked; waiting for seat resume");
                Ok(())
            }
            result => result,
        }
    }

    pub fn shutdown(&self) {
        let mut device = self.device.borrow_mut();
        let Device {
            shell, renderer, ..
        } = &mut *device;
        shell.destroy(renderer);
    }
}

impl Device {
    fn reconcile_power(&mut self, state: &mut Compositor) -> Result<bool> {
        if state.idle.off() {
            if !self.powered_off {
                let proof = state
                    .lock_generation()
                    .filter(|generation| {
                        state.protection.last_presented_generation == Some(*generation)
                            && state.output_geometry.presented_revision
                                == Some(state.output_geometry.revision)
                    })
                    .map(|generation| crate::idle::OffPresentation {
                        generation,
                        output_revision: state.output_geometry.revision,
                        off_count: state.idle.off_count + 1,
                        phase: state.protection.gate.phase,
                    });
                self.surface.surface().clear()?;
                state.idle.off_presentation = proof;
                self.powered_off = true;
                state.idle.output_off = true;
                state.idle.off_count += 1;
                tracing::info!("idle panel powered off after protected presentation");
            }
            self.requested = false;
            return Ok(false);
        }
        if self.powered_off {
            self.surface.reset_buffers();
            self.damage = OutputDamageTracker::from_output(&self.output);
            self.powered_off = false;
        }
        Ok(true)
    }
    fn draw(&mut self, state: &mut Compositor) -> Result<()> {
        let started = std::time::Instant::now();
        self.requested = false;
        let size = self
            .output
            .current_mode()
            .context("DRM mode disappeared")?
            .size;
        let scene_size = self.output.current_transform().transform_size(size);
        let (scene, commands) =
            winit::build_scene(&mut self.renderer, state, &mut self.shell, scene_size)?;
        let menu_retirement = state.menu_retirement_token();
        if menu_retirement.is_some()
            || state.sleep.gate.barrier.is_some()
            || state.idle.barrier().is_some()
            || matches!(
                state.protection.gate.phase,
                tarot_core::session::LockPhase::Preparing
                    | tarot_core::session::LockPhase::Awaiting
            )
            || state.snapshot.is_some() && state.start_time.elapsed() >= state.snapshot_after
        {
            self.damage = OutputDamageTracker::from_output(&self.output);
        }
        let (mut buffer, age) = self.surface.next_buffer()?;
        let mut framebuffer = self.renderer.bind(&mut buffer)?;
        let result = self.damage.render_output(
            &mut self.renderer,
            &mut framebuffer,
            age as usize,
            &scene,
            state.clear_color(),
        )?;
        state.idle.render_states = Some(result.states);
        // GBM scanout uses the buffer's top row directly, unlike the nested
        // window target with its Flipped180 output transform.
        if result.damage.is_some() {
            state.metrics.record(started.elapsed());
        }
        winit::audit_lock_blank(state, &self.shell, size)?;
        winit::capture_framebuffer(state, &self.shell, size, false);
        drop(framebuffer);
        if let Some(damage) = result.damage {
            self.surface
                .queue_buffer(Some(result.sync), Some(damage.clone()), ())?;
            self.pending = true;
            self.queued_lock_generation = state.lock_generation();
            self.queued_lock_surface = state.active_lock_surface().is_some();
            self.queued_recovery = state.recovery_visible();
            self.queued_sleep = state.sleep.gate.barrier;
            self.queued_idle = state.idle.barrier();
            self.queued_output_revision = state.output_geometry.revision;
            self.queued_menu_retirement = menu_retirement;
        }
        winit::complete_frame(state, &self.output, commands);
        Ok(())
    }
}

pub fn init(
    event_loop: &mut EventLoop<CalloopData>,
    data: &mut CalloopData,
    options: &Options,
) -> Result<DrmSession> {
    let path = options
        .drm_device
        .as_ref()
        .context("direct DRM requires --drm-device")?;
    let (mut session, notifier) =
        LibSeatSession::new().context("acquiring a local seat with libseat")?;
    let fd = session
        .open(
            path,
            OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK,
        )
        .map_err(|error| {
            anyhow::anyhow!("opening {} through libseat: {error:?}", path.display())
        })?;
    let fd = DrmDeviceFd::new(DeviceFd::from(fd));
    let (mut drm, drm_events) = DrmDevice::new(fd.clone(), true)?;
    let resources = drm.resource_handles()?;
    let connector = resources
        .connectors()
        .iter()
        .filter_map(|handle| drm.get_connector(*handle, true).ok())
        .find(|info| info.state() == connector::State::Connected && !info.modes().is_empty())
        .context("no connected display on selected DRM device")?;
    let mode = connector
        .modes()
        .iter()
        .find(|mode| mode.mode_type().contains(ModeTypeFlags::PREFERRED))
        .unwrap_or(&connector.modes()[0])
        .to_owned();
    let crtc = connector
        .encoders()
        .iter()
        .filter_map(|handle| drm.get_encoder(*handle).ok())
        .flat_map(|encoder| resources.filter_crtcs(encoder.possible_crtcs()))
        .next()
        .context("no CRTC for the connected display")?;
    let gbm = GbmDevice::new(fd)?;
    // SAFETY: This GBM device stays alive through the EGL display/context, and
    // all graphics operations remain on this event-loop thread.
    let egl = unsafe { EGLDisplay::new(gbm.clone()) }?;
    let context = EGLContext::new(&egl)?;
    // SAFETY: The new context is owned by this thread and never shared with workers.
    let renderer = unsafe { GlowRenderer::new(context) }?;
    let formats = renderer
        .egl_context()
        .dmabuf_render_formats()
        .iter()
        .copied()
        .collect::<Vec<_>>();
    let allocator = GbmAllocator::new(gbm, GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT);
    let surface = GbmBufferedSurface::new(
        drm.create_surface(crtc, mode, &[connector.handle()])?,
        allocator,
        &[Fourcc::Argb8888, Fourcc::Abgr8888],
        formats,
    )?;
    let output = Output::new(
        "Tarot-panel".into(),
        PhysicalProperties {
            size: connector
                .size()
                .map(|(w, h)| (w as i32, h as i32))
                .unwrap_or((0, 0))
                .into(),
            subpixel: Subpixel::Unknown,
            make: "Pine64".into(),
            model: "DRM panel".into(),
        },
    );
    let wl_mode = Mode::from(mode);
    data.state.frame_clock =
        tarot_core::frame::FrameClock::from_refresh_millihz(wl_mode.refresh.max(1) as u32);
    output.create_global::<Compositor>(&data.display_handle);
    output.change_current_state(
        Some(wl_mode),
        Some(crate::output::transform(
            options.orientation.unwrap_or_default(),
        )),
        Some(Scale::Integer(options.scale as i32)),
        Some((0, 0).into()),
    );
    output.set_preferred(wl_mode);
    data.state.space.map_output(&output, (0, 0));
    data.state.configure_output_geometry(
        wl_mode.size.w as u32,
        wl_mode.size.h as u32,
        options.scale,
        options.orientation.unwrap_or_default(),
    );
    let device = Rc::new(RefCell::new(Device {
        damage: OutputDamageTracker::from_output(&output),
        output,
        shell: ShellRenderer::new(),
        renderer,
        surface,
        drm,
        active: session.is_active(),
        _session: session.clone(),
        pending: false,
        queued_lock_generation: None,
        queued_lock_surface: false,
        queued_recovery: false,
        queued_sleep: None,
        queued_idle: None,
        queued_output_revision: 0,
        queued_menu_retirement: None,
        powered_off: false,
        requested: true,
    }));
    let (formats, main_device) = crate::renderer::dmabuf_capabilities(&device.borrow().renderer);
    let import_device = device.clone();
    data.state.enable_dmabuf(
        formats,
        main_device,
        Box::new(move |dmabuf| {
            match import_device
                .borrow_mut()
                .renderer
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
    let input_session = LibinputSessionInterface::from(session.clone());
    let mut input = Libinput::new_with_udev(input_session);
    input
        .udev_assign_seat(&session.seat())
        .map_err(|_| anyhow::anyhow!("assigning libinput seat"))?;
    event_loop
        .handle()
        .insert_source(
            LibinputInputBackend::new(input.clone()),
            |event, _, data| {
                data.state.process_input_event(event);
                data.state.redraw = true;
            },
        )
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let seat_device = device.clone();
    event_loop
        .handle()
        .insert_source(notifier, move |event, _, data| {
            let mut device = seat_device.borrow_mut();
            match event {
                SessionEvent::PauseSession => {
                    data.state.idle.seat_inactive = true;
                    data.state.cancel_input_sequences();
                    data.state.reconfigure_menu_motion();
                    input.suspend();
                    device.active = false;
                    device.drm.pause();
                    tracing::info!("DRM session paused");
                }
                SessionEvent::ActivateSession => {
                    data.state.idle.seat_inactive = false;
                    data.state.idle_system_wake();
                    let result = device
                        .drm
                        .activate(false)
                        .map_err(anyhow::Error::from)
                        .and_then(|()| {
                            input
                                .resume()
                                .map_err(|_| anyhow::anyhow!("resuming libinput"))
                        });
                    if let Err(error) = result {
                        fail(&mut data.state, error);
                        return;
                    }
                    device.active = true;
                    device.pending = false;
                    device.queued_lock_generation = None;
                    device.queued_sleep = None;
                    device.queued_idle = None;
                    device.queued_menu_retirement = None;
                    device.surface.reset_buffers();
                    device.damage = OutputDamageTracker::from_output(&device.output);
                    data.state.ui_dirty = true;
                    data.state.redraw = true;
                    tracing::info!("DRM session resumed");
                }
            }
        })
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let frame_device = device.clone();
    event_loop
        .handle()
        .insert_source(drm_events, move |event, _, data| {
            let mut device = frame_device.borrow_mut();
            match event {
                DrmEvent::VBlank(event_crtc) if event_crtc == crtc => {
                    if let Err(error) = device.surface.frame_submitted() {
                        let error = anyhow::Error::from(error);
                        if seat_interruption(&error) {
                            device.active = false;
                            device.requested = true;
                            device.drm.pause();
                        } else {
                            fail(&mut data.state, error);
                        }
                        return;
                    }
                    if device.pending {
                        if data.state.idle.output_off {
                            data.state.idle.output_off = false;
                            data.state.idle.off_presentation = None;
                            data.state.idle.wake_count += 1;
                        }
                        data.state
                            .record_presentation(device.queued_lock_generation.is_some());
                        data.state.metrics.drm_presentations =
                            data.state.metrics.drm_presentations.saturating_add(1);
                    }
                    let generation = device.queued_lock_generation;
                    let current_geometry = device.pending
                        && data
                            .state
                            .output_geometry
                            .presented(device.queued_output_revision);
                    if device.pending
                        && current_geometry
                        && let Some(generation) = device.queued_lock_generation.take()
                    {
                        data.state.lock_frame_presented(
                            generation,
                            device.queued_lock_surface,
                            device.queued_recovery,
                        );
                    }
                    if current_geometry {
                        data.state
                            .sleep_frame_presented(device.queued_sleep.take(), generation);
                        data.state
                            .idle_frame_presented(device.queued_idle.take(), generation);
                        if let Some(revision) = device.queued_menu_retirement.take() {
                            data.state.menu_frame_presented(revision);
                        }
                    }
                    device.pending = false;
                    if data.state.idle.off()
                        && let Err(error) = device.reconcile_power(&mut data.state)
                    {
                        if seat_interruption(&error) {
                            device.active = false;
                            device.requested = true;
                            device.drm.pause();
                        } else {
                            fail(&mut data.state, error);
                        }
                        return;
                    }
                    if device.requested {
                        data.state.redraw = true;
                    }
                }
                DrmEvent::Error(error) => {
                    fail(&mut data.state, anyhow::anyhow!("DRM event: {error:?}"))
                }
                _ => {}
            }
        })
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    tracing::info!(path = %path.display(), ?mode, "direct DRM output initialized");
    Ok(DrmSession { device })
}

fn seat_interruption(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        matches!(
            cause.downcast_ref::<DrmError>(),
            Some(DrmError::DeviceInactive)
        ) || matches!(cause.downcast_ref::<DrmError>(), Some(DrmError::Access(access))
                if access.source.kind() == std::io::ErrorKind::PermissionDenied)
    })
}

fn fail(state: &mut Compositor, error: anyhow::Error) {
    tracing::error!(%error, "direct DRM backend failed");
    state.fatal_error = Some(error.to_string());
    state.loop_signal.stop();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revoked_master_is_deferred_even_through_error_context() {
        assert!(seat_interruption(
            &anyhow::Error::new(DrmError::DeviceInactive).context("queueing scanout")
        ));
    }

    #[test]
    fn other_device_failures_remain_fatal() {
        assert!(!seat_interruption(&anyhow::Error::new(
            DrmError::DrmMasterFailed
        )));
        assert!(!seat_interruption(&anyhow::Error::new(
            std::io::Error::from(std::io::ErrorKind::PermissionDenied)
        )));
        assert!(!seat_interruption(&anyhow::anyhow!("permission denied")));
    }
}
