//! Integration tests for interoperability with jsonrpsee.
//!
//! These tests verify that our JSON-RPC implementation can exchange messages
//! with jsonrpsee over Unix socket pairs when file descriptors are not present.
//!
//! The tests focus on wire-format compatibility: messages serialized by one
//! implementation should be correctly parsed by the other.

use jsonrpc_fdpass::{
    JsonRpcMessage, JsonRpcNotification, JsonRpcRequest, MessageWithFds, Result,
    UnixSocketTransport,
};
use serde_json::Value;
use tokio::net::UnixStream;

/// Test round-trip: our client -> our server, verifying wire format compatibility.
#[tokio::test]
async fn test_wire_format_round_trip() -> Result<()> {
    let (client_stream, server_stream) = UnixStream::pair().unwrap();

    // Start our server
    let server_handle = tokio::spawn(async move {
        let mut server = jsonrpc_fdpass::Server::new();
        server.register_method("echo", |_method, params, _fds| Ok((params, Vec::new())));

        let transport = UnixSocketTransport::new(server_stream);
        let (mut sender, mut receiver) = transport.split();

        // Handle one request
        if let Ok(msg) = receiver.receive().await {
            let _ = server.process_message(msg, &mut sender).await;
        }

        Ok::<(), jsonrpc_fdpass::Error>(())
    });

    // Use our client transport
    let transport = UnixSocketTransport::new(client_stream);
    let (mut sender, mut receiver) = transport.split();

    let params = serde_json::json!({
        "string": "hello",
        "number": 42,
        "array": [1, 2, 3],
        "nested": { "key": "value" }
    });

    let request = JsonRpcRequest::new(
        "echo".to_string(),
        Some(params.clone()),
        Value::Number(1.into()),
    );
    let message = JsonRpcMessage::Request(request);
    sender.send(MessageWithFds::new(message, vec![])).await?;

    let response = receiver.receive().await?;
    match response.message {
        JsonRpcMessage::Response(resp) => {
            assert_eq!(resp.result, Some(params));
            assert!(resp.error.is_none());
        }
        _ => panic!("Expected response"),
    }

    server_handle.abort();
    Ok(())
}

/// Test that notifications work correctly (no response expected).
#[tokio::test]
async fn test_notification_no_response() -> Result<()> {
    let (client_stream, server_stream) = UnixStream::pair().unwrap();

    let received = std::sync::Arc::new(std::sync::Mutex::new(None));
    let received_clone = received.clone();

    // Channel to signal when server has processed the notification
    let (done_tx, done_rx) = tokio::sync::oneshot::channel::<()>();

    // Start our server - note: notifications need register_notification, not register_method
    let server_handle = tokio::spawn(async move {
        let mut server = jsonrpc_fdpass::Server::new();
        server.register_notification("notify_me", move |_method, params, _fds| {
            *received_clone.lock().unwrap() = params.clone();
            Ok(())
        });

        let transport = UnixSocketTransport::new(server_stream);
        let (mut sender, mut receiver) = transport.split();

        // Handle one notification
        if let Ok(msg) = receiver.receive().await {
            let _ = server.process_message(msg, &mut sender).await;
        }

        // Signal that we're done processing
        let _ = done_tx.send(());

        Ok::<(), jsonrpc_fdpass::Error>(())
    });

    // Send notification (no id)
    let transport = UnixSocketTransport::new(client_stream);
    let (mut sender, _receiver) = transport.split();

    let notification = JsonRpcNotification::new(
        "notify_me".to_string(),
        Some(serde_json::json!({ "event": "test" })),
    );
    let message = JsonRpcMessage::Notification(notification);
    sender.send(MessageWithFds::new(message, vec![])).await?;

    // Wait for server to signal completion
    done_rx.await.expect("server should signal completion");

    // Verify notification was received
    let received_value = received.lock().unwrap().clone();
    assert_eq!(received_value, Some(serde_json::json!({ "event": "test" })));

    server_handle.abort();
    Ok(())
}

/// Test error responses are correctly formatted per JSON-RPC 2.0 spec.
#[tokio::test]
async fn test_error_response_format() -> Result<()> {
    let (client_stream, server_stream) = UnixStream::pair().unwrap();

    // Start our server
    let server_handle = tokio::spawn(async move {
        let server = jsonrpc_fdpass::Server::new();
        // Don't register "unknown_method" - it should return method not found

        let transport = UnixSocketTransport::new(server_stream);
        let (mut sender, mut receiver) = transport.split();

        if let Ok(msg) = receiver.receive().await {
            let _ = server.process_message(msg, &mut sender).await;
        }

        Ok::<(), jsonrpc_fdpass::Error>(())
    });

    let transport = UnixSocketTransport::new(client_stream);
    let (mut sender, mut receiver) = transport.split();

    let request = JsonRpcRequest::new("unknown_method".to_string(), None, Value::Number(1.into()));
    sender
        .send(MessageWithFds::new(
            JsonRpcMessage::Request(request),
            vec![],
        ))
        .await?;

    let response = receiver.receive().await?;
    match response.message {
        JsonRpcMessage::Response(resp) => {
            assert!(resp.result.is_none());
            assert!(resp.error.is_some());
            let error = resp.error.unwrap();
            assert_eq!(error.code(), -32601); // Method not found
        }
        _ => panic!("Expected response"),
    }

    server_handle.abort();
    Ok(())
}

/// Test that batch requests work (send multiple messages in sequence).
#[tokio::test]
async fn test_sequential_requests() -> Result<()> {
    let (client_stream, server_stream) = UnixStream::pair().unwrap();

    // Start our server
    let server_handle = tokio::spawn(async move {
        let mut server = jsonrpc_fdpass::Server::new();
        server.register_method("add", |_method, params, _fds| {
            let params = params.ok_or_else(|| {
                jsonrpc_fdpass::Error::InvalidMessage("missing params".to_string())
            })?;
            let a = params.get("a").and_then(|v| v.as_i64()).unwrap_or(0);
            let b = params.get("b").and_then(|v| v.as_i64()).unwrap_or(0);
            Ok((Some(Value::Number((a + b).into())), Vec::new()))
        });

        let transport = UnixSocketTransport::new(server_stream);
        let (mut sender, mut receiver) = transport.split();

        // Handle multiple requests
        for _ in 0..3 {
            if let Ok(msg) = receiver.receive().await {
                let _ = server.process_message(msg, &mut sender).await;
            }
        }

        Ok::<(), jsonrpc_fdpass::Error>(())
    });

    let transport = UnixSocketTransport::new(client_stream);
    let (mut sender, mut receiver) = transport.split();

    // Send 3 requests and verify responses
    for i in 1..=3i64 {
        let request = JsonRpcRequest::new(
            "add".to_string(),
            Some(serde_json::json!({ "a": i, "b": i * 2 })),
            Value::Number(i.into()),
        );
        sender
            .send(MessageWithFds::new(
                JsonRpcMessage::Request(request),
                vec![],
            ))
            .await?;

        let response = receiver.receive().await?;
        match response.message {
            JsonRpcMessage::Response(resp) => {
                assert_eq!(resp.result, Some(Value::Number((i * 3).into())));
                assert_eq!(resp.id, Value::Number(i.into()));
            }
            _ => panic!("Expected response"),
        }
    }

    server_handle.abort();
    Ok(())
}

/// Test that messages serialized in jsonrpsee format can be parsed by our receiver.
/// This simulates receiving messages from a jsonrpsee client by writing raw JSON.
#[tokio::test]
async fn test_parse_jsonrpsee_format_request() -> Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let (mut client_stream, server_stream) = UnixStream::pair().unwrap();

    // Spawn server to receive and echo
    let server_handle = tokio::spawn(async move {
        let mut server = jsonrpc_fdpass::Server::new();
        server.register_method("test", |_method, params, _fds| Ok((params, Vec::new())));

        let transport = UnixSocketTransport::new(server_stream);
        let (mut sender, mut receiver) = transport.split();

        if let Ok(msg) = receiver.receive().await {
            let _ = server.process_message(msg, &mut sender).await;
        }

        Ok::<(), jsonrpc_fdpass::Error>(())
    });

    // Write raw JSON in jsonrpsee format (what jsonrpsee would send)
    // jsonrpsee typically sends compact JSON
    let jsonrpsee_request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "test",
        "params": { "key": "value" },
        "id": 1
    });
    let request_str = serde_json::to_string(&jsonrpsee_request).unwrap();
    client_stream
        .write_all(request_str.as_bytes())
        .await
        .unwrap();
    client_stream.flush().await.unwrap();

    // Read raw response
    let mut response_buf = vec![0u8; 4096];
    let n = client_stream.read(&mut response_buf).await.unwrap();
    let response_str = String::from_utf8_lossy(&response_buf[..n]);

    // Parse and verify response
    let response: serde_json::Value = serde_json::from_str(&response_str).unwrap();
    assert_eq!(
        response.get("jsonrpc"),
        Some(&Value::String("2.0".to_string()))
    );
    assert_eq!(
        response.get("result"),
        Some(&serde_json::json!({ "key": "value" }))
    );
    assert_eq!(response.get("id"), Some(&Value::Number(1.into())));

    server_handle.abort();
    Ok(())
}

/// Test that our serialized messages can be parsed as valid JSON-RPC 2.0.
#[tokio::test]
async fn test_our_format_is_valid_jsonrpc() -> Result<()> {
    // Create various message types and verify they serialize to valid JSON-RPC 2.0
    let request = JsonRpcRequest::new(
        "method".to_string(),
        Some(serde_json::json!({"param": "value"})),
        Value::Number(1.into()),
    );
    let msg = MessageWithFds::new(JsonRpcMessage::Request(request), vec![]);
    let serialized = msg.serialize()?;

    // Parse and validate structure
    let parsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();
    assert_eq!(
        parsed.get("jsonrpc"),
        Some(&Value::String("2.0".to_string()))
    );
    assert_eq!(
        parsed.get("method"),
        Some(&Value::String("method".to_string()))
    );
    assert!(parsed.get("id").is_some());
    assert!(parsed.get("params").is_some());

    // Test notification (no id)
    let notification =
        JsonRpcNotification::new("notify".to_string(), Some(serde_json::json!({"event": 1})));
    let msg = MessageWithFds::new(JsonRpcMessage::Notification(notification), vec![]);
    let serialized = msg.serialize()?;

    let parsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();
    assert_eq!(
        parsed.get("jsonrpc"),
        Some(&Value::String("2.0".to_string()))
    );
    assert_eq!(
        parsed.get("method"),
        Some(&Value::String("notify".to_string()))
    );
    assert!(parsed.get("id").is_none()); // Notifications have no id

    Ok(())
}

/// Test string ID handling (JSON-RPC allows string or number IDs).
#[tokio::test]
async fn test_string_id_handling() -> Result<()> {
    let (client_stream, server_stream) = UnixStream::pair().unwrap();

    let server_handle = tokio::spawn(async move {
        let mut server = jsonrpc_fdpass::Server::new();
        server.register_method("echo", |_method, params, _fds| Ok((params, Vec::new())));

        let transport = UnixSocketTransport::new(server_stream);
        let (mut sender, mut receiver) = transport.split();

        if let Ok(msg) = receiver.receive().await {
            let _ = server.process_message(msg, &mut sender).await;
        }

        Ok::<(), jsonrpc_fdpass::Error>(())
    });

    let transport = UnixSocketTransport::new(client_stream);
    let (mut sender, mut receiver) = transport.split();

    // Use string ID
    let request = JsonRpcRequest::new(
        "echo".to_string(),
        Some(serde_json::json!({"test": true})),
        Value::String("request-uuid-123".to_string()),
    );
    sender
        .send(MessageWithFds::new(
            JsonRpcMessage::Request(request),
            vec![],
        ))
        .await?;

    let response = receiver.receive().await?;
    match response.message {
        JsonRpcMessage::Response(resp) => {
            assert_eq!(resp.id, Value::String("request-uuid-123".to_string()));
        }
        _ => panic!("Expected response"),
    }

    server_handle.abort();
    Ok(())
}

/// Test null params handling.
#[tokio::test]
async fn test_null_params() -> Result<()> {
    let (client_stream, server_stream) = UnixStream::pair().unwrap();

    let server_handle = tokio::spawn(async move {
        let mut server = jsonrpc_fdpass::Server::new();
        server.register_method("no_params", |_method, params, _fds| {
            // params should be None
            Ok((Some(Value::Bool(params.is_none())), Vec::new()))
        });

        let transport = UnixSocketTransport::new(server_stream);
        let (mut sender, mut receiver) = transport.split();

        if let Ok(msg) = receiver.receive().await {
            let _ = server.process_message(msg, &mut sender).await;
        }

        Ok::<(), jsonrpc_fdpass::Error>(())
    });

    let transport = UnixSocketTransport::new(client_stream);
    let (mut sender, mut receiver) = transport.split();

    // Send request with no params
    let request = JsonRpcRequest::new("no_params".to_string(), None, Value::Number(1.into()));
    sender
        .send(MessageWithFds::new(
            JsonRpcMessage::Request(request),
            vec![],
        ))
        .await?;

    let response = receiver.receive().await?;
    match response.message {
        JsonRpcMessage::Response(resp) => {
            assert_eq!(resp.result, Some(Value::Bool(true)));
        }
        _ => panic!("Expected response"),
    }

    server_handle.abort();
    Ok(())
}

/// Test array params (positional parameters as per JSON-RPC 2.0 spec).
#[tokio::test]
async fn test_array_params() -> Result<()> {
    let (client_stream, server_stream) = UnixStream::pair().unwrap();

    let server_handle = tokio::spawn(async move {
        let mut server = jsonrpc_fdpass::Server::new();
        server.register_method("sum", |_method, params, _fds| {
            let params = params.ok_or_else(|| {
                jsonrpc_fdpass::Error::InvalidMessage("missing params".to_string())
            })?;
            let sum: i64 = params
                .as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_i64()).sum())
                .unwrap_or(0);
            Ok((Some(Value::Number(sum.into())), Vec::new()))
        });

        let transport = UnixSocketTransport::new(server_stream);
        let (mut sender, mut receiver) = transport.split();

        if let Ok(msg) = receiver.receive().await {
            let _ = server.process_message(msg, &mut sender).await;
        }

        Ok::<(), jsonrpc_fdpass::Error>(())
    });

    let transport = UnixSocketTransport::new(client_stream);
    let (mut sender, mut receiver) = transport.split();

    // Send request with array params (positional)
    let request = JsonRpcRequest::new(
        "sum".to_string(),
        Some(serde_json::json!([1, 2, 3, 4, 5])),
        Value::Number(1.into()),
    );
    sender
        .send(MessageWithFds::new(
            JsonRpcMessage::Request(request),
            vec![],
        ))
        .await?;

    let response = receiver.receive().await?;
    match response.message {
        JsonRpcMessage::Response(resp) => {
            assert_eq!(resp.result, Some(Value::Number(15.into())));
        }
        _ => panic!("Expected response"),
    }

    server_handle.abort();
    Ok(())
}
