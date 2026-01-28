use sim_core::analysis::AnalysisPlan;
use sim_core::circuit::AnalysisCmd;
use sim_core::engine::Engine;
use sim_core::netlist::{build_circuit, elaborate_netlist, parse_netlist};
use sim_core::result_store::{AnalysisType, ResultStore, RunStatus};

fn parse_and_build(netlist: &str) -> sim_core::circuit::Circuit {
    let ast = parse_netlist(netlist);
    assert!(ast.errors.is_empty(), "parse errors: {:?}", ast.errors);
    let elab = elaborate_netlist(&ast);
    assert_eq!(elab.error_count, 0, "elaboration errors");
    build_circuit(&ast, &elab)
}

#[test]
fn tran_waveform_stores_multiple_time_points() {
    // Simple resistive circuit (no reactive elements for faster simulation)
    let netlist = r#"
V1 in 0 DC 1
R1 in out 1k
R2 out 0 1k
.tran 1u 10u
.end
"#;
    let circuit = parse_and_build(netlist);
    let mut engine = Engine::new_default(circuit);
    let mut store = ResultStore::new();

    let plan = AnalysisPlan {
        cmd: AnalysisCmd::Tran {
            tstep: 1e-6,
            tstop: 1e-5,
            tstart: 0.0,
            tmax: 1e-5,
        },
    };

    let run_id = engine.run_with_store(&plan, &mut store);
    let run = &store.runs[run_id.0];

    // Verify TRAN analysis type
    assert!(matches!(run.analysis, AnalysisType::Tran));

    // Verify we have multiple time points stored
    assert!(run.tran_times.len() > 1, "Expected multiple time points, got {}", run.tran_times.len());
    assert_eq!(run.tran_times.len(), run.tran_solutions.len(), "Times and solutions should have same length");

    // Verify time starts at 0
    assert!((run.tran_times[0] - 0.0).abs() < 1e-15, "First time point should be 0");

    // Verify time increases monotonically
    for i in 1..run.tran_times.len() {
        assert!(run.tran_times[i] > run.tran_times[i-1],
            "Time should increase: t[{}]={} not > t[{}]={}",
            i, run.tran_times[i], i-1, run.tran_times[i-1]);
    }
}

#[test]
fn tran_waveform_solution_has_correct_nodes() {
    let netlist = r#"
V1 in 0 DC 1
R1 in out 1k
R2 out 0 1k
.tran 1u 5u
.end
"#;
    let circuit = parse_and_build(netlist);
    let node_count = circuit.nodes.id_to_name.len();

    let mut engine = Engine::new_default(circuit);
    let mut store = ResultStore::new();

    let plan = AnalysisPlan {
        cmd: AnalysisCmd::Tran {
            tstep: 1e-6,
            tstop: 1e-5,
            tstart: 0.0,
            tmax: 1e-5,
        },
    };

    let run_id = engine.run_with_store(&plan, &mut store);
    let run = &store.runs[run_id.0];

    // Each solution vector should have the same size as node count
    for (i, sol) in run.tran_solutions.iter().enumerate() {
        assert_eq!(sol.len(), node_count,
            "Solution at time {} should have {} nodes, got {}",
            run.tran_times[i], node_count, sol.len());
    }
}

#[test]
fn tran_psf_output_format() {
    let netlist = r#"
V1 in 0 DC 1
R1 in out 1k
R2 out 0 1k
.tran 1u 5u
.end
"#;
    let circuit = parse_and_build(netlist);
    let mut engine = Engine::new_default(circuit);
    let mut store = ResultStore::new();

    let plan = AnalysisPlan {
        cmd: AnalysisCmd::Tran {
            tstep: 1e-6,
            tstop: 1e-5,
            tstart: 0.0,
            tmax: 1e-5,
        },
    };

    let run_id = engine.run_with_store(&plan, &mut store);
    let run = &store.runs[run_id.0];

    // Test PSF output
    let mut path = std::env::temp_dir();
    path.push("myspice_tran_test.psf");

    sim_core::psf::write_psf_tran(
        &run.tran_times,
        &run.node_names,
        &run.tran_solutions,
        &path,
        6,
    ).unwrap();

    let content = std::fs::read_to_string(&path).unwrap();

    // Verify PSF content
    assert!(content.contains("PSF_TEXT"), "Should have PSF header");
    assert!(content.contains("[Transient Analysis]"), "Should have TRAN section");
    assert!(content.contains("points ="), "Should have points count");
    assert!(content.contains("[Signals]"), "Should have signals section");
    assert!(content.contains("time"), "Should have time signal");
    assert!(content.contains("[Data]"), "Should have data section");

    // Clean up
    std::fs::remove_file(&path).ok();
}
