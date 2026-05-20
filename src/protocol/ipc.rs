//! IPC (Inter-Process Communication) for single-instance URL forwarding
//!
//! When a second instance is launched with a rustle:// URI, it forwards the URI
//! to the primary instance via a local socket and exits.

use std::sync::Mutex;

/// Well-known socket name for Rustle single-instance IPC
const IPC_SOCKET_NAME: &str = "rustle_instance";

/// Channel types for URI forwarding
pub type UriSender = tokio::sync::mpsc::UnboundedSender<String>;
pub type UriReceiver = tokio::sync::mpsc::UnboundedReceiver<String>;

/// Create a new URI channel
pub fn uri_channel() -> (UriSender, UriReceiver) {
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

/// Spawn an IPC listener in a tokio background task.
///
/// Binds a local socket and forwards received URIs to the application
/// via the provided mpsc sender.
pub fn spawn_ipc_listener(tx: UriSender) -> tokio::task::JoinHandle<()> {
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
                            match String::from_utf8(buf) {
                                Ok(uri) => {
                                    let uri = uri.trim().to_string();
                                    tracing::info!("IPC received URI: {}", uri);
                                    let _ = tx.send(uri);
                                }
                                Err(e) => {
                                    tracing::warn!("IPC received invalid UTF-8: {}", e);
                                }
                            }
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
        .write_all(uri.as_bytes())
        .map_err(|e| format!("Failed to send URI: {}", e))?;

    tracing::info!("URI forwarded to primary instance: {}", uri);
    Ok(())
}
