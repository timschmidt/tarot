//! Private GTK/GIO menu wire adapter. Call only on an owned worker: all D-Bus
//! operations have deadlines, and a reply does not prove document completion.
mod socket;
pub mod transport;

pub mod worker;
