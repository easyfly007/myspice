#[derive(Debug, Clone)]
pub struct NetlistAst {
    pub title: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ElaboratedNetlist {
    pub instance_count: usize,
}
