use sim_core::netlist::parse_netlist;
use sim_core::netlist::{elaborate_netlist, parse_netlist_file, ControlKind, Stmt};
use std::path::PathBuf;

#[test]
fn netlist_parser_skeleton_runs() {
    let input = "* comment\nR1 in out 1k\n.op\n.end\n";
    let ast = parse_netlist(input);
    assert!(ast.errors.is_empty());
    assert!(ast.statements.len() >= 2);
}

#[test]
fn netlist_parser_extracts_nodes_and_value() {
    let input = "R1 in out 10k\n.end\n";
    let ast = parse_netlist(input);
    let device = ast
        .statements
        .iter()
        .find_map(|stmt| match stmt {
            Stmt::Device(dev) => Some(dev),
            _ => None,
        })
        .expect("device not found");
    assert_eq!(device.nodes, vec!["in".to_string(), "out".to_string()]);
    assert_eq!(device.value, Some("10k".to_string()));
}

#[test]
fn netlist_parser_recognizes_model_and_subckt() {
    let input = ".model nmos bsim4 vth0=0.4\n.subckt inv in out vdd vss\n.ends\n";
    let ast = parse_netlist(input);
    let controls: Vec<_> = ast
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Control(ctrl) => Some(ctrl),
            _ => None,
        })
        .collect();

    let model = controls
        .iter()
        .find(|ctrl| matches!(ctrl.kind, ControlKind::Model))
        .expect("model not found");
    assert_eq!(model.model_name.as_deref(), Some("nmos"));
    assert_eq!(model.model_type.as_deref(), Some("bsim4"));

    let subckt = controls
        .iter()
        .find(|ctrl| matches!(ctrl.kind, ControlKind::Subckt))
        .expect("subckt not found");
    assert_eq!(subckt.subckt_name.as_deref(), Some("inv"));
    assert_eq!(subckt.subckt_ports.len(), 4);
}

#[test]
fn netlist_parser_reports_missing_fields() {
    let input = "R1 in 10k\nM1 d g s nmos\n.end\n";
    let ast = parse_netlist(input);
    assert!(!ast.errors.is_empty());
}

#[test]
fn netlist_elaboration_counts_statements() {
    let ast = parse_netlist("R1 in out 1k\n.op\n.end\n");
    let elab = elaborate_netlist(&ast);
    assert_eq!(elab.instances.len(), 1);
    assert_eq!(elab.control_count, 2);
}

#[test]
fn netlist_parser_validates_controlled_sources() {
    let input = "E1 out 0 in 0 2\nF1 out 0 Vctrl 10\n.end\n";
    let ast = parse_netlist(input);
    assert!(ast.errors.is_empty());
}

#[test]
fn netlist_elaboration_expands_subckt() {
    let input = ".subckt buf in out\nR1 in out 1k\n.ends\nX1 a b buf\n.end\n";
    let ast = parse_netlist(input);
    let elab = elaborate_netlist(&ast);
    assert_eq!(elab.instances.len(), 1);
    assert_eq!(elab.instances[0].name, "X1.R1");
    assert_eq!(
        elab.instances[0].nodes,
        vec!["a".to_string(), "b".to_string()]
    );
}

#[test]
fn netlist_elaboration_applies_params() {
    let input = ".param RVAL=5k\nR1 in out RVAL\n.end\n";
    let ast = parse_netlist(input);
    let elab = elaborate_netlist(&ast);
    assert_eq!(elab.instances.len(), 1);
    assert_eq!(elab.instances[0].value.as_deref(), Some("5k"));
}

#[test]
fn netlist_parser_expands_include() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
        .join("netlists")
        .join("include_parent.cir");
    let ast = parse_netlist_file(&root);
    assert!(ast.errors.is_empty());
    let device_count = ast
        .statements
        .iter()
        .filter(|stmt| matches!(stmt, Stmt::Device(_)))
        .count();
    assert!(device_count >= 2);
}
