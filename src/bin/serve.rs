//! Localhost-only HTTP server + static UI for testing uor-xiv without IPFS.

use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use axum::body::Bytes;
use axum::extract::{Multipart, Path, State};
use axum::http::header;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use clap::Parser;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tower_http::cors::CorsLayer;
use uor_xiv::sandbox::WasmRunLimits;
use uor_xiv::store::LocalFsStore;
use uor_xiv::workspace::{fork_workspace, load_workspace, merge_workspaces, put_entry, save_workspace, MergeStrategy};
use uor_xiv::ContentStore;
use uor_xiv::{persist_wasm_run, WorkspaceRoot};

/// Embedded at compile time so `/` and `/static/*` work regardless of process cwd (Windows-friendly).
static INDEX_HTML: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/static/index.html"));
static STYLE_CSS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/static/style.css"));
static APP_JS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/static/app.js"));

#[derive(Parser, Debug)]
#[command(name = "uor-xiv-serve")]
struct Args {
    /// Directory for LocalFsStore (blobs + dags).
    #[arg(long, value_name = "DIR")]
    store: PathBuf,
    #[arg(long, default_value_t = 8787)]
    port: u16,
    /// Must be loopback: 127.0.0.1, ::1, or localhost.
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
}

#[derive(Clone)]
struct AppState {
    store: Arc<LocalFsStore>,
}

fn as_store(state: &AppState) -> &dyn ContentStore {
    &*state.store
}

struct ApiError(anyhow::Error);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("{:#}", self.0),
        )
            .into_response()
    }
}

impl<E: Into<anyhow::Error>> From<E> for ApiError {
    fn from(e: E) -> Self {
        ApiError(e.into())
    }
}

type ApiResult<T> = Result<Json<T>, ApiError>;

#[derive(Serialize)]
struct CidResponse {
    cid: String,
}

#[derive(Serialize)]
struct OkResponse {
    ok: bool,
}

#[derive(Deserialize)]
struct PutEntryBody {
    name: String,
    target_cid: String,
}

#[derive(Deserialize)]
struct MergeBody {
    base: String,
    other: String,
    strategy: String,
}

#[derive(Serialize)]
struct RunResponse {
    run_cid: String,
    exit_code: i32,
    stdout_cid: String,
    stderr_cid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    stdout_preview: Option<String>,
}

fn parse_loopback(host: &str, port: u16) -> Result<SocketAddr> {
    let ip: IpAddr = if host.eq_ignore_ascii_case("localhost") {
        "127.0.0.1".parse().context("parse 127.0.0.1")?
    } else {
        host.parse()
            .map_err(|_| anyhow!("invalid host {:?} (use 127.0.0.1, ::1, or localhost)", host))?
    };
    if !ip.is_loopback() {
        return Err(anyhow!(
            "refusing to bind to non-loopback {} (browser preview is local-only)",
            ip
        ));
    }
    Ok(SocketAddr::new(ip, port))
}

async fn post_blob(State(state): State<AppState>, body: Bytes) -> ApiResult<CidResponse> {
    let cid = as_store(&state).add_blob(&body)?;
    Ok(Json(CidResponse { cid }))
}

async fn get_blob(
    State(state): State<AppState>,
    Path(cid): Path<String>,
) -> Result<Response, ApiError> {
    let bytes = as_store(&state).cat_blob(&cid)?;
    Ok(([(axum::http::header::CONTENT_TYPE, "application/octet-stream")], bytes).into_response())
}

async fn new_workspace(State(state): State<AppState>) -> ApiResult<CidResponse> {
    let w = WorkspaceRoot::empty();
    let cid = save_workspace(as_store(&state), &w)?;
    Ok(Json(CidResponse { cid }))
}

async fn get_workspace(
    State(state): State<AppState>,
    Path(cid): Path<String>,
) -> ApiResult<WorkspaceRoot> {
    let w = load_workspace(as_store(&state), &cid)?;
    Ok(Json(w))
}

async fn put_entry_handler(
    State(state): State<AppState>,
    Path(cid): Path<String>,
    Json(body): Json<PutEntryBody>,
) -> ApiResult<CidResponse> {
    let w = load_workspace(as_store(&state), &cid)?;
    let w = put_entry(w, body.name, body.target_cid);
    let new_cid = save_workspace(as_store(&state), &w)?;
    Ok(Json(CidResponse { cid: new_cid }))
}

async fn fork_workspace_handler(
    State(state): State<AppState>,
    Path(cid): Path<String>,
) -> ApiResult<CidResponse> {
    let w = load_workspace(as_store(&state), &cid)?;
    let forked = fork_workspace(&w, &cid);
    let new_cid = save_workspace(as_store(&state), &forked)?;
    Ok(Json(CidResponse { cid: new_cid }))
}

fn parse_strategy(s: &str) -> Result<MergeStrategy> {
    match s.to_ascii_lowercase().as_str() {
        "strict" => Ok(MergeStrategy::Strict),
        "ours" => Ok(MergeStrategy::Ours),
        "theirs" => Ok(MergeStrategy::Theirs),
        _ => Err(anyhow!("strategy must be strict, ours, or theirs")),
    }
}

async fn merge_workspace(State(state): State<AppState>, Json(body): Json<MergeBody>) -> ApiResult<CidResponse> {
    let strategy = parse_strategy(&body.strategy)?;
    let w_base = load_workspace(as_store(&state), &body.base)?;
    let w_other = load_workspace(as_store(&state), &body.other)?;
    let merged = merge_workspaces(&w_base, &body.base, &w_other, &body.other, strategy)
        .map_err(|e| anyhow!("{}", e))?;
    let new_cid = save_workspace(as_store(&state), &merged)?;
    Ok(Json(CidResponse { cid: new_cid }))
}

async fn publish_workspace(
    State(state): State<AppState>,
    Path(cid): Path<String>,
) -> ApiResult<OkResponse> {
    as_store(&state).pin(&cid)?;
    Ok(Json(OkResponse { ok: true }))
}

async fn get_dag(
    State(state): State<AppState>,
    Path(cid): Path<String>,
) -> ApiResult<Value> {
    let v = as_store(&state).dag_get_json(&cid)?;
    Ok(Json(v))
}

async fn run_wasm(State(state): State<AppState>, mut multipart: Multipart) -> ApiResult<RunResponse> {
    let mut wasm: Option<Vec<u8>> = None;
    let mut fuel: u64 = 10_000_000;
    let mut guest_args: Vec<String> = Vec::new();
    let mut input_cids: Vec<String> = Vec::new();

    while let Some(field) = multipart.next_field().await.map_err(ApiError::from)? {
        let name = field.name().unwrap_or("").to_string();
        let bytes = field.bytes().await.map_err(ApiError::from)?;
        match name.as_str() {
            "wasm" => wasm = Some(bytes.to_vec()),
            "fuel" => {
                fuel = String::from_utf8_lossy(&bytes)
                    .parse()
                    .map_err(|_| anyhow!("invalid fuel"))?;
            }
            "args" => {
                let s = String::from_utf8_lossy(&bytes);
                let arr: Vec<String> = serde_json::from_str(s.trim()).map_err(|e| anyhow!("args JSON: {}", e))?;
                guest_args = arr;
            }
            "input_cids" => {
                let s = String::from_utf8_lossy(&bytes);
                let arr: Vec<String> =
                    serde_json::from_str(s.trim()).map_err(|e| anyhow!("input_cids JSON: {}", e))?;
                input_cids = arr;
            }
            _ => {}
        }
    }

    let wasm_bytes = wasm.ok_or_else(|| anyhow!("missing multipart field `wasm`"))?;
    let limits = WasmRunLimits {
        fuel,
        ..Default::default()
    };

    let store = as_store(&state);
    let wasm_cid = store.add_blob(&wasm_bytes)?;
    let (record, run_cid) = persist_wasm_run(
        store,
        &wasm_cid,
        &wasm_bytes,
        input_cids,
        &guest_args,
        &limits,
    )?;

    let stdout_blob = store.cat_blob(&record.stdout_cid)?;
    let preview = if stdout_blob.len() <= 4096 {
        Some(String::from_utf8_lossy(&stdout_blob).into_owned())
    } else {
        Some(String::from_utf8_lossy(&stdout_blob[..4096]).into_owned() + "…")
    };

    Ok(Json(RunResponse {
        run_cid,
        exit_code: record.exit_code,
        stdout_cid: record.stdout_cid,
        stderr_cid: record.stderr_cid,
        stdout_preview: preview,
    }))
}

async fn serve_index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn serve_style() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        STYLE_CSS,
    )
}

async fn serve_app_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        APP_JS,
    )
}

async fn health() -> &'static str {
    "ok"
}

fn api_router() -> Router<AppState> {
    Router::new()
        .route("/blobs", post(post_blob))
        .route("/blobs/:cid", get(get_blob))
        .route("/workspaces/merge", post(merge_workspace))
        .route("/workspaces", post(new_workspace))
        .route("/workspaces/:cid", get(get_workspace))
        .route("/workspaces/:cid/entries", post(put_entry_handler))
        .route("/workspaces/:cid/fork", post(fork_workspace_handler))
        .route("/workspaces/:cid/publish", post(publish_workspace))
        .route("/dags/:cid", get(get_dag))
        .route("/run", post(run_wasm))
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let addr = parse_loopback(&args.host, args.port)?;
    let store = Arc::new(LocalFsStore::open(&args.store)?);
    let state = AppState { store };

    // Static UI is `include_str!`d above; API nest carries `AppState` only.
    let app = Router::new()
        .route("/", get(serve_index))
        .route("/index.html", get(serve_index))
        .route("/static/style.css", get(serve_style))
        .route("/static/app.js", get(serve_app_js))
        .route("/health", get(health))
        .nest("/api", api_router().with_state(state.clone()))
        .layer(CorsLayer::permissive());

    let url = format!("http://{}", addr);
    println!("uor-xiv-serve (loopback only)");
    println!("  store: {}", args.store.display());
    println!("  open in browser: {}/", url);
    println!("  health check: {}/health", url);
    println!("  (no auth; do not expose to untrusted networks)");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
