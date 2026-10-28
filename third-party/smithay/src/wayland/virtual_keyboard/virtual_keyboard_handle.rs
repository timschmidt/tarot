use std::os::unix::io::OwnedFd;
use std::{
    collections::HashMap,
    fmt,
    sync::{Arc, Mutex},
};

use tracing::debug;
use wayland_protocols_misc::zwp_virtual_keyboard_v1::server::zwp_virtual_keyboard_v1::Error::NoKeymap;
use wayland_protocols_misc::zwp_virtual_keyboard_v1::server::zwp_virtual_keyboard_v1::{
    self, ZwpVirtualKeyboardV1,
};
use wayland_server::{
    backend::ClientId,
    protocol::{
        wl_keyboard::{KeyState, KeymapFormat},
        wl_surface::WlSurface,
    },
    Client, DataInit, Dispatch, DisplayHandle, Resource,
};
use xkbcommon::xkb;

use crate::input::keyboard::{KeyboardTarget, KeymapFile, ModifiersState};
use crate::{
    input::{Seat, SeatHandler},
    utils::{Serial, SERIAL_COUNTER},
    wayland::seat::{keyboard::for_each_focused_kbds, WaylandFocus},
};

use super::VirtualKeyboardManagerState;

#[derive(Debug, Default)]
pub(crate) struct VirtualKeyboard {
    state: Option<VirtualKeyboardState>,
}

struct VirtualKeyboardState {
    keymap: KeymapFile,
    mods: ModifiersState,
    state: xkb::State,
    pressed: HashMap<u32, (WlSurface, Option<Serial>)>,
}

impl fmt::Debug for VirtualKeyboardState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VirtualKeyboardState")
            .field("keymap", &self.keymap)
            .field("mods", &self.mods)
            .field("state", &self.state.get_raw_ptr())
            .finish()
    }
}

// This is OK because all parts of `xkb` will remain on the
// same thread
unsafe impl Send for VirtualKeyboard {}

/// Handle to a virtual keyboard instance
#[derive(Debug, Clone, Default)]
pub(crate) struct VirtualKeyboardHandle {
    pub(crate) inner: Arc<Mutex<VirtualKeyboard>>,
}

/// User data of ZwpVirtualKeyboardV1 object
pub struct VirtualKeyboardUserData<D: SeatHandler> {
    pub(super) handle: VirtualKeyboardHandle,
    pub(crate) seat: Seat<D>,
}

impl<D: SeatHandler> fmt::Debug for VirtualKeyboardUserData<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VirtualKeyboardUserData")
            .field("handle", &self.handle)
            .field("seat", &self.seat.arc)
            .finish()
    }
}

impl<D> Dispatch<ZwpVirtualKeyboardV1, VirtualKeyboardUserData<D>, D> for VirtualKeyboardManagerState
where
    D: Dispatch<ZwpVirtualKeyboardV1, VirtualKeyboardUserData<D>>,
    D: SeatHandler + 'static,
    <D as SeatHandler>::KeyboardFocus: WaylandFocus,
{
    fn request(
        user_data: &mut D,
        _client: &Client,
        virtual_keyboard: &ZwpVirtualKeyboardV1,
        request: zwp_virtual_keyboard_v1::Request,
        data: &VirtualKeyboardUserData<D>,
        _dh: &DisplayHandle,
        _data_init: &mut DataInit<'_, D>,
    ) {
        if matches!(
            &request,
            zwp_virtual_keyboard_v1::Request::Key { .. } | zwp_virtual_keyboard_v1::Request::Modifiers { .. }
        ) && !user_data.allow_virtual_keyboard_input(&data.seat)
        {
            // A previously bound input method is not a protected input source.
            // Also discard its retained modifiers so they cannot reappear when
            // ordinary input resumes, even if it omits a release/reset request.
            if let Some(state) = data.handle.inner.lock().unwrap().state.as_mut() {
                state.state.update_mask(0, 0, 0, 0, 0, 0);
                state.mods.update_with(&state.state);
                state.pressed.clear();
            }
            return;
        }
        match request {
            zwp_virtual_keyboard_v1::Request::Keymap { format, fd, size } => {
                update_keymap(data, format, fd, size as usize);
            }
            zwp_virtual_keyboard_v1::Request::Key { time, key, state } => {
                // Ensure keymap was initialized.
                let mut virtual_data = data.handle.inner.lock().unwrap();
                let vk_state = match virtual_data.state.as_mut() {
                    Some(vk_state) => vk_state,
                    None => {
                        virtual_keyboard.post_error(NoKeymap, "`key` sent before keymap.");
                        return;
                    }
                };

                // Ensure virtual keyboard's keymap is active.
                let keyboard_handle = data.seat.get_keyboard().unwrap();
                let mut internal = keyboard_handle.arc.internal.lock().unwrap();
                let focus = internal.focus.as_mut().map(|(focus, _)| focus);
                keyboard_handle.send_keymap(
                    user_data,
                    &focus,
                    &vk_state.keymap,
                    vk_state.mods,
                    Some(virtual_keyboard.id()),
                );

                if state != 1 {
                    vk_state.pressed.remove(&key);
                }
                if let Some(wl_surface) = focus.and_then(|f| f.wl_surface()) {
                    if state == 1 {
                        vk_state.pressed.insert(
                            key,
                            (wl_surface.clone().into_owned(), keyboard_handle.last_enter()),
                        );
                    }
                    for_each_focused_kbds(&data.seat, &wl_surface, |kbd| {
                        // This should be wl_keyboard::KeyState, but the protocol does not state
                        // the parameter is an enum.
                        let key_state = if state == 1 {
                            KeyState::Pressed
                        } else {
                            KeyState::Released
                        };

                        kbd.key(SERIAL_COUNTER.next_serial().0, time, key, key_state);
                    });
                }
            }
            zwp_virtual_keyboard_v1::Request::Modifiers {
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
            } => {
                // Ensure keymap was initialized.
                let mut virtual_data = data.handle.inner.lock().unwrap();
                let state = match virtual_data.state.as_mut() {
                    Some(state) => state,
                    None => {
                        virtual_keyboard.post_error(NoKeymap, "`modifiers` sent before keymap.");
                        return;
                    }
                };

                // Update virtual keyboard's modifier state.
                state
                    .state
                    .update_mask(mods_depressed, mods_latched, mods_locked, 0, 0, group);
                state.mods.update_with(&state.state);

                // Ensure virtual keyboard's keymap is active.
                let keyboard_handle = data.seat.get_keyboard().unwrap();
                let mut internal = keyboard_handle.arc.internal.lock().unwrap();
                let focus = internal.focus.as_mut().map(|(focus, _)| focus);
                let state_sent = keyboard_handle.send_keymap(
                    user_data,
                    &focus,
                    &state.keymap,
                    state.mods,
                    Some(virtual_keyboard.id()),
                );

                // Report modifiers change to all keyboards.
                if !state_sent {
                    if let Some(focus) = focus {
                        focus.modifiers(&data.seat, user_data, state.mods, SERIAL_COUNTER.next_serial());
                    }
                }
            }
            zwp_virtual_keyboard_v1::Request::Destroy => {
                // Nothing to do
            }
            _ => unreachable!(),
        }
    }

    fn destroyed(
        state: &mut D,
        _client: ClientId,
        virtual_keyboard: &ZwpVirtualKeyboardV1,
        data: &VirtualKeyboardUserData<D>,
    ) {
        let Some(virtual_state) = data.handle.inner.lock().unwrap().state.take() else {
            return;
        };
        let Some(keyboard) = data.seat.get_keyboard() else {
            return;
        };
        // Keymap IDs identify map contents, not the protocol object that owns
        // the active modifiers. Inactive objects must not reset another source.
        let owned = keyboard
            .arc
            .active_virtual_keyboard
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|(id, _)| id == &virtual_keyboard.id());
        if !owned {
            return;
        }
        let ordinary = state.allow_virtual_keyboard_input(&data.seat);
        let mut internal = keyboard.arc.internal.lock().unwrap();
        let physical_keys = internal.forwarded_pressed_keys.clone();
        let modifiers = internal.mods_state;
        let focus = internal.focus.as_mut().map(|(focus, _)| focus);
        if let Some(surface) = focus.as_ref().and_then(|f| f.wl_surface()).filter(|_| ordinary) {
            let enter = keyboard.last_enter();
            for (key, (target, serial)) in virtual_state.pressed {
                if surface.as_ref() == &target
                    && enter == serial
                    && !physical_keys
                        .iter()
                        .any(|code| code.raw().checked_sub(8) == Some(key))
                {
                    for_each_focused_kbds(&data.seat, &surface, |kbd| {
                        kbd.key(SERIAL_COUNTER.next_serial().0, 0, key, KeyState::Released);
                    });
                }
            }
        }
        // Restore actual physical state, not a fabricated all-zero state.
        // This also resets a focused client's modifiers without a new key.
        let keymap = keyboard.arc.keymap.lock().unwrap();
        keyboard.send_keymap(state, &focus, &keymap, modifiers, None);
    }
}

/// Handle the zwp_virtual_keyboard_v1::keymap request.
///
/// The `true` returns when keymap was properly loaded.
fn update_keymap<D>(data: &VirtualKeyboardUserData<D>, format: u32, fd: OwnedFd, size: usize)
where
    D: SeatHandler + 'static,
{
    // Only libxkbcommon compatible keymaps are supported.
    if format != KeymapFormat::XkbV1 as u32 {
        debug!("Unsupported keymap format: {format:?}");
        return;
    }

    let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
    // SAFETY: we can map the keymap into the memory.
    let new_keymap = match unsafe {
        xkb::Keymap::new_from_fd(
            &context,
            fd,
            size,
            xkb::KEYMAP_FORMAT_TEXT_V1,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
    } {
        Ok(Some(new_keymap)) => new_keymap,
        Ok(None) => {
            debug!("Invalid libxkbcommon keymap");
            return;
        }
        Err(err) => {
            debug!("Could not map the keymap: {err:?}");
            return;
        }
    };

    // Store active virtual keyboard map.
    let mut inner = data.handle.inner.lock().unwrap();
    let mods = inner.state.take().map(|state| state.mods).unwrap_or_default();
    inner.state = Some(VirtualKeyboardState {
        mods,
        keymap: KeymapFile::new(&new_keymap),
        state: xkb::State::new(&new_keymap),
        pressed: HashMap::new(),
    });
}
