//! # JSON-RPC 2.0 with Unix File Descriptor Passing
//!
//! This crate provides an implementation of JSON-RPC 2.0 with file descriptor passing over Unix
//! domain sockets. It enables reliable inter-process communication (IPC) with the ability to
//! pass file descriptors alongside JSON-RPC messages.
//!
//! ## Features
//!
//! - **JSON-RPC 2.0 compliance**: Full support for requests, responses, and notifications
//! - **File descriptor passing**: Pass file descriptors using Unix socket ancillary data
//! - **Streaming JSON parsing**: Self-delimiting JSON messages without newline requirements
//! - **Async support**: Built on tokio for high-performance async I/O
//! - **Type-safe**: Rust's type system ensures correct message handling
//!
//! ## Quick Start
//!
//! ### Server Example
//!
//! ```rust,no_run
//! use jsonrpc_fdpass::{Server, Result};
//! use std::fs::File;
//! use serde_json::Value;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     let mut server = Server::new();
//!     
//!     server.register_method("read_file", |_method, _params, fds| {
//!         if let Some(fd) = fds.into_iter().next() {
//!             let mut file = File::from(fd);
//!             let mut contents = String::new();
//!             use std::io::Read;
//!             file.read_to_string(&mut contents).unwrap();
//!             Ok((Some(Value::String(contents)), Vec::new()))
//!         } else {
//!             Err(jsonrpc_fdpass::Error::InvalidMessage("No FD provided".into()))
//!         }
//!     });
//!     
//!     server.listen("/tmp/test.sock").await
//! }
//! ```
//!
//! ### Client Example
//!
//! ```rust,no_run
//! use jsonrpc_fdpass::{
//!     JsonRpcRequest, JsonRpcMessage, MessageWithFds, UnixSocketTransport, Result
//! };
//! use std::fs::File;
//! use std::os::unix::io::OwnedFd;
//! use serde_json::json;
//! use tokio::net::UnixStream;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     let stream = UnixStream::connect("/tmp/test.sock").await?;
//!     let transport = UnixSocketTransport::new(stream);
//!     let (mut sender, mut receiver) = transport.split();
//!     
//!     let file = File::open("example.txt").unwrap();
//!     let fd: OwnedFd = file.into();
//!     
//!     let request = JsonRpcRequest::new(
//!         "read_file".to_string(),
//!         Some(json!({"filename": "example.txt"})),
//!         json!(1),
//!     );
//!     let message = MessageWithFds::new(JsonRpcMessage::Request(request), vec![fd]);
//!     sender.send(message).await?;
//!     
//!     let response = receiver.receive().await?;
//!     println!("Response: {:?}", response.message);
//!     
//!     Ok(())
//! }
//! ```
//!
//! ## Protocol Details
//!
//! This implementation is a minimal extension to JSON-RPC 2.0 that adds file descriptor
//! passing over Unix domain sockets:
//!
//! - Uses Unix domain sockets (SOCK_STREAM)
//! - Standard JSON-RPC 2.0 message format with no additional framing requirements
//! - JSON objects are self-delimiting; no newline or length-prefix framing is required
//! - File descriptors are passed as ancillary data via sendmsg(2)/recvmsg(2)
//! - Each sendmsg() call contains exactly one complete JSON-RPC message
//!
//! ### File Descriptor Count Field
//!
//! When file descriptors are attached to a message, the `fds` field at the top level
//! of the JSON object specifies how many FDs are attached:
//!
//! ```json
//! {
//!   "jsonrpc": "2.0",
//!   "method": "read_file",
//!   "params": {"filename": "example.txt"},
//!   "id": 1,
//!   "fds": 1
//! }
//! ```
//!
//! File descriptors are passed positionally—the application layer defines the semantic
//! mapping between FD positions and parameters.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

/// Error types for JSON-RPC operations.
pub mod error;
/// JSON-RPC message types and serialization.
pub mod message;
/// JSON-RPC 2.0 server implementation with file descriptor passing.
pub mod server;
/// Low-level Unix socket transport with ancillary data support.
pub mod transport;

pub use error::{Error, Result};
pub use jsonrpsee::types::error::{
    CALL_EXECUTION_FAILED_CODE, ErrorCode, ErrorObject as JsonRpcError, INTERNAL_ERROR_CODE,
    INTERNAL_ERROR_MSG, INVALID_PARAMS_CODE, INVALID_PARAMS_MSG, INVALID_REQUEST_CODE,
    INVALID_REQUEST_MSG, METHOD_NOT_FOUND_CODE, METHOD_NOT_FOUND_MSG, PARSE_ERROR_CODE,
    PARSE_ERROR_MSG,
};
pub use message::{
    FILE_DESCRIPTOR_ERROR_CODE, JsonRpcMessage, JsonRpcNotification, JsonRpcRequest,
    JsonRpcResponse, MessageWithFds, file_descriptor_error,
};
pub use server::Server;
pub use transport::{DEFAULT_MAX_FDS_PER_SENDMSG, Receiver, Sender, UnixSocketTransport};
