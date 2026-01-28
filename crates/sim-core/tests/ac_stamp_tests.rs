//! AC Stamp Unit Tests
//!
//! Tests for individual device AC stamps.

use sim_core::circuit::{DeviceKind, Instance};
use sim_core::complex_mna::ComplexMnaBuilder;
use sim_core::stamp::{DeviceStamp, InstanceStamp};
use std::collections::HashMap;

fn make_instance(name: &str, kind: DeviceKind, nodes: Vec<usize>, value: Option<&str>) -> Instance {
    Instance {
        name: name.to_string(),
        kind,
        nodes: nodes.into_iter().map(sim_core::circuit::NodeId).collect(),
        model: None,
        params: HashMap::new(),
        value: value.map(String::from),
        control: None,
        ac_mag: None,
        ac_phase: None,
    }
}

#[test]
fn resistor_ac_stamp_is_real() {
    let mut mna = ComplexMnaBuilder::new(3);
    let r1 = make_instance("R1", DeviceKind::R, vec![1, 2], Some("1k"));
    let stamp = InstanceStamp { instance: r1 };

    let omega = 2.0 * std::f64::consts::PI * 1000.0; // 1 kHz
    let mut ctx = mna.context(omega);
    stamp.stamp_ac(&mut ctx, &[0.0; 3]).unwrap();

    let (_, _, ax) = mna.builder.finalize();

    // All entries should be real (imaginary part = 0)
    for val in &ax {
        assert!(
            val.im.abs() < 1e-15,
            "Resistor stamp should be purely real"
        );
    }
}

#[test]
fn capacitor_ac_stamp_is_imaginary() {
    let mut mna = ComplexMnaBuilder::new(3);
    let c1 = make_instance("C1", DeviceKind::C, vec![1, 2], Some("1u"));
    let stamp = InstanceStamp { instance: c1 };

    let omega = 2.0 * std::f64::consts::PI * 1000.0; // 1 kHz
    let mut ctx = mna.context(omega);
    stamp.stamp_ac(&mut ctx, &[0.0; 3]).unwrap();

    let (_, _, ax) = mna.builder.finalize();

    // All entries should be imaginary (real part = 0)
    for val in &ax {
        assert!(
            val.re.abs() < 1e-15,
            "Capacitor stamp should be purely imaginary"
        );
    }
}

#[test]
fn capacitor_ac_admittance_scales_with_frequency() {
    let c1 = make_instance("C1", DeviceKind::C, vec![1, 0], Some("1u"));
    let stamp = InstanceStamp { instance: c1.clone() };

    // At 1 kHz
    let mut mna1 = ComplexMnaBuilder::new(2);
    let omega1 = 2.0 * std::f64::consts::PI * 1000.0;
    let mut ctx1 = mna1.context(omega1);
    stamp.stamp_ac(&mut ctx1, &[0.0; 2]).unwrap();
    let (_, _, ax1) = mna1.builder.finalize();

    // At 10 kHz
    let stamp2 = InstanceStamp { instance: c1 };
    let mut mna2 = ComplexMnaBuilder::new(2);
    let omega2 = 2.0 * std::f64::consts::PI * 10000.0;
    let mut ctx2 = mna2.context(omega2);
    stamp2.stamp_ac(&mut ctx2, &[0.0; 2]).unwrap();
    let (_, _, ax2) = mna2.builder.finalize();

    // Admittance at 10 kHz should be 10x admittance at 1 kHz
    // Compare diagonal element (node 1)
    let y1 = ax1.iter().find(|v| v.im.abs() > 1e-15).unwrap().im;
    let y2 = ax2.iter().find(|v| v.im.abs() > 1e-15).unwrap().im;

    let ratio = y2 / y1;
    assert!(
        (ratio - 10.0).abs() < 0.01,
        "Capacitor admittance should scale linearly with frequency, got ratio {:.3}",
        ratio
    );
}

#[test]
fn inductor_ac_stamp_allocates_aux() {
    let mut mna = ComplexMnaBuilder::new(2);
    let l1 = make_instance("L1", DeviceKind::L, vec![1, 0], Some("1m"));
    let stamp = InstanceStamp { instance: l1 };

    let omega = 2.0 * std::f64::consts::PI * 1000.0;
    let mut ctx = mna.context(omega);
    stamp.stamp_ac(&mut ctx, &[0.0; 2]).unwrap();

    // Inductor should allocate an auxiliary variable
    assert!(mna.aux.id_to_name.len() > 0, "Inductor should allocate aux variable");
    assert_eq!(mna.builder.n, 3, "Matrix size should increase by 1 for aux variable");
}

#[test]
fn voltage_source_ac_stamp_sets_excitation() {
    let mut mna = ComplexMnaBuilder::new(2);
    let mut v1 = make_instance("V1", DeviceKind::V, vec![1, 0], Some("0"));
    v1.ac_mag = Some(1.0);
    v1.ac_phase = Some(45.0);
    let stamp = InstanceStamp { instance: v1 };

    let omega = 2.0 * std::f64::consts::PI * 1000.0;
    let mut ctx = mna.context(omega);
    stamp.stamp_ac(&mut ctx, &[0.0; 2]).unwrap();

    // Check that RHS has the AC excitation
    let rhs = &mna.rhs;
    let aux_idx = 2; // Auxiliary variable index

    // Magnitude should be 1, phase should be 45 degrees
    let expected_mag = 1.0;
    let expected_phase_rad = 45.0 * std::f64::consts::PI / 180.0;

    assert!(rhs.len() > aux_idx);
    let actual = rhs[aux_idx];
    let actual_mag = actual.norm();
    let actual_phase = actual.arg();

    assert!(
        (actual_mag - expected_mag).abs() < 1e-10,
        "AC magnitude should be 1.0, got {}",
        actual_mag
    );
    assert!(
        (actual_phase - expected_phase_rad).abs() < 1e-10,
        "AC phase should be 45°, got {} rad",
        actual_phase
    );
}

#[test]
fn diode_ac_stamp_uses_dc_solution() {
    let mut mna = ComplexMnaBuilder::new(2);
    let d1 = make_instance("D1", DeviceKind::D, vec![1, 0], None);
    let stamp = InstanceStamp { instance: d1 };

    let omega = 2.0 * std::f64::consts::PI * 1000.0;

    // With forward bias (0.7V across diode)
    let dc_sol_forward = vec![0.0, 0.7];
    let mut ctx = mna.context(omega);
    stamp.stamp_ac(&mut ctx, &dc_sol_forward).unwrap();
    let (ap_f, ai_f, ax_f) = mna.builder.finalize();

    // With reverse bias (-0.5V across diode)
    let d2 = make_instance("D2", DeviceKind::D, vec![1, 0], None);
    let stamp2 = InstanceStamp { instance: d2 };
    let mut mna2 = ComplexMnaBuilder::new(2);
    let dc_sol_reverse = vec![0.0, -0.5];
    let mut ctx2 = mna2.context(omega);
    stamp2.stamp_ac(&mut ctx2, &dc_sol_reverse).unwrap();
    let (ap_r, ai_r, ax_r) = mna2.builder.finalize();

    // Find diagonal conductance at node 1
    let find_diag = |ap: &[i64], ai: &[i64], ax: &[num_complex::Complex64], node: usize| -> f64 {
        let start = ap[node] as usize;
        let end = ap[node + 1] as usize;
        for k in start..end {
            if ai[k] as usize == node {
                return ax[k].re;
            }
        }
        0.0
    };

    let g_forward = find_diag(&ap_f, &ai_f, &ax_f, 1);
    let g_reverse = find_diag(&ap_r, &ai_r, &ax_r, 1);

    // Forward bias should have higher conductance than reverse bias
    assert!(
        g_forward > g_reverse,
        "Forward bias conductance ({:.2e}) should be higher than reverse ({:.2e})",
        g_forward, g_reverse
    );
}

#[test]
fn vccs_ac_stamp_applies_transconductance() {
    let mut mna = ComplexMnaBuilder::new(3);
    // G1 out 0 in 0 1m (gm = 1mS)
    let g1 = make_instance("G1", DeviceKind::G, vec![1, 0, 2, 0], Some("1m"));
    let stamp = InstanceStamp { instance: g1 };

    let omega = 2.0 * std::f64::consts::PI * 1000.0;
    let mut ctx = mna.context(omega);
    stamp.stamp_ac(&mut ctx, &[0.0; 3]).unwrap();

    // VCCS should not allocate auxiliary variable
    assert_eq!(mna.aux.id_to_name.len(), 0);
    assert_eq!(mna.builder.n, 3);
}
