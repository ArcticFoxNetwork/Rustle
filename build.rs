fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icons/icon.ico");
        res.compile().unwrap();
    }
}
