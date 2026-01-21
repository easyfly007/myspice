use sim_core::mna::{AuxVarTable, SparseBuilder};
use sim_core::mna::MnaBuilder;
use sim_core::stamp::{DeviceStamp, InstanceStamp};
use sim_core::circuit::{DeviceKind, Instance, NodeId};
use std::collections::HashMap;

#[test]
fn aux_var_table_allocates_unique_ids() {
    let mut table = AuxVarTable::new();
    let id1 = table.allocate("V1");
    let id2 = table.allocate("V2");
    let id1_again = table.allocate("V1");
    assert_eq!(id1, id1_again);
    assert_ne!(id1, id2);
}

#[test]
fn sparse_builder_accepts_inserts() {
    let mut builder = SparseBuilder::new(3);
    builder.insert(0, 0, 1.0);
    builder.insert(1, 0, -1.0);
    assert_eq!(builder.col_entries[0].len(), 1);
    assert_eq!(builder.col_entries[1].len(), 1);
}

#[test]
fn mna_builder_allocates_aux_for_voltage() {
    let mut builder = MnaBuilder::new(2);
    let instance = Instance {
        name: "V1".to_string(),
        kind: DeviceKind::V,
        nodes: vec![NodeId(0), NodeId(1)],
        model: None,
        params: HashMap::new(),
        value: Some("1".to_string()),
        control: None,
    };
    let stamp = InstanceStamp { instance };
    let mut ctx = builder.context();
    stamp.stamp_dc(&mut ctx, None).unwrap();
    assert_eq!(builder.builder.n, 3);
    assert_eq!(builder.rhs.len(), 3);
}

#[test]
fn dc_op_mna_entries_for_r_and_i() {
    let mut builder = MnaBuilder::new(2);

    let r1 = Instance {
        name: "R1".to_string(),
        kind: DeviceKind::R,
        nodes: vec![NodeId(1), NodeId(0)],
        model: None,
        params: HashMap::new(),
        value: Some("1k".to_string()),
        control: None,
    };
    let i1 = Instance {
        name: "I1".to_string(),
        kind: DeviceKind::I,
        nodes: vec![NodeId(1), NodeId(0)],
        model: None,
        params: HashMap::new(),
        value: Some("1m".to_string()),
        control: None,
    };

    let mut ctx = builder.context();
    InstanceStamp { instance: r1 }.stamp_dc(&mut ctx, None).unwrap();
    InstanceStamp { instance: i1 }.stamp_dc(&mut ctx, None).unwrap();

    let g = 1.0 / 1000.0;
    assert_eq!(sum_entry(&builder.builder, 1, 1), g);
    assert_eq!(sum_entry(&builder.builder, 0, 0), g);
    assert_eq!(sum_entry(&builder.builder, 1, 0), -g);
    assert_eq!(sum_entry(&builder.builder, 0, 1), -g);
    assert!((builder.rhs[1] + 0.001).abs() < 1e-12);
    assert!((builder.rhs[0] - 0.001).abs() < 1e-12);
}

#[test]
fn inductor_dc_stamp_as_short() {
    let mut builder = MnaBuilder::new(2);
    let l1 = Instance {
        name: "L1".to_string(),
        kind: DeviceKind::L,
        nodes: vec![NodeId(1), NodeId(0)],
        model: None,
        params: HashMap::new(),
        value: Some("1m".to_string()),
        control: None,
    };
    let mut ctx = builder.context();
    InstanceStamp { instance: l1 }.stamp_dc(&mut ctx, None).unwrap();
    assert!(sum_entry(&builder.builder, 1, 1) > 0.0);
}

#[test]
fn source_scale_applies_to_current() {
    let mut builder = MnaBuilder::new(2);
    let i1 = Instance {
        name: "I1".to_string(),
        kind: DeviceKind::I,
        nodes: vec![NodeId(1), NodeId(0)],
        model: None,
        params: HashMap::new(),
        value: Some("1m".to_string()),
        control: None,
    };
    let mut ctx = builder.context_with(0.0, 0.5);
    InstanceStamp { instance: i1 }.stamp_dc(&mut ctx, None).unwrap();
    assert!((builder.rhs[1] + 0.0005).abs() < 1e-12);
    assert!((builder.rhs[0] - 0.0005).abs() < 1e-12);
}

#[test]
fn gmin_applies_to_diode_stamp() {
    let mut builder = MnaBuilder::new(2);
    let d1 = Instance {
        name: "D1".to_string(),
        kind: DeviceKind::D,
        nodes: vec![NodeId(1), NodeId(0)],
        model: None,
        params: HashMap::new(),
        value: None,
        control: None,
    };
    let mut ctx = builder.context_with(1e-6, 1.0);
    InstanceStamp { instance: d1 }.stamp_dc(&mut ctx, None).unwrap();
    assert_eq!(sum_entry(&builder.builder, 1, 1), 1e-6);
    assert_eq!(sum_entry(&builder.builder, 0, 0), 1e-6);
}

#[test]
fn diode_stamp_uses_solution_when_provided() {
    let mut builder = MnaBuilder::new(2);
    let d1 = Instance {
        name: "D1".to_string(),
        kind: DeviceKind::D,
        nodes: vec![NodeId(1), NodeId(0)],
        model: None,
        params: HashMap::new(),
        value: None,
        control: None,
    };
    let mut ctx = builder.context_with(1e-12, 1.0);
    let x = vec![0.0, 0.7];
    InstanceStamp { instance: d1 }.stamp_dc(&mut ctx, Some(&x)).unwrap();
    assert!(sum_entry(&builder.builder, 1, 1) > 1e-12);
}

fn sum_entry(builder: &SparseBuilder, row: usize, col: usize) -> f64 {
    builder.col_entries[col]
        .iter()
        .filter(|(r, _)| *r == row)
        .map(|(_, v)| *v)
        .sum()
}
