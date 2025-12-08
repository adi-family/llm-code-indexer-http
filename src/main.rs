// Copyright (c) 2024-2025 Ihor
// SPDX-License-Identifier: BSL-1.1
// See LICENSE file for details

use anyhow::Result;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

struct AppState {
    adi: RwLock<Option<adi_core::Adi>>,
    project_path: PathBuf,
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    10
}

#[derive(Serialize)]
#[allow(dead_code)]
struct ErrorResponse {
    error: String,
}

#[derive(Serialize)]
#[allow(dead_code)]
struct SuccessResponse<T> {
    data: T,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse args
    let args: Vec<String> = std::env::args().collect();
    let project_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        std::env::current_dir()?
    };

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);

    // Setup logging
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(filter)
        .init();

    info!("Starting ADI HTTP server");
    info!("Project path: {}", project_path.display());

    // Initialize ADI
    let adi = match adi_core::Adi::open(&project_path).await {
        Ok(adi) => Some(adi),
        Err(e) => {
            tracing::warn!("Failed to initialize ADI: {}. Run /index first.", e);
            None
        }
    };

    let state = Arc::new(AppState {
        adi: RwLock::new(adi),
        project_path: project_path.canonicalize()?,
    });

    let app = Router::new()
        .route("/", get(health))
        .route("/health", get(health))
        .route("/status", get(status))
        .route("/index", post(index_project))
        .route("/search", get(search))
        .route("/symbols", get(search_symbols))
        .route("/symbols/:id", get(get_symbol))
        .route("/files", get(search_files))
        .route("/files/*path", get(get_file))
        .route("/tree", get(get_tree))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "service": "adi-http",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

async fn status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let adi = state.adi.read().await;

    match adi.as_ref() {
        Some(adi) => match adi.status() {
            Ok(status) => (StatusCode::OK, Json(serde_json::to_value(status).unwrap())),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            ),
        },
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": "ADI not initialized. POST /index first." })),
        ),
    }
}

async fn index_project(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Re-initialize ADI
    let adi = match adi_core::Adi::open(&state.project_path).await {
        Ok(adi) => adi,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            );
        }
    };

    // Index
    let progress = match adi.index().await {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            );
        }
    };

    // Store new ADI instance
    *state.adi.write().await = Some(adi);

    (StatusCode::OK, Json(serde_json::to_value(progress).unwrap()))
}

async fn search(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
) -> impl IntoResponse {
    let adi = state.adi.read().await;

    match adi.as_ref() {
        Some(adi) => match adi.search(&query.q, query.limit).await {
            Ok(results) => (StatusCode::OK, Json(serde_json::to_value(results).unwrap())),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            ),
        },
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": "ADI not initialized" })),
        ),
    }
}

async fn search_symbols(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
) -> impl IntoResponse {
    let adi = state.adi.read().await;

    match adi.as_ref() {
        Some(adi) => match adi.search_symbols(&query.q, query.limit).await {
            Ok(results) => (StatusCode::OK, Json(serde_json::to_value(results).unwrap())),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            ),
        },
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": "ADI not initialized" })),
        ),
    }
}

async fn get_symbol(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let adi = state.adi.read().await;

    match adi.as_ref() {
        Some(adi) => match adi.get_symbol(adi_core::SymbolId(id)) {
            Ok(symbol) => (StatusCode::OK, Json(serde_json::to_value(symbol).unwrap())),
            Err(e) => (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": e.to_string() })),
            ),
        },
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": "ADI not initialized" })),
        ),
    }
}

async fn search_files(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
) -> impl IntoResponse {
    let adi = state.adi.read().await;

    match adi.as_ref() {
        Some(adi) => match adi.search_files(&query.q, query.limit).await {
            Ok(results) => (StatusCode::OK, Json(serde_json::to_value(results).unwrap())),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            ),
        },
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": "ADI not initialized" })),
        ),
    }
}

async fn get_file(
    State(state): State<Arc<AppState>>,
    Path(path): Path<String>,
) -> impl IntoResponse {
    let adi = state.adi.read().await;

    match adi.as_ref() {
        Some(adi) => match adi.get_file(std::path::Path::new(&path)) {
            Ok(file_info) => (
                StatusCode::OK,
                Json(serde_json::to_value(file_info).unwrap()),
            ),
            Err(e) => (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": e.to_string() })),
            ),
        },
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": "ADI not initialized" })),
        ),
    }
}

async fn get_tree(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let adi = state.adi.read().await;

    match adi.as_ref() {
        Some(adi) => match adi.get_tree() {
            Ok(tree) => (StatusCode::OK, Json(serde_json::to_value(tree).unwrap())),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            ),
        },
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": "ADI not initialized" })),
        ),
    }
}
