use std::path::Path;

use anyhow::Result;

#[cfg(windows)]
pub fn create(name: &str, exe: &Path) -> Result<()> {
    use std::fs;
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    for link in locations(name) {
        if let Some(parent) = link.parent() {
            fs::create_dir_all(parent)?;
        }
        let script = format!(
            "$s=(New-Object -ComObject WScript.Shell).CreateShortcut('{}');$s.TargetPath='{}';$s.WorkingDirectory='{}';$s.Save()",
            quote(&link),
            quote(exe),
            quote(exe.parent().unwrap_or(Path::new(""))),
        );
        let status = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .creation_flags(CREATE_NO_WINDOW)
            .status()?;
        if !status.success() {
            anyhow::bail!("nie udalo sie utworzyc skrotu");
        }
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

#[cfg(windows)]
fn quote(path: &Path) -> String {
    path.display().to_string().replace('\'', "''")
}

#[cfg(not(windows))]
pub fn create(_name: &str, _exe: &Path) -> Result<()> {
    Ok(())
}

#[cfg(not(windows))]
pub fn remove(_name: &str) {}
