use sim_core::circuit::{DeviceKind, Instance, NodeId};
use sim_core::mna::MnaBuilder;
use sim_core::stamp::{DeviceStamp, InstanceStamp, TransientState};
use std::collections::HashMap;

#[test]
fn diode_stamp_allows_basic_nodes() {
    let mut builder = MnaBuilder::new(2);
    let diode = Instance {
        name: "D1".to_string(),
        kind: DeviceKind::D,
        nodes: vec![NodeId(1), NodeId(0)],
        model: None,
        params: HashMap::new(),
        value: None,
        control: None,
    };
    let mut ctx = builder.context();
    InstanceStamp { instance: diode }.stamp_dc(&mut ctx, None).unwrap();
}

#[test]
fn mos_stamp_allows_basic_nodes() {
    let mut builder = MnaBuilder::new(4);
    let mos = Instance {
        name: "M1".to_string(),
        kind: DeviceKind::M,
        nodes: vec![NodeId(1), NodeId(2), NodeId(3), NodeId(0)],
        model: None,
        params: HashMap::new(),
        value: None,
        control: None,
    };
    let mut ctx = builder.context();
    InstanceStamp { instance: mos }.stamp_dc(&mut ctx, None).unwrap();
}

#[test]
fn capacitor_tran_stamp_basic() {
    let mut builder = MnaBuilder::new(2);
    let cap = Instance {
        name: "C1".to_string(),
        kind: DeviceKind::C,
        nodes: vec![NodeId(1), NodeId(0)],
        model: None,
        params: HashMap::new(),
        value: Some("1u".to_string()),
        control: None,
    };
    let mut ctx = builder.context();
    let mut state = TransientState::default();
    InstanceStamp { instance: cap }
        .stamp_tran(&mut ctx, Some(&vec![0.0, 1.0]), 1e-6, &mut state)
        .unwrap();
    assert!(builder.rhs[1].is_finite());
}

#[test]
fn inductor_tran_stamp_basic() {
    let mut builder = MnaBuilder::new(2);
    let ind = Instance {
        name: "L1".to_string(),
        kind: DeviceKind::L,
        nodes: vec![NodeId(1), NodeId(0)],
        model: None,
        params: HashMap::new(),
        value: Some("1m".to_string()),
        control: None,
    };
    let mut ctx = builder.context();
    let mut state = TransientState::default();
    InstanceStamp { instance: ind }
        .stamp_tran(&mut ctx, Some(&vec![0.0, 0.0, 0.0]), 1e-6, &mut state)
        .unwrap();
    assert!(builder.builder.n >= 3);
}

#[test]
fn update_transient_state_tracks_cap_voltage() {
    let cap = Instance {
        name: "C1".to_string(),
        kind: DeviceKind::C,
        nodes: vec![NodeId(1), NodeId(0)],
        model: None,
        params: HashMap::new(),
        value: Some("1u".to_string()),
        control: None,
    };
    let mut state = TransientState::default();
    sim_core::stamp::update_transient_state(&[cap], &[0.0, 2.0], &mut state);
    assert_eq!(state.cap_voltage.get("C1").copied(), Some(2.0));
}
