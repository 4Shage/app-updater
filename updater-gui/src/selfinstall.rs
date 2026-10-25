use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use updater_lib::install::{self, make_executable, prune_old, MAX_VERSION_FILES};
use updater_lib::shortcut;
use updater_lib::store::Store;

const SHORTCUT_NAME: &str = "App Updater";

/// Kopiuje updater do katalogu domowego, tworzy skrot i uruchamia kopie.
/// Zwraca true, gdy uruchomiono zainstalowana kopie i ten proces ma sie zakonczyc.
pub fn run() -> bool {
    if cfg!(debug_assertions) {
        return false;
    }
    copy_self().unwrap_or(false)
}

fn copy_self() -> Option<bool> {
    let current = std::env::current_exe().ok()?;
    let home = Store::home_dir();
    let here = current.canonicalize().unwrap_or_else(|_| current.clone());
    if here.starts_with(home.canonicalize().unwrap_or_else(|_| home.clone())) {
        prune_old(&home, &current, MAX_VERSION_FILES);
        return Some(false);
    }

    let target = home.join(format!("updater{}", std::env::consts::EXE_SUFFIX));
    fs::create_dir_all(&home).ok()?;
    if target.exists() {
        let old = target.with_extension("old");
        let _ = fs::remove_file(&old);
        let _ = fs::rename(&target, old);
    }
    fs::copy(&current, &target).ok()?;
    make_executable(&target).ok()?;
    let _ = shortcut::create(SHORTCUT_NAME, &target);

    Command::new(&target).spawn().ok()?;
    Some(true)
}

/// Usuwa aplikacje zarzadzane przez updater, skroty, ustawienia i na koniec katalog domowy.
pub fn uninstall(store: &mut Store) -> Result<()> {
    let root = store.install_root();
    let managed: Vec<String> = store
        .apps
        .iter()
        .filter(|(_, app)| app.dir.starts_with(&root))
        .map(|(id, _)| id.clone())
        .collect();
    for id in managed {
        install::uninstall(store, &id)
            .with_context(|| format!("nie udalo sie usunac {id} (zamknij aplikacje)"))?;
    }

    shortcut::remove(SHORTCUT_NAME);
    Store::delete_state();
    remove_home_later(&Store::home_dir());
    Ok(())
}

#[cfg(windows)]
fn remove_home_later(home: &Path) {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let _ = Command::new("cmd")
        .raw_arg(format!("/C ping -n 3 127.0.0.1 >nul & rmdir /s /q \"{}\"", home.display()))
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
}

#[cfg(not(windows))]
fn remove_home_later(home: &Path) {
    let _ = Command::new("sh")
        .args(["-c", "sleep 2; rm -rf \"$0\""])
        .arg(home)
        .spawn();
}
