use thiserror::Error;

// TODO: Reorganizes errors
#[derive(Error, PartialEq, Clone, Debug)]
pub enum Error {
    #[error("the current window size is invalid")]
    InvalidWindowSize,
    #[error("key or click was not sent due to the window not focused or other error")]
    KeyNotSent,
    #[error("window matching provided class and title cannot be found")]
    WindowNotFound,
    #[error("capture frame is not available")]
    FrameNotAvailable,
    #[error("key not found")]
    KeyNotFound,
    #[error("macOS API error {0}: {1}")]
    MacOS(u32, String),
}

impl Error {
    #[inline]
    pub(crate) fn from_last_mac_error() -> Error {
        // Default implementation for macOS errors
        Error::MacOS(0, "Unknown macOS error".to_string())
    }
}