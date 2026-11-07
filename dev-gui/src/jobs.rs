use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use common::AppEntry;
use eframe::egui;
use updater_lib::api::Api;

use crate::config::{Config, ServerProfile};

#[derive(Clone)]
pub enum Status {
    Unknown,
    Connecting,
    Online,
    Error(String),
}

#[derive(Clone)]
pub struct Server {
    pub profile: ServerProfile,
    pub apps: Vec<AppEntry>,
    pub status: Status,
}

pub struct Model {
    pub servers: Vec<Server>,
    pub message: String,
    pub busy: bool,
}

pub type Shared = Arc<Mutex<Model>>;

#[derive(Clone, Default)]
pub struct AppForm {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub platform: String,
    pub notes: String,
    pub file: Option<PathBuf>,
}

impl Model {
    pub fn load() -> Self {
        let servers = Config::load()
            .servers
            .into_iter()
            .map(|profile| Server { profile, apps: vec![], status: Status::Unknown })
            .collect();
        Self { servers, message: String::new(), busy: false }
    }

    fn save(&mut self) {
        let config = Config { servers: self.servers.iter().map(|s| s.profile.clone()).collect() };
        if let Err(error) = config.save() {
            self.message = format!("Nie udało się zapisać listy serwerów: {error:#}");
        }
    }

    pub fn upsert_server(&mut self, mut profile: ServerProfile) -> u64 {
        if profile.id == 0 {
            profile.id = self.servers.iter().map(|s| s.profile.id).max().unwrap_or(0) + 1;
        }
        let id = profile.id;
        match self.servers.iter_mut().find(|s| s.profile.id == id) {
            Some(server) => {
                server.profile = profile;
                server.status = Status::Unknown;
            }
            None => self.servers.push(Server { profile, apps: vec![], status: Status::Unknown }),
        }
        self.save();
        id
    }

    pub fn remove_server(&mut self, id: u64) {
        self.servers.retain(|s| s.profile.id != id);
        self.save();
    }

    fn server_mut(&mut self, id: u64) -> Option<&mut Server> {
        self.servers.iter_mut().find(|s| s.profile.id == id)
    }

    fn api(&self, id: u64) -> Result<Api> {
        let server = self.servers.iter().find(|s| s.profile.id == id).context("brak serwera")?;
        Ok(Api::new(&server.profile.url).with_password(Some(server.profile.password.clone())))
    }
}

fn run<F>(shared: &Shared, ctx: &egui::Context, job: F)
where
    F: FnOnce(&Shared) -> Result<String> + Send + 'static,
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
        let result = job(&shared);
        let mut model = shared.lock().unwrap();
        model.busy = false;
        model.message = result.unwrap_or_else(|e| format!("Błąd: {e:#}"));
        ctx.request_repaint();
    });
}

fn api_for(shared: &Shared, id: u64) -> Result<Api> {
    shared.lock().unwrap().api(id)
}

fn reload(shared: &Shared, id: u64, api: &Api) -> Result<()> {
    let apps = api.list_apps()?;
    if let Some(server) = shared.lock().unwrap().server_mut(id) {
        server.apps = apps;
        server.status = Status::Online;
    }
    Ok(())
}

pub fn refresh(shared: &Shared, ctx: &egui::Context, id: u64) {
    let api = {
        let mut model = shared.lock().unwrap();
        if let Some(server) = model.server_mut(id) {
            server.status = Status::Connecting;
        }
        model.api(id)
    };
    let Ok(api) = api else { return };
    let shared = shared.clone();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let result = api.dev_check().and_then(|_| api.list_apps());
        if let Some(server) = shared.lock().unwrap().server_mut(id) {
            match result {
                Ok(apps) => {
                    server.apps = apps;
                    server.status = Status::Online;
                }
                Err(error) => server.status = Status::Error(format!("{error:#}")),
            }
        }
        ctx.request_repaint();
    });
}

pub fn refresh_all(shared: &Shared, ctx: &egui::Context) {
    let ids: Vec<u64> = shared.lock().unwrap().servers.iter().map(|s| s.profile.id).collect();
    for id in ids {
        refresh(shared, ctx, id);
    }
}

pub fn upload(shared: &Shared, ctx: &egui::Context, server: u64, form: AppForm) {
    run(shared, ctx, move |shared| {
        let api = api_for(shared, server)?;
        let file = form.file.as_ref().context("nie wybrano pliku")?;
        let name = if form.name.trim().is_empty() { &form.id } else { &form.name };
        api.dev_upsert_app(&form.id, name, &form.description)?;
        api.dev_upload(&form.id, &form.version, &form.platform, &form.notes, file)?;
        reload(shared, server, &api)?;
        Ok(format!("Wysłano {} {}", form.id, form.version))
    });
}

pub fn save_meta(shared: &Shared, ctx: &egui::Context, server: u64, form: AppForm) {
    run(shared, ctx, move |shared| {
        let api = api_for(shared, server)?;
        let name = if form.name.trim().is_empty() { &form.id } else { &form.name };
        api.dev_upsert_app(&form.id, name, &form.description)?;
        reload(shared, server, &api)?;
        Ok(format!("Zapisano {}", form.id))
    });
}

pub fn delete_release(shared: &Shared, ctx: &egui::Context, server: u64, app: String, version: String, platform: String) {
    run(shared, ctx, move |shared| {
        let api = api_for(shared, server)?;
        api.dev_delete_release(&app, &version, &platform)?;
        reload(shared, server, &api)?;
        Ok(format!("Usunięto {app} {version} [{platform}]"))
    });
}

pub fn delete_app(shared: &Shared, ctx: &egui::Context, server: u64, app: String) {
    run(shared, ctx, move |shared| {
        let api = api_for(shared, server)?;
        api.dev_delete_app(&app)?;
        reload(shared, server, &api)?;
        Ok(format!("Usunięto aplikację {app}"))
    });
}
