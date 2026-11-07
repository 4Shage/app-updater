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

Wydania uruchamia wypchniecie taga. Nazwa uruchomienia w Actions = nazwa taga (np. `server-v0.1.4`).

| Co | Wersja w | Tag | Efekt |
| --- | --- | --- | --- |
| serwer | `server/Cargo.toml` | `server-v<wersja>` | obraz Docker w GHCR (bez wydania na GitHubie) |
| klient | `[workspace.package]` w glownym `Cargo.toml` | `client-v<wersja>` | wydanie `client-v<wersja>` |
| panel dev | jak klient | `dev-v<wersja>` | wydanie `dev-v<wersja>` |

Przyklad (serwer): zmien wersje w `server/Cargo.toml`, `cargo update`, commit, potem

    git tag server-v0.1.4
    git push origin master server-v0.1.4

Workflow sprawdza, czy tag zgadza sie z wersja w `Cargo.toml`, i konczy sie bledem, jesli nie.
