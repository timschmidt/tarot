// Generated from the separately licensed, unmodified protocol in third-party/gtk.
#![allow(
    dead_code,
    unused_imports,
    non_camel_case_types,
    non_upper_case_globals
)]
#![allow(non_snake_case, unused_unsafe, unused_variables, clippy::all)]
use smithay::reexports::wayland_server;
use wayland_server::protocol::*;
pub mod __interfaces {
    use smithay::reexports::wayland_server;
    use wayland_server::protocol::__interfaces::*;
    wayland_scanner::generate_interfaces!("third-party/gtk/gtk-shell.xml");
}
use __interfaces::*;
wayland_scanner::generate_server_code!("third-party/gtk/gtk-shell.xml");
