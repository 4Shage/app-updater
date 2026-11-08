use std::fs;
use std::path::Path;

use anyhow::{anyhow, Result};
use common::{current_platform, is_newer, Release};

use crate::api::Api;
use crate::install::{extract_archive, find_exe, is_archive, make_executable};

pub fn check(server: &str, app_id: &str, current_version: &str) -> Result<Option<Release>> {
    let apps = Api::new(server).list_apps()?;
    Ok(apps
        .iter()
        .find(|a| a.id == app_id)
        .and_then(|a| a.latest(&current_platform()))
        .filter(|r| is_newer(&r.version, current_version))
        .cloned())
}

/// Podmienia biezacy plik exe nowa wersja. Zwraca nowa wersje, jesli byla aktualizacja.
/// Wydanie moze byc pojedynczym plikiem albo archiwum zip / tar.gz z plikiem o nazwie `app_id`.
pub fn self_update(server: &str, app_id: &str, current_version: &str) -> Result<Option<String>> {
    let Some(release) = check(server, app_id, current_version)? else {
        return Ok(None);
    };
    let tmp = std::env::temp_dir().join(format!("selfupdate-{app_id}-{}", release.filename));
    Api::new(server).download(app_id, &release, &tmp, |_, _| {})?;

    let result = replace_current(&tmp, app_id);
    let _ = fs::remove_file(&tmp);
    result?;
    Ok(Some(release.version))
}

fn replace_current(downloaded: &Path, app_id: &str) -> Result<()> {
    if !is_archive(downloaded) {
        make_executable(downloaded)?;
        self_replace::self_replace(downloaded)?;
        return Ok(());
    }

    let dir = std::env::temp_dir().join(format!("selfupdate-{app_id}-extracted"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir)?;
    let result = (|| -> Result<()> {
        extract_archive(downloaded, &dir, app_id)?;
        let exe = find_exe(&dir, app_id).ok_or_else(|| anyhow!("brak pliku {app_id} w archiwum"))?;
        make_executable(&exe)?;
        self_replace::self_replace(&exe)?;
        Ok(())
    })();
    let _ = fs::remove_dir_all(&dir);
    result
}

/// Usuwa biezacy plik exe oraz katalog `dir`, w ktorym sie znajduje.
pub fn self_delete(dir: &Path) -> Result<()> {
    self_replace::self_delete_outside_path(dir)?;
    Ok(())
}
