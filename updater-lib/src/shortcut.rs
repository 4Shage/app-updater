use std::path::Path;

use anyhow::Result;

#[cfg(windows)]
pub fn create(name: &str, exe: &Path) -> Result<()> {
    for link in locations(name) {
        if let Some(parent) = link.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut shortcut = mslnk::ShellLink::new(exe)?;
        shortcut.set_working_dir(exe.parent().map(|dir| dir.display().to_string()));
        shortcut.create_lnk(&link)?;
    }
    Ok(())
}

#[cfg(windows)]
pub fn remove(name: &str) {
    for link in locations(name) {
        let _ = std::fs::remove_file(link);
    }
}

#[cfg(windows)]
fn locations(name: &str) -> Vec<std::path::PathBuf> {
    let file = format!("{}.lnk", name.replace(['\\', '/', ':', '*', '?', '"', '<', '>', '|'], "_"));
    let start_menu = dirs::data_dir().map(|d| d.join(r"Microsoft\Windows\Start Menu\Programs").join(&file));
    let desktop = dirs::desktop_dir().map(|d| d.join(&file));
    [start_menu, desktop].into_iter().flatten().collect()
}

#[cfg(not(windows))]
pub fn create(_name: &str, _exe: &Path) -> Result<()> {
    Ok(())
}

#[cfg(not(windows))]
pub fn remove(_name: &str) {}
