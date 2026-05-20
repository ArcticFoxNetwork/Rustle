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
