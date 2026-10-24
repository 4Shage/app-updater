use std::path::PathBuf;

use updater_lib::api::Api;
use updater_lib::install;
use updater_lib::store::Store;

pub fn run(args: &[String]) -> Option<i32> {
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["--update-all"] => Some(update_all()),
        ["--register", id, version, exe] => Some(register(id, version, exe)),
        _ => None,
    }
}

fn update_all() -> i32 {
    let mut store = Store::load();
    let api = Api::new(&store.server_url);
    match install::update_all(&api, &mut store, |_, _, _| {}) {
        Ok(updated) if updated.is_empty() => {
            println!("wszystko aktualne");
            0
        }
        Ok(updated) => {
            updated.iter().for_each(|u| println!("zaktualizowano: {u}"));
            0
        }
        Err(e) => {
            eprintln!("blad: {e:#}");
            1
        }
    }
}

fn register(id: &str, version: &str, exe: &str) -> i32 {
    let mut store = Store::load();
    store.register(id, version, PathBuf::from(exe));
    match store.save() {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("blad: {e:#}");
            1
        }
    }
}
