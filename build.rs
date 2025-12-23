fn main() {
    #[cfg(windows)]
    {
        if std::path::Path::new("assets/icons/icon.ico").exists() {
            let mut res = winresource::WindowsResource::new();
            res.set_icon("assets/icons/icon.ico");
            res.compile().unwrap();
        } else {
            println!("cargo:warning=icon.ico not found, skipping icon embedding");
        }
    }
}
