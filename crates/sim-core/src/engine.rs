use crate::analysis::{
    estimate_error_weighted, AnalysisPlan, ErrorEstimate, TimeStepConfig, TimeStepState,
};
use crate::circuit::Circuit;
use crate::mna::MnaBuilder;
use crate::solver::KluSolver;
use crate::stamp::{update_transient_state, DeviceStamp, InstanceStamp, TransientState};
use crate::newton::{debug_dump_newton, run_newton, GminSchedule, NewtonConfig, SourceSchedule};
use crate::solver::LinearSolver;

#[derive(Debug, Clone)]
pub struct Engine {
    pub circuit: Circuit,
}

impl Engine {
    pub fn new(circuit: Circuit) -> Self {
        Self { circuit }
    }

    pub fn run(&self, plan: &AnalysisPlan) {
        println!("engine: run {:?}", plan.cmd);
        match plan.cmd {
            crate::circuit::AnalysisCmd::Tran { .. } => self.run_tran(),
            _ => self.run_dc(),
        }
    }

    pub fn run_dc(&self) {
        let config = NewtonConfig::default();
        let node_count = self.circuit.nodes.id_to_name.len();
        let mut x = vec![0.0; node_count];
        let gmin_start = (config.gmin * 1e3).max(1e-6);
        let mut gmin_sched = GminSchedule::new(config.gmin_steps, gmin_start, config.gmin);
        let mut solver = KluSolver::new(node_count);

        for _ in 0..=config.gmin_steps {
            let gmin = gmin_sched.value();
            let mut source_sched = SourceSchedule::new(config.source_steps);

            for _ in 0..=config.source_steps {
                let source_scale = source_sched.scale();
                let result = run_newton(&config, &mut x, |x| {
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
                }, &mut solver);

                debug_dump_newton(&result);
                if result.converged {
                    break;
                }
                source_sched.advance();
            }

            gmin_sched.advance();
        }
    }

    pub fn run_tran(&self) {
        let node_count = self.circuit.nodes.id_to_name.len();
        let mut x = vec![0.0; node_count];
        let mut state = TransientState::default();
        let mut solver = KluSolver::new(node_count);
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

        while step_state.time < config.tstop {
            let mut converged = false;
            let mut x_iter = x.clone();
            let gmin_start = (NewtonConfig::default().gmin * 1e3).max(1e-6);
            let mut gmin_sched =
                GminSchedule::new(NewtonConfig::default().gmin_steps, gmin_start, NewtonConfig::default().gmin);

            for _ in 0..=NewtonConfig::default().gmin_steps {
                let gmin = gmin_sched.value();
                let mut source_sched = SourceSchedule::new(NewtonConfig::default().source_steps);

                for _ in 0..=NewtonConfig::default().source_steps {
                    let source_scale = source_sched.scale();
                    let mut solver = KluSolver::new(node_count);
                    let result = run_newton(&NewtonConfig::default(), &mut x_iter, |x| {
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
                    }, &mut solver);

                    if result.converged {
                        converged = true;
                        break;
                    }
                    source_sched.advance();
                }

                if converged {
                    break;
                }
                gmin_sched.advance();
            }

            if !converged {
                step_state.dt = (step_state.dt * 0.5).max(config.min_dt);
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
    }
}

pub fn debug_dump_engine(engine: &Engine) {
    println!(
        "engine: nodes={} instances={}",
        engine.circuit.nodes.id_to_name.len(),
        engine.circuit.instances.instances.len()
    );
}
