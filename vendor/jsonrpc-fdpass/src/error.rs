use thiserror::Error;

/// Errors that can occur during JSON-RPC operations.
#[derive(Error, Debug)]
pub enum Error {
    /// JSON serialization or deserialization failed.
    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    /// An I/O error occurred.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// The message contained invalid JSON.
    #[error("Protocol framing error: invalid JSON in message")]
    FramingError,

    /// The `fds` field in the message doesn't match the number of received file descriptors.
    #[error(
        "File descriptor count mismatch: fds field specifies {expected}, but {found} FDs available"
    )]
    MismatchedCount {
        /// Number of file descriptors specified in the `fds` field.
        expected: usize,
        /// Number of file descriptors actually available.
        found: usize,
    },

    /// A system call failed.
    #[error("System call error: {0}")]
    SystemCall(String),

    /// The connection was closed by the peer.
    #[error("Connection closed")]
    ConnectionClosed,

    /// The message format was invalid.
    #[error("Invalid message format: {0}")]
    InvalidMessage(String),
}

/// A specialized Result type for JSON-RPC operations.
pub type Result<T> = std::result::Result<T, Error>;
