mod app_menus;
mod apps;
mod audio;
mod audio_inhibit;
mod authorization;
mod bluetooth;
mod card_renderer;
mod clock;
mod control;
#[cfg(feature = "drm")]
mod drm;
mod families;
mod feedback;
mod focus;
mod grabs;
mod gtk_protocol;
mod handlers;
mod hardware;
mod icons;
mod idle;
mod input;
mod input_method_process;
mod keyboard;
mod lock;
mod metrics;
mod network;
mod notifications;
mod output;
mod popups;
mod power;
mod preferences;
mod recovery;
mod renderer;
mod sensors;
mod session_idle_host;
mod sleep;
mod state;
mod text;
mod window_state;
mod winit;

use anyhow::{Context, Result};
use clap::Parser;
use smithay::reexports::{
    calloop::{
        EventLoop,
        timer::{TimeoutAction, Timer},
    },
    wayland_server::{Display, DisplayHandle},
};
pub use state::Compositor;
use std::time::{Duration, Instant};

pub struct CalloopData {
    state: Compositor,
    display_handle: DisplayHandle,
}

enum Backend {
    Nested(winit::NestedSession),
    #[cfg(feature = "drm")]
    Drm(drm::DrmSession),
}

impl Backend {
    fn request_redraw(&self, _state: &mut Compositor) -> Result<()> {
        match self {
            Self::Nested(session) => session.request_redraw(_state),
            #[cfg(feature = "drm")]
            Self::Drm(session) => return session.request_redraw(_state),
        }
        Ok(())
    }

    fn shutdown(&self) {
        match self {
            Self::Nested(session) => session.shutdown(),
            #[cfg(feature = "drm")]
            Self::Drm(session) => session.shutdown(),
        }
    }
}

#[derive(Debug, Parser)]
#[command(version, about = "Tarot phone shell and Wayland compositor")]
pub struct Options {
    /// Physical width of the nested window.
    #[arg(long, default_value_t = 360, value_parser = clap::value_parser!(u32).range(240..=4096))]
    width: u32,
    /// Physical height of the nested window.
    #[arg(long, default_value_t = 720, value_parser = clap::value_parser!(u32).range(240..=4096))]
    height: u32,
    /// Integer output/UI scale (use 2 for the PinePhone Pro panel).
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..=4))]
    scale: u32,
    /// Initial orientation, overriding a saved locked orientation if supplied.
    #[arg(long)]
    orientation: Option<tarot_core::orientation::Orientation>,
    #[arg(long)]
    fullscreen: bool,
    /// Finish shell transitions immediately; direct finger tracking stays available.
    #[arg(long)]
    reduced_motion: bool,
    /// Run directly on this DRM device through libseat (experimental single-panel backend).
    #[cfg(feature = "drm")]
    #[arg(long)]
    drm_device: Option<std::path::PathBuf>,
    /// Capture Tarot's own composed framebuffer once (development diagnostics).
    #[arg(long)]
    screenshot: Option<std::path::PathBuf>,
    #[arg(long, default_value_t = 1000)]
    screenshot_after_ms: u64,
    /// Exit automatically after a bounded smoke-test run.
    #[arg(long)]
    exit_after_ms: Option<u64>,
    /// Enable owner-only development control in a private runtime directory.
    #[arg(long)]
    control_socket: Option<std::path::PathBuf>,
    /// Publish this session's display details to its explicitly private D-Bus activation environment.
    #[arg(long)]
    publish_session_environment: bool,
    /// Own the notification service on this launcher's explicitly private session bus.
    #[arg(long, requires = "publish_session_environment")]
    notifications: bool,
    /// Override the versioned shell settings file (defaults to XDG_CONFIG_HOME).
    #[arg(long)]
    config_file: Option<std::path::PathBuf>,
    /// Enable battery, display, Wi-Fi, Bluetooth and audio controls.
    #[arg(long)]
    system_services: bool,
    /// Hold a login1 sleep-delay inhibitor until a new protected frame completes.
    #[arg(long, requires_all = ["lock_client", "system_services"], conflicts_with = "lock_probe_unlocked_start")]
    sleep_coordination: bool,
    /// Publish protected panel-off readiness to this GUI session's idle hint (opt-in).
    #[arg(long, requires = "sleep_coordination")]
    session_idle: bool,
    /// Also permit session idle on observed external power (opt-in).
    #[arg(long, requires = "session_idle")]
    session_idle_on_external_power: bool,
    /// Lock after this many milliseconds without seat activity (opt-in).
    #[arg(long, requires="lock_client", conflicts_with="lock_probe_unlocked_start", value_parser=clap::value_parser!(u64).range(100..=86400000))]
    idle_lock_ms: Option<u64>,
    /// Use saved idle preferences, defaulting to lock at 2m and screen off at 3m.
    #[arg(
        long,
        requires = "lock_client",
        conflicts_with = "lock_probe_unlocked_start"
    )]
    idle_preferences: bool,
    /// Handle physical power/volume keys within this shell session.
    #[arg(
        long,
        requires = "lock_client",
        conflicts_with = "lock_probe_unlocked_start"
    )]
    hardware_keys: bool,
    /// Power off the direct panel after inactivity; must be >= idle-lock-ms.
    #[arg(long, requires="idle_lock_ms", value_parser=clap::value_parser!(u64).range(100..=86400000))]
    idle_off_ms: Option<u64>,
    /// Use an isolated diagnostic bus instead of the system bus.
    #[arg(long, requires = "system_services")]
    platform_bus: Option<String>,
    /// Nonprivileged helper for an explicitly private authorization fixture.
    #[arg(long, requires = "platform_bus")]
    authorization_helper: Option<std::path::PathBuf>,
    /// Diagnostic backlight readback directory; writes always use login1.
    #[arg(long, requires = "platform_bus")]
    backlight_root: Option<std::path::PathBuf>,
    /// Override the PulseAudio-compatible server for isolated diagnostics.
    #[arg(long, requires = "system_services")]
    audio_server: Option<String>,
    /// Experimental trusted locker program, connected through a private socket.
    #[arg(long)]
    lock_client: Option<std::path::PathBuf>,
    /// Diagnostic protocol fixture: spawn unlocked and disable automatic recovery.
    #[arg(long, requires = "lock_client")]
    lock_probe_unlocked_start: bool,
    #[arg(long, requires = "lock_client", allow_hyphen_values = true)]
    lock_client_arg: Vec<std::ffi::OsString>,
    /// Audit the initial protected framebuffer as opaque black (development only).
    #[arg(long, requires = "lock_client")]
    verify_lock_blank: bool,
    /// Capture only the compositor-owned recovery view (explicit QA, never apps).
    #[arg(long, requires = "lock_client")]
    recovery_screenshot: Option<std::path::PathBuf>,
    /// Expected opaque fixture color for protected-surface audit (RRGGBBFF).
    #[arg(long, requires = "verify_lock_blank", value_parser=parse_lock_color)]
    verify_lock_surface_rgba: Option<[u8; 4]>,
    /// Launch a trusted input method on a private inherited Wayland connection.
    #[arg(long)]
    input_method: Option<std::path::PathBuf>,
    /// An argument for the trusted input method (repeat for multiple arguments).
    #[arg(long, requires = "input_method", allow_hyphen_values = true)]
    input_method_arg: Vec<std::ffi::OsString>,
    /// Start a client with Tarot's WAYLAND_DISPLAY; arguments follow --.
    #[arg(last = true)]
    client: Vec<String>,
}

fn parse_lock_color(text: &str) -> std::result::Result<[u8; 4], String> {
    if text.len() != 8 || !text.is_ascii() {
        return Err("Use RRGGBBFF".into());
    }
    let mut rgba = [0u8; 4];
    for (i, out) in rgba.iter_mut().enumerate() {
        *out = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).map_err(|_| "Use RRGGBBFF")?;
    }
    if rgba[3] != 255 {
        return Err("Fixture audit requires an opaque color".into());
    }
    Ok(rgba)
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tarot=info,smithay=warn".into()),
        )
        .init();
    let mut options = Options::parse();
    let physical = {
        #[cfg(feature = "drm")]
        {
            options.drm_device.is_some()
        }
        #[cfg(not(feature = "drm"))]
        {
            false
        }
    };
    anyhow::ensure!(
        !options.sleep_coordination || physical || options.platform_bus.is_some(),
        "Real sleep coordination requires direct DRM; nested testing requires an isolated --platform-bus"
    );
    let mut event_loop: EventLoop<CalloopData> = EventLoop::try_new()?;
    anyhow::ensure!(
        !options.hardware_keys || options.idle_preferences || options.idle_lock_ms.is_some(),
        "Hardware keys require --idle-preferences or --idle-lock-ms"
    );
    anyhow::ensure!(
        options.idle_off_ms.is_none() || physical,
        "Panel power-off requires direct DRM"
    );
    let idle_policy = options
        .idle_lock_ms
        .map(|lock| {
            tarot_core::idle::Policy::new(
                Duration::from_millis(lock),
                options.idle_off_ms.map(Duration::from_millis),
                Duration::ZERO,
            )
            .context("idle-off-ms must be >= idle-lock-ms")
        })
        .transpose()?;
    let display: Display<Compositor> = Display::new()?;
    let display_handle = display.handle();
    let state = Compositor::new(&mut event_loop, display);
    let mut data = CalloopData {
        state,
        display_handle,
    };
    data.state.idle.policy = idle_policy;
    data.state.hardware.enabled = options.hardware_keys;
    data.state.shell.display.panel_power_available = physical;
    // Reserve protection before either backend can render a normal scene.
    if options.lock_client.is_some() && !options.lock_probe_unlocked_start {
        data.state
            .protection
            .gate
            .reserve()
            .context("reserving startup protection")?;
        data.state.protection.startup_reserved = true;
    }
    {
        // Diagnostic service fixtures must not read or modify host preferences.
        let isolated_config = if (options.platform_bus.is_some()
            || (!physical && options.control_socket.is_some()))
            && options.config_file.is_none()
        {
            Some(tempfile::tempdir()?)
        } else {
            None
        };
        let config = options.config_file.clone().map(Ok).unwrap_or_else(|| {
            isolated_config
                .as_ref()
                .map(|d| Ok(d.path().join("settings.toml")))
                .unwrap_or_else(|| {
                    preferences::config_path(
                        std::env::var_os("XDG_CONFIG_HOME"),
                        std::env::var_os("HOME"),
                    )
                })
        })?;
        let server = preferences::start(event_loop.handle(), config, isolated_config)?;
        let display = &server.initial.display;
        data.state.shell.rotation_locked = display.rotation_locked;
        if options.orientation.is_none() && display.rotation_locked {
            options.orientation = Some(display.locked_orientation);
        }
        data.state.shell.display.idle = display.idle;
        data.state.shell.display.sleep = display.sleep;
        data.state.shell.display.idle_configurable =
            options.idle_preferences && options.idle_lock_ms.is_none();
        if data.state.shell.display.idle_configurable {
            data.state.idle.policy = tarot_core::idle::Policy::new(
                display.idle.lock_after(),
                physical.then(|| display.idle.off_after()).flatten(),
                Duration::ZERO,
            );
        }
        data.state.preferences = Some(server);
        data.state.refresh_preferences_status();
    }
    data.state.protection.recovery_capture = options.recovery_screenshot.clone();
    data.state.protection.verify_blank = options.verify_lock_blank;
    data.state.protection.verify_surface_rgba = options.verify_lock_surface_rgba;
    data.state
        .animation
        .set_reduced_motion(options.reduced_motion);
    data.state.shell.reduced_motion = options.reduced_motion;
    #[cfg(feature = "drm")]
    let backend = if options.drm_device.is_some() {
        Backend::Drm(drm::init(&mut event_loop, &mut data, &options)?)
    } else {
        Backend::Nested(winit::init(&mut event_loop, &mut data, &options)?)
    };
    #[cfg(not(feature = "drm"))]
    let backend = Backend::Nested(winit::init(&mut event_loop, &mut data, &options)?);
    if options.publish_session_environment && !options.notifications {
        apps::publish_activation_environment(&data.state.socket_name)?;
    }
    apps::init(event_loop.handle(), &mut data.state)?;
    icons::start(event_loop.handle(), &mut data.state)?;
    clock::start(event_loop.handle(), &mut data.state)?;
    data.state.protection.verify_blank = options.verify_lock_blank;
    data.state.protection.verify_surface_rgba = options.verify_lock_surface_rgba;
    if let Some(program) = options.lock_client {
        lock::start(
            &mut data.state,
            event_loop.handle(),
            program,
            options.lock_client_arg,
            options.lock_probe_unlocked_start,
        )?;
    }
    if options.system_services {
        data.state.sensors = Some(sensors::start(
            event_loop.handle(),
            &mut data.state,
            options.platform_bus.clone(),
        )?);
        // A diagnostic platform bus must never implicitly control host audio.
        let audio_server = options.audio_server.clone().or_else(|| {
            options
                .platform_bus
                .as_ref()
                .map(|_| "unix:/nonexistent/tarot-audio".into())
        });
        data.state.audio = Some(audio::start(
            event_loop.handle(),
            &mut data.state,
            audio_server,
        )?);
        data.state.authorization = Some(authorization::start(
            event_loop.handle(),
            &mut data.state,
            authorization::Options {
                bus: options.platform_bus.clone(),
                helper: options.authorization_helper.clone(),
            },
        )?);
        data.state.bluetooth = Some(bluetooth::start(
            event_loop.handle(),
            &mut data.state,
            options.platform_bus.clone(),
        )?);
        let authorization_readiness = data
            .state
            .authorization
            .as_ref()
            .map(authorization::Server::readiness)
            .unwrap_or_default();
        data.state.network = Some(network::start(
            event_loop.handle(),
            &mut data.state,
            options.platform_bus.clone(),
            authorization_readiness,
        )?);
        data.state.system_services = Some(power::start(
            event_loop.handle(),
            &mut data.state,
            power::Options {
                bus: options.platform_bus.clone(),
                backlights: options
                    .backlight_root
                    .clone()
                    .unwrap_or_else(|| "/sys/class/backlight".into()),
            },
        )?);
    }
    if options.notifications {
        let preferences = data.state.preferences.as_ref().unwrap().handle.clone();
        if options.system_services {
            data.state.feedback = Some(feedback::start(
                event_loop.handle(),
                &mut data.state,
                preferences.clone(),
            )?);
        }
        let feedback = data.state.feedback.as_ref().map(|s| s.handle.clone());
        data.state.notifications = Some(notifications::start(
            event_loop.handle(),
            &mut data.state,
            preferences,
            feedback,
        )?);
    }
    let _sleep = if options.sleep_coordination {
        Some(sleep::start(
            event_loop.handle(),
            &mut data.state,
            options.platform_bus.clone(),
            physical,
        )?)
    } else {
        None
    };
    if options.sleep_coordination {
        data.state.audio_inhibit = Some(audio_inhibit::start(
            event_loop.handle(),
            &mut data.state,
            options.platform_bus.clone(),
            physical,
        )?);
    }
    if options.session_idle {
        anyhow::ensure!(
            data.state.idle.policy.is_some(),
            "Session idle publishing requires an idle policy"
        );
        data.state.session_idle_worker = Some(session_idle_host::start(
            event_loop.handle(),
            &mut data.state,
            options.platform_bus.clone(),
            physical,
            options.session_idle_on_external_power,
        )?);
    }
    let _control = options
        .control_socket
        .as_ref()
        .map(|path| control::start(path, &event_loop.handle()))
        .transpose()?;
    if let Some(program) = &options.input_method {
        input_method_process::start(
            &mut data.state,
            event_loop.handle(),
            program.clone(),
            options.input_method_arg.clone(),
        );
    }
    if let Some(path) = options.screenshot {
        data.state.snapshot = Some(path);
        data.state.snapshot_after = Duration::from_millis(options.screenshot_after_ms);
        event_loop
            .handle()
            .insert_source(
                Timer::from_duration(data.state.snapshot_after),
                |_, _, data| {
                    data.state.redraw = true;
                    TimeoutAction::Drop
                },
            )
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    }
    if let Some(milliseconds) = options.exit_after_ms {
        event_loop
            .handle()
            .insert_source(
                Timer::from_duration(Duration::from_millis(milliseconds)),
                |_, _, data| {
                    data.state.loop_signal.stop();
                    TimeoutAction::Drop
                },
            )
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    }
    tracing::info!(socket = ?data.state.socket_name, "Tarot Wayland session ready");

    if let Some((program, arguments)) = options.client.split_first() {
        let child = std::process::Command::new(program)
            .args(arguments)
            .env("WAYLAND_DISPLAY", &data.state.socket_name)
            .env("GDK_BACKEND", "wayland")
            .env("QT_QPA_PLATFORM", "wayland")
            .env("XDG_CURRENT_DESKTOP", "Tarot")
            .env("XDG_SESSION_DESKTOP", "tarot")
            .env("XDG_SESSION_TYPE", "wayland")
            .env_remove("DISPLAY")
            .stdin(std::process::Stdio::null())
            .spawn()
            .with_context(|| format!("launching client {program}"))?;
        if let Some(sender) = &data.state.app_events {
            apps::monitor_child(
                event_loop.handle(),
                child,
                apps::ChildTag::Application(0),
                sender.clone(),
            );
        }
    }

    let handle = event_loop.handle();
    let run_result = event_loop.run(None, &mut data, |data| {
        data.state.sync_app_menu();
        data.state.hardware_tick();
        data.state.idle_tick();
        if let Err(error) = data.state.schedule_hardware(&handle) {
            data.state.fatal_error = Some(error.to_string());
            data.state.loop_signal.stop();
        }
        if let Some(sensors) = &data.state.sensors {
            sensors.watch(data.state.sensor_rotation_wanted());
        }
        if data.state.redraw {
            data.state.redraw = false;
            if let Err(error) = backend.request_redraw(&mut data.state) {
                tracing::error!(%error, "rendering failed");
                data.state.fatal_error = Some(error.to_string());
                data.state.loop_signal.stop();
            }
        }
        if let Some(deadline) = data.state.repaint_at.take() {
            let delay = deadline
                .saturating_duration_since(Instant::now())
                .max(Duration::from_millis(1));
            if let Err(error) = handle.insert_source(Timer::from_duration(delay), |_, _, data| {
                data.state.redraw = true;
                data.state.ui_dirty = true;
                TimeoutAction::Drop
            }) {
                tracing::warn!(%error, "could not schedule shell repaint");
            }
        }
        if let Some(deadline) = data.state.frame_callback_at.take()
            && !data.state.frame_callback_timer_pending
        {
            data.state.frame_callback_timer_pending = true;
            let delay = deadline
                .saturating_duration_since(Instant::now())
                .max(Duration::from_millis(1));
            if let Err(error) = handle.insert_source(Timer::from_duration(delay), |_, _, data| {
                data.state.frame_callback_timer_pending = false;
                data.state.redraw = true;
                TimeoutAction::Drop
            }) {
                data.state.fatal_error = Some(format!(
                    "could not schedule native frame callbacks: {error}"
                ));
                data.state.loop_signal.stop();
            }
        }
        if let Some(deadline) = data.state.hidden_frame_callback_at.take()
            && !data.state.hidden_frame_callback_timer_pending
        {
            data.state.hidden_frame_callback_timer_pending = true;
            let delay = deadline
                .saturating_duration_since(Instant::now())
                .max(Duration::from_millis(1));
            if let Err(error) = handle.insert_source(Timer::from_duration(delay), |_, _, data| {
                data.state.hidden_frame_callback_timer_pending = false;
                data.state.redraw = true;
                TimeoutAction::Drop
            }) {
                data.state.fatal_error = Some(format!(
                    "could not schedule hidden client frame callbacks: {error}"
                ));
                data.state.loop_signal.stop();
            }
        }
        if let Err(error) = data.display_handle.flush_clients() {
            tracing::warn!(%error, "flushing Wayland clients");
        }
        if let Err(error) = data.state.schedule_idle(&handle) {
            data.state.fatal_error = Some(error.to_string());
            data.state.loop_signal.stop();
        }
        data.state.session_idle_tick();
    });
    // Clear the persistent idle hint before removing protected display/sleep
    // coordination. The worker reports uncertainty if its bounded clear fails.
    data.state.session_idle_worker.take();
    backend.shutdown();
    run_result?;
    if let Some(error) = data.state.fatal_error {
        anyhow::bail!(error);
    }
    if let Some(error) = data.state.snapshot_error {
        anyhow::bail!("diagnostic capture failed: {error}");
    }
    Ok(())
}
