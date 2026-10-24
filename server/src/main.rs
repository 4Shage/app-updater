use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Multipart, Path, State};
use axum::http::header::{CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use common::{AppEntry, AppMeta, Release};
use sha2::{Digest, Sha256};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tokio_util::io::ReaderStream;

type ApiError = (StatusCode, String);
type ApiResult<T> = Result<T, ApiError>;

struct AppState {
    index: Mutex<Vec<AppEntry>>,
    root: PathBuf,
    password: String,
}

type Shared = Arc<AppState>;

fn bad(e: impl std::fmt::Display) -> ApiError {
    (StatusCode::BAD_REQUEST, e.to_string())
}

fn internal(e: impl std::fmt::Display) -> ApiError {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

fn not_found() -> ApiError {
    (StatusCode::NOT_FOUND, "nie znaleziono".into())
}

fn check_safe(value: &str) -> ApiResult<()> {
    let ok = !value.is_empty()
        && value.len() <= 64
        && !value.starts_with('.')
        && value.chars().all(|c| c.is_ascii_alphanumeric() || "._-+".contains(c));
    if ok { Ok(()) } else { Err(bad(format!("niedozwolona wartosc: {value}"))) }
}

fn constant_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

async fn authorize(state: &AppState, headers: &HeaderMap) -> ApiResult<()> {
    let given = headers
        .get("x-dev-password")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if constant_eq(given.as_bytes(), state.password.as_bytes()) {
        return Ok(());
    }
    tokio::time::sleep(Duration::from_millis(700)).await;
    Err((StatusCode::UNAUTHORIZED, "zle haslo".into()))
}

fn release_dir(root: &FsPath, id: &str, version: &str, platform: &str) -> PathBuf {
    root.join("files").join(id).join(version).join(platform)
}

async fn save_index(state: &AppState, index: &[AppEntry]) -> ApiResult<()> {
    let json = serde_json::to_vec_pretty(index).map_err(internal)?;
    let tmp = state.root.join("index.json.tmp");
    fs::write(&tmp, json).await.map_err(internal)?;
    fs::rename(&tmp, state.root.join("index.json")).await.map_err(internal)
}

async fn list_apps(State(state): State<Shared>) -> Json<Vec<AppEntry>> {
    Json(state.index.lock().await.clone())
}

async fn download(
    State(state): State<Shared>,
    Path((id, version, platform)): Path<(String, String, String)>,
) -> ApiResult<Response> {
    let release = {
        let index = state.index.lock().await;
        index
            .iter()
            .find(|a| a.id == id)
            .and_then(|a| a.releases.iter().find(|r| r.version == version && r.platform == platform))
            .cloned()
            .ok_or_else(not_found)?
    };
    let path = release_dir(&state.root, &id, &version, &platform).join(&release.filename);
    let file = fs::File::open(path).await.map_err(internal)?;
    Response::builder()
        .header(CONTENT_TYPE, "application/octet-stream")
        .header(CONTENT_LENGTH, release.size)
        .body(Body::from_stream(ReaderStream::new(file)))
        .map_err(internal)
}

async fn dev_check(State(state): State<Shared>, headers: HeaderMap) -> ApiResult<StatusCode> {
    authorize(&state, &headers).await?;
    Ok(StatusCode::OK)
}

async fn upsert_app(
    State(state): State<Shared>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(meta): Json<AppMeta>,
) -> ApiResult<StatusCode> {
    authorize(&state, &headers).await?;
    check_safe(&id)?;
    let mut index = state.index.lock().await;
    match index.iter_mut().find(|a| a.id == id) {
        Some(app) => {
            app.name = meta.name;
            app.description = meta.description;
        }
        None => index.push(AppEntry {
            id,
            name: meta.name,
            description: meta.description,
            releases: vec![],
        }),
    }
    save_index(&state, &index).await?;
    Ok(StatusCode::OK)
}

async fn upload_release(
    State(state): State<Shared>,
    Path(id): Path<String>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> ApiResult<Json<Release>> {
    authorize(&state, &headers).await?;
    check_safe(&id)?;

    let (mut version, mut platform, mut notes) = (String::new(), String::new(), String::new());
    let mut saved: Option<(String, String, u64)> = None;

    while let Some(mut field) = multipart.next_field().await.map_err(bad)? {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "version" => version = field.text().await.map_err(bad)?,
            "platform" => platform = field.text().await.map_err(bad)?,
            "notes" => notes = field.text().await.map_err(bad)?,
            "file" => {
                check_safe(&version)?;
                check_safe(&platform)?;
                let filename = field
                    .file_name()
                    .and_then(|n| FsPath::new(n).file_name())
                    .and_then(|n| n.to_str())
                    .map(str::to_string)
                    .ok_or_else(|| bad("brak nazwy pliku"))?;
                check_safe(&filename)?;

                let dir = release_dir(&state.root, &id, &version, &platform);
                fs::create_dir_all(&dir).await.map_err(internal)?;
                let mut file = fs::File::create(dir.join(&filename)).await.map_err(internal)?;
                let mut hasher = Sha256::new();
                let mut size = 0u64;
                while let Some(chunk) = field.chunk().await.map_err(bad)? {
                    hasher.update(&chunk);
                    size += chunk.len() as u64;
                    file.write_all(&chunk).await.map_err(internal)?;
                }
                file.flush().await.map_err(internal)?;
                saved = Some((filename, hex::encode(hasher.finalize()), size));
            }
            _ => {}
        }
    }

    let (filename, sha256, size) = saved.ok_or_else(|| bad("brak pola file"))?;
    if !common::is_valid_version(&version) {
        return Err(bad("wersja musi byc w formacie semver, np. 1.2.3"));
    }
    let release = Release {
        version,
        platform,
        notes,
        filename,
        sha256,
        size,
        published_at: SystemTime::now().duration_since(UNIX_EPOCH).map_err(internal)?.as_secs(),
    };

    let mut index = state.index.lock().await;
    if !index.iter().any(|a| a.id == id) {
        index.push(AppEntry { id: id.clone(), name: id.clone(), ..Default::default() });
    }
    let app = index.iter_mut().find(|a| a.id == id).ok_or_else(not_found)?;
    app.releases
        .retain(|r| !(r.version == release.version && r.platform == release.platform));
    app.releases.push(release.clone());
    save_index(&state, &index).await?;
    Ok(Json(release))
}

async fn delete_release(
    State(state): State<Shared>,
    Path((id, version, platform)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> ApiResult<StatusCode> {
    authorize(&state, &headers).await?;
    check_safe(&id)?;
    check_safe(&version)?;
    check_safe(&platform)?;
    let mut index = state.index.lock().await;
    let app = index.iter_mut().find(|a| a.id == id).ok_or_else(not_found)?;
    app.releases.retain(|r| !(r.version == version && r.platform == platform));
    save_index(&state, &index).await?;
    let _ = fs::remove_dir_all(release_dir(&state.root, &id, &version, &platform)).await;
    Ok(StatusCode::OK)
}

#[tokio::main]
async fn main() {
    let bind = std::env::var("BIND").unwrap_or_else(|_| "0.0.0.0:8080".into());
    let root = PathBuf::from(std::env::var("DATA_DIR").unwrap_or_else(|_| "data".into()));
    let password = std::env::var("DEV_PASSWORD").expect("ustaw zmienna DEV_PASSWORD");

    fs::create_dir_all(&root).await.expect("nie mozna utworzyc DATA_DIR");
    let index: Vec<AppEntry> = match fs::read(root.join("index.json")).await {
        Ok(bytes) => serde_json::from_slice(&bytes).expect("uszkodzony index.json"),
        Err(_) => vec![],
    };

    let state = Arc::new(AppState { index: Mutex::new(index), root, password });

    let app = Router::new()
        .route("/api/apps", get(list_apps))
        .route("/api/apps/:id/:version/:platform/download", get(download))
        .route("/api/dev/check", get(dev_check))
        .route("/api/dev/apps/:id", put(upsert_app))
        .route("/api/dev/apps/:id/releases", post(upload_release))
        .route("/api/dev/apps/:id/:version/:platform", delete(delete_release))
        .layer(DefaultBodyLimit::disable())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&bind).await.expect("bind");
    println!("updater-server nasluchuje na {bind}");
    axum::serve(listener, app).await.expect("serve");
}
