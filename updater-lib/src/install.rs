use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use anyhow::{bail, Result};
use common::{current_platform, is_newer, Release};

use crate::api::Api;
use crate::shortcut;
use crate::store::{Installed, Store};

pub const MAX_VERSION_FILES: usize = 2;

pub fn install_release(
    api: &Api,
    store: &Store,
    app_id: &str,
    name: &str,
    release: &Release,
    progress: impl FnMut(u64, u64),
) -> Result<Installed> {
    let existing = store.apps.get(app_id);
    let dir = existing
        .map(|i| i.dir.clone())
        .unwrap_or_else(|| store.install_root().join(app_id));
    fs::create_dir_all(&dir)?;

    let tmp = std::env::temp_dir().join(format!("updater-{app_id}-{}", release.filename));
    api.download(app_id, release, &tmp, progress)?;

    let current_exe = existing.and_then(|i| i.exe.clone());
    let exe = if is_archive(&tmp) {
        if let Some(exe) = &current_exe {
            move_aside(exe);
        }
        extract_archive(&tmp, &dir, app_id)?;
        find_exe(&dir, app_id).or(current_exe)
    } else {
        let default_name = format!("{app_id}{}", std::env::consts::EXE_SUFFIX);
        let target = current_exe.unwrap_or_else(|| dir.join(default_name));
        replace_file(&tmp, &target)?;
        Some(target)
    };
    let _ = fs::remove_file(&tmp);

    if let Some(exe) = &exe {
        prune_old(&dir, exe, MAX_VERSION_FILES);
    }

    if let Some(exe) = exe.as_ref().filter(|_| existing.is_none()) {
        let _ = shortcut::create(name, exe);
    }

    Ok(Installed { name: name.into(), version: release.version.clone(), dir, exe })
}

pub fn uninstall(store: &mut Store, app_id: &str) -> Result<()> {
    let Some(installed) = store.apps.get(app_id).cloned() else {
        bail!("aplikacja nie jest zainstalowana");
    };
    if installed.dir.starts_with(store.install_root()) {
        if installed.dir.exists() {
            fs::remove_dir_all(&installed.dir)?;
        }
    } else if let Some(exe) = installed.exe.as_ref().filter(|e| e.exists()) {
        fs::remove_file(exe)?;
    }
    let name = if installed.name.is_empty() { app_id } else { &installed.name };
    shortcut::remove(name, installed.exe.as_deref());
    store.apps.remove(app_id);
    store.save()
}

pub fn update_all(
    api: &Api,
    store: &mut Store,
    mut progress: impl FnMut(&str, u64, u64),
) -> Result<Vec<String>> {
    let remote = api.list_apps()?;
    let platform = current_platform();
    let mut updated = vec![];

    for app in &remote {
        let Some(installed) = store.apps.get(&app.id) else { continue };
        let Some(release) = app.latest(&platform) else { continue };
        if !is_newer(&release.version, &installed.version) {
            continue;
        }
        let result = install_release(api, store, &app.id, &app.name, release, |d, t| progress(&app.id, d, t))?;
        store.apps.insert(app.id.clone(), result);
        store.save()?;
        updated.push(format!("{} {}", app.name, release.version));
    }
    Ok(updated)
}

#[derive(Clone, Copy)]
enum ArchiveKind {
    Zip,
    TarGz,
}

/// Rozpoznaje archiwum po poczatkowych bajtach, niezaleznie od nazwy pliku.
fn sniff(path: &Path) -> Option<ArchiveKind> {
    let mut magic = [0u8; 4];
    File::open(path).ok()?.read_exact(&mut magic).ok()?;
    if magic[..2] == [0x1f, 0x8b] {
        Some(ArchiveKind::TarGz)
    } else if &magic == b"PK\x03\x04" {
        Some(ArchiveKind::Zip)
    } else {
        None
    }
}

pub fn is_archive(path: &Path) -> bool {
    sniff(path).is_some()
}

pub fn extract_archive(archive: &Path, dir: &Path, app_id: &str) -> Result<()> {
    match sniff(archive) {
        Some(ArchiveKind::Zip) => extract_zip(archive, dir)?,
        Some(ArchiveKind::TarGz) => extract_tar_gz(archive, dir)?,
        None => bail!("nieznany format archiwum"),
    }
    if let Some(exe) = find_exe(dir, app_id) {
        make_executable(&exe)?;
    }
    Ok(())
}

fn extract_zip(zip_path: &Path, dir: &Path) -> Result<()> {
    let mut archive = zip::ZipArchive::new(File::open(zip_path)?)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let Some(relative) = entry.enclosed_name() else {
            bail!("niebezpieczna sciezka w archiwum zip");
        };
        if entry.is_symlink() {
            continue;
        }
        let out = dir.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)?;
        }
        std::io::copy(&mut entry, &mut File::create(&out)?)?;
    }
    Ok(())
}

type TarReader = tar::Archive<flate2::read::GzDecoder<File>>;

fn open_tar(path: &Path) -> Result<TarReader> {
    Ok(tar::Archive::new(flate2::read::GzDecoder::new(File::open(path)?)))
}

/// Gdy wszystko w archiwum siedzi w jednym katalogu glownym, zwraca 1 (ten katalog jest pomijany).
fn tar_strip_depth(path: &Path) -> Result<usize> {
    let mut archive = open_tar(path)?;
    let mut roots = HashSet::new();
    let mut nested = false;
    for entry in archive.entries()? {
        let entry = entry?;
        let entry_path = entry.path()?;
        let mut parts = entry_path.components().filter(|c| !matches!(c, Component::CurDir));
        let Some(first) = parts.next() else { continue };
        roots.insert(first.as_os_str().to_owned());
        nested |= parts.next().is_some();
    }
    Ok(usize::from(roots.len() == 1 && nested))
}

fn extract_tar_gz(archive_path: &Path, dir: &Path) -> Result<()> {
    let strip = tar_strip_depth(archive_path)?;
    let mut archive = open_tar(archive_path)?;
    for entry in archive.entries()? {
        let mut entry = entry?;
        let kind = entry.header().entry_type();
        if !kind.is_file() && !kind.is_dir() {
            continue;
        }
        let mode = entry.header().mode().unwrap_or(0o644);

        let mut relative = PathBuf::new();
        {
            let path = entry.path()?;
            for component in path.components().filter(|c| !matches!(c, Component::CurDir)).skip(strip) {
                match component {
                    Component::Normal(part) => relative.push(part),
                    _ => bail!("niebezpieczna sciezka w archiwum tar"),
                }
            }
        }
        if relative.as_os_str().is_empty() {
            continue;
        }

        let out = dir.join(&relative);
        if kind.is_dir() {
            fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)?;
        }
        std::io::copy(&mut entry, &mut File::create(&out)?)?;
        set_mode(&out, (mode & 0o755) | 0o600)?;
    }
    Ok(())
}

fn set_mode(path: &Path, mode: u32) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    }
    #[cfg(not(unix))]
    let _ = (path, mode);
    Ok(())
}

/// Szuka pliku `<id>` lub `<id>.exe` w katalogu aplikacji (i jeden poziom nizej).
pub fn find_exe(dir: &Path, app_id: &str) -> Option<PathBuf> {
    let names = [app_id.to_string(), format!("{app_id}.exe")];
    let in_dir = |d: &Path| names.iter().map(|n| d.join(n)).find(|p| p.is_file());
    in_dir(dir).or_else(|| {
        fs::read_dir(dir)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .find_map(|sub| in_dir(&sub))
    })
}

fn move_aside(target: &Path) {
    if !target.exists() {
        return;
    }
    let old = target.with_extension("old");
    let _ = fs::remove_file(&old);
    let _ = fs::rename(target, old);
}

fn replace_file(src: &Path, target: &Path) -> Result<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    move_aside(target);
    if fs::rename(src, target).is_err() {
        fs::copy(src, target)?;
    }
    make_executable(target)
}

pub fn make_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    }
    let _ = path;
    Ok(())
}

pub fn prune_all(store: &Store) {
    for installed in store.apps.values() {
        if let Some(exe) = &installed.exe {
            prune_old(&installed.dir, exe, MAX_VERSION_FILES);
        }
    }
}

/// Zostawia w katalogu najwyzej `keep` plikow wersji: biezacy exe i najnowsze poprzednie.
pub fn prune_old(dir: &Path, exe: &Path, keep: usize) {
    let (Some(stem), Some(name)) = (exe.file_stem().and_then(|s| s.to_str()), exe.file_name()) else {
        return;
    };
    let ext = exe.extension().and_then(|e| e.to_str()).unwrap_or("");
    let Ok(entries) = fs::read_dir(dir) else { return };

    let mut old: Vec<(SystemTime, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let file = path.file_name()?;
            if file == name || !path.is_file() {
                return None;
            }
            let rest = file.to_str()?.strip_prefix(stem)?;
            if !rest.starts_with(|c: char| !c.is_alphanumeric()) {
                return None;
            }
            let file_ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if file_ext != "old" && !(file_ext == ext && is_executable(&path)) {
                return None;
            }
            Some((entry.metadata().ok()?.modified().ok()?, path))
        })
        .collect();

    old.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    for (_, path) in old.into_iter().skip(keep.saturating_sub(1)) {
        let _ = fs::remove_file(path);
    }
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata().map(|m| m.permissions().mode() & 0o111 != 0).unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        true
    }
}
