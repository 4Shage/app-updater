use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use common::{current_platform, is_newer, AppEntry};
use eframe::egui;
use updater_lib::store::Store;

use crate::jobs::{self, DevForm, Model, Shared};

#[derive(PartialEq)]
enum Tab {
    Apps,
    Dev,
}

enum Action {
    Install(String),
    Launch(PathBuf),
}

pub struct UpdaterApp {
    shared: Shared,
    tab: Tab,
    first_frame: bool,
    show_password: bool,
    password_input: String,
    form: DevForm,
}

impl UpdaterApp {
    pub fn new() -> Self {
        Self {
            shared: Arc::new(Mutex::new(Model::new())),
            tab: Tab::Apps,
            first_frame: true,
            show_password: false,
            password_input: String::new(),
            form: DevForm { platform: current_platform(), ..Default::default() },
        }
    }

    fn password_window(&mut self, ctx: &egui::Context) {
        if !self.show_password {
            return;
        }
        let mut submit = false;
        let mut cancel = false;
        egui::Window::new("Tryb dev").collapsible(false).resizable(false).show(ctx, |ui| {
            let edit = ui.add(egui::TextEdit::singleline(&mut self.password_input).password(true));
            edit.request_focus();
            let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            ui.horizontal(|ui| {
                submit = ui.button("OK").clicked() || enter;
                cancel = ui.button("Anuluj").clicked();
            });
        });
        if submit {
            jobs::unlock(&self.shared, ctx, std::mem::take(&mut self.password_input));
        }
        if submit || cancel {
            self.show_password = false;
            self.password_input.clear();
        }
    }

    fn apps_tab(&self, ui: &mut egui::Ui, remote: &[AppEntry], store: &Store, busy: bool) -> Option<Action> {
        let platform = current_platform();
        let mut action = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            if remote.is_empty() {
                ui.label("Brak aplikacji. Odśwież listę.");
            }
            for app in remote {
                let installed = store.apps.get(&app.id);
                let latest = app.latest(&platform);
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.strong(&app.name);
                        ui.label(egui::RichText::new(&app.id).weak());
                    });
                    if !app.description.is_empty() {
                        ui.label(&app.description);
                    }
                    if let Some(notes) = latest.map(|l| &l.notes).filter(|n| !n.is_empty()) {
                        ui.label(egui::RichText::new(notes).small());
                    }
                    ui.horizontal(|ui| {
                        let Some(latest) = latest else {
                            ui.label("brak wersji dla tej platformy");
                            return;
                        };
                        match installed {
                            None => {
                                ui.label(format!("najnowsza: {}", latest.version));
                                if ui.add_enabled(!busy, egui::Button::new("Zainstaluj")).clicked() {
                                    action = Some(Action::Install(app.id.clone()));
                                }
                            }
                            Some(local) => {
                                if is_newer(&latest.version, &local.version) {
                                    ui.label(format!("{} → {}", local.version, latest.version));
                                    if ui.add_enabled(!busy, egui::Button::new("Aktualizuj")).clicked() {
                                        action = Some(Action::Install(app.id.clone()));
                                    }
                                } else {
                                    ui.label(format!("{} (aktualna)", local.version));
                                }
                                if let Some(exe) = &local.exe {
                                    if ui.button("Uruchom").clicked() {
                                        action = Some(Action::Launch(exe.clone()));
                                    }
                                }
                            }
                        }
                    });
                });
            }
        });
        action
    }

    fn dev_tab(&mut self, ui: &mut egui::Ui, remote: &[AppEntry], busy: bool) {
        let ctx = ui.ctx().clone();
        egui::Grid::new("dev_form").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
            for (label, value) in [
                ("ID aplikacji", &mut self.form.id),
                ("Nazwa", &mut self.form.name),
                ("Opis", &mut self.form.description),
                ("Wersja (semver)", &mut self.form.version),
                ("Platforma", &mut self.form.platform),
                ("Notatki", &mut self.form.notes),
            ] {
                ui.label(label);
                ui.text_edit_singleline(value);
                ui.end_row();
            }
            ui.label("Plik (exe lub zip)");
            ui.horizontal(|ui| {
                if ui.button("Wybierz…").clicked() {
                    self.form.file = rfd::FileDialog::new().pick_file();
                }
                ui.label(self.form.file.as_ref().map(|p| p.display().to_string()).unwrap_or_default());
            });
            ui.end_row();
        });
        ui.horizontal(|ui| {
            let ready = !self.form.id.is_empty() && !self.form.version.is_empty() && self.form.file.is_some();
            if ui.add_enabled(!busy && ready, egui::Button::new("Wyślij na serwer")).clicked() {
                jobs::upload(&self.shared, &ctx, self.form.clone());
            }
            if ui.button("Zablokuj tryb dev").clicked() {
                self.shared.lock().unwrap().dev_password = None;
                self.tab = Tab::Apps;
            }
        });

        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            for app in remote {
                ui.strong(&app.id);
                for release in &app.releases {
                    ui.horizontal(|ui| {
                        ui.label(format!("{} [{}] {}", release.version, release.platform, release.filename));
                        if ui.add_enabled(!busy, egui::Button::new("Usuń")).clicked() {
                            jobs::delete_release(
                                &self.shared,
                                &ctx,
                                app.id.clone(),
                                release.version.clone(),
                                release.platform.clone(),
                            );
                        }
                    });
                }
            }
        });
    }
}

impl eframe::App for UpdaterApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        if self.first_frame {
            self.first_frame = false;
            jobs::refresh(&self.shared, ctx);
        }
        if ctx.input(|i| i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(egui::Key::D)) {
            self.show_password = true;
        }
        self.password_window(ctx);

        let (remote, store, busy, status, progress, dev_unlocked) = {
            let m = self.shared.lock().unwrap();
            (m.remote.clone(), m.store.clone(), m.busy, m.status.clone(), m.progress.clone(), m.dev_password.is_some())
        };

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("App Updater");
                if ui.add_enabled(!busy, egui::Button::new("Odśwież")).clicked() {
                    jobs::refresh(&self.shared, ctx);
                }
                if ui.add_enabled(!busy, egui::Button::new("Aktualizuj wszystko")).clicked() {
                    jobs::update_all(&self.shared, ctx);
                }
            });
            ui.horizontal(|ui| {
                ui.label("Serwer:");
                let mut url = store.server_url.clone();
                let response = ui.add(egui::TextEdit::singleline(&mut url).desired_width(320.0));
                if response.changed() {
                    self.shared.lock().unwrap().store.server_url = url;
                }
                if response.lost_focus() {
                    let _ = self.shared.lock().unwrap().store.save();
                }
            });
            if dev_unlocked {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.tab, Tab::Apps, "Aplikacje");
                    ui.selectable_value(&mut self.tab, Tab::Dev, "Dev");
                });
            }
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            if let Some((name, fraction)) = &progress {
                ui.add(egui::ProgressBar::new(*fraction).text(format!("{name} {:.0}%", fraction * 100.0)));
            }
            ui.label(&status);
        });

        let mut action = None;
        egui::CentralPanel::default().show(ctx, |ui| match self.tab {
            Tab::Dev if dev_unlocked => self.dev_tab(ui, &remote, busy),
            _ => action = self.apps_tab(ui, &remote, &store, busy),
        });

        match action {
            Some(Action::Install(id)) => jobs::install_one(&self.shared, ctx, id),
            Some(Action::Launch(exe)) => {
                let _ = std::process::Command::new(exe).spawn();
            }
            None => {}
        }
    }
}
