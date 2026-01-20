#[derive(Debug, Clone)]
pub struct RunId(pub String);

#[derive(Debug, Clone)]
pub struct ResultStore {
    pub run_count: usize,
}
