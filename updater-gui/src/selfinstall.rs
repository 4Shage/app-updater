use std::fs;
use std::process::Command;

use updater_lib::install::make_executable;
use updater_lib::shortcut;
use updater_lib::store::Store;

const SHORTCUT_NAME: &str = "App Updater";

/// Kopiuje updater do katalogu domowego, tworzy skrot i uruchamia kopie.
/// Zwraca true, gdy uruchomiono zainstalowana kopie i ten proces ma sie zakonczyc.
pub fn run() -> bool {
    if cfg!(debug_assertions) {
        return false;
    }
    install().unwrap_or(false)
}

fn install() -> Option<bool> {
    let current = std::env::current_exe().ok()?;
    let home = Store::home_dir();
    let here = current.canonicalize().unwrap_or_else(|_| current.clone());
    if here.starts_with(home.canonicalize().unwrap_or_else(|_| home.clone())) {
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
