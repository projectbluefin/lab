use crate::error::{Error, Result};
use jsonrpsee::types::error::ErrorObject as JsonRpcError;
use serde::{Deserialize, Serialize};
use std::os::unix::io::OwnedFd;

/// The JSON key for the file descriptor count field.
pub const FDS_KEY: &str = "fds";
/// The JSON-RPC protocol version.
pub const JSONRPC_VERSION: &str = "2.0";

/// Read the file descriptor count from a JSON message.
/// Returns 0 if the `fds` field is absent.
pub fn get_fd_count(value: &serde_json::Value) -> usize {
    value
        .get(FDS_KEY)
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or(0)
}

/// Helper to skip serializing fds field when it's None or 0
fn skip_if_zero_or_none(fds: &Option<usize>) -> bool {
    fds.is_none_or(|n| n == 0)
}

/// A JSON-RPC 2.0 request message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    /// The JSON-RPC protocol version (always "2.0").
    pub jsonrpc: String,
    /// The name of the method to invoke.
    pub method: String,
    /// Optional parameters for the method.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
    /// The request identifier.
    pub id: serde_json::Value,
    /// Number of file descriptors attached to this message.
    #[serde(skip_serializing_if = "skip_if_zero_or_none")]
    pub fds: Option<usize>,
}

/// A JSON-RPC 2.0 response message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    /// The JSON-RPC protocol version (always "2.0").
    pub jsonrpc: String,
    /// The result of the method invocation (present on success).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// The error object (present on failure).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError<'static>>,
    /// The request identifier this response corresponds to.
    pub id: serde_json::Value,
    /// Number of file descriptors attached to this message.
    #[serde(skip_serializing_if = "skip_if_zero_or_none")]
    pub fds: Option<usize>,
}

/// A JSON-RPC 2.0 notification message (a request without an id).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    /// The JSON-RPC protocol version (always "2.0").
    pub jsonrpc: String,
    /// The name of the method to invoke.
    pub method: String,
    /// Optional parameters for the method.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
    /// Number of file descriptors attached to this message.
    #[serde(skip_serializing_if = "skip_if_zero_or_none")]
    pub fds: Option<usize>,
}

/// A JSON-RPC 2.0 message (request, response, or notification).
#[derive(Debug, Clone)]
pub enum JsonRpcMessage {
    /// A request message expecting a response.
    Request(JsonRpcRequest),
    /// A response to a previous request.
    Response(JsonRpcResponse),
    /// A notification (no response expected).
    Notification(JsonRpcNotification),
}

impl JsonRpcRequest {
    /// Create a new JSON-RPC request.
    pub fn new(method: String, params: Option<serde_json::Value>, id: serde_json::Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method,
            params,
            id,
            fds: None,
        }
    }
}

impl JsonRpcResponse {
    /// Create a successful JSON-RPC response.
    pub fn success(result: serde_json::Value, id: serde_json::Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            result: Some(result),
            error: None,
            id,
            fds: None,
        }
    }

    /// Create an error JSON-RPC response.
    pub fn error(error: JsonRpcError<'static>, id: serde_json::Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            result: None,
            error: Some(error),
            id,
            fds: None,
        }
    }
}

impl JsonRpcNotification {
    /// Create a new JSON-RPC notification.
    pub fn new(method: String, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method,
            params,
            fds: None,
        }
    }
}

impl JsonRpcMessage {
    /// Convert this message to a JSON value.
    pub fn to_json_value(&self) -> Result<serde_json::Value> {
        match self {
            JsonRpcMessage::Request(req) => Ok(serde_json::to_value(req)?),
            JsonRpcMessage::Response(res) => Ok(serde_json::to_value(res)?),
            JsonRpcMessage::Notification(notif) => Ok(serde_json::to_value(notif)?),
        }
    }

    /// Parse a JSON-RPC message from a JSON value.
    pub fn from_json_value(value: serde_json::Value) -> Result<Self> {
        if let serde_json::Value::Object(obj) = &value {
            if obj.contains_key("method") && obj.contains_key("id") {
                let request: JsonRpcRequest = serde_json::from_value(value)?;
                Ok(JsonRpcMessage::Request(request))
            } else if obj.contains_key("result") || obj.contains_key("error") {
                let response: JsonRpcResponse = serde_json::from_value(value)?;
                Ok(JsonRpcMessage::Response(response))
            } else if obj.contains_key("method") {
                let notification: JsonRpcNotification = serde_json::from_value(value)?;
                Ok(JsonRpcMessage::Notification(notification))
            } else {
                Err(Error::InvalidMessage("Invalid JSON-RPC message".into()))
            }
        } else {
            Err(Error::InvalidMessage("Expected JSON object".into()))
        }
    }
}

/// A JSON-RPC message paired with file descriptors to send or that were received.
#[derive(Debug)]
pub struct MessageWithFds {
    /// The JSON-RPC message.
    pub message: JsonRpcMessage,
    /// File descriptors attached to this message.
    pub file_descriptors: Vec<OwnedFd>,
}

impl JsonRpcMessage {
    /// Set the fds count on the message
    pub fn set_fds(&mut self, count: usize) {
        let fds = if count > 0 { Some(count) } else { None };
        match self {
            JsonRpcMessage::Request(req) => req.fds = fds,
            JsonRpcMessage::Response(res) => res.fds = fds,
            JsonRpcMessage::Notification(notif) => notif.fds = fds,
        }
    }

    /// Get the fds count from the message
    pub fn get_fds(&self) -> usize {
        match self {
            JsonRpcMessage::Request(req) => req.fds.unwrap_or(0),
            JsonRpcMessage::Response(res) => res.fds.unwrap_or(0),
            JsonRpcMessage::Notification(notif) => notif.fds.unwrap_or(0),
        }
    }
}

impl MessageWithFds {
    /// Create a new message with file descriptors.
    pub fn new(message: JsonRpcMessage, file_descriptors: Vec<OwnedFd>) -> Self {
        Self {
            message,
            file_descriptors,
        }
    }

    /// Serialize the message, setting the `fds` field to match the number of attached FDs.
    pub fn serialize(&self) -> Result<String> {
        self.serialize_impl(false)
    }

    /// Serialize the message with pretty-printing.
    pub fn serialize_pretty(&self) -> Result<String> {
        self.serialize_impl(true)
    }

    fn serialize_impl(&self, pretty: bool) -> Result<String> {
        // Clone the message so we can set the fds field
        let mut message = self.message.clone();
        message.set_fds(self.file_descriptors.len());

        let message_json = message.to_json_value()?;
        let json_str = if pretty {
            serde_json::to_string_pretty(&message_json)?
        } else {
            serde_json::to_string(&message_json)?
        };
        Ok(json_str)
    }

    /// Create a MessageWithFds from parsed JSON and file descriptors.
    /// The `fds` field in the JSON must match the number of provided FDs.
    pub fn from_json_with_fds(json_str: &str, fds: Vec<OwnedFd>) -> Result<Self> {
        let message_json: serde_json::Value = serde_json::from_str(json_str)?;
        let expected_count = get_fd_count(&message_json);

        if expected_count != fds.len() {
            return Err(Error::MismatchedCount {
                expected: expected_count,
                found: fds.len(),
            });
        }

        let message = JsonRpcMessage::from_json_value(message_json)?;
        Ok(Self::new(message, fds))
    }
}

/// JSON-RPC error code for file descriptor errors.
pub const FILE_DESCRIPTOR_ERROR_CODE: i32 = -32050;

/// Create a JSON-RPC error object for file descriptor errors.
pub fn file_descriptor_error() -> JsonRpcError<'static> {
    JsonRpcError::owned(
        FILE_DESCRIPTOR_ERROR_CODE,
        "File Descriptor Error",
        None::<serde_json::Value>,
    )
}

#[cfg(kani)]
mod verification {
    use super::*;

    // =========================================================================
    // Proofs for skip_if_zero_or_none
    // =========================================================================

    /// Verify that skip_if_zero_or_none returns true for None
    #[kani::proof]
    fn check_skip_none() {
        let result = skip_if_zero_or_none(&None);
        kani::assert(result, "None should be skipped");
    }

    /// Verify that skip_if_zero_or_none returns true for Some(0)
    #[kani::proof]
    fn check_skip_zero() {
        let result = skip_if_zero_or_none(&Some(0));
        kani::assert(result, "Some(0) should be skipped");
    }

    /// Verify that skip_if_zero_or_none returns false for any non-zero value
    #[kani::proof]
    fn check_skip_nonzero() {
        let n: usize = kani::any();
        kani::assume(n > 0);
        let result = skip_if_zero_or_none(&Some(n));
        kani::assert(!result, "Some(n > 0) should not be skipped");
    }

    // =========================================================================
    // Proofs for JsonRpcMessage::get_fds
    // =========================================================================

    /// Verify get_fds returns 0 when fds field is None
    #[kani::proof]
    fn check_get_fds_none() {
        let msg = JsonRpcMessage::Notification(JsonRpcNotification {
            jsonrpc: String::new(),
            method: String::new(),
            params: None,
            fds: None,
        });
        let result = msg.get_fds();
        kani::assert(result == 0, "None fds should return 0");
    }

    /// Verify get_fds returns the value when fds field is Some(n)
    #[kani::proof]
    fn check_get_fds_some() {
        let n: usize = kani::any();
        let msg = JsonRpcMessage::Notification(JsonRpcNotification {
            jsonrpc: String::new(),
            method: String::new(),
            params: None,
            fds: Some(n),
        });
        let result = msg.get_fds();
        kani::assert(result == n, "get_fds should return the fds value");
    }
}
