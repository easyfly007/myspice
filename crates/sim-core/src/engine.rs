use crate::analysis::{
    estimate_error_weighted, AnalysisPlan, ErrorEstimate, TimeStepConfig, TimeStepState,
};
use crate::circuit::Circuit;
use crate::mna::MnaBuilder;
use crate::result_store::{AnalysisType, ResultStore, RunId, RunResult, RunStatus};
use crate::solver::{DefaultSolver, LinearSolver};
use crate::stamp::{update_transient_state, DeviceStamp, InstanceStamp, TransientState};
use crate::newton::{debug_dump_newton_with_tag, run_newton_with_stepping, NewtonConfig};

#[derive(Debug)]
pub struct Engine {
    pub circuit: Circuit,
    solver: DefaultSolver,
}

impl Engine {
    pub fn new(circuit: Circuit) -> Self {
        let node_count = circuit.nodes.id_to_name.len();
        Self {
            circuit,
            solver: DefaultSolver::new(node_count),
        }
    }

    /// 当电路大小变化时，重新初始化 solver
    pub fn resize_solver(&mut self) {
        let node_count = self.circuit.nodes.id_to_name.len();
        self.solver = DefaultSolver::new(node_count);
    }

    pub fn run(&mut self, plan: &AnalysisPlan) {
        println!("engine: run {:?}", plan.cmd);
        match plan.cmd {
            crate::circuit::AnalysisCmd::Tran { .. } => self.run_tran(),
            _ => self.run_dc(),
        }
    }

    pub fn run_with_store(&mut self, plan: &AnalysisPlan, store: &mut ResultStore) -> RunId {
        let result = match plan.cmd {
            crate::circuit::AnalysisCmd::Tran { .. } => self.run_tran_result(),
            crate::circuit::AnalysisCmd::Dc { .. } => self.run_dc_result(AnalysisType::Dc),
            _ => self.run_dc_result(AnalysisType::Op),
        };
        store.add_run(result)
    }

    pub fn run_dc(&mut self) {
        let _ = self.run_dc_result(AnalysisType::Op);
    }

    pub fn run_tran(&mut self) {
        let _ = self.run_tran_result();
    }

    fn run_dc_result(&mut self, analysis: AnalysisType) -> RunResult {
        let config = NewtonConfig::default();
        let node_count = self.circuit.nodes.id_to_name.len();
        let mut x = vec![0.0; node_count];
        self.solver.prepare(node_count);
        let result = run_newton_with_stepping(&config, &mut x, |x, gmin, source_scale| {
            let mut mna = MnaBuilder::new(node_count);
            for inst in &self.circuit.instances.instances {
                let stamp = InstanceStamp {
                    instance: inst.clone(),
                };
                let mut ctx = mna.context_with(gmin, source_scale);
                let _ = stamp.stamp_dc(&mut ctx, Some(x));
            }
            let (ap, ai, ax) = mna.builder.finalize();
            (ap, ai, ax, mna.rhs, mna.builder.n)
        }, &mut self.solver);

        debug_dump_newton_with_tag("dc", &result);
        let status = match result.reason {
            crate::newton::NewtonExitReason::Converged => RunStatus::Converged,
            crate::newton::NewtonExitReason::MaxIters => RunStatus::MaxIters,
            crate::newton::NewtonExitReason::SolverFailure => RunStatus::Failed,
        };
        RunResult {
            id: RunId(0),
            analysis,
            status,
            iterations: result.iterations,
            node_names: self.circuit.nodes.id_to_name.clone(),
            solution: if matches!(status, RunStatus::Converged) {
                x
            } else {
                Vec::new()
            },
            message: result.message,
        }
    }

    fn run_tran_result(&mut self) -> RunResult {
        let node_count = self.circuit.nodes.id_to_name.len();
        let mut x = vec![0.0; node_count];
        let mut state = TransientState::default();
        self.solver.prepare(node_count);
        let config = TimeStepConfig {
            tstep: 1e-6,
            tstop: 1e-5,
            tstart: 0.0,
            tmax: 1e-5,
            min_dt: 1e-9,
            max_dt: 1e-4,
            abs_tol: 1e-9,
            rel_tol: 1e-6,
        };
        let mut step_state = TimeStepState {
            time: config.tstart,
            step: 0,
            dt: config.tstep,
            last_dt: config.tstep,
            accepted: true,
        };
        let mut final_status = RunStatus::Converged;

        while step_state.time < config.tstop {
            let mut x_iter = x.clone();
            let result = run_newton_with_stepping(&NewtonConfig::default(), &mut x_iter, |x, gmin, source_scale| {
                let mut mna = MnaBuilder::new(node_count);
                for inst in &self.circuit.instances.instances {
                    let stamp = InstanceStamp {
                        instance: inst.clone(),
                    };
                    let mut ctx = mna.context_with(gmin, source_scale);
                    let _ = stamp.stamp_tran(
                        &mut ctx,
                        Some(x),
                        step_state.dt,
                        &mut state,
                    );
                }
                let (ap, ai, ax) = mna.builder.finalize();
                (ap, ai, ax, mna.rhs, mna.builder.n)
            }, &mut self.solver);

            debug_dump_newton_with_tag("tran", &result);
            if !result.converged {
                step_state.dt = (step_state.dt * 0.5).max(config.min_dt);
                final_status = RunStatus::Failed;
                continue;
            }

            let ErrorEstimate { accept, .. } =
                estimate_error_weighted(&x, &x_iter, config.abs_tol, config.rel_tol);
            step_state.accepted = accept;
            if accept {
                x = x_iter;
                update_transient_state(&self.circuit.instances.instances, &x, &mut state);
                step_state.time += step_state.dt;
                step_state.step += 1;
                step_state.last_dt = step_state.dt;
                if step_state.dt < config.max_dt {
                    step_state.dt = (step_state.dt * 1.5).min(config.max_dt);
                }
            } else {
                step_state.dt = (step_state.dt * 0.5).max(config.min_dt);
            }
        }

        RunResult {
            id: RunId(0),
            analysis: AnalysisType::Tran,
            status: final_status,
            iterations: step_state.step,
            node_names: self.circuit.nodes.id_to_name.clone(),
            solution: if matches!(final_status, RunStatus::Converged) {
                x
            } else {
                Vec::new()
            },
            message: None,
        }
    }
}

pub fn debug_dump_engine(engine: &Engine) {
    println!(
        "engine: nodes={} instances={}",
        engine.circuit.nodes.id_to_name.len(),
        engine.circuit.instances.instances.len()
    );
}
