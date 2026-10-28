//! Convert compositor coordinates at the final input target, including held grabs.
//! Smithay keeps the original target during a grab; its origin remains zero here.

use crate::Compositor;
use smithay::{
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

/// The exact physical rectangles passed to Smithay's Fit/CENTER render transform.
#[derive(Clone, Copy, Debug)]
pub struct RenderGeometry {
    pub target: Rectangle<i32, Physical>,
    pub reference: Rectangle<i32, Physical>,
    pub opacity: f32,
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
            output_scale,
        })
    }

    fn factor(self) -> f64 {
        (f64::from(self.target.size.w) / f64::from(self.reference.size.w))
            .min(f64::from(self.target.size.h) / f64::from(self.reference.size.h))
    }

    /// Inverse of constrain_render_elements: scale around location, then round
    /// the alignment/reference offset once, exactly as pinned Smithay does.
    pub fn to_window(self, position: Point<f64, Logical>) -> Point<f64, Logical> {
        let factor = self.factor();
        let location = self.target.loc - self.reference.loc;
        let offset: Point<f64, Physical> = (
            f64::from(self.reference.loc.x) * (1.0 - factor)
                + (f64::from(self.target.size.w) - f64::from(self.reference.size.w) * factor) / 2.0,
            f64::from(self.reference.loc.y) * (1.0 - factor)
                + (f64::from(self.target.size.h) - f64::from(self.reference.size.h) * factor) / 2.0,
        )
            .into();
        let origin = location + offset.to_i32_round();
        (
            (position.x * self.output_scale - f64::from(origin.x)) / (factor * self.output_scale),
            (position.y * self.output_scale - f64::from(origin.y)) / (factor * self.output_scale),
        )
            .into()
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
        let placement = data.presented_surfaces.get(&self.surface)?;
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
