//! Platform-level rustle:// URL scheme registration.
//!
//! Uses the `sysuri` crate for cross-platform registration:
//! - Linux: `.desktop` file + `xdg-mime`
//! - Windows: `HKCU\Software\Classes` registry keys
//! - macOS: `.app` bundle `Info.plist`

use std::path::PathBuf;

/// Register rustle:// as the default handler for the rustle URI scheme.
///
/// Safe to call multiple times — subsequent calls are no-ops if already registered.
pub fn register_protocol_scheme() -> Result<(), String> {
    #[cfg(debug_assertions)]
    let scheme_name = "rustle-dev";
    #[cfg(not(debug_assertions))]
    let scheme_name = "rustle";

    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("rustle"));
    let scheme = sysuri::UriScheme::new(scheme_name, "Rustle Music Player", exe);

    sysuri::register(&scheme)
        .map_err(|e| format!("Failed to register {}:// protocol: {}", scheme_name, e))
}

/// Check if the rustle:// scheme is currently registered.
pub fn is_protocol_registered() -> bool {
    #[cfg(debug_assertions)]
    let scheme_name = "rustle-dev";
    #[cfg(not(debug_assertions))]
    let scheme_name = "rustle";

    sysuri::is_registered(scheme_name).unwrap_or(false)
}

/// Setup macOS URL event handler to capture `kAEGetURL` Apple Events.
///
/// On macOS, `rustle://` links arrive as Apple Events rather than CLI args.
/// This handler intercepts them and forwards URLs through the IPC channel.
#[cfg(target_os = "macos")]
pub fn setup_macos_url_handler(tx: crate::protocol::ipc::IpcSender) {
    use std::sync::Mutex;
    use objc2::{define_class, msg_send, sel, ClassType};
    use objc2::rc::Retained;
    use objc2::runtime::{NSObject, NSObjectProtocol};
    use objc2_foundation::{NSAppleEventDescriptor, NSAppleEventManager};

    tracing::info!("Setting up macOS URL handler for rustle://");

    // Store the sender in a global so the handler method can forward URLs
    static SENDER: Mutex<Option<crate::protocol::ipc::IpcSender>> = Mutex::new(None);
    *SENDER.lock().unwrap() = Some(tx);

    // Define a custom ObjC class whose method will be called for GURL events.
    // No #[thread_kind] — defaults to AllocAnyThread, impls Send + Sync.
    define_class!(
        #[unsafe(super = NSObject)]
        struct RustleURLHandler;

        unsafe impl NSObjectProtocol for RustleURLHandler {}

        impl RustleURLHandler {
            #[unsafe(method(handleGetURLEvent:withReplyEvent:))]
            fn handle_get_url_event(
                &self,
                event: &NSAppleEventDescriptor,
                _reply: &NSAppleEventDescriptor,
            ) {
                // keyDirectObject = '----' (0x2d2d2d2d)
                let desc: Option<Retained<NSAppleEventDescriptor>> = unsafe {
                    msg_send![event, paramDescriptorForKeyword: 0x2d2d2d2du32]
                };
                if let Some(desc) = desc {
                    if let Some(url) = desc.stringValue() {
                        let url = url.to_string();
                        tracing::info!("macOS URL handler received: {}", url);
                        if let Some(sender) = SENDER.lock().unwrap().as_ref() {
                            let _ = sender.send(
                                crate::protocol::ipc::IpcMessage::Uri(url),
                            );
                        }
                    }
                }
            }
        }
    );

    // Constructor: ClassType::alloc() + NSObject.init()
    impl RustleURLHandler {
        fn new() -> Retained<Self> {
            let this = Self::alloc();
            unsafe { msg_send![super(this), init] }
        }
    }

    // NSAppleEventManager does NOT retain its handler — we hold it alive.
    static HANDLER: Mutex<Option<Retained<RustleURLHandler>>> = Mutex::new(None);
    let handler = RustleURLHandler::new();

    // kAEGetURL: eventClass = 'GURL', eventID = 'GURL'
    let manager = unsafe { NSAppleEventManager::sharedAppleEventManager() };

    unsafe {
        let _: () = msg_send![
            &manager,
            setEventHandler: &*handler,
            andSelector: sel!(handleGetURLEvent:withReplyEvent:),
            forEventClass: 0x4755524cu32,
            andEventID: 0x4755524cu32,
        ];
    }

    *HANDLER.lock().unwrap() = Some(handler);
    tracing::info!("macOS URL handler installed");
}

#[cfg(not(target_os = "macos"))]
pub fn setup_macos_url_handler(_tx: crate::protocol::ipc::IpcSender) {}
