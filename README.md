# App Updater

Workspace Rust: `common` (typy), `server` (axum), `updater-lib` (biblioteka do zlinkowania), `updater-gui` (binarka `updater`).

## Serwer

    DEV_PASSWORD=tajne DATA_DIR=./data BIND=0.0.0.0:8080 cargo run --release -p updater-server

Docker: `docker build -f server/Dockerfile -t updater-server .`

Wystaw za reverse proxy z HTTPS - haslo dev leci w naglowku `x-dev-password`.

## Klient standalone

    cargo run --release -p updater-gui

- `Ctrl+Shift+D` - ukryty tryb dev (haslo), pojawia sie zakladka Dev: upload wydan, usuwanie.
- `updater --update-all` - aktualizacja bez GUI.
- `updater --register <id> <wersja> <sciezka_exe>` - dodaje juz zainstalowana aplikacje do zarzadzania.

Wydanie to pojedynczy plik exe albo `.zip` (rozpakowywany do katalogu aplikacji; glowny plik musi nazywac sie `<id>` lub `<id>.exe`).
Wersje w formacie semver (`1.2.3`). Platforma: `windows-x86_64`, `linux-x86_64`, `linux-aarch64`...

## Zlinkowanie w swojej aplikacji

    # Cargo.toml
    updater-lib = { path = "../app-updater/updater-lib" }

    // self-update (wydanie = pojedynczy exe)
    if let Ok(Some(v)) = updater_lib::selfupdate::self_update("https://updates.example.com", "mojapp", env!("CARGO_PKG_VERSION")) {
        println!("zaktualizowano do {v}, uruchom ponownie");
    }

Albo odpal `updater` z poziomu aplikacji (`Command::new("updater").arg("--update-all")`).
