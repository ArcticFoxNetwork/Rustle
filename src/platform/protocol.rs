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
pub fn setup_macos_url_handler(tx: std::sync::mpsc::Sender<String>) {
    use objc2::class;
    use objc2::rc::Retained;
    use objc2_foundation::NSAppleEventManager;

    tracing::info!("Setting up macOS URL handler for rustle://");

    // kAEGetURL event class/ID constants (from Carbon/AE/AERegistry.h)
    // eventClass = 'GURL', eventID = 'GURL'
    let event_class: u32 = 0x4755524c; // 'GURL'
    let event_id: u32 = 0x4755524c; // 'GURL'

    let manager = unsafe { NSAppleEventManager::sharedAppleEventManager() };

    // Install handler for Get URL events
    // The handler closure is called when macOS sends an open URL event
    // It extracts the direct object (the URL string) and forwards it
    let handler = move |_event: &objc2_foundation::NSAppleEventDescriptor,
                        _reply: &objc2_foundation::NSAppleEventDescriptor| {
        // Extract the direct object descriptor containing the URL
        // paramKeyword '----' (keyDirectObject) = 0x2d2d2d2d
        let key_direct_object: u32 = 0x2d2d2d2d;
        if let Some(url_desc) = unsafe { _event.paramDescriptorForKeyword(key_direct_object) } {
            let url_str = url_desc.stringValue();
            if let Some(url) = url_str {
                let url_string = url.to_string();
                tracing::info!("macOS URL handler received: {}", url_string);
                let _ = tx.send(url_string);
            }
        }
    };

    // Use NSAppleEventManager to set the handler
    // Note: This requires macOS 10.0+. objc2 0.6 provides the necessary bindings.
    unsafe {
        let _ = manager.setEventHandler(&handler, event_class, event_id);
    }
    tracing::info!("macOS URL handler installed");
}

#[cfg(not(target_os = "macos"))]
pub fn setup_macos_url_handler(_tx: std::sync::mpsc::Sender<String>) {}
