//! IPC (Inter-Process Communication) for single-instance URL forwarding
//!
//! When a second instance is launched with a rustle:// URI, it forwards the URI
//! to the primary instance via a local socket and exits.
//!
//! On normal launch, a new instance first checks whether a primary instance is
//! already running — if so, it sends a Focus command and exits, enforcing
//! single-instance behavior.

use std::sync::Mutex;

/// Well-known socket name for Rustle single-instance IPC
const IPC_SOCKET_NAME: &str = "rustle_instance";

/// Messages exchanged between instances via the IPC socket
#[derive(Debug, Clone)]
pub enum IpcMessage {
    /// A rustle:// URI to process
    Uri(String),
    /// Show and focus the main window
    Focus,
}

/// Channel types for IPC message forwarding
pub type IpcSender = tokio::sync::mpsc::UnboundedSender<IpcMessage>;
pub type IpcReceiver = tokio::sync::mpsc::UnboundedReceiver<IpcMessage>;

/// Create a new IPC channel
pub fn ipc_channel() -> (IpcSender, IpcReceiver) {
    tokio::sync::mpsc::unbounded_channel()
}

/// Stores a URI from CLI args that should be processed on startup
static PENDING_STARTUP_URI: Mutex<Option<String>> = Mutex::new(None);

/// Store a URI to be processed when the application starts
pub fn set_pending_startup_uri(uri: String) {
    if let Ok(mut guard) = PENDING_STARTUP_URI.lock() {
        *guard = Some(uri);
    }
}

/// Take and clear the pending startup URI
pub fn take_pending_startup_uri() -> Option<String> {
    PENDING_STARTUP_URI.lock().ok()?.take()
}

/// Wire format prefixes
const PREFIX_FOCUS: &[u8] = b"FOCUS";
const PREFIX_URI: &[u8] = b"URI:";

/// Spawn an IPC listener in a tokio background task.
///
/// Binds a local socket and forwards received messages to the application
/// via the provided mpsc sender.
pub fn spawn_ipc_listener(tx: IpcSender) -> tokio::task::JoinHandle<()> {
    use interprocess::local_socket::ToNsName;
    use interprocess::local_socket::traits::tokio::Listener;
    use interprocess::local_socket::{GenericNamespaced, ListenerOptions};
    use tokio::io::AsyncReadExt;

    tokio::spawn(async move {
        let name = match IPC_SOCKET_NAME.to_ns_name::<GenericNamespaced>() {
            Ok(n) => n,
            Err(e) => {
                tracing::error!("Failed to create IPC socket name: {}", e);
                return;
            }
        };

        let listener = match ListenerOptions::new().name(name).create_tokio() {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!("Failed to bind IPC socket: {}", e);
                return;
            }
        };

        tracing::info!("IPC listener started on '{}'", IPC_SOCKET_NAME);

        loop {
            match listener.accept().await {
                Ok(mut stream) => {
                    let mut buf = vec![0u8; 4096];
                    match stream.read(&mut buf).await {
                        Ok(n) if n > 0 => {
                            buf.truncate(n);
                            let msg = if buf.starts_with(PREFIX_FOCUS) {
                                IpcMessage::Focus
                            } else if buf.starts_with(PREFIX_URI) {
                                match String::from_utf8(buf[PREFIX_URI.len()..].to_vec()) {
                                    Ok(uri) => {
                                        let uri = uri.trim().to_string();
                                        IpcMessage::Uri(uri)
                                    }
                                    Err(e) => {
                                        tracing::warn!("IPC received invalid UTF-8: {}", e);
                                        continue;
                                    }
                                }
                            } else {
                                // Backward compat: raw URI without prefix
                                match String::from_utf8(buf) {
                                    Ok(uri) => {
                                        let uri = uri.trim().to_string();
                                        IpcMessage::Uri(uri)
                                    }
                                    Err(e) => {
                                        tracing::warn!("IPC received invalid UTF-8: {}", e);
                                        continue;
                                    }
                                }
                            };
                            tracing::info!("IPC received: {:?}", msg);
                            let _ = tx.send(msg);
                        }
                        Ok(_) => {
                            tracing::warn!("IPC received empty message");
                        }
                        Err(e) => {
                            tracing::warn!("IPC read error: {}", e);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("IPC accept error: {}", e);
                }
            }
        }
    })
}

/// Forward a URI to the primary instance via local socket.
///
/// Called by secondary instances. Returns Ok(()) if the URI was
/// successfully forwarded, or Err if no primary instance is running.
pub fn forward_uri_to_primary(uri: &str) -> Result<(), String> {
    use std::io::Write;

    let payload = format!("URI:{}", uri);
    write_to_socket(&payload)
}

/// Send a focus command to the primary instance via local socket.
///
/// Called by secondary instances on normal launch. Returns Ok(())
/// if the focus command was forwarded, or Err if no primary instance is running.
pub fn forward_focus_to_primary() -> Result<(), String> {
    use std::io::Write;

    write_to_socket("FOCUS")
}

fn write_to_socket(payload: &str) -> Result<(), String> {
    use interprocess::local_socket::ConnectOptions;
    use interprocess::local_socket::GenericNamespaced;
    use interprocess::local_socket::ToNsName;
    use std::io::Write;

    let name = IPC_SOCKET_NAME
        .to_ns_name::<GenericNamespaced>()
        .map_err(|e| format!("Failed to create socket name: {}", e))?;

    let mut stream = ConnectOptions::new()
        .name(name)
        .connect_sync()
        .map_err(|e| format!("No primary instance running: {}", e))?;

    stream
        .write_all(payload.as_bytes())
        .map_err(|e| format!("Failed to send IPC message: {}", e))?;

    tracing::info!("IPC forwarded: {}", payload);
    Ok(())
}
