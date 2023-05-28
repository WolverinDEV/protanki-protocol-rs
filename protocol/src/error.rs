use std::{io, str::Utf8Error};

use thiserror::Error;

pub type ProtocolResult<T> = Result<T, ProtocolError>;

#[derive(Error, Debug)]
pub enum ConnectionClosedError {
    #[error("remote host closed the connection")]
    Disconnected,

    #[error("read failed: {0}")]
    ReadError(io::Error),
    
    #[error("write failed: {0}")]
    WriteError(io::Error),
    
    #[error("an error occurred: {0}")]
    ProtocolError(Box<ProtocolError>),
}

#[derive(Error, Debug)]
pub enum ProtocolError {
    #[error("crypto operation failed")]
    GenericError,

    #[error("packet id {0} is unknown")]
    PacketUnknownId(i32),

    #[error("packet too large ({0} bytes)")]
    PacketTooLarge(usize),

    #[error("packet too small ({0} bytes)")]
    PacketTooSmall(usize),

    #[error("tried to encode/decode a too large var int")]
    CodecVarIntTooLarge,

    #[error("decoded text contains an invalid utf-8 character")]
    CodecUtf8DecodeError(Utf8Error),

    #[error("unknown ordinal {0} for {1}")]
    EnumInvalidOrdinal(u64, String),

    #[error("unknown enum not serializable")]
    EnumUnknown,

    #[error("io error: {0}")]
    IOError(#[from] io::Error),

    #[error("connection has been closed: {0}")]
    ConnectionClosed(#[from] ConnectionClosedError),

    #[error("connection has been closed while performing an action")]
    ConnectionAborted,
    
    #[error("unexpected packet")]
    UnexpectedPacket,
}
