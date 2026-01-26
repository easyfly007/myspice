use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::path::{Path as FsPath, PathBuf};
use std::sync::{Arc, Mutex};

use sim_core::analysis::AnalysisPlan;
use sim_core::circuit::AnalysisCmd;
use sim_core::engine::Engine;
use sim_core::netlist::{build_circuit, elaborate_netlist, parse_netlist, parse_netlist_file};
use sim_core::result_store::{ResultStore, RunId, RunResult};

pub struct HttpServerConfig {
    pub bind_addr: String,
}

#[derive(Clone)]
struct ApiState {
    store: Arc<Mutex<ResultStore>>,
}

#[derive(Debug, Deserialize)]
struct RunOpRequest {
    netlist: Option<String>,
    path: Option<String>,
}

#[derive(Debug, Serialize)]
struct RunResponse {
    run_id: usize,
    analysis: String,
    status: String,
    iterations: usize,
    nodes: Vec<String>,
    solution: Vec<f64>,
    message: Option<String>,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    code: String,
    message: String,
    details: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: ErrorBody,
}

pub async fn run(config: HttpServerConfig) -> Result<(), String> {
    let state = ApiState {
        store: Arc::new(Mutex::new(ResultStore::new())),
    };
    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind(&config.bind_addr)
        .await
        .map_err(|err| format!("bind {} failed: {}", config.bind_addr, err))?;
    axum::serve(listener, app)
        .await
        .map_err(|err| format!("server error: {}", err))
}

fn build_router(state: ApiState) -> Router {
    Router::new()
        .route("/v1/run/op", post(run_op))
        .route("/v1/runs/:id", get(get_run))
        .with_state(state)
}

async fn run_op(
    State(state): State<ApiState>,
    Json(payload): Json<RunOpRequest>,
) -> impl IntoResponse {
    let input = match select_input(payload.netlist, payload.path) {
        Ok(value) => value,
        Err(err) => return err,
    };

    let ast = match input {
        NetlistInput::Text(netlist) => parse_netlist(&netlist),
        NetlistInput::Path(path) => parse_netlist_file(&path),
    };
    if !ast.errors.is_empty() {
        let details = ast
            .errors
            .iter()
            .map(|err| format!("line {}: {}", err.line, err.message))
            .collect();
        return api_error(
            StatusCode::BAD_REQUEST,
            "PARSE_ERROR",
            "netlist parse failed",
            Some(details),
        );
    }

    let elab = elaborate_netlist(&ast);
    if elab.error_count > 0 {
        return api_error(
            StatusCode::BAD_REQUEST,
            "ELAB_ERROR",
            &format!("netlist elaboration failed: {}", elab.error_count),
            None,
        );
    }

    let circuit = build_circuit(&ast, &elab);
    let plan = AnalysisPlan {
        cmd: AnalysisCmd::Op,
    };
    let mut engine = Engine::new_default(circuit);
    let mut store = match state.store.lock() {
        Ok(guard) => guard,
        Err(_) => {
            return api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "STORE_ERROR",
                "result store is unavailable",
                None,
            );
        }
    };
    let run_id = engine.run_with_store(&plan, &mut store);
    let run = match store.runs.get(run_id.0).cloned() {
        Some(run) => run,
        None => {
            return api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "RUN_NOT_FOUND",
                "run result not found",
                None,
            );
        }
    };

    Json(run_to_response(run_id, run)).into_response()
}

async fn get_run(State(state): State<ApiState>, Path(id): Path<usize>) -> impl IntoResponse {
    let store = match state.store.lock() {
        Ok(guard) => guard,
        Err(_) => {
            return api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "STORE_ERROR",
                "result store is unavailable",
                None,
            );
        }
    };
    let run = match store.runs.get(id).cloned() {
        Some(run) => run,
        None => {
            return api_error(
                StatusCode::NOT_FOUND,
                "RUN_NOT_FOUND",
                "run_id not found",
                None,
            );
        }
    };
    Json(run_to_response(RunId(id), run)).into_response()
}

fn run_to_response(run_id: RunId, run: RunResult) -> RunResponse {
    RunResponse {
        run_id: run_id.0,
        analysis: format!("{:?}", run.analysis),
        status: format!("{:?}", run.status),
        iterations: run.iterations,
        nodes: run.node_names,
        solution: run.solution,
        message: run.message,
    }
}

fn api_error(
    status: StatusCode,
    code: &str,
    message: &str,
    details: Option<Vec<String>>,
) -> axum::response::Response {
    let body = ErrorResponse {
        error: ErrorBody {
            code: code.to_string(),
            message: message.to_string(),
            details,
        },
    };
    (status, Json(body)).into_response()
}

enum NetlistInput {
    Text(String),
    Path(PathBuf),
}

fn select_input(
    netlist: Option<String>,
    path: Option<String>,
) -> Result<NetlistInput, axum::response::Response> {
    if let Some(netlist) = netlist {
        return Ok(NetlistInput::Text(netlist));
    }
    if let Some(path) = path {
        let resolved = resolve_netlist_path(&path).map_err(|err| {
            api_error(
                StatusCode::BAD_REQUEST,
                "FILE_READ_ERROR",
                &err,
                None,
            )
        })?;
        return Ok(NetlistInput::Path(resolved));
    }
    Err(api_error(
        StatusCode::BAD_REQUEST,
        "INVALID_REQUEST",
        "missing netlist or path",
        None,
    ))
}

fn resolve_netlist_path(path: &str) -> Result<PathBuf, String> {
    let base = std::env::current_dir().map_err(|err| err.to_string())?;
    let candidate = FsPath::new(path);
    let full = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        base.join(candidate)
    };
    let canonical = full.canonicalize().map_err(|err| err.to_string())?;
    if !canonical.starts_with(&base) {
        return Err("path is outside the current workspace".to_string());
    }
    Ok(canonical)
}
