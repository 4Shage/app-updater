# App Updater

Workspace Rust:

- `common` - wspolne typy
- `server` - serwer (axum), obraz Docker
- `updater-lib` - biblioteka do zlinkowania w swoich aplikacjach
- `gui-common` - wspolny styl egui
- `updater-gui` - klient `updater` (instalacja, aktualizacje, odinstalowanie)
- `dev-gui` - panel developera `updater-dev` (wiele serwerow, zapamietane hasla, upload wydan)

## Serwer

    DEV_PASSWORD=tajne DATA_DIR=./data BIND=0.0.0.0:8080 cargo run --release -p updater-server

Obraz: `ghcr.io/<owner>/<repo>-server` (publikowany przez `server-release.yml`).
Wystaw za reverse proxy z HTTPS - haslo dev leci w naglowku `x-dev-password`.

## Klient

    cargo run --release -p updater-gui

- `updater --update-all` - aktualizacja bez GUI
- `updater --register <id> <wersja> <sciezka_exe>` - dodaje juz zainstalowana aplikacje

Updater aktualizuje sie sam, jesli na serwerze jest aplikacja o ID `updater` w nowszej wersji.

## Panel developera

    cargo run --release -p updater-dev-gui

Serwery i hasla zapisuja sie w `%APPDATA%\AppUpdaterDev\servers.json` (jawnym tekstem).
Wydanie to pojedynczy plik exe albo `.zip`; wersje w formacie semver; platforma np. `windows-x86_64`.

## Wersja i wydania

Klient i panel dev maja wspolna wersje: `[workspace.package]` w glownym `Cargo.toml`. Po jej zmianie:

- `client-release.yml` - wydanie `v<wersja>` z klientem
- `dev-release.yml` - wydanie `dev-v<wersja>` z panelem developera

Serwer ma osobna wersje w `server/Cargo.toml`. Jej zmiana uruchamia tylko `server-release.yml`
(obraz Docker do GHCR, bez wydania na GitHubie). Po zmianie wersji uruchom `cargo update`
i zacommituj `Cargo.lock` (workflowy uzywaja `--locked`).
