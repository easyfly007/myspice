#[derive(Debug, Clone)]
pub struct MnaSystem {
    pub size: usize,
}

pub fn debug_dump_mna(system: &MnaSystem) {
    println!("mna: size={}", system.size);
}
