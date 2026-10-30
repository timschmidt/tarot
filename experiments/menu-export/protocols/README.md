# GTK protocol source

`gtk-shell.xml` is the unmodified upstream file from GTK commit `b3d08d3e60136da2522de3313cf4479e1d6fe20f`. Its SHA-256 is `cc2717ba21182f637843e593f7aa2e543c1d6554a886053f400d81906967659d`. `GTK-COPYING` preserves the upstream project's accompanying license text, SHA-256 `b7993225104d90ddd8024fd838faf300bea5e83d91203eab98e29512acebd69c`. These upstream files retain their licensing; Tarot's source license does not replace it.

[Protocol at the pinned commit](https://github.com/GNOME/gtk/blob/b3d08d3e60136da2522de3313cf4479e1d6fe20f/gdk/wayland/protocol/gtk-shell.xml) · [Upstream COPYING](https://github.com/GNOME/gtk/blob/b3d08d3e60136da2522de3313cf4479e1d6fe20f/COPYING).

The isolated experiment generates Rust client/server bindings at compile time. It binds version 4 and sends zero global-menu capability flags. This does not advertise a Tarot compositor capability or establish general GTK-shell compatibility. The fixtures use the actual `get_gtk_surface` and `set_dbus_properties` requests on private, unmapped `wl_surface` resources.
