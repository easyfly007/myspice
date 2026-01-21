use sim_core::result_store::{AnalysisType, ResultStore, RunId, RunResult, RunStatus};
use std::path::PathBuf;

#[test]
fn psf_text_writer_outputs_basic_content() {
    let mut store = ResultStore::new();
    let run = RunResult {
        id: RunId(0),
        analysis: AnalysisType::Op,
        status: RunStatus::Converged,
        iterations: 1,
        node_names: vec!["0".to_string(), "n1".to_string()],
        solution: vec![0.0, 1.0],
        message: None,
    };
    let run_id = store.add_run(run);

    let mut path = std::env::temp_dir();
    path.push("myspice_psf_test.txt");
    store.write_psf_text(run_id, &path).unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("PSF_TEXT"));
    assert!(content.contains("analysis=Op"));
    assert!(content.contains("n1 1"));
}
