//! Native text-input bridge. Only the explicitly launched helper can bind IME.

use crate::{Compositor, state::ClientState};
use anyhow::{Context, Result};
use smithay::{
    delegate_input_method_manager, delegate_text_input_manager,
    desktop::{PopupKind, PopupManager, find_popup_root_surface},
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Rectangle},
    wayland::input_method::{InputMethodHandler, PopupSurface},
};
use std::{
    ffi::OsString,
    os::{
        fd::AsRawFd,
        unix::{net::UnixStream, process::CommandExt},
    },
    path::Path,
    process::{Child, Command, Stdio},
    sync::Arc,
};

pub struct SpawnedInputMethod {
    pub child: Child,
    pub control: UnixStream,
    pub client: smithay::reexports::wayland_server::Client,
}

pub fn spawn_input_method(
    state: &Compositor,
    program: &Path,
    args: &[OsString],
) -> Result<SpawnedInputMethod> {
    let (server, client) =
        UnixStream::pair().context("creating private input-method connection")?;
    let (control, child_control) =
        UnixStream::pair().context("creating private keyboard control")?;
    control.set_nonblocking(true)?;
    let registered = state
        .display_handle
        .clone()
        .insert_client(
            server,
            Arc::new(ClientState {
                trusted_input_method: true,
                ..Default::default()
            }),
        )
        .context("registering trusted input method")?;
    let mut command = Command::new(program);
    command
        .args(args)
        .env("WAYLAND_SOCKET", client.as_raw_fd().to_string())
        .env(
            "TAROT_KEYBOARD_CONTROL_FD",
            child_control.as_raw_fd().to_string(),
        )
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("DISPLAY")
        .stdin(Stdio::null());
    // SAFETY: the child performs only the async-signal-safe fcntl syscall
    // before exec. The captured stream is owned, and stays open through spawn.
    // Other clients never receive this socket or the trusted ClientState.
    unsafe {
        command.pre_exec(move || {
            rustix::io::fcntl_setfd(&client, rustix::io::FdFlags::empty())?;
            rustix::io::fcntl_setfd(&child_control, rustix::io::FdFlags::empty())?;
            Ok(())
        });
    }
    let child = command
        .spawn()
        .with_context(|| format!("launching input method {}", program.display()))?;
    Ok(SpawnedInputMethod {
        child,
        control,
        client: registered,
    })
}

impl InputMethodHandler for Compositor {
    fn new_popup(&mut self, surface: PopupSurface) {
        let _ = self.popups.track_popup(PopupKind::InputMethod(surface));
        self.redraw = true;
    }

    fn dismiss_popup(&mut self, surface: PopupSurface) {
        let popup = PopupKind::InputMethod(surface);
        if let Ok(root) = find_popup_root_surface(&popup) {
            let _ = PopupManager::dismiss_popup(&root, &popup);
        }
        self.redraw = true;
    }

    fn popup_repositioned(&mut self, _surface: PopupSurface) {
        self.redraw = true;
    }

    fn parent_geometry(&self, parent: &WlSurface) -> Rectangle<i32, Logical> {
        self.space
            .elements()
            .find(|window| {
                window
                    .toplevel()
                    .is_some_and(|top| top.wl_surface() == parent)
            })
            .map(|window| window.geometry())
            .unwrap_or_default()
    }
}

delegate_text_input_manager!(Compositor);
delegate_input_method_manager!(Compositor);
