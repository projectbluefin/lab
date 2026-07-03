//! Integration test with skopeo experimental-image-proxy -J
//!
//! This test spawns skopeo with the JSON-RPC 2.0 protocol flag and verifies
//! that the Rust jsonrpc-fdpass client can communicate with it.
//!
//! Run with: cargo test --test skopeo_integration -- --ignored

use jsonrpc_fdpass::{JsonRpcMessage, JsonRpcRequest, MessageWithFds, UnixSocketTransport};
use serde_json::Value;
use std::os::fd::FromRawFd;
use std::os::unix::io::{AsRawFd, OwnedFd};
use std::process::{Command, Stdio};

/// Path to the skopeo binary built with JSON-RPC support.
/// This can be overridden via the SKOPEO_PATH environment variable.
fn skopeo_path() -> String {
    std::env::var("SKOPEO_PATH").unwrap_or_else(|_| {
        let home = std::env::var("HOME").expect("HOME not set");
        format!("{}/src/github/containers/skopeo/skopeo", home)
    })
}

/// Create a Unix socket pair suitable for passing to skopeo.
fn create_socketpair() -> std::io::Result<(OwnedFd, OwnedFd)> {
    use rustix::net::{AddressFamily, SocketFlags, SocketType};

    // Create a SOCK_STREAM socket pair (skopeo uses SOCK_STREAM for JSON-RPC mode)
    let (fd1, fd2) = rustix::net::socketpair(
        AddressFamily::UNIX,
        SocketType::STREAM,
        SocketFlags::CLOEXEC,
        None,
    )?;

    Ok((fd1, fd2))
}

/// Spawn skopeo with the JSON-RPC protocol.
fn spawn_skopeo(sockfd: &OwnedFd) -> std::io::Result<std::process::Child> {
    let skopeo = skopeo_path();
    let fd_num = sockfd.as_raw_fd();

    // Clear CLOEXEC on the fd we're passing to skopeo
    rustix::io::fcntl_setfd(sockfd, rustix::io::FdFlags::empty())?;

    Command::new(&skopeo)
        .args([
            "experimental-image-proxy",
            "-J",
            "--sockfd",
            &fd_num.to_string(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}

#[tokio::test]
#[ignore] // Run with: cargo test --test skopeo_integration -- --ignored
async fn test_skopeo_jsonrpc_initialize() -> jsonrpc_fdpass::Result<()> {
    // Check if skopeo binary exists
    let skopeo = skopeo_path();
    if !std::path::Path::new(&skopeo).exists() {
        eprintln!("Skipping test: skopeo binary not found at {}", skopeo);
        eprintln!(
            "Build it with: cd ~/src/github/containers/skopeo && go build -tags 'exclude_graphdriver_btrfs exclude_graphdriver_devicemapper' ./cmd/skopeo"
        );
        return Ok(());
    }

    // Create socket pair
    let (client_fd, server_fd) = create_socketpair()?;

    // Spawn skopeo with the server end
    let mut child = spawn_skopeo(&server_fd)?;

    // Drop the server end in our process - skopeo owns it now
    drop(server_fd);

    // Convert client fd to tokio UnixStream
    let std_stream = unsafe { std::os::unix::net::UnixStream::from_raw_fd(client_fd.as_raw_fd()) };
    // Prevent the OwnedFd from closing the fd since std_stream now owns it
    std::mem::forget(client_fd);

    std_stream.set_nonblocking(true)?;
    let stream = tokio::net::UnixStream::from_std(std_stream)?;

    // Create transport
    let transport = UnixSocketTransport::new(stream);
    let (mut sender, mut receiver) = transport.split();

    // Send Initialize request (skopeo expects an empty array for params)
    let request = JsonRpcRequest::new(
        "Initialize".to_string(),
        Some(Value::Array(vec![])),
        Value::Number(1.into()),
    );
    let message = JsonRpcMessage::Request(request);
    let message_with_fds = MessageWithFds::new(message, vec![]);

    eprintln!("Sending Initialize request to skopeo...");
    sender.send(message_with_fds).await?;

    // Receive response
    eprintln!("Waiting for response...");
    let response = receiver.receive().await?;

    // Verify response
    match response.message {
        JsonRpcMessage::Response(resp) => {
            eprintln!("Got response: {:?}", resp);

            // Check for errors
            if let Some(error) = resp.error {
                panic!("Skopeo returned error: {:?}", error);
            }

            // Verify we got a result
            let result = resp.result.expect("Expected result in response");
            eprintln!("Result: {}", serde_json::to_string_pretty(&result)?);

            // The JSON-RPC Initialize returns {"value": {"version": "1.0.0", "v1_compat": "0.2.8"}}
            let value = result
                .get("value")
                .expect("Expected 'value' field in result");
            let version = value
                .get("version")
                .expect("Expected 'version' field")
                .as_str()
                .expect("version should be a string");
            let v1_compat = value
                .get("v1_compat")
                .expect("Expected 'v1_compat' field")
                .as_str()
                .expect("v1_compat should be a string");

            eprintln!("Skopeo JSON-RPC version: {}", version);
            eprintln!("Skopeo v1 compat version: {}", v1_compat);

            // Basic sanity checks
            assert!(!version.is_empty(), "version should not be empty");
            assert!(!v1_compat.is_empty(), "v1_compat should not be empty");

            // Send Shutdown to cleanly terminate skopeo
            let shutdown_request = JsonRpcRequest::new(
                "Shutdown".to_string(),
                Some(Value::Array(vec![])),
                Value::Number(2.into()),
            );
            let shutdown_message = JsonRpcMessage::Request(shutdown_request);
            let shutdown_with_fds = MessageWithFds::new(shutdown_message, vec![]);

            eprintln!("Sending Shutdown request...");
            sender.send(shutdown_with_fds).await?;

            // Wait for shutdown response
            let shutdown_response = receiver.receive().await?;
            match shutdown_response.message {
                JsonRpcMessage::Response(resp) => {
                    eprintln!("Shutdown response: {:?}", resp);
                    if let Some(result) = resp.result {
                        let success = result.get("success").and_then(|v| v.as_bool());
                        assert_eq!(success, Some(true), "Shutdown should succeed");
                    }
                }
                _ => panic!("Expected response to Shutdown"),
            }
        }
        _ => panic!("Expected response message, got {:?}", response.message),
    }

    // Wait for child to exit
    let status = child.wait()?;
    eprintln!("Skopeo exited with status: {}", status);

    // Skopeo should exit cleanly after shutdown
    assert!(
        status.success(),
        "Skopeo should exit cleanly after Shutdown"
    );

    Ok(())
}

#[tokio::test]
#[ignore] // Run with: cargo test --test skopeo_integration -- --ignored
async fn test_skopeo_jsonrpc_error_handling() -> jsonrpc_fdpass::Result<()> {
    // Check if skopeo binary exists
    let skopeo = skopeo_path();
    if !std::path::Path::new(&skopeo).exists() {
        eprintln!("Skipping test: skopeo binary not found at {}", skopeo);
        return Ok(());
    }

    // Create socket pair
    let (client_fd, server_fd) = create_socketpair()?;

    // Spawn skopeo
    let mut child = spawn_skopeo(&server_fd)?;
    drop(server_fd);

    // Convert to tokio stream
    let std_stream = unsafe { std::os::unix::net::UnixStream::from_raw_fd(client_fd.as_raw_fd()) };
    std::mem::forget(client_fd);
    std_stream.set_nonblocking(true)?;
    let stream = tokio::net::UnixStream::from_std(std_stream)?;

    let transport = UnixSocketTransport::new(stream);
    let (mut sender, mut receiver) = transport.split();

    // First, Initialize
    let init_request = JsonRpcRequest::new(
        "Initialize".to_string(),
        Some(Value::Array(vec![])),
        Value::Number(1.into()),
    );
    sender
        .send(MessageWithFds::new(
            JsonRpcMessage::Request(init_request),
            vec![],
        ))
        .await?;
    let _ = receiver.receive().await?;

    // Now call an unknown method - should get an error
    let unknown_request = JsonRpcRequest::new(
        "NonExistentMethod".to_string(),
        Some(Value::Array(vec![])),
        Value::Number(2.into()),
    );
    sender
        .send(MessageWithFds::new(
            JsonRpcMessage::Request(unknown_request),
            vec![],
        ))
        .await?;

    let response = receiver.receive().await?;
    match response.message {
        JsonRpcMessage::Response(resp) => {
            eprintln!("Error response: {:?}", resp);
            assert!(resp.error.is_some(), "Expected error response");
            let error = resp.error.unwrap();
            eprintln!("Error code: {}, message: {}", error.code(), error.message());
            assert!(
                error.message().contains("unknown method"),
                "Error should mention unknown method"
            );
        }
        _ => panic!("Expected response"),
    }

    // Clean shutdown
    let shutdown = JsonRpcRequest::new(
        "Shutdown".to_string(),
        Some(Value::Array(vec![])),
        Value::Number(3.into()),
    );
    sender
        .send(MessageWithFds::new(
            JsonRpcMessage::Request(shutdown),
            vec![],
        ))
        .await?;
    let _ = receiver.receive().await?;

    let status = child.wait()?;
    assert!(status.success());

    Ok(())
}
