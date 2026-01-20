#[derive(Debug, Clone)]
pub struct RunId(pub String);

#[derive(Debug, Clone)]
pub struct ResultStore {
    pub run_count: usize,
}

pub fn debug_dump_result_store(store: &ResultStore) {
    println!("result_store: runs={}", store.run_count);
}
