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
pub fn remove(name: &str, _exe: Option<&Path>) {
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

#[cfg(target_os = "linux")]
use std::{fs, path::PathBuf};

#[cfg(target_os = "linux")]
const MARKER: &str = "X-AppUpdater=true";

/// Linkuje exe do ~/.local/bin i tworzy wpis .desktop w menu aplikacji oraz na pulpicie.
#[cfg(target_os = "linux")]
pub fn create(name: &str, exe: &Path) -> Result<()> {
    use std::os::unix::fs::{symlink, PermissionsExt};

    let id = launcher_id(exe, name);
    if let Some(bin_dir) = dirs::executable_dir() {
        fs::create_dir_all(&bin_dir)?;
        let link = bin_dir.join(&id);
        match link.symlink_metadata() {
            Ok(meta) if meta.file_type().is_symlink() => {
                let ours = fs::read_link(&link).is_ok_and(|target| target == exe) || !link.exists();
                if ours {
                    fs::remove_file(&link)?;
                    symlink(exe, &link)?;
                }
            }
            Ok(_) => {}
            Err(_) => symlink(exe, &link)?,
        }
    }

    let entry = desktop_entry(name, exe);
    for path in desktop_files(&id) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, &entry)?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn remove(name: &str, exe: Option<&Path>) {
    let Some(exe) = exe else { return };
    let id = launcher_id(exe, name);

    if let Some(bin_dir) = dirs::executable_dir() {
        let link = bin_dir.join(&id);
        if fs::read_link(&link).is_ok_and(|target| target == exe) {
            let _ = fs::remove_file(link);
        }
    }
    for path in desktop_files(&id) {
        if fs::read_to_string(&path).is_ok_and(|content| content.contains(MARKER)) {
            let _ = fs::remove_file(path);
        }
    }
}

#[cfg(target_os = "linux")]
fn launcher_id(exe: &Path, name: &str) -> String {
    exe.file_name()
        .and_then(|n| n.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| name.to_lowercase().replace(' ', "-"))
}

#[cfg(target_os = "linux")]
fn desktop_files(id: &str) -> Vec<PathBuf> {
    let file = format!("{id}.desktop");
    let applications = dirs::data_dir().map(|d| d.join("applications").join(&file));
    let desktop = dirs::desktop_dir().filter(|d| d.is_dir()).map(|d| d.join(&file));
    [applications, desktop].into_iter().flatten().collect()
}

#[cfg(target_os = "linux")]
fn desktop_entry(name: &str, exe: &Path) -> String {
    let dir = exe.parent().map(|d| d.display().to_string()).unwrap_or_default();
    let exec: String = exe
        .display()
        .to_string()
        .chars()
        .flat_map(|c| if "\"`$\\".contains(c) { vec!['\\', c] } else { vec![c] })
        .collect();
    format!(
        "[Desktop Entry]\nType=Application\nName={name}\nExec=\"{exec}\"\nPath={dir}\nTerminal=false\nCategories=Utility;\n{MARKER}\n"
    )
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn create(_name: &str, _exe: &Path) -> Result<()> {
    Ok(())
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn remove(_name: &str, _exe: Option<&Path>) {}
