use std::sync::{Arc, Mutex};

use common::{current_platform, parse_version, AppEntry};
use eframe::egui::{self, Align, Align2, Layout, RichText};
use gui_common::{badge, card, GRAY, GREEN, ORANGE, RED};

use crate::config::ServerProfile;
use crate::format;
use crate::jobs::{self, AppForm, Model, Server, Shared, Status};

const PLATFORMS: [&str; 5] = [
    "windows-x86_64",
    "linux-x86_64",
    "linux-aarch64",
    "macos-aarch64",
    "macos-x86_64",
];

#[derive(Clone, PartialEq)]
enum Selection {
    None,
    New,
    App(String),
}

enum Confirm {
    App(String),
    Release { app: String, version: String, platform: String },
    Server(u64),
}

struct ServerForm {
    id: u64,
    name: String,
    url: String,
    password: String,
    show_password: bool,
}

impl ServerForm {
    fn blank() -> Self {
        Self { id: 0, name: String::new(), url: String::new(), password: String::new(), show_password: false }
    }

    fn from_profile(profile: &ServerProfile) -> Self {
        Self {
            id: profile.id,
            name: profile.name.clone(),
            url: profile.url.clone(),
            password: profile.password.clone(),
            show_password: false,
        }
    }
}

fn blank_form() -> AppForm {
    AppForm { platform: current_platform(), ..Default::default() }
}

fn latest_label(app: &AppEntry) -> String {
    app.releases
        .iter()
        .map(|r| parse_version(&r.version))
        .max()
        .map_or_else(|| "brak wydań".to_string(), |v| v.to_string())
}

pub struct DevApp {
    shared: Shared,
    first_frame: bool,
    selected: Option<u64>,
    selection: Selection,
    form: AppForm,
    server_form: Option<ServerForm>,
    confirm: Option<Confirm>,
    pending_select: Option<String>,
}

impl DevApp {
    pub fn new() -> Self {
        Self {
            shared: Arc::new(Mutex::new(Model::load())),
            first_frame: true,
            selected: None,
            selection: Selection::None,
            form: blank_form(),
            server_form: None,
            confirm: None,
            pending_select: None,
        }
    }

    fn select_server(&mut self, id: Option<u64>) {
        self.selected = id;
        self.selection = Selection::None;
        self.form = blank_form();
    }

    fn pick_app(&mut self, app: &AppEntry) {
        let mut form = blank_form();
        form.id = app.id.clone();
        form.name = app.name.clone();
        form.description = app.description.clone();
        if let Some(mut next) = app.releases.iter().map(|r| parse_version(&r.version)).max() {
            next.patch += 1;
            form.version = next.to_string();
        }
        self.form = form;
        self.selection = Selection::App(app.id.clone());
    }

    fn servers_panel(&mut self, ui: &mut egui::Ui, servers: &[Server]) {
        ui.add_space(4.0);
        ui.heading("Serwery");
        ui.add_space(4.0);
        for server in servers {
            let id = server.profile.id;
            let selected = self.selected == Some(id);
            ui.horizontal(|ui| {
                ui.colored_label(status_color(&server.status), "●");
                let label = ui
                    .selectable_label(selected, &server.profile.name)
                    .on_hover_text(format!("{}\n{}", server.profile.url, status_text(&server.status)));
                if label.clicked() && !selected {
                    self.select_server(Some(id));
                }
            });
        }
        ui.add_space(8.0);
        if ui.button("+ Dodaj serwer").clicked() {
            self.server_form = Some(ServerForm::blank());
        }
        if let Some(server) = servers.iter().find(|s| Some(s.profile.id) == self.selected) {
            ui.separator();
            if ui.button("Edytuj serwer").clicked() {
                self.server_form = Some(ServerForm::from_profile(&server.profile));
            }
            if ui.button("Usuń serwer").clicked() {
                self.confirm = Some(Confirm::Server(server.profile.id));
            }
        }
    }

    fn apps_panel(&mut self, ui: &mut egui::Ui, server: &Server) {
        ui.add_space(4.0);
        ui.heading("Aplikacje");
        ui.add_space(4.0);
        if ui.button("+ Nowa aplikacja").clicked() {
            self.form = blank_form();
            self.selection = Selection::New;
        }
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            if server.apps.is_empty() {
                ui.label(RichText::new("Brak aplikacji").weak());
            }
            for app in &server.apps {
                let selected = self.selection == Selection::App(app.id.clone());
                let label = format!("{}  ·  {}", app.name, latest_label(app));
                if ui.selectable_label(selected, label).clicked() {
                    self.pick_app(app);
                }
            }
        });
    }

    fn detail_panel(&mut self, ui: &mut egui::Ui, server: &Server, busy: bool) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.heading(&server.profile.name);
            match &server.status {
                Status::Online => badge(ui, "Połączono", GREEN),
                Status::Connecting => badge(ui, "Łączenie…", ORANGE),
                Status::Unknown => badge(ui, "Nie połączono", GRAY),
                Status::Error(_) => badge(ui, "Błąd połączenia", RED),
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Odśwież").clicked() {
                    jobs::refresh(&self.shared, ui.ctx(), server.profile.id);
                }
            });
        });
        ui.label(RichText::new(&server.profile.url).weak());
        if let Status::Error(error) = &server.status {
            ui.colored_label(RED, error);
        }
        ui.separator();

        let can_edit = !busy && matches!(server.status, Status::Online);
        match self.selection.clone() {
            Selection::None => {
                ui.add_space(40.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("Wybierz aplikację z listy albo utwórz nową.").weak());
                });
            }
            Selection::New => self.app_editor(ui, server, None, can_edit),
            Selection::App(id) => match server.apps.iter().find(|a| a.id == id) {
                Some(app) => self.app_editor(ui, server, Some(app), can_edit),
                None => self.selection = Selection::None,
            },
        }
    }

    fn app_editor(&mut self, ui: &mut egui::Ui, server: &Server, app: Option<&AppEntry>, can_edit: bool) {
        let server_id = server.profile.id;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            card(ui, |ui| {
                ui.strong("Informacje");
                egui::Grid::new("meta").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                    ui.label("ID");
                    if app.is_some() {
                        ui.label(RichText::new(&self.form.id).monospace());
                    } else {
                        ui.add(egui::TextEdit::singleline(&mut self.form.id).hint_text("np. mojapp"));
                    }
                    ui.end_row();
                    ui.label("Nazwa");
                    ui.add(egui::TextEdit::singleline(&mut self.form.name).desired_width(f32::INFINITY));
                    ui.end_row();
                    ui.label("Opis");
                    ui.add(
                        egui::TextEdit::multiline(&mut self.form.description)
                            .desired_rows(2)
                            .desired_width(f32::INFINITY),
                    );
                    ui.end_row();
                });
                ui.horizontal(|ui| {
                    let has_id = !self.form.id.trim().is_empty();
                    if ui.add_enabled(can_edit && has_id, egui::Button::new("Zapisz opis")).clicked() {
                        jobs::save_meta(&self.shared, ui.ctx(), server_id, self.form.clone());
                    }
                    if app.is_some() && ui.add_enabled(can_edit, egui::Button::new("Usuń aplikację")).clicked() {
                        self.confirm = Some(Confirm::App(self.form.id.clone()));
                    }
                });
            });

            if let Some(app) = app {
                card(ui, |ui| {
                    ui.strong("Wydania");
                    if app.releases.is_empty() {
                        ui.label(RichText::new("Brak wydań").weak());
                        return;
                    }
                    let mut releases: Vec<_> = app.releases.iter().collect();
                    releases.sort_by(|a, b| {
                        parse_version(&b.version)
                            .cmp(&parse_version(&a.version))
                            .then_with(|| a.platform.cmp(&b.platform))
                    });
                    egui::Grid::new("releases").striped(true).spacing([18.0, 6.0]).show(ui, |ui| {
                        for header in ["Wersja", "Platforma", "Rozmiar", "Data", "Plik", ""] {
                            ui.label(RichText::new(header).strong());
                        }
                        ui.end_row();
                        for release in releases {
                            let version = ui.label(&release.version);
                            if !release.notes.is_empty() {
                                version.on_hover_text(&release.notes);
                            }
                            ui.label(&release.platform);
                            ui.label(format::size(release.size));
                            ui.label(format::date(release.published_at));
                            ui.label(&release.filename);
                            if ui.add_enabled(can_edit, egui::Button::new("Usuń")).clicked() {
                                self.confirm = Some(Confirm::Release {
                                    app: app.id.clone(),
                                    version: release.version.clone(),
                                    platform: release.platform.clone(),
                                });
                            }
                            ui.end_row();
                        }
                    });
                });
            }

            card(ui, |ui| {
                ui.strong("Nowe wydanie");
                egui::Grid::new("release").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                    ui.label("Wersja");
                    ui.add(egui::TextEdit::singleline(&mut self.form.version).hint_text("1.2.3"));
                    ui.end_row();
                    ui.label("Platforma");
                    egui::ComboBox::from_id_salt("platform")
                        .selected_text(self.form.platform.clone())
                        .show_ui(ui, |ui| {
                            for platform in PLATFORMS {
                                ui.selectable_value(&mut self.form.platform, platform.to_string(), platform);
                            }
                        });
                    ui.end_row();
                    ui.label("Notatki");
                    ui.add(
                        egui::TextEdit::multiline(&mut self.form.notes)
                            .desired_rows(2)
                            .desired_width(f32::INFINITY),
                    );
                    ui.end_row();
                    ui.label("Plik");
                    ui.horizontal(|ui| {
                        let picked = ui.button("Wybierz…").clicked().then(|| rfd::FileDialog::new().pick_file());
                        if let Some(Some(path)) = picked {
                            self.form.file = Some(path);
                        }
                        let name = self
                            .form
                            .file
                            .as_ref()
                            .and_then(|p| p.file_name())
                            .map_or_else(|| "nie wybrano".to_string(), |n| n.to_string_lossy().to_string());
                        ui.label(name);
                    });
                    ui.end_row();
                });
                let ready = can_edit
                    && !self.form.id.trim().is_empty()
                    && !self.form.version.trim().is_empty()
                    && self.form.file.is_some();
                let label = if app.is_some() { "Wyślij wydanie" } else { "Utwórz i wyślij wydanie" };
                if ui.add_enabled(ready, egui::Button::new(label)).clicked() {
                    if app.is_none() {
                        self.pending_select = Some(self.form.id.clone());
                    }
                    jobs::upload(&self.shared, ui.ctx(), server_id, self.form.clone());
                }
            });
        });
    }

    fn server_window(&mut self, ctx: &egui::Context) {
        let Some(form) = self.server_form.as_mut() else { return };
        let (mut save, mut cancel) = (false, false);
        let title = if form.id == 0 { "Nowy serwer" } else { "Edytuj serwer" };
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                egui::Grid::new("server_form").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                    ui.label("Nazwa");
                    ui.text_edit_singleline(&mut form.name);
                    ui.end_row();
                    ui.label("Adres");
                    ui.add(egui::TextEdit::singleline(&mut form.url).hint_text("https://updates.example.com"));
                    ui.end_row();
                    ui.label("Hasło dev");
                    let hide = !form.show_password;
                    ui.add(egui::TextEdit::singleline(&mut form.password).password(hide));
                    ui.end_row();
                });
                ui.checkbox(&mut form.show_password, "Pokaż hasło");
                ui.horizontal(|ui| {
                    let valid = !form.name.trim().is_empty() && !form.url.trim().is_empty();
                    save = ui.add_enabled(valid, egui::Button::new("Zapisz")).clicked();
                    cancel = ui.button("Anuluj").clicked();
                });
            });

        if save {
            if let Some(form) = self.server_form.take() {
                let profile = ServerProfile {
                    id: form.id,
                    name: form.name.trim().to_string(),
                    url: form.url.trim().trim_end_matches('/').to_string(),
                    password: form.password,
                };
                let id = self.shared.lock().unwrap().upsert_server(profile);
                self.select_server(Some(id));
                jobs::refresh(&self.shared, ctx, id);
            }
        } else if cancel {
            self.server_form = None;
        }
    }

    fn confirm_window(&mut self, ctx: &egui::Context) {
        let text = match &self.confirm {
            None => return,
            Some(Confirm::App(id)) => format!("Usunąć aplikację {id} razem ze wszystkimi wydaniami?"),
            Some(Confirm::Release { app, version, platform }) => {
                format!("Usunąć wydanie {version} [{platform}] aplikacji {app}?")
            }
            Some(Confirm::Server(_)) => "Usunąć serwer z listy? Zapisane hasło zostanie skasowane.".to_string(),
        };
        let mut decision = None;
        egui::Window::new("Potwierdź")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(text);
                ui.horizontal(|ui| {
                    if ui.button("Usuń").clicked() {
                        decision = Some(true);
                    }
                    if ui.button("Anuluj").clicked() {
                        decision = Some(false);
                    }
                });
            });
        match decision {
            Some(true) => {
                let server = self.selected.unwrap_or_default();
                match self.confirm.take() {
                    Some(Confirm::App(id)) => {
                        self.selection = Selection::None;
                        jobs::delete_app(&self.shared, ctx, server, id);
                    }
                    Some(Confirm::Release { app, version, platform }) => {
                        jobs::delete_release(&self.shared, ctx, server, app, version, platform);
                    }
                    Some(Confirm::Server(id)) => self.shared.lock().unwrap().remove_server(id),
                    None => {}
                }
            }
            Some(false) => self.confirm = None,
            None => {}
        }
    }
}

fn status_color(status: &Status) -> egui::Color32 {
    match status {
        Status::Online => GREEN,
        Status::Connecting => ORANGE,
        Status::Error(_) => RED,
        Status::Unknown => GRAY,
    }
}

fn status_text(status: &Status) -> String {
    match status {
        Status::Online => "Połączono".to_string(),
        Status::Connecting => "Łączenie…".to_string(),
        Status::Unknown => "Nie połączono".to_string(),
        Status::Error(error) => format!("Błąd: {error}"),
    }
}

impl eframe::App for DevApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        if self.first_frame {
            self.first_frame = false;
            jobs::refresh_all(&self.shared, ctx);
        }

        let (servers, message, busy) = {
            let model = self.shared.lock().unwrap();
            (model.servers.clone(), model.message.clone(), model.busy)
        };

        let selected_valid = self.selected.is_some_and(|id| servers.iter().any(|s| s.profile.id == id));
        if !selected_valid {
            self.select_server(servers.first().map(|s| s.profile.id));
        }

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if busy {
                    ui.spinner();
                }
                ui.label(&message);
            });
        });

        egui::SidePanel::left("servers").default_width(190.0).show(ctx, |ui| {
            self.servers_panel(ui, &servers);
        });

        match self.selected.and_then(|id| servers.iter().find(|s| s.profile.id == id)) {
            Some(server) => {
                if !busy {
                    let pending = self.pending_select.take().and_then(|id| server.apps.iter().find(|a| a.id == id));
                    if let Some(app) = pending {
                        self.pick_app(app);
                    }
                }
                egui::SidePanel::left("apps").default_width(230.0).show(ctx, |ui| {
                    self.apps_panel(ui, server);
                });
                egui::CentralPanel::default().show(ctx, |ui| {
                    self.detail_panel(ui, server, busy);
                });
            }
            None => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(80.0);
                        ui.heading("Brak serwerów");
                        ui.label("Dodaj serwer, aby zarządzać aplikacjami.");
                    });
                });
            }
        }

        self.server_window(ctx);
        self.confirm_window(ctx);
    }
}
