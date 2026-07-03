use jsonrpc_fdpass::{
    JsonRpcMessage, JsonRpcNotification, JsonRpcRequest, MessageWithFds, Result, Server,
    UnixSocketTransport,
};
use serde_json::Value;
use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::io::OwnedFd;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::net::{UnixListener, UnixStream};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let socket_path = temp_dir.path().join("test.sock");

    // Create the listener first
    let listener = UnixListener::bind(&socket_path)?;

    // Start server with the pre-allocated listener
    let _server_handle = tokio::spawn(async move { run_server(listener).await });

    // Run client (no race condition since socket is already bound)
    run_client(socket_path).await
}

async fn run_server(listener: UnixListener) -> Result<()> {
    let mut server = Server::new();

    // Register a method that reads from a file descriptor
    server.register_method("read_file", |_method, _params, fds| {
        if fds.is_empty() {
            return Err(jsonrpc_fdpass::Error::InvalidMessage(
                "Expected file descriptor".to_string(),
            ));
        }

        let fd = fds.into_iter().next().unwrap();
        let mut file = File::from(fd);
        let mut contents = String::new();

        file.read_to_string(&mut contents)
            .map_err(jsonrpc_fdpass::Error::Io)?;

        info!("Server read from file: {}", contents.trim());
        Ok((Some(Value::String(contents)), Vec::new()))
    });

    // Register a notification handler
    server.register_notification("log_message", |_method, params, _fds| {
        if let Some(Value::Object(map)) = params {
            if let Some(Value::String(message)) = map.get("message") {
                info!("Server received log: {}", message);
            }
        }
        Ok(())
    });

    info!("Server listening");

    // Accept one connection and handle it
    if let Ok((stream, _)) = listener.accept().await {
        let transport = UnixSocketTransport::new(stream);
        let (mut sender, mut receiver) = transport.split();

        // Handle messages from this connection
        while let Ok(message_with_fds) = receiver.receive().await {
            if server
                .process_message(message_with_fds, &mut sender)
                .await
                .is_err()
            {
                break;
            }
        }
    }

    Ok(())
}

async fn run_client(socket_path: PathBuf) -> Result<()> {
    let stream = UnixStream::connect(&socket_path).await?;
    let transport = UnixSocketTransport::new(stream);
    let (mut sender, mut _receiver) = transport.split();

    // Create a temporary file to send to the server
    let mut temp_file = tempfile::NamedTempFile::new().map_err(jsonrpc_fdpass::Error::Io)?;

    write!(temp_file, "Hello from client file!").unwrap();
    temp_file.flush().unwrap();

    let fd: OwnedFd = temp_file.into_file().into();

    let params = serde_json::json!({
        "description": "Read the attached file"
    });

    info!("Client sending file descriptor to server");
    let request = JsonRpcRequest::new("read_file".to_string(), Some(params), serde_json::json!(1));
    let message = MessageWithFds::new(JsonRpcMessage::Request(request), vec![fd]);
    sender.send(message).await?;

    // Send notification
    let params = serde_json::json!({
        "message": "This is a test notification"
    });

    info!("Client sending notification");
    let notification = JsonRpcNotification::new("log_message".to_string(), Some(params));
    let message = MessageWithFds::new(JsonRpcMessage::Notification(notification), vec![]);
    sender.send(message).await?;

    info!("Client finished");
    Ok(())
}
