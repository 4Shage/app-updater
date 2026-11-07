fn main() {
    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set("ProductName", "App Updater Dev");
        resource.set("FileDescription", "App Updater Dev");
        if let Err(error) = resource.compile() {
            println!("cargo:warning=nie udalo sie osadzic zasobow exe: {error}");
        }
    }
}
