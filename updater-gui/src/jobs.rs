use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use common::{current_platform, is_newer, AppEntry};
use eframe::egui;
use updater_lib::api::Api;
use updater_lib::install;
use updater_lib::store::Store;

pub const SELF_ID: &str = "updater";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct Model {
    pub store: Store,
    pub remote: Vec<AppEntry>,
    pub status: String,
    pub busy: bool,
    pub progress: Option<(String, f32)>,
    pub dev_password: Option<String>,
    pub self_update: Option<String>,
}

impl Model {
    pub fn new() -> Self {
        Self {
            store: Store::load(),
            remote: vec![],
            status: String::new(),
            busy: false,
            progress: None,
            dev_password: None,
            self_update: None,
        }
    }

    fn api(&self) -> Api {
        Api::new(&self.store.server_url).with_password(self.dev_password.clone())
    }
}

pub type Shared = Arc<Mutex<Model>>;

#[derive(Clone, Default)]
pub struct DevForm {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub platform: String,
    pub notes: String,
    pub file: Option<PathBuf>,
}

fn run<F>(shared: &Shared, ctx: &egui::Context, job: F)
where
    F: FnOnce(&Shared, &egui::Context) -> Result<String> + Send + 'static,
{
    {
        let mut model = shared.lock().unwrap();
        if model.busy {
            return;
        }
        model.busy = true;
    }
    let shared = shared.clone();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let result = job(&shared, &ctx);
        let mut model = shared.lock().unwrap();
        model.busy = false;
        model.progress = None;
        model.status = result.unwrap_or_else(|e| format!("Błąd: {e:#}"));
        ctx.request_repaint();
    });
}

fn report_progress(shared: &Shared, ctx: &egui::Context, name: &str, done: u64, total: u64) {
    shared.lock().unwrap().progress = Some((name.to_string(), done as f32 / total.max(1) as f32));
    ctx.request_repaint();
}

fn fetch_remote(shared: &Shared) -> Result<usize> {
    let api = shared.lock().unwrap().api();
    let apps = api.list_apps()?;
    let count = apps.len();
    let newer = apps
        .iter()
        .find(|a| a.id == SELF_ID)
        .and_then(|a| a.latest(&current_platform()))
        .filter(|r| is_newer(&r.version, VERSION))
        .map(|r| r.version.clone());
    let mut model = shared.lock().unwrap();
    model.remote = apps;
    model.self_update = newer;
    Ok(count)
}

pub fn refresh(shared: &Shared, ctx: &egui::Context) {
    run(shared, ctx, |shared, _| {
        Ok(format!(
            "Pobrano listę: {} aplikacji",
            fetch_remote(shared)?
        ))
    });
}

pub fn install_one(shared: &Shared, ctx: &egui::Context, id: String) {
    run(shared, ctx, move |shared, ctx| {
        let (api, mut store, app, release) = {
            let model = shared.lock().unwrap();
            let app = model
                .remote
                .iter()
                .find(|a| a.id == id)
                .cloned()
                .context("brak aplikacji na serwerze")?;
            let release = app
                .latest(&current_platform())
                .cloned()
                .context("brak wersji dla tej platformy")?;
            (model.api(), model.store.clone(), app, release)
        };
        let installed =
            install::install_release(&api, &store, &app.id, &app.name, &release, |d, t| {
                report_progress(shared, ctx, &app.name, d, t)
            })?;
        store.apps.insert(app.id.clone(), installed);
        store.save()?;
        shared.lock().unwrap().store = store;
        Ok(format!("Zainstalowano {} {}", app.name, release.version))
    });
}

pub fn update_all(shared: &Shared, ctx: &egui::Context) {
    run(shared, ctx, |shared, ctx| {
        let (api, mut store) = {
            let model = shared.lock().unwrap();
            (model.api(), model.store.clone())
        };
        let updated = install::update_all(&api, &mut store, |name, d, t| {
            report_progress(shared, ctx, name, d, t)
        })?;
        shared.lock().unwrap().store = store;
        fetch_remote(shared)?;
        Ok(if updated.is_empty() {
            "Wszystko aktualne".into()
        } else {
            format!("Zaktualizowano: {}", updated.join(", "))
        })
    });
}

pub fn unlock(shared: &Shared, ctx: &egui::Context, password: String) {
    run(shared, ctx, move |shared, _| {
        let api = Api::new(&shared.lock().unwrap().store.server_url)
            .with_password(Some(password.clone()));
        api.dev_check()?;
        shared.lock().unwrap().dev_password = Some(password);
        Ok("Tryb dev odblokowany".into())
    });
}

pub fn upload(shared: &Shared, ctx: &egui::Context, form: DevForm) {
    run(shared, ctx, move |shared, _| {
        let api = shared.lock().unwrap().api();
        let file = form.file.as_ref().context("nie wybrano pliku")?;
        let name = if form.name.is_empty() {
            &form.id
        } else {
            &form.name
        };
        api.dev_upsert_app(&form.id, name, &form.description)?;
        api.dev_upload(&form.id, &form.version, &form.platform, &form.notes, file)?;
        fetch_remote(shared)?;
        Ok(format!("Wysłano {} {}", form.id, form.version))
    });
}

pub fn delete_release(
    shared: &Shared,
    ctx: &egui::Context,
    id: String,
    version: String,
    platform: String,
) {
    run(shared, ctx, move |shared, _| {
        let api = shared.lock().unwrap().api();
        api.dev_delete_release(&id, &version, &platform)?;
        fetch_remote(shared)?;
        Ok(format!("Usunięto {id} {version}"))
    });
}

pub fn save_meta(shared: &Shared, ctx: &egui::Context, form: DevForm) {
    run(shared, ctx, move |shared, _| {
        let api = shared.lock().unwrap().api();
        let name = if form.name.is_empty() {
            &form.id
        } else {
            &form.name
        };
        api.dev_upsert_app(&form.id, name, &form.description)?;
        fetch_remote(shared)?;
        Ok(format!("Zapisano opis {}", form.id))
    });
}

pub fn delete_app(shared: &Shared, ctx: &egui::Context, id: String) {
    run(shared, ctx, move |shared, _| {
        let api = shared.lock().unwrap().api();
        api.dev_delete_app(&id)?;
        fetch_remote(shared)?;
        Ok(format!("Usunięto aplikację {id}"))
    });
}

pub fn uninstall(shared: &Shared, ctx: &egui::Context, id: String) {
    run(shared, ctx, move |shared, _| {
        let mut store = shared.lock().unwrap().store.clone();
        install::uninstall(&mut store, &id)
            .context("nie udalo sie usunac (czy aplikacja jest zamknieta?)")?;
        shared.lock().unwrap().store = store;
        Ok(format!("Odinstalowano {id}"))
    });
}

pub fn self_update(shared: &Shared, ctx: &egui::Context) {
    run(shared, ctx, |shared, _| {
        let server = shared.lock().unwrap().store.server_url.clone();
        match updater_lib::selfupdate::self_update(&server, SELF_ID, VERSION)? {
            Some(_) => restart(),
            None => Ok("Updater jest aktualny".into()),
        }
    });
}

fn restart() -> ! {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe).spawn();
    }
    std::process::exit(0)
}
