use anyhow::{bail, Result};
use common::{current_platform, is_newer, Release};

use crate::api::Api;
use crate::install::make_executable;

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
/// Wymaga, by release byl pojedynczym plikiem wykonywalnym (nie zip).
pub fn self_update(server: &str, app_id: &str, current_version: &str) -> Result<Option<String>> {
    let Some(release) = check(server, app_id, current_version)? else {
        return Ok(None);
    };
    if release.filename.ends_with(".zip") {
        bail!("self_update wymaga wydania w postaci pojedynczego pliku exe");
    }
    let tmp = std::env::temp_dir().join(format!("selfupdate-{app_id}-{}", release.filename));
    Api::new(server).download(app_id, &release, &tmp, |_, _| {})?;
    make_executable(&tmp)?;
    self_replace::self_replace(&tmp)?;
    let _ = std::fs::remove_file(&tmp);
    Ok(Some(release.version))
}
