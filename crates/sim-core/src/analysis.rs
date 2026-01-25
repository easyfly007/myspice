use crate::circuit::AnalysisCmd;

#[derive(Debug, Clone)]
pub struct ConvergenceConfig {
    pub max_iters: usize,
    pub abs_tol: f64,
    pub rel_tol: f64,
    pub gmin: f64,
    pub damping: f64,
}

#[derive(Debug, Clone)]
pub struct ConvergenceState {
    pub iter: usize,
    pub last_norm: f64,
    pub converged: bool,
}

#[derive(Debug, Clone)]
pub struct TimeStepConfig {
    pub tstep: f64,
    pub tstop: f64,
    pub tstart: f64,
    pub tmax: f64,
    pub min_dt: f64,
    pub max_dt: f64,
    pub abs_tol: f64,
    pub rel_tol: f64,
}

#[derive(Debug, Clone)]
pub struct TimeStepState {
    pub time: f64,
    pub step: usize,
    pub dt: f64,
    pub last_dt: f64,
    pub accepted: bool,
}

#[derive(Debug, Clone)]
pub struct AnalysisPlan {
    pub cmd: AnalysisCmd,
}

pub fn debug_dump_analysis(plan: &AnalysisPlan) {
    println!("analysis: {:?}", plan.cmd);
}

#[derive(Debug, Clone)]
pub struct NewtonPlan {
    pub config: crate::newton::NewtonConfig,
}

#[derive(Debug, Clone)]
pub struct ErrorEstimate {
    pub error_norm: f64,
    pub accept: bool,
}

pub fn estimate_error(prev: &[f64], next: &[f64], tol: f64) -> ErrorEstimate {
    let mut max_err = 0.0;
    for (p, n) in prev.iter().zip(next.iter()) {
        let err = (n - p).abs();
        if err > max_err {
            max_err = err;
        }
    }
    ErrorEstimate {
        error_norm: max_err,
        accept: max_err <= tol,
    }
}

pub fn estimate_error_weighted(
    prev: &[f64],
    next: &[f64],
    abs_tol: f64,
    rel_tol: f64,
) -> ErrorEstimate {
    let mut max_ratio = 0.0;
    for (p, n) in prev.iter().zip(next.iter()) {
        let denom = abs_tol + rel_tol * p.abs().max(n.abs());
        if denom == 0.0 {
            continue;
        }
        let ratio = (n - p).abs() / denom;
        if ratio > max_ratio {
            max_ratio = ratio;
        }
    }
    ErrorEstimate {
        error_norm: max_ratio,
        accept: max_ratio <= 1.0,
    }
}
