// Client bindings for the separately licensed, pinned GTK extension.
#![allow(
    dead_code,
    unused_imports,
    non_camel_case_types,
    non_upper_case_globals,
    non_snake_case,
    unused_unsafe,
    unused_variables,
    clippy::all
)]
use wayland_client;
use wayland_client::protocol::*;
pub mod __interfaces {
    use wayland_client::protocol::__interfaces::*;
    wayland_scanner::generate_interfaces!("third-party/gtk/gtk-shell.xml");
}
use __interfaces::*;
wayland_scanner::generate_client_code!("third-party/gtk/gtk-shell.xml");
