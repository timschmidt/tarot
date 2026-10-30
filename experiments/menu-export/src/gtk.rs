// SPDX-License-Identifier: MIT OR Apache-2.0
// Generated bindings use the unmodified, separately licensed upstream XML.
#![allow(
    dead_code,
    unused_imports,
    non_camel_case_types,
    non_upper_case_globals
)]
#![allow(non_snake_case, unused_unsafe, unused_variables, clippy::all)]
pub mod client {
    use wayland_client;
    use wayland_client::protocol::*;
    pub mod __interfaces {
        use wayland_client::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("protocols/gtk-shell.xml");
    }
    use __interfaces::*;
    wayland_scanner::generate_client_code!("protocols/gtk-shell.xml");
}
pub mod server {
    use wayland_server;
    use wayland_server::protocol::*;
    pub mod __interfaces {
        use wayland_server::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("protocols/gtk-shell.xml");
    }
    use __interfaces::*;
    wayland_scanner::generate_server_code!("protocols/gtk-shell.xml");
}
