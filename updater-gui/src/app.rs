use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use common::{current_platform, is_newer, AppEntry};
use eframe::egui::{self, Align, Align2, Layout, RichText};
use gui_common::{badge, card, GRAY, GREEN, ORANGE};
use updater_lib::store::Store;

use crate::jobs::{self, Model, Shared, SELF_ID, VERSION};

enum Action {
    Install(String),
    Launch(PathBuf),
    AskUninstall(String),
}

enum Confirm {
    App(String),
    SelfUninstall,
}

enum AppState {
    NoRelease,
    NotInstalled(String),
    UpToDate(String),
    Update { from: String, to: String },
}

impl AppState {
    fn of(app: &AppEntry, store: &Store, platform: &str) -> Self {
        let Some(latest) = app.latest(platform) else {
            return Self::NoRelease;
        };
        match store.apps.get(&app.id) {
            None => Self::NotInstalled(latest.version.clone()),
            Some(local) if is_newer(&latest.version, &local.version) => {
                Self::Update { from: local.version.clone(), to: latest.version.clone() }
            }
            Some(local) => Self::UpToDate(local.version.clone()),
        }
    }

    fn rank(&self) -> u8 {
        match self {
            Self::Update { .. } => 0,
            Self::UpToDate(_) => 1,
            Self::NotInstalled(_) => 2,
            Self::NoRelease => 3,
        }
    }
}

pub struct UpdaterApp {
    shared: Shared,
    first_frame: bool,
    confirm: Option<Confirm>,
}

impl UpdaterApp {
    pub fn new() -> Self {
        Self { shared: Arc::new(Mutex::new(Model::new())), first_frame: true, confirm: None }
    }

    fn confirm_window(&mut self, ctx: &egui::Context) {
        let (title, text) = match &self.confirm {
            None => return,
            Some(Confirm::App(id)) => (
                "Odinstalować aplikację?",
                format!("Pliki i skróty aplikacji {id} zostaną usunięte."),
            ),
            Some(Confirm::SelfUninstall) => (
                "Odinstalować updater?",
                "Zostaną usunięte: updater, jego skróty, ustawienia oraz wszystkie aplikacje zainstalowane przez updater."
                    .to_string(),
            ),
        };
        let mut decision = None;
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(text);
                ui.horizontal(|ui| {
                    if ui.button("Odinstaluj").clicked() {
                        decision = Some(true);
                    }
                    if ui.button("Anuluj").clicked() {
                        decision = Some(false);
                    }
                });
            });
        match decision {
            Some(true) => match self.confirm.take() {
                Some(Confirm::App(id)) => jobs::uninstall(&self.shared, ctx, id),
                Some(Confirm::SelfUninstall) => jobs::uninstall_self(&self.shared, ctx),
                None => {}
            },
            Some(false) => self.confirm = None,
            None => {}
        }
    }

    fn app_card(ui: &mut egui::Ui, app: &AppEntry, state: &AppState, exe: Option<&PathBuf>, busy: bool) -> Option<Action> {
        let mut action = None;
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(&app.name).strong().size(17.0));
                    if !app.description.is_empty() {
                        ui.label(RichText::new(&app.description).weak());
                    }
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let installed = matches!(state, AppState::UpToDate(_) | AppState::Update { .. });
                    if matches!(state, AppState::NotInstalled(_) | AppState::Update { .. }) {
                        let label = if installed { "Aktualizuj" } else { "Zainstaluj" };
                        if ui.add_enabled(!busy, egui::Button::new(label)).clicked() {
                            action = Some(Action::Install(app.id.clone()));
                        }
                    }
                    if installed {
                        if ui.add_enabled(!busy, egui::Button::new("Odinstaluj")).clicked() {
                            action = Some(Action::AskUninstall(app.id.clone()));
                        }
                        match exe {
                            Some(exe) if ui.button("Uruchom").clicked() => action = Some(Action::Launch(exe.clone())),
                            _ => {}
                        }
                    }
                    match state {
                        AppState::NoRelease => badge(ui, "Brak wersji dla tej platformy", GRAY),
                        AppState::NotInstalled(v) => badge(ui, &format!("Dostępna {v}"), GRAY),
                        AppState::UpToDate(v) => badge(ui, &format!("Aktualna {v}"), GREEN),
                        AppState::Update { from, to } => badge(ui, &format!("{from} → {to}"), ORANGE),
                    }
                });
            });
        });
        action
    }
}

impl eframe::App for UpdaterApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        if self.first_frame {
            self.first_frame = false;
            jobs::refresh(&self.shared, ctx);
        }

        let (remote, store, busy, status, progress, self_update) = {
            let m = self.shared.lock().unwrap();
            (m.remote.clone(), m.store.clone(), m.busy, m.status.clone(), m.progress.clone(), m.self_update.clone())
        };

        let platform = current_platform();
        let mut apps: Vec<(&AppEntry, AppState)> = remote
            .iter()
            .filter(|a| a.id != SELF_ID)
            .map(|a| (a, AppState::of(a, &store, &platform)))
            .collect();
        apps.sort_by_key(|(app, state)| (state.rank(), app.name.to_lowercase()));
        let updates = apps.iter().filter(|(_, s)| matches!(s, AppState::Update { .. })).count();

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.heading("App Updater");
                ui.label(RichText::new(format!("v{VERSION}")).weak());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.menu_button("⚙", |ui| {
                        if ui.button("Odinstaluj updater").clicked() {
                            self.confirm = Some(Confirm::SelfUninstall);
                            ui.close_menu();
                        }
                    });
                    let label = if updates > 0 {
                        format!("Aktualizuj wszystko ({updates})")
                    } else {
                        "Aktualizuj wszystko".to_string()
                    };
                    if ui.add_enabled(!busy && updates > 0, egui::Button::new(label)).clicked() {
                        jobs::update_all(&self.shared, ctx);
                    }
                    if ui.add_enabled(!busy, egui::Button::new("Odśwież")).clicked() {
                        jobs::refresh(&self.shared, ctx);
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.label("Serwer");
                let mut url = store.server_url.clone();
                let response = ui.add(egui::TextEdit::singleline(&mut url).desired_width(f32::INFINITY));
                if response.changed() {
                    self.shared.lock().unwrap().store.server_url = url;
                }
                if response.lost_focus() {
                    let _ = self.shared.lock().unwrap().store.save();
                }
            });
            ui.add_space(4.0);
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if busy {
                    ui.spinner();
                }
                if let Some((name, fraction)) = &progress {
                    ui.add(
                        egui::ProgressBar::new(*fraction)
                            .desired_width(240.0)
                            .text(format!("{name} {:.0}%", fraction * 100.0)),
                    );
                }
                ui.label(&status);
            });
        });

        let mut action = None;
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                if let Some(version) = &self_update {
                    card(ui, |ui| {
                        ui.horizontal(|ui| {
                            badge(ui, "Nowa wersja updatera", ORANGE);
                            ui.label(version);
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.add_enabled(!busy, egui::Button::new("Zaktualizuj updater")).clicked() {
                                    jobs::self_update(&self.shared, ctx);
                                }
                            });
                        });
                    });
                }
                if apps.is_empty() {
                    ui.label(RichText::new("Brak aplikacji na serwerze. Sprawdź adres serwera i odśwież listę.").weak());
                }
                for (app, state) in &apps {
                    let exe = store.apps.get(&app.id).and_then(|i| i.exe.as_ref());
                    if let Some(chosen) = Self::app_card(ui, app, state, exe, busy) {
                        action = Some(chosen);
                    }
                }
            });
        });

        self.confirm_window(ctx);

        match action {
            Some(Action::Install(id)) => jobs::install_one(&self.shared, ctx, id),
            Some(Action::AskUninstall(id)) => self.confirm = Some(Confirm::App(id)),
            Some(Action::Launch(exe)) => {
                let _ = std::process::Command::new(exe).spawn();
            }
            None => {}
        }
    }
}
