fn main() {
    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set("ProductName", "App Updater");
        resource.set("FileDescription", "App Updater");
        if let Err(error) = resource.compile() {
            println!("cargo:warning=nie udalo sie osadzic zasobow exe: {error}");
        }
    }
}
