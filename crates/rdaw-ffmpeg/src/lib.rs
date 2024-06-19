use std::ffi::c_int;

mod decoder;
mod error;
mod frame;
mod input;
mod media;
mod packet;
mod reader;

pub use self::decoder::Decoder;
pub use self::error::{Error, ErrorKind, Result};
pub use self::frame::{FilledFrame, Frame};
pub use self::input::InputContext;
pub use self::packet::{FilledPacket, Packet};
pub use self::reader::ReaderContext;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct StreamIdx(pub c_int);
