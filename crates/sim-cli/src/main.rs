use std::env;
use std::path::{Path, PathBuf};

use sim_core::analysis::AnalysisPlan;
use sim_core::circuit::AnalysisCmd;
use sim_core::engine::Engine;
use sim_core::netlist::{build_circuit, elaborate_netlist, parse_netlist_file};
use sim_core::result_store::{AnalysisType, ResultStore, RunStatus};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn print_help() {
    println!(
        r#"MySpice SPICE Circuit Simulator

USAGE:
    sim-cli <NETLIST> [OPTIONS]

ARGS:
    <NETLIST>               Path to SPICE netlist file

OPTIONS:
    -h, --help              Print help information
    -V, --version           Print version information
    -o, --psf <PATH>        Write results to PSF text file
    -a, --analysis <TYPE>   Analysis type: op, dc, tran (default: from netlist or op)
    --dc-source <NAME>      DC sweep source name
    --dc-start <VALUE>      DC sweep start voltage
    --dc-stop <VALUE>       DC sweep stop voltage
    --dc-step <VALUE>       DC sweep step size
    --precision <N>         Output precision (1-15 significant digits, default: 6)

EXAMPLES:
    sim-cli circuit.cir                          # Run analysis from netlist
    sim-cli circuit.cir --psf out.psf            # Export to PSF file
    sim-cli circuit.cir -a dc --dc-source V1 \
        --dc-start 0 --dc-stop 5 --dc-step 0.1   # DC sweep
    sim-cli circuit.cir -a tran                  # Transient analysis"#
    );
}

fn print_version() {
    println!("myspice {}", VERSION);
}

fn main() {
    let mut args = env::args().skip(1).peekable();
    let mut netlist_path: Option<String> = None;
    let mut psf_path: Option<PathBuf> = None;
    let mut analysis: Option<String> = None;
    let mut dc_source: Option<String> = None;
    let mut dc_start: Option<f64> = None;
    let mut dc_stop: Option<f64> = None;
    let mut dc_step: Option<f64> = None;
    let mut precision: usize = 6;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            "--version" | "-V" => {
                print_version();
                std::process::exit(0);
            }
            "--psf" | "-o" => {
                let Some(path) = args.next() else {
                    eprintln!("missing value for {}", arg);
                    std::process::exit(2);
                };
                psf_path = Some(PathBuf::from(path));
            }
            "--analysis" | "-a" => {
                let Some(value) = args.next() else {
                    eprintln!("missing value for {}", arg);
                    std::process::exit(2);
                };
                analysis = Some(value.to_ascii_lowercase());
            }
            "--dc-source" => {
                let Some(value) = args.next() else {
                    eprintln!("missing value for {}", arg);
                    std::process::exit(2);
                };
                dc_source = Some(value);
            }
            "--dc-start" => {
                let Some(value) = args.next() else {
                    eprintln!("missing value for {}", arg);
                    std::process::exit(2);
                };
                dc_start = value.parse::<f64>().ok();
            }
            "--dc-stop" => {
                let Some(value) = args.next() else {
                    eprintln!("missing value for {}", arg);
                    std::process::exit(2);
                };
                dc_stop = value.parse::<f64>().ok();
            }
            "--dc-step" => {
                let Some(value) = args.next() else {
                    eprintln!("missing value for {}", arg);
                    std::process::exit(2);
                };
                dc_step = value.parse::<f64>().ok();
            }
            "--precision" => {
                let Some(value) = args.next() else {
                    eprintln!("missing value for {}", arg);
                    std::process::exit(2);
                };
                precision = match value.parse::<usize>() {
                    Ok(p) if (1..=15).contains(&p) => p,
                    _ => {
                        eprintln!("precision must be between 1 and 15");
                        std::process::exit(2);
                    }
                };
            }
            _ => {
                if netlist_path.is_none() {
                    netlist_path = Some(arg);
                } else if psf_path.is_none() {
                    // backward compatibility: second positional as output path
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
    let (cmd, sweep) = select_analysis(&analysis, &circuit, dc_source, dc_start, dc_stop, dc_step);

    let mut engine = Engine::new_default(circuit);
    let mut store = ResultStore::new();

    if let Some(sweep) = sweep {
        run_dc_sweep(&mut engine, &mut store, cmd, sweep.clone(), psf_path.as_deref(), precision);
    } else {
        let plan = AnalysisPlan { cmd };
        let run_id = engine.run_with_store(&plan, &mut store);
        let run = &store.runs[run_id.0];

        if !matches!(run.status, RunStatus::Converged) {
            eprintln!("run failed: status={:?} message={:?}", run.status, run.message);
            std::process::exit(1);
        }

        // Print results based on analysis type
        match run.analysis {
            AnalysisType::Tran => {
                println!("tran status: {:?} steps={}", run.status, run.iterations);
                println!("Final values:");
                for (idx, name) in run.node_names.iter().enumerate() {
                    let value = run.solution.get(idx).copied().unwrap_or(0.0);
                    println!("  V({}) = {:.*e}", name, precision, value);
                }
            }
            _ => {
                println!("run status: {:?} iterations={}", run.status, run.iterations);
                for (idx, name) in run.node_names.iter().enumerate() {
                    let value = run.solution.get(idx).copied().unwrap_or(0.0);
                    println!("V({}) = {:.*e}", name, precision, value);
                }
            }
        }

        if let Some(path) = psf_path {
            if let Err(err) = store.write_psf_text(run_id, &path, precision) {
                eprintln!("failed to write psf: {}", err);
                std::process::exit(1);
            }
            println!("psf written: {}", path.display());
        }
    }
}

#[derive(Clone)]
struct DcSweep {
    source: String,
    start: f64,
    stop: f64,
    step: f64,
}

fn select_analysis(
    analysis: &Option<String>,
    circuit: &sim_core::circuit::Circuit,
    dc_source: Option<String>,
    dc_start: Option<f64>,
    dc_stop: Option<f64>,
    dc_step: Option<f64>,
) -> (AnalysisCmd, Option<DcSweep>) {
    let from_netlist = circuit.analysis.first().cloned();
    let analysis = analysis.as_deref();

    match analysis {
        Some("op") => (AnalysisCmd::Op, None),
        Some("dc") => {
            let sweep = build_dc_sweep(dc_source, dc_start, dc_stop, dc_step)
                .or_else(|| extract_dc_sweep(from_netlist));
            let Some(sweep) = sweep else {
                eprintln!("dc analysis requires source/start/stop/step or .dc in netlist");
                std::process::exit(2);
            };
            (
                AnalysisCmd::Dc {
                    source: sweep.source.clone(),
                    start: sweep.start,
                    stop: sweep.stop,
                    step: sweep.step,
                },
                Some(sweep),
            )
        }
        Some("tran") => {
            let cmd = match from_netlist {
                Some(AnalysisCmd::Tran {
                    tstep,
                    tstop,
                    tstart,
                    tmax,
                }) => AnalysisCmd::Tran {
                    tstep,
                    tstop,
                    tstart,
                    tmax,
                },
                _ => AnalysisCmd::Tran {
                    tstep: 1e-6,
                    tstop: 1e-5,
                    tstart: 0.0,
                    tmax: 1e-5,
                },
            };
            (cmd, None)
        }
        _ => match from_netlist {
            Some(AnalysisCmd::Dc {
                source,
                start,
                stop,
                step,
            }) => {
                let sweep = DcSweep {
                    source: source.clone(),
                    start,
                    stop,
                    step,
                };
                (
                    AnalysisCmd::Dc {
                        source,
                        start,
                        stop,
                        step,
                    },
                    Some(sweep),
                )
            }
            Some(cmd) => (cmd, None),
            None => (AnalysisCmd::Op, None),
        },
    }
}

fn build_dc_sweep(
    source: Option<String>,
    start: Option<f64>,
    stop: Option<f64>,
    step: Option<f64>,
) -> Option<DcSweep> {
    match (source, start, stop, step) {
        (Some(source), Some(start), Some(stop), Some(step)) => Some(DcSweep {
            source,
            start,
            stop,
            step,
        }),
        _ => None,
    }
}

fn extract_dc_sweep(cmd: Option<AnalysisCmd>) -> Option<DcSweep> {
    match cmd {
        Some(AnalysisCmd::Dc {
            source,
            start,
            stop,
            step,
        }) => Some(DcSweep {
            source,
            start,
            stop,
            step,
        }),
        _ => None,
    }
}

fn run_dc_sweep(
    engine: &mut Engine,
    store: &mut ResultStore,
    cmd: AnalysisCmd,
    sweep: DcSweep,
    psf_path: Option<&Path>,
    precision: usize,
) {
    if sweep.step <= 0.0 {
        eprintln!("dc step must be > 0");
        std::process::exit(2);
    }
    println!(
        "dc sweep: {} from {} to {} step {}",
        sweep.source, sweep.start, sweep.stop, sweep.step
    );

    let mut sweep_values: Vec<f64> = Vec::new();
    let mut sweep_results: Vec<Vec<f64>> = Vec::new();
    let mut node_names: Vec<String> = Vec::new();

    let mut value = sweep.start;
    let mut guard = 0usize;
    while value <= sweep.stop + sweep.step * 0.5 {
        apply_dc_source(engine, &sweep.source, value);
        let plan = AnalysisPlan { cmd: cmd.clone() };
        let run_id = engine.run_with_store(&plan, store);
        let run = &store.runs[run_id.0];
        if !matches!(run.status, RunStatus::Converged) {
            eprintln!(
                "dc sweep failed at {}={}: status={:?} message={:?}",
                sweep.source, value, run.status, run.message
            );
            std::process::exit(1);
        }

        // Capture node names from first run
        if node_names.is_empty() {
            node_names = run.node_names.clone();
        }

        // Collect sweep data
        sweep_values.push(value);
        sweep_results.push(run.solution.clone());

        // Print to stdout
        print!("{}={:.*e}", sweep.source, precision, value);
        for (idx, name) in run.node_names.iter().enumerate() {
            let v = run.solution.get(idx).copied().unwrap_or(0.0);
            print!(" V({})={:.*e}", name, precision, v);
        }
        println!();

        value += sweep.step;
        guard += 1;
        if guard > 1_000_000 {
            eprintln!("dc sweep aborted: too many steps");
            std::process::exit(2);
        }
    }

    // Write PSF output if requested
    if let Some(path) = psf_path {
        if let Err(err) = sim_core::psf::write_psf_sweep(
            &sweep.source,
            &sweep_values,
            &node_names,
            &sweep_results,
            path,
            precision,
        ) {
            eprintln!("failed to write psf: {}", err);
            std::process::exit(1);
        }
        println!("psf written: {}", path.display());
    }
}

fn apply_dc_source(engine: &mut Engine, source: &str, value: f64) {
    let mut found = false;
    for inst in &mut engine.circuit.instances.instances {
        if inst.name.eq_ignore_ascii_case(source) {
            inst.value = Some(value.to_string());
            found = true;
            break;
        }
    }
    if !found {
        eprintln!("dc source not found: {}", source);
        std::process::exit(2);
    }
}
