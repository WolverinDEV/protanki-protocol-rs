#![feature(type_name_of_val)]
#![allow(dead_code)]
#![feature(cursor_remaining)]

mod socket;
pub use socket::*;

mod connection;
pub use connection::*;

mod error;
pub use error::*;

pub mod codec;
pub mod packets;
pub mod crypto;
pub mod resources;