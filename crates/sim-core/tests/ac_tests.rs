//! AC Analysis Integration Tests
//!
//! Tests for AC small-signal frequency-domain analysis.

use sim_core::circuit::{AcSweepType, AnalysisCmd, Circuit, DeviceKind, Instance};
use sim_core::analysis::AnalysisPlan;
use sim_core::engine::Engine;
use sim_core::result_store::{AnalysisType, ResultStore, RunStatus};
use std::collections::HashMap;

/// Create a simple RC lowpass filter circuit.
/// V1 -- R1 -- out -- C1 -- gnd
/// Cutoff frequency fc = 1/(2*pi*R*C) = 1/(2*pi*1k*1u) = 159.15 Hz
fn make_rc_lowpass() -> Circuit {
    let mut circuit = Circuit::new();

    // Create nodes
    let gnd = circuit.nodes.ensure_node("0");
    let vin = circuit.nodes.ensure_node("in");
    let vout = circuit.nodes.ensure_node("out");

    // V1: AC voltage source with 1V magnitude
    circuit.instances.insert(Instance {
        name: "V1".to_string(),
        kind: DeviceKind::V,
        nodes: vec![vin, gnd],
        model: None,
        params: HashMap::new(),
        value: Some("0".to_string()),
        control: None,
        ac_mag: Some(1.0),
        ac_phase: Some(0.0),
    });

    // R1: 1k ohm
    circuit.instances.insert(Instance {
        name: "R1".to_string(),
        kind: DeviceKind::R,
        nodes: vec![vin, vout],
        model: None,
        params: HashMap::new(),
        value: Some("1k".to_string()),
        control: None,
        ac_mag: None,
        ac_phase: None,
    });

    // C1: 1uF
    circuit.instances.insert(Instance {
        name: "C1".to_string(),
        kind: DeviceKind::C,
        nodes: vec![vout, gnd],
        model: None,
        params: HashMap::new(),
        value: Some("1u".to_string()),
        control: None,
        ac_mag: None,
        ac_phase: None,
    });

    circuit
}

#[test]
fn ac_analysis_runs_on_rc_lowpass() {
    let circuit = make_rc_lowpass();
    let mut engine = Engine::new_default(circuit);
    let mut store = ResultStore::new();

    let plan = AnalysisPlan {
        cmd: AnalysisCmd::Ac {
            sweep_type: AcSweepType::Dec,
            points: 10,
            fstart: 1.0,
            fstop: 10000.0,
        },
    };

    let run_id = engine.run_with_store(&plan, &mut store);
    let run = &store.runs[run_id.0];

    assert!(matches!(run.status, RunStatus::Converged));
    assert!(matches!(run.analysis, AnalysisType::Ac));
    assert!(!run.ac_frequencies.is_empty());
    assert_eq!(run.ac_frequencies.len(), run.ac_solutions.len());
}

#[test]
fn ac_analysis_frequency_response_shape() {
    let circuit = make_rc_lowpass();
    let mut engine = Engine::new_default(circuit);
    let mut store = ResultStore::new();

    // Use linear sweep for easier verification
    let plan = AnalysisPlan {
        cmd: AnalysisCmd::Ac {
            sweep_type: AcSweepType::Lin,
            points: 100,
            fstart: 1.0,
            fstop: 1000.0,
        },
    };

    let run_id = engine.run_with_store(&plan, &mut store);
    let run = &store.runs[run_id.0];

    assert!(matches!(run.status, RunStatus::Converged));

    // Find the output node index
    let out_idx = run.node_names.iter().position(|n| n == "out").unwrap();

    // At low frequencies (1 Hz), output should be close to input (0 dB)
    let (low_freq_mag_db, _) = run.ac_solutions[0][out_idx];
    assert!(low_freq_mag_db > -1.0, "Low frequency gain should be ~0 dB, got {}", low_freq_mag_db);

    // At high frequencies (1 kHz), output should be attenuated
    let (high_freq_mag_db, _) = run.ac_solutions.last().unwrap()[out_idx];
    assert!(high_freq_mag_db < low_freq_mag_db, "High frequency should be attenuated");
}

#[test]
fn ac_analysis_lowpass_rolloff() {
    let circuit = make_rc_lowpass();
    let mut engine = Engine::new_default(circuit);
    let mut store = ResultStore::new();

    let plan = AnalysisPlan {
        cmd: AnalysisCmd::Ac {
            sweep_type: AcSweepType::Dec,
            points: 20,
            fstart: 10.0,
            fstop: 10000.0,
        },
    };

    let run_id = engine.run_with_store(&plan, &mut store);
    let run = &store.runs[run_id.0];

    assert!(matches!(run.status, RunStatus::Converged));

    let out_idx = run.node_names.iter().position(|n| n == "out").unwrap();

    // Get output magnitude at various frequencies
    let (out_mag_low, _) = run.ac_solutions[0][out_idx];
    let (out_mag_high, _) = run.ac_solutions.last().unwrap()[out_idx];

    // Verify lowpass filter rolloff behavior:
    // Output magnitude should decrease significantly from low to high frequency
    let rolloff = out_mag_low - out_mag_high;
    assert!(
        rolloff > 30.0, // Expect at least 30 dB rolloff over 3 decades
        "Expected >30 dB rolloff from 10 Hz to 10 kHz, got {:.2} dB",
        rolloff
    );

    // Verify monotonic decrease (lowpass behavior)
    let mut prev_mag = out_mag_low;
    for (i, sol) in run.ac_solutions.iter().enumerate().skip(1) {
        let (mag, _) = sol[out_idx];
        assert!(
            mag <= prev_mag + 0.5, // Allow small numerical tolerance
            "Output magnitude should decrease monotonically, but increased at freq index {}",
            i
        );
        prev_mag = mag;
    }
}

#[test]
fn ac_analysis_decade_sweep_generates_correct_points() {
    let circuit = make_rc_lowpass();
    let mut engine = Engine::new_default(circuit);
    let mut store = ResultStore::new();

    let plan = AnalysisPlan {
        cmd: AnalysisCmd::Ac {
            sweep_type: AcSweepType::Dec,
            points: 10, // 10 points per decade
            fstart: 1.0,
            fstop: 1000.0, // 3 decades
        },
    };

    let run_id = engine.run_with_store(&plan, &mut store);
    let run = &store.runs[run_id.0];

    assert!(matches!(run.status, RunStatus::Converged));

    // Should have approximately 31 points (10 per decade * 3 decades + 1)
    assert!(run.ac_frequencies.len() >= 25, "Expected ~31 points for 3 decades at 10 pts/dec");
    assert!(run.ac_frequencies.len() <= 35, "Expected ~31 points for 3 decades at 10 pts/dec");

    // First frequency should be 1 Hz
    assert!((run.ac_frequencies[0] - 1.0).abs() < 0.1);

    // Last frequency should be 1000 Hz
    assert!((run.ac_frequencies.last().unwrap() - 1000.0).abs() < 10.0);
}

#[test]
fn ac_analysis_linear_sweep_generates_correct_points() {
    let circuit = make_rc_lowpass();
    let mut engine = Engine::new_default(circuit);
    let mut store = ResultStore::new();

    let plan = AnalysisPlan {
        cmd: AnalysisCmd::Ac {
            sweep_type: AcSweepType::Lin,
            points: 101, // 101 points from 0 to 1000
            fstart: 0.1,
            fstop: 1000.0,
        },
    };

    let run_id = engine.run_with_store(&plan, &mut store);
    let run = &store.runs[run_id.0];

    assert!(matches!(run.status, RunStatus::Converged));
    assert_eq!(run.ac_frequencies.len(), 101);
}
