//! Linux process ownership and native application menu transport shared by the
//! compositor and its protocol acceptance clients.
pub mod boot_destination;
pub mod local_bus;
pub mod native_menu;
pub mod process_identity;
pub mod session_idle;
pub mod sleep_policy;

pub mod polkit;
