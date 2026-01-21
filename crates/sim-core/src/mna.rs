#[derive(Debug, Clone)]
pub struct MnaSystem {
    pub size: usize,
}

pub fn debug_dump_mna(system: &MnaSystem) {
    println!("mna: size={}", system.size);
}

#[derive(Debug, Clone)]
pub struct AuxVarTable {
    pub name_to_id: std::collections::HashMap<String, usize>,
    pub id_to_name: Vec<String>,
}

impl AuxVarTable {
    pub fn new() -> Self {
        Self {
            name_to_id: std::collections::HashMap::new(),
            id_to_name: Vec::new(),
        }
    }

    pub fn allocate(&mut self, name: &str) -> usize {
        if let Some(id) = self.name_to_id.get(name) {
            return *id;
        }
        let id = self.id_to_name.len();
        self.name_to_id.insert(name.to_string(), id);
        self.id_to_name.push(name.to_string());
        id
    }

    pub fn allocate_with_flag(&mut self, name: &str) -> (usize, bool) {
        if let Some(id) = self.name_to_id.get(name) {
            return (*id, false);
        }
        let id = self.id_to_name.len();
        self.name_to_id.insert(name.to_string(), id);
        self.id_to_name.push(name.to_string());
        (id, true)
    }
}

#[derive(Debug, Clone)]
pub struct SparseBuilder {
    pub n: usize,
    pub col_entries: Vec<Vec<(usize, f64)>>,
}

impl SparseBuilder {
    pub fn new(n: usize) -> Self {
        Self {
            n,
            col_entries: vec![Vec::new(); n],
        }
    }

    pub fn insert(&mut self, col: usize, row: usize, value: f64) {
        if col >= self.n {
            return;
        }
        self.col_entries[col].push((row, value));
    }

    pub fn clear_values(&mut self) {
        for col in &mut self.col_entries {
            for entry in col.iter_mut() {
                entry.1 = 0.0;
            }
        }
    }

    pub fn resize(&mut self, new_n: usize) {
        if new_n <= self.n {
            return;
        }
        self.col_entries.resize_with(new_n, Vec::new);
        self.n = new_n;
    }

    pub fn finalize(&mut self) -> (Vec<i64>, Vec<i64>, Vec<f64>) {
        let mut ap = Vec::with_capacity(self.n + 1);
        let mut ai = Vec::new();
        let mut ax = Vec::new();

        let mut nnz = 0;
        ap.push(0);
        for col in &mut self.col_entries {
            col.sort_by_key(|(row, _)| *row);
            for (row, value) in col.iter() {
                ai.push(*row as i64);
                ax.push(*value);
                nnz += 1;
            }
            ap.push(nnz as i64);
        }

        (ap, ai, ax)
    }
}

#[derive(Debug)]
pub struct StampContext<'a> {
    pub builder: &'a mut SparseBuilder,
    pub rhs: &'a mut Vec<f64>,
    pub aux: &'a mut AuxVarTable,
    pub node_count: usize,
    pub gmin: f64,
    pub source_scale: f64,
}

impl<'a> StampContext<'a> {
    pub fn add(&mut self, i: usize, j: usize, value: f64) {
        self.builder.insert(j, i, value);
    }

    pub fn add_rhs(&mut self, i: usize, value: f64) {
        if let Some(entry) = self.rhs.get_mut(i) {
            *entry += value;
        }
    }

    pub fn allocate_aux(&mut self, name: &str) -> usize {
        let (aux_id, is_new) = self.aux.allocate_with_flag(name);
        let index = self.node_count + aux_id;
        if is_new {
            self.builder.resize(self.node_count + self.aux.id_to_name.len());
            self.rhs.resize(self.builder.n, 0.0);
        }
        index
    }
}

#[derive(Debug)]
pub struct MnaBuilder {
    pub node_count: usize,
    pub size: usize,
    pub rhs: Vec<f64>,
    pub builder: SparseBuilder,
    pub aux: AuxVarTable,
}

impl MnaBuilder {
    pub fn new(node_count: usize) -> Self {
        let size = node_count;
        Self {
            node_count,
            size,
            rhs: vec![0.0; size],
            builder: SparseBuilder::new(size),
            aux: AuxVarTable::new(),
        }
    }

    pub fn context(&mut self) -> StampContext<'_> {
        StampContext {
            builder: &mut self.builder,
            rhs: &mut self.rhs,
            aux: &mut self.aux,
            node_count: self.node_count,
            gmin: 0.0,
            source_scale: 1.0,
        }
    }

    pub fn context_with(&mut self, gmin: f64, source_scale: f64) -> StampContext<'_> {
        StampContext {
            builder: &mut self.builder,
            rhs: &mut self.rhs,
            aux: &mut self.aux,
            node_count: self.node_count,
            gmin,
            source_scale,
        }
    }
}
