use crate::error::{Error, Result};
use crate::message::{JsonRpcMessage, JsonRpcResponse, MessageWithFds, file_descriptor_error};
use crate::transport::{Sender, UnixSocketTransport};
use jsonrpsee::types::error::ErrorObject;
use serde_json::Value;
use std::collections::HashMap;
use std::os::unix::io::OwnedFd;
use std::path::Path;
use std::sync::Arc;
use tokio::net::{UnixListener, UnixStream};
use tracing::{debug, error, info};

/// Handler function for JSON-RPC methods.
///
/// Takes the method name, optional parameters, and received file descriptors.
/// Returns the result value and any file descriptors to send with the response.
pub type MethodHandler = Box<
    dyn Fn(&str, Option<Value>, Vec<OwnedFd>) -> Result<(Option<Value>, Vec<OwnedFd>)>
        + Send
        + Sync,
>;

/// Handler function for JSON-RPC notifications.
///
/// Takes the method name, optional parameters, and received file descriptors.
pub type NotificationHandler =
    Box<dyn Fn(&str, Option<Value>, Vec<OwnedFd>) -> Result<()> + Send + Sync>;

/// A JSON-RPC 2.0 server with file descriptor passing support.
pub struct Server {
    methods: HashMap<String, MethodHandler>,
    notifications: HashMap<String, NotificationHandler>,
}

impl Server {
    /// Create a new JSON-RPC server.
    pub fn new() -> Self {
        Self {
            methods: HashMap::new(),
            notifications: HashMap::new(),
        }
    }

    /// Register a handler for a JSON-RPC method.
    pub fn register_method<F>(&mut self, name: &str, handler: F)
    where
        F: Fn(&str, Option<Value>, Vec<OwnedFd>) -> Result<(Option<Value>, Vec<OwnedFd>)>
            + Send
            + Sync
            + 'static,
    {
        self.methods.insert(name.to_string(), Box::new(handler));
    }

    /// Register a handler for a JSON-RPC notification.
    pub fn register_notification<F>(&mut self, name: &str, handler: F)
    where
        F: Fn(&str, Option<Value>, Vec<OwnedFd>) -> Result<()> + Send + Sync + 'static,
    {
        self.notifications
            .insert(name.to_string(), Box::new(handler));
    }

    /// Start listening for connections on the given Unix socket path.
    pub async fn listen<P: AsRef<Path>>(self, path: P) -> Result<()> {
        let listener = UnixListener::bind(path)?;
        let server = Arc::new(self);

        info!("Server listening on Unix socket");

        while let Ok((stream, _)) = listener.accept().await {
            let server = Arc::clone(&server);
            tokio::spawn(async move {
                if let Err(e) = server.handle_connection(stream).await {
                    error!("Connection handler error: {}", e);
                }
            });
        }

        Ok(())
    }

    async fn handle_connection(&self, stream: UnixStream) -> Result<()> {
        let transport = UnixSocketTransport::new(stream);
        let (mut sender, mut receiver) = transport.split();

        debug!("New connection established");

        loop {
            match receiver.receive().await {
                Ok(message_with_fds) => {
                    if let Err(e) = self.process_message(message_with_fds, &mut sender).await {
                        error!("Error processing message: {}", e);
                        break;
                    }
                }
                Err(Error::ConnectionClosed) => {
                    debug!("Connection closed");
                    break;
                }
                Err(e) => {
                    error!("Error receiving message: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }

    /// Process a single JSON-RPC message and send the response.
    pub async fn process_message(
        &self,
        message_with_fds: MessageWithFds,
        sender: &mut Sender,
    ) -> Result<()> {
        match message_with_fds.message {
            JsonRpcMessage::Request(request) => {
                let id = request.id.clone();
                let method = &request.method;
                let params = request.params.clone();

                debug!("Processing request: {}", method);

                let response = if let Some(handler) = self.methods.get(method) {
                    match handler(method, params, message_with_fds.file_descriptors) {
                        Ok((result, response_fds)) => {
                            let response =
                                JsonRpcResponse::success(result.unwrap_or(Value::Null), id);
                            let message = JsonRpcMessage::Response(response);
                            MessageWithFds::new(message, response_fds)
                        }
                        Err(_) => {
                            let error = file_descriptor_error();
                            let response = JsonRpcResponse::error(error, id);
                            let message = JsonRpcMessage::Response(response);
                            MessageWithFds::new(message, Vec::new())
                        }
                    }
                } else {
                    let error =
                        ErrorObject::owned(-32601, "Method not found".to_string(), None::<Value>);
                    let response = JsonRpcResponse::error(error, id);
                    let message = JsonRpcMessage::Response(response);
                    MessageWithFds::new(message, Vec::new())
                };

                sender.send(response).await?;
            }
            JsonRpcMessage::Notification(notification) => {
                debug!("Processing notification: {}", notification.method);

                if let Some(handler) = self.notifications.get(&notification.method) {
                    if let Err(e) = handler(
                        &notification.method,
                        notification.params,
                        message_with_fds.file_descriptors,
                    ) {
                        error!("Notification handler error: {}", e);
                    }
                }
            }
            JsonRpcMessage::Response(_) => {
                debug!("Received response (unexpected on server side)");
            }
        }

        Ok(())
    }
}

impl Default for Server {
    fn default() -> Self {
        Self::new()
    }
}
