//! Output geometry changes occur on the render thread, between page flips.
use crate::Compositor;
use smithay::utils::{Logical, Point, Transform};
use tarot_core::{Layout, orientation::Orientation};

pub fn transform(orientation: Orientation) -> Transform {
    match orientation {
        Orientation::Normal => Transform::Normal,
        Orientation::LeftUp => Transform::_90,
        Orientation::BottomUp => Transform::_180,
        Orientation::RightUp => Transform::_270,
    }
}

impl Compositor {
    pub fn request_orientation(&mut self, orientation: Orientation) {
        self.output_geometry.request(orientation);
        self.redraw = true;
    }

    pub fn configure_output_geometry(
        &mut self,
        width: u32,
        height: u32,
        scale: u32,
        orientation: Orientation,
    ) -> bool {
        let previous = self.output_geometry.clone();
        if !self
            .output_geometry
            .apply(width, height, scale, orientation)
        {
            return false;
        }
        self.ui_events.clear();
        self.cancel_input_sequences();
        tarot_ui::cancel_pointer_input(&mut self.ui_events);
        if let Some(pointer) = self.seat.get_pointer() {
            let serial = smithay::utils::SERIAL_COUNTER.next_serial();
            pointer.unset_grab(self, serial, 0);
            self.suppressed_pointer_buttons
                .extend(self.pointer_buttons.iter().copied());
            // Leave the old surface before releasing its implicit button state.
            // A release on that surface could complete an obsolete click.
            pointer.motion(
                self,
                None,
                &smithay::input::pointer::MotionEvent {
                    location: {
                        let old = pointer.current_location();
                        self.output_geometry
                            .reproject_pointer(&previous, old.x, old.y)
                            .into()
                    },
                    serial,
                    time: 0,
                },
            );
            for button in self.pointer_buttons.clone() {
                pointer.button(
                    self,
                    &smithay::input::pointer::ButtonEvent {
                        button,
                        state: smithay::backend::input::ButtonState::Released,
                        serial,
                        time: 0,
                    },
                );
            }
            pointer.frame(self);
        }
        let (width, height) = self.output_geometry.logical_size();
        self.scale = scale as i32;
        self.layout = Layout {
            width: width as f32,
            height: height as f32,
            notification_height: 0.0,
            keyboard_height: 0.0,
        };
        if let Some(keyboard) = &mut self.keyboard {
            keyboard.height = keyboard.height.clamp(1, (height * 0.5).max(1.0) as i32);
        }
        self.refresh_keyboard();
        let now = self.start_time.elapsed();
        self.notification_motion
            .retarget(self.notification_motion.target(), now, true);
        self.layout.notification_height = self.notification_motion.target();
        self.reconfigure_menu_motion();
        self.configure_windows();
        self.configure_lock_surface();
        self.animation
            .reconfigure(&self.shell, self.layout, self.start_time.elapsed());
        self.presentation = tarot_core::motion::Presentation::settled(&self.shell, self.layout);
        self.presented_windows.clear();
        self.presented_surfaces.clear();
        self.idle.render_states = None;
        self.protection.last_presented_generation = None;
        self.protection.last_presented_surface = None;
        self.protection.recovery_presented = None;
        self.protection.recovery_verified = None;
        self.protection.surface_verified = None;
        self.protection.blank_verified = None;
        self.protection.blank_verified_generation = None;
        self.ui_dirty = true;
        self.redraw = true;
        true
    }

    /// Absolute devices always report panel-normal coordinates. The nested
    /// backend's GL framebuffer flip is deliberately excluded from input.
    pub fn oriented_position(&self, normalized: Point<f64, Logical>) -> Point<f64, Logical> {
        let (x, y) = self
            .output_geometry
            .orientation
            .map_normalized(normalized.x, normalized.y);
        (
            x * f64::from(self.layout.width),
            y * f64::from(self.layout.height),
        )
            .into()
    }
}
