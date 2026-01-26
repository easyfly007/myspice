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
use sim_core::circuit::{AnalysisCmd, Circuit};
use sim_core::engine::Engine;
use sim_core::netlist::{build_circuit, elaborate_netlist, parse_netlist, parse_netlist_file};
use sim_core::result_store::{ResultStore, RunId, RunResult};

use crate::schema::Summary;

pub struct HttpServerConfig {
    pub bind_addr: String,
}

#[derive(Clone)]
struct ApiState {
    store: Arc<Mutex<ResultStore>>,
    last_circuit: Arc<Mutex<Option<Circuit>>>,
}

#[derive(Debug, Deserialize)]
struct RunOpRequest {
    netlist: Option<String>,
    path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RunDcRequest {
    netlist: Option<String>,
    path: Option<String>,
    source: Option<String>,
    start: Option<f64>,
    stop: Option<f64>,
    step: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct RunTranRequest {
    netlist: Option<String>,
    path: Option<String>,
    tstep: Option<f64>,
    tstop: Option<f64>,
    tstart: Option<f64>,
    tmax: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct ExportRequest {
    path: String,
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
struct RunSummary {
    run_id: usize,
    analysis: String,
    status: String,
    iterations: usize,
}

#[derive(Debug, Serialize)]
struct RunsResponse {
    runs: Vec<RunSummary>,
}

#[derive(Debug, Serialize)]
struct NodesResponse {
    nodes: Vec<String>,
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

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    body: ErrorResponse,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (self.status, Json(self.body)).into_response()
    }
}

pub async fn run(config: HttpServerConfig) -> Result<(), String> {
    let state = ApiState {
        store: Arc::new(Mutex::new(ResultStore::new())),
        last_circuit: Arc::new(Mutex::new(None)),
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
        .route("/v1/run/dc", post(run_dc))
        .route("/v1/run/tran", post(run_tran))
        .route("/v1/runs", get(list_runs))
        .route("/v1/runs/:id", get(get_run))
        .route("/v1/runs/:id/export", post(export_run))
        .route("/v1/summary", get(get_summary))
        .route("/v1/nodes", get(get_nodes))
        .with_state(state)
}

async fn run_op(
    State(state): State<ApiState>,
    Json(payload): Json<RunOpRequest>,
) -> Result<Json<RunResponse>, ApiError> {
    let response = handle_run_op(&state, payload)?;
    Ok(Json(response))
}

async fn run_dc(
    State(state): State<ApiState>,
    Json(payload): Json<RunDcRequest>,
) -> Result<Json<RunResponse>, ApiError> {
    let response = handle_run_dc(&state, payload)?;
    Ok(Json(response))
}

async fn run_tran(
    State(state): State<ApiState>,
    Json(payload): Json<RunTranRequest>,
) -> Result<Json<RunResponse>, ApiError> {
    let response = handle_run_tran(&state, payload)?;
    Ok(Json(response))
}

async fn list_runs(State(state): State<ApiState>) -> Result<Json<RunsResponse>, ApiError> {
    let store = state
        .store
        .lock()
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "STORE_ERROR", "result store is unavailable", None))?;
    let runs = store
        .runs
        .iter()
        .enumerate()
        .map(|(idx, run)| RunSummary {
            run_id: idx,
            analysis: format!("{:?}", run.analysis),
            status: format!("{:?}", run.status),
            iterations: run.iterations,
        })
        .collect();
    Ok(Json(RunsResponse { runs }))
}

async fn get_run(
    State(state): State<ApiState>,
    Path(id): Path<usize>,
) -> Result<Json<RunResponse>, ApiError> {
    let store = state
        .store
        .lock()
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "STORE_ERROR", "result store is unavailable", None))?;
    let run = store
        .runs
        .get(id)
        .cloned()
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "RUN_NOT_FOUND", "run_id not found", None))?;
    Ok(Json(run_to_response(RunId(id), run)))
}

async fn export_run(
    State(state): State<ApiState>,
    Path(id): Path<usize>,
    Json(payload): Json<ExportRequest>,
) -> Result<Json<RunResponse>, ApiError> {
    let path = resolve_output_path(&payload.path)
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, "FILE_WRITE_ERROR", &err, None))?;
    let store = state
        .store
        .lock()
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "STORE_ERROR", "result store is unavailable", None))?;
    let run = store
        .runs
        .get(id)
        .cloned()
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "RUN_NOT_FOUND", "run_id not found", None))?;
    store
        .write_psf_text(RunId(id), &path)
        .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, "EXPORT_ERROR", &format!("export failed: {}", err), None))?;
    Ok(Json(run_to_response(RunId(id), run)))
}

async fn get_summary(State(state): State<ApiState>) -> Result<Json<Summary>, ApiError> {
    let circuit = load_last_circuit(&state)?;
    let summary = Summary {
        node_count: circuit.nodes.id_to_name.len(),
        device_count: circuit.instances.instances.len(),
        model_count: circuit.models.models.len(),
    };
    Ok(Json(summary))
}

async fn get_nodes(State(state): State<ApiState>) -> Result<Json<NodesResponse>, ApiError> {
    let circuit = load_last_circuit(&state)?;
    Ok(Json(NodesResponse {
        nodes: circuit.nodes.id_to_name,
    }))
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
) -> ApiError {
    ApiError {
        status,
        body: ErrorResponse {
            error: ErrorBody {
                code: code.to_string(),
                message: message.to_string(),
                details,
            },
        },
    }
}

enum NetlistInput {
    Text(String),
    Path(PathBuf),
}

fn select_input(
    netlist: Option<String>,
    path: Option<String>,
) -> Result<NetlistInput, ApiError> {
    if let Some(netlist) = netlist {
        return Ok(NetlistInput::Text(netlist));
    }
    if let Some(path) = path {
        let resolved = resolve_netlist_path(&path)
            .map_err(|err| api_error(StatusCode::BAD_REQUEST, "FILE_READ_ERROR", &err, None))?;
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

fn resolve_output_path(path: &str) -> Result<PathBuf, String> {
    resolve_netlist_path(path)
}

fn store_last_circuit(state: &ApiState, circuit: &Circuit) {
    if let Ok(mut slot) = state.last_circuit.lock() {
        *slot = Some(circuit.clone());
    }
}

fn load_last_circuit(state: &ApiState) -> Result<Circuit, ApiError> {
    let slot = state.last_circuit.lock().map_err(|_| {
        api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORE_ERROR",
            "session state is unavailable",
            None,
        )
    })?;
    match slot.clone() {
        Some(circuit) => Ok(circuit),
        None => Err(api_error(
            StatusCode::BAD_REQUEST,
            "NO_ACTIVE_CIRCUIT",
            "no circuit is available yet",
            None,
        )),
    }
}

fn select_dc_cmd(
    payload: &RunDcRequest,
    circuit: &Circuit,
) -> Result<AnalysisCmd, ApiError> {
    if payload.source.is_some()
        || payload.start.is_some()
        || payload.stop.is_some()
        || payload.step.is_some()
    {
        let source = payload
            .source
            .clone()
            .ok_or_else(|| api_error(StatusCode::BAD_REQUEST, "INVALID_REQUEST", "missing dc source", None))?;
        let start = payload
            .start
            .ok_or_else(|| api_error(StatusCode::BAD_REQUEST, "INVALID_REQUEST", "missing dc start", None))?;
        let stop = payload
            .stop
            .ok_or_else(|| api_error(StatusCode::BAD_REQUEST, "INVALID_REQUEST", "missing dc stop", None))?;
        let step = payload
            .step
            .ok_or_else(|| api_error(StatusCode::BAD_REQUEST, "INVALID_REQUEST", "missing dc step", None))?;
        return Ok(AnalysisCmd::Dc {
            source,
            start,
            stop,
            step,
        });
    }

    if let Some(cmd) = circuit.analysis.iter().find_map(|cmd| {
        if let AnalysisCmd::Dc { .. } = cmd {
            Some(cmd.clone())
        } else {
            None
        }
    }) {
        return Ok(cmd);
    }

    Err(api_error(
        StatusCode::BAD_REQUEST,
        "INVALID_REQUEST",
        "dc analysis parameters not provided and not found in netlist",
        None,
    ))
}

fn select_tran_cmd(
    payload: &RunTranRequest,
    circuit: &Circuit,
) -> Result<AnalysisCmd, ApiError> {
    if payload.tstep.is_some() || payload.tstop.is_some() {
        let tstep = payload
            .tstep
            .ok_or_else(|| api_error(StatusCode::BAD_REQUEST, "INVALID_REQUEST", "missing tran tstep", None))?;
        let tstop = payload
            .tstop
            .ok_or_else(|| api_error(StatusCode::BAD_REQUEST, "INVALID_REQUEST", "missing tran tstop", None))?;
        let tstart = payload.tstart.unwrap_or(0.0);
        let tmax = payload.tmax.unwrap_or(tstop);
        return Ok(AnalysisCmd::Tran {
            tstep,
            tstop,
            tstart,
            tmax,
        });
    }

    if let Some(cmd) = circuit.analysis.iter().find_map(|cmd| {
        if let AnalysisCmd::Tran { .. } = cmd {
            Some(cmd.clone())
        } else {
            None
        }
    }) {
        return Ok(cmd);
    }

    Err(api_error(
        StatusCode::BAD_REQUEST,
        "INVALID_REQUEST",
        "tran analysis parameters not provided and not found in netlist",
        None,
    ))
}

fn load_netlist(input: NetlistInput) -> Result<sim_core::netlist::NetlistAst, ApiError> {
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
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "PARSE_ERROR",
            "netlist parse failed",
            Some(details),
        ));
    }
    Ok(ast)
}

fn handle_run_op(state: &ApiState, payload: RunOpRequest) -> Result<RunResponse, ApiError> {
    let input = select_input(payload.netlist.clone(), payload.path.clone())?;
    let ast = load_netlist(input)?;
    let elab = elaborate_netlist(&ast);
    if elab.error_count > 0 {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "ELAB_ERROR",
            &format!("netlist elaboration failed: {}", elab.error_count),
            None,
        ));
    }

    let circuit = build_circuit(&ast, &elab);
    store_last_circuit(state, &circuit);
    run_analysis(state, circuit, AnalysisCmd::Op)
}

fn handle_run_dc(state: &ApiState, payload: RunDcRequest) -> Result<RunResponse, ApiError> {
    let input = select_input(payload.netlist.clone(), payload.path.clone())?;
    let ast = load_netlist(input)?;
    let elab = elaborate_netlist(&ast);
    if elab.error_count > 0 {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "ELAB_ERROR",
            &format!("netlist elaboration failed: {}", elab.error_count),
            None,
        ));
    }

    let circuit = build_circuit(&ast, &elab);
    store_last_circuit(state, &circuit);
    let cmd = select_dc_cmd(&payload, &circuit)?;
    run_analysis(state, circuit, cmd)
}

fn handle_run_tran(state: &ApiState, payload: RunTranRequest) -> Result<RunResponse, ApiError> {
    let input = select_input(payload.netlist.clone(), payload.path.clone())?;
    let ast = load_netlist(input)?;
    let elab = elaborate_netlist(&ast);
    if elab.error_count > 0 {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "ELAB_ERROR",
            &format!("netlist elaboration failed: {}", elab.error_count),
            None,
        ));
    }

    let circuit = build_circuit(&ast, &elab);
    store_last_circuit(state, &circuit);
    let cmd = select_tran_cmd(&payload, &circuit)?;
    run_analysis(state, circuit, cmd)
}

fn run_analysis(
    state: &ApiState,
    circuit: Circuit,
    cmd: AnalysisCmd,
) -> Result<RunResponse, ApiError> {
    let plan = AnalysisPlan { cmd };
    let mut engine = Engine::new_default(circuit);
    let mut store = state.store.lock().map_err(|_| {
        api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORE_ERROR",
            "result store is unavailable",
            None,
        )
    })?;
    let run_id = engine.run_with_store(&plan, &mut store);
    let run = store
        .runs
        .get(run_id.0)
        .cloned()
        .ok_or_else(|| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "RUN_NOT_FOUND",
                "run result not found",
                None,
            )
        })?;
    Ok(run_to_response(run_id, run))
}
