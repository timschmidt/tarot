//! Explicit diagnostic metadata. The API accepts no text, key or position data.
use std::io::Write;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy)]
pub enum Stage {
    SurfaceCreate,
    SurfaceHide,
    Configure,
    ContextApply,
    ContextDone,
    ContextReset,
    TouchDown,
    TouchUp,
    TouchCancel,
    PointerPress,
    PointerRelease,
    PointerLeave,
    InputFrame,
    ActionText,
    Backspace,
    Enter,
    Hide,
}

impl Stage {
    fn name(self) -> &'static str {
        match self {
            Self::SurfaceCreate => "surface-create",
            Self::SurfaceHide => "surface-hide",
            Self::Configure => "configure",
            Self::ContextApply => "context-apply",
            Self::ContextDone => "context-done",
            Self::ContextReset => "context-reset",
            Self::TouchDown => "touch-down",
            Self::TouchUp => "touch-up",
            Self::TouchCancel => "touch-cancel",
            Self::PointerPress => "pointer-press",
            Self::PointerRelease => "pointer-release",
            Self::PointerLeave => "pointer-leave",
            Self::InputFrame => "input-frame",
            Self::ActionText => "action-text",
            Self::Backspace => "backspace",
            Self::Enter => "enter",
            Self::Hide => "hide",
        }
    }
}

pub struct Snapshot {
    pub serial: u32,
    pub generation: u64,
    pub active: bool,
    pub hidden: bool,
    pub configured: bool,
    pub frame_pending: bool,
    pub pending_activation: bool,
    pub queued: usize,
    pub contacts: usize,
}

impl Snapshot {
    pub fn emit(self, stage: Stage, elapsed: Duration, frame_events: usize, actions: usize) {
        let unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let record = format!(
            "{{\"event\":\"keyboard-metadata\",\"stage\":\"{}\",\"unix_ms\":{},\"elapsed_ms\":{},\"serial\":{},\"generation\":{},\"active\":{},\"hidden\":{},\"configured\":{},\"frame_pending\":{},\"pending_activation\":{},\"queued\":{},\"contacts\":{},\"frame_events\":{},\"actions\":{}}}\n",
            stage.name(),
            unix_ms,
            elapsed.as_millis(),
            self.serial,
            self.generation,
            self.active,
            self.hidden,
            self.configured,
            self.frame_pending,
            self.pending_activation,
            self.queued,
            self.contacts,
            frame_events,
            actions,
        );
        // Formatting directly into stderr can split one record across writes
        // and interleave it with another child's output in the session log.
        let _ = std::io::stderr().lock().write_all(record.as_bytes());
    }
}
