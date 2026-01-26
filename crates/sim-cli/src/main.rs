use std::env;
use std::path::{Path, PathBuf};

use sim_core::analysis::AnalysisPlan;
use sim_core::circuit::AnalysisCmd;
use sim_core::engine::Engine;
use sim_core::netlist::{build_circuit, elaborate_netlist, parse_netlist_file};
use sim_core::result_store::{ResultStore, RunStatus};

fn main() {
    let mut args = env::args().skip(1).peekable();
    let mut netlist_path: Option<String> = None;
    let mut psf_path: Option<PathBuf> = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--psf" | "-o" => {
                let Some(path) = args.next() else {
                    eprintln!("missing value for {}", arg);
                    std::process::exit(2);
                };
                psf_path = Some(PathBuf::from(path));
            }
            _ => {
                if netlist_path.is_none() {
                    netlist_path = Some(arg);
                } else if psf_path.is_none() {
                    // 兼容：第二个非参数当作输出路径
                    psf_path = Some(PathBuf::from(arg));
                } else {
                    eprintln!("unexpected argument: {}", arg);
                    std::process::exit(2);
                }
            }
        }
    }

    let Some(netlist_path) = netlist_path else {
        eprintln!("usage: sim-cli <netlist> [--psf <path>]");
        std::process::exit(2);
    };

    let path = Path::new(&netlist_path);
    if !path.exists() {
        eprintln!("netlist not found: {}", netlist_path);
        std::process::exit(2);
    }

    let ast = parse_netlist_file(path);
    if !ast.errors.is_empty() {
        eprintln!("netlist parse errors:");
        for err in &ast.errors {
            eprintln!("  line {}: {}", err.line, err.message);
        }
        std::process::exit(2);
    }

    let elab = elaborate_netlist(&ast);
    if elab.error_count > 0 {
        eprintln!("netlist elaboration errors: {}", elab.error_count);
        std::process::exit(2);
    }

    let circuit = build_circuit(&ast, &elab);
    let plan_cmd = circuit
        .analysis
        .first()
        .cloned()
        .unwrap_or(AnalysisCmd::Op);
    let plan = AnalysisPlan { cmd: plan_cmd };
    let mut engine = Engine::new_default(circuit);
    let mut store = ResultStore::new();
    let run_id = engine.run_with_store(&plan, &mut store);
    let run = &store.runs[run_id.0];

    if !matches!(run.status, RunStatus::Converged) {
        eprintln!("run failed: status={:?} message={:?}", run.status, run.message);
        std::process::exit(1);
    }

    println!("run status: {:?} iterations={}", run.status, run.iterations);
    for (idx, name) in run.node_names.iter().enumerate() {
        let value = run.solution.get(idx).copied().unwrap_or(0.0);
        println!("V({}) = {}", name, value);
    }

    if let Some(path) = psf_path {
        if let Err(err) = store.write_psf_text(run_id, &path) {
            eprintln!("failed to write psf: {}", err);
            std::process::exit(1);
        }
        println!("psf written: {}", path.display());
    }
}
