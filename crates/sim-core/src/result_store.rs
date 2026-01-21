#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RunId(pub usize);

#[derive(Debug, Clone)]
pub enum AnalysisType {
    Op,
    Dc,
    Tran,
}

#[derive(Debug, Clone)]
pub enum RunStatus {
    Converged,
    MaxIters,
    Failed,
}

#[derive(Debug, Clone)]
pub struct RunResult {
    pub id: RunId,
    pub analysis: AnalysisType,
    pub status: RunStatus,
    pub iterations: usize,
    pub node_names: Vec<String>,
    pub solution: Vec<f64>,
    pub message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ResultStore {
    pub runs: Vec<RunResult>,
}

impl ResultStore {
    pub fn new() -> Self {
        Self { runs: Vec::new() }
    }

    pub fn add_run(&mut self, mut run: RunResult) -> RunId {
        let id = RunId(self.runs.len());
        run.id = id;
        self.runs.push(run);
        id
    }

    pub fn write_psf_text(&self, id: RunId, path: &std::path::Path) -> std::io::Result<()> {
        let run = self
            .runs
            .get(id.0)
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "run not found"))?;
        crate::psf::write_psf_text(run, path)
    }
}

pub fn debug_dump_result_store(store: &ResultStore) {
    println!("result_store: runs={}", store.runs.len());
}
