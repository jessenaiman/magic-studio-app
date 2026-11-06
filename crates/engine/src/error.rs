//! Engine errors: one error type for every engine command, with context.
//!
//! Format follows VectorCraft's `crates/engine/src/error.rs` (same shape, Magic variants).

use std::fmt;

#[derive(Debug)]
pub enum EngineError {
    UnknownCommand(String),
    CommandFailed(String, String),
    InvalidParams(String),
    Io(std::io::Error),
    Image(image::ImageError),
    ExternalTool(String, String),
    NotImplemented,
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::UnknownCommand(cmd) => write!(f, "unknown command `{cmd}`"),
            EngineError::CommandFailed(id, msg) => write!(f, "command `{id}` failed: {msg}"),
            EngineError::InvalidParams(msg) => write!(f, "invalid parameters: {msg}"),
            EngineError::Io(e) => write!(f, "io error: {e}"),
            EngineError::Image(e) => write!(f, "image error: {e}"),
            EngineError::ExternalTool(tool, msg) => {
                write!(f, "external tool `{tool}` failed: {msg}")
            }
            EngineError::NotImplemented => write!(f, "not yet implemented"),
        }
    }
}

impl std::error::Error for EngineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            EngineError::Io(e) => Some(e),
            EngineError::Image(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for EngineError {
    fn from(e: std::io::Error) -> Self {
        EngineError::Io(e)
    }
}

impl From<image::ImageError> for EngineError {
    fn from(e: image::ImageError) -> Self {
        EngineError::Image(e)
    }
}

pub type Result<T> = std::result::Result<T, EngineError>;
