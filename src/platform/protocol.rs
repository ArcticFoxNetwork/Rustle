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

/// Setup macOS URL event handler to capture `GURL` (Get URL) Apple Events.
///
/// On macOS, `rustle://` URLs arrive via Apple Events rather than CLI args.
/// This registers a handler with `NSAppleEventManager` to intercept `kAEGetURL`
/// events and forward them via the provided channel.
///
/// Must be called after the `NSApplication` has been initialized (i.e., after
/// the winit event loop has started).
#[cfg(target_os = "macos")]
pub fn setup_macos_url_handler(tx: crate::protocol::ipc::IpcSender) {
    use std::sync::Mutex;
    use objc2::{define_class, msg_send, sel, ClassType};
    use objc2::rc::Retained;
    use objc2::runtime::{NSObject, NSObjectProtocol};
    use objc2_foundation::{NSAppleEventDescriptor, NSAppleEventManager};

    tracing::info!("Setting up macOS URL handler for rustle://");

    // Store the sender in a global so the handler method can access it
    static SENDER: Mutex<Option<crate::protocol::ipc::IpcSender>> = Mutex::new(None);
    *SENDER.lock().unwrap() = Some(tx);

    // Define a custom Objective-C class to handle Apple Events
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
                let key_direct_object: u32 = 0x2d2d2d2d;
                if let Some(url_desc) = event.paramDescriptorForKeyword(key_direct_object) {
                    if let Some(url) = url_desc.stringValue() {
                        let url_string = url.to_string();
                        tracing::info!("macOS URL handler received: {}", url_string);
                        if let Some(sender) = SENDER.lock().unwrap().as_ref() {
                            let _ = sender.send(crate::protocol::ipc::IpcMessage::Uri(url_string));
                        }
                    }
                }
            }
        }
    );

    // Hold the handler in a static to prevent deallocation.
    // NSAppleEventManager does NOT retain its handler, so we must keep it alive.
    static HANDLER: Mutex<Option<Retained<RustleURLHandler>>> = Mutex::new(None);
    let handler = RustleURLHandler::new();

    // kAEGetURL: eventClass = 'GURL', eventID = 'GURL'
    let event_class: u32 = 0x4755524c;
    let event_id: u32 = 0x4755524c;

    let manager = unsafe { NSAppleEventManager::sharedAppleEventManager() };

    unsafe {
        let _: () = msg_send![
            &manager,
            setEventHandler: &*handler,
            andSelector: sel!(handleGetURLEvent:withReplyEvent:),
            forEventClass: event_class,
            andEventID: event_id,
        ];
    }

    *HANDLER.lock().unwrap() = Some(handler);

    tracing::info!("macOS URL handler installed");
}

#[cfg(not(target_os = "macos"))]
pub fn setup_macos_url_handler(_tx: crate::protocol::ipc::IpcSender) {}
