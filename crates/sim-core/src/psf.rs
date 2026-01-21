use crate::result_store::RunResult;
use std::fs;
use std::path::Path;

pub fn write_psf_text(run: &RunResult, path: &Path) -> std::io::Result<()> {
    let mut out = String::new();
    out.push_str("PSF_TEXT\n");
    out.push_str(&format!("analysis={:?}\n", run.analysis));
    out.push_str(&format!("status={:?}\n", run.status));
    out.push_str(&format!("iterations={}\n", run.iterations));
    out.push_str("signals:\n");
    for name in &run.node_names {
        out.push_str(&format!("- {}\n", name));
    }
    out.push_str("values:\n");
    for (idx, value) in run.solution.iter().enumerate() {
        let name = run
            .node_names
            .get(idx)
            .cloned()
            .unwrap_or_else(|| format!("n{}", idx));
        out.push_str(&format!("{} {}\n", name, value));
    }
    fs::write(path, out)
}
