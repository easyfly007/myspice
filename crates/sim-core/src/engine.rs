use crate::analysis::{
    estimate_error_weighted, AnalysisPlan, ErrorEstimate, TimeStepConfig, TimeStepState,
};
use crate::circuit::Circuit;
use crate::mna::MnaBuilder;
use crate::result_store::{AnalysisType, ResultStore, RunId, RunResult, RunStatus};
use crate::solver::{create_solver, LinearSolver, SolverType};
use crate::stamp::{update_transient_state, DeviceStamp, InstanceStamp, TransientState};
use crate::newton::{debug_dump_newton_with_tag, run_newton_with_stepping, NewtonConfig};

pub struct Engine {
    pub circuit: Circuit,
    solver: Box<dyn LinearSolver>,
    solver_type: SolverType,
}

impl Engine {
    /// 使用指定的求解器类型创建 Engine
    pub fn new(circuit: Circuit, solver_type: SolverType) -> Self {
        let node_count = circuit.nodes.id_to_name.len();
        Self {
            circuit,
            solver: create_solver(solver_type, node_count),
            solver_type,
        }
    }

    /// 使用默认求解器（Dense）创建 Engine
    pub fn new_default(circuit: Circuit) -> Self {
        Self::new(circuit, SolverType::default())
    }

    /// 当电路大小变化时，重新初始化 solver
    pub fn resize_solver(&mut self) {
        let node_count = self.circuit.nodes.id_to_name.len();
        self.solver = create_solver(self.solver_type, node_count);
    }

    /// 切换求解器类型
    pub fn set_solver_type(&mut self, solver_type: SolverType) {
        self.solver_type = solver_type;
        self.resize_solver();
    }

    pub fn run(&mut self, plan: &AnalysisPlan) {
        println!("engine: run {:?}", plan.cmd);
        match plan.cmd {
            crate::circuit::AnalysisCmd::Tran { .. } => self.run_tran(),
            _ => self.run_dc(),
        }
    }

    pub fn run_with_store(&mut self, plan: &AnalysisPlan, store: &mut ResultStore) -> RunId {
        let result = match &plan.cmd {
            crate::circuit::AnalysisCmd::Tran { tstep, tstop, tstart, tmax } => {
                self.run_tran_result_with_params(*tstep, *tstop, *tstart, *tmax)
            }
            crate::circuit::AnalysisCmd::Dc { source, start, stop, step } => {
                self.run_dc_sweep_result(source, *start, *stop, *step)
            }
            _ => self.run_dc_result(AnalysisType::Op),
        };
        store.add_run(result)
    }

    pub fn run_dc(&mut self) {
        let _ = self.run_dc_result(AnalysisType::Op);
    }

    pub fn run_tran(&mut self) {
        // Use default parameters for standalone run
        let _ = self.run_tran_result_with_params(1e-6, 1e-5, 0.0, 1e-5);
    }

    fn run_dc_result(&mut self, analysis: AnalysisType) -> RunResult {
        let config = NewtonConfig::default();
        let node_count = self.circuit.nodes.id_to_name.len();
        let mut x = vec![0.0; node_count];
        self.solver.prepare(node_count);
        let gnd = self.circuit.nodes.gnd_id.0;
        let result = run_newton_with_stepping(&config, &mut x, |x, gmin, source_scale| {
            let mut mna = MnaBuilder::new(node_count);
            for inst in &self.circuit.instances.instances {
                let stamp = InstanceStamp {
                    instance: inst.clone(),
                };
                let mut ctx = mna.context_with(gmin, source_scale);
                let _ = stamp.stamp_dc(&mut ctx, Some(x));
            }
            // 固定地节点，避免矩阵奇异
            mna.builder.insert(gnd, gnd, 1.0);
            let (ap, ai, ax) = mna.builder.finalize();
            (ap, ai, ax, mna.rhs, mna.builder.n)
        }, self.solver.as_mut());

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
            sweep_var: None,
            sweep_values: Vec::new(),
            sweep_solutions: Vec::new(),
            tran_times: Vec::new(),
            tran_solutions: Vec::new(),
        }
    }

    /// Run TRAN analysis with specified parameters and store waveform data
    ///
    /// This function performs transient analysis from `tstart` to `tstop` using
    /// adaptive time stepping. It stores the solution at each accepted time point
    /// in `tran_times` and `tran_solutions`.
    ///
    /// # Arguments
    /// * `tstep` - Suggested time step for output
    /// * `tstop` - Stop time
    /// * `tstart` - Start time (usually 0)
    /// * `tmax` - Maximum internal time step
    ///
    /// # Returns
    /// RunResult containing:
    /// - `tran_times`: Vector of time points where solutions were computed
    /// - `tran_solutions`: Vector of solution vectors at each time point
    /// - `solution`: Final solution at tstop
    fn run_tran_result_with_params(
        &mut self,
        tstep: f64,
        tstop: f64,
        tstart: f64,
        tmax: f64,
    ) -> RunResult {
        let node_count = self.circuit.nodes.id_to_name.len();
        let mut x = vec![0.0; node_count];
        let mut state = TransientState::default();
        self.solver.prepare(node_count);

        let config = TimeStepConfig {
            tstep,
            tstop,
            tstart,
            tmax,
            min_dt: tstep * 1e-6,  // Minimum step is 1e-6 of tstep
            max_dt: tmax,
            abs_tol: 1e-9,
            rel_tol: 1e-6,
        };

        let mut step_state = TimeStepState {
            time: config.tstart,
            step: 0,
            dt: config.tstep.min(config.tmax),
            last_dt: config.tstep,
            accepted: true,
        };

        let mut final_status = RunStatus::Converged;
        let gnd = self.circuit.nodes.gnd_id.0;

        // Waveform storage vectors
        let mut tran_times: Vec<f64> = Vec::new();
        let mut tran_solutions: Vec<Vec<f64>> = Vec::new();

        // Run initial DC operating point (t=tstart)
        let dc_result = run_newton_with_stepping(&NewtonConfig::default(), &mut x, |x, gmin, source_scale| {
            let mut mna = MnaBuilder::new(node_count);
            for inst in &self.circuit.instances.instances {
                let stamp = InstanceStamp {
                    instance: inst.clone(),
                };
                let mut ctx = mna.context_with(gmin, source_scale);
                let _ = stamp.stamp_dc(&mut ctx, Some(x));
            }
            mna.builder.insert(gnd, gnd, 1.0);
            let (ap, ai, ax) = mna.builder.finalize();
            (ap, ai, ax, mna.rhs, mna.builder.n)
        }, self.solver.as_mut());

        debug_dump_newton_with_tag("tran_dc_op", &dc_result);

        if !dc_result.converged {
            return RunResult {
                id: RunId(0),
                analysis: AnalysisType::Tran,
                status: RunStatus::Failed,
                iterations: 0,
                node_names: self.circuit.nodes.id_to_name.clone(),
                solution: Vec::new(),
                message: Some("DC operating point failed to converge".to_string()),
                sweep_var: None,
                sweep_values: Vec::new(),
                sweep_solutions: Vec::new(),
                tran_times: Vec::new(),
                tran_solutions: Vec::new(),
            };
        }

        // Store initial point (t=tstart)
        tran_times.push(config.tstart);
        tran_solutions.push(x.clone());

        // Initialize transient state from DC solution
        update_transient_state(&self.circuit.instances.instances, &x, &mut state);

        // Time stepping loop
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
                mna.builder.insert(gnd, gnd, 1.0);
                let (ap, ai, ax) = mna.builder.finalize();
                (ap, ai, ax, mna.rhs, mna.builder.n)
            }, self.solver.as_mut());

            debug_dump_newton_with_tag("tran", &result);

            if !result.converged {
                // Reduce time step and retry
                step_state.dt = (step_state.dt * 0.5).max(config.min_dt);
                if step_state.dt <= config.min_dt {
                    final_status = RunStatus::Failed;
                    break;
                }
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

                // Store accepted time point and solution
                tran_times.push(step_state.time);
                tran_solutions.push(x.clone());

                // Increase time step for next iteration (adaptive stepping)
                if step_state.dt < config.max_dt {
                    step_state.dt = (step_state.dt * 1.5).min(config.max_dt);
                }
            } else {
                // Reduce time step and retry
                step_state.dt = (step_state.dt * 0.5).max(config.min_dt);
            }
        }

        RunResult {
            id: RunId(0),
            analysis: AnalysisType::Tran,
            status: final_status,
            iterations: step_state.step,
            node_names: self.circuit.nodes.id_to_name.clone(),
            solution: x,  // Final solution
            message: None,
            sweep_var: None,
            sweep_values: Vec::new(),
            sweep_solutions: Vec::new(),
            tran_times,
            tran_solutions,
        }
    }

    /// Run DC sweep analysis
    /// Sweeps the specified source from start to stop with given step size
    fn run_dc_sweep_result(&mut self, source: &str, start: f64, stop: f64, step: f64) -> RunResult {
        let config = NewtonConfig::default();
        let node_count = self.circuit.nodes.id_to_name.len();
        let gnd = self.circuit.nodes.gnd_id.0;

        // Find the source instance index
        let source_lower = source.to_ascii_lowercase();
        let source_idx = self.circuit.instances.instances.iter()
            .position(|inst| inst.name.to_ascii_lowercase() == source_lower);

        if source_idx.is_none() {
            return RunResult {
                id: RunId(0),
                analysis: AnalysisType::Dc,
                status: RunStatus::Failed,
                iterations: 0,
                node_names: self.circuit.nodes.id_to_name.clone(),
                solution: Vec::new(),
                message: Some(format!("DC sweep source '{}' not found", source)),
                sweep_var: Some(source.to_string()),
                sweep_values: Vec::new(),
                sweep_solutions: Vec::new(),
                tran_times: Vec::new(),
                tran_solutions: Vec::new(),
            };
        }
        let source_idx = source_idx.unwrap();

        // Calculate sweep points
        let mut sweep_values = Vec::new();
        let step_size = if stop >= start { step.abs() } else { -step.abs() };

        if step_size.abs() < 1e-15 {
            // Zero step - just do single point at start
            sweep_values.push(start);
        } else {
            // Calculate number of points to avoid floating point accumulation errors
            let range = stop - start;
            let n_points = ((range / step_size).abs().floor() as usize) + 1;

            for i in 0..n_points {
                let value = start + (i as f64) * step_size;
                sweep_values.push(value);
            }

            // Ensure we include the exact stop value if close enough
            if let Some(&last) = sweep_values.last() {
                if (last - stop).abs() > 1e-12 && sweep_values.len() < 10000 {
                    // Don't add if we're very close to stop already
                    if (last - stop).abs() / step_size.abs() > 0.5 {
                        sweep_values.push(stop);
                    }
                }
            }
        }

        let mut sweep_solutions = Vec::new();
        let mut total_iterations = 0;
        let mut final_status = RunStatus::Converged;
        let mut final_message = None;

        // Use previous solution as initial guess for next point (continuation)
        let mut x = vec![0.0; node_count];
        self.solver.prepare(node_count);

        for &sweep_val in &sweep_values {
            // Update source value
            self.circuit.instances.instances[source_idx].value = Some(sweep_val.to_string());

            // Run Newton iteration at this sweep point
            let result = run_newton_with_stepping(&config, &mut x, |x, gmin, source_scale| {
                let mut mna = MnaBuilder::new(node_count);
                for inst in &self.circuit.instances.instances {
                    let stamp = InstanceStamp {
                        instance: inst.clone(),
                    };
                    let mut ctx = mna.context_with(gmin, source_scale);
                    let _ = stamp.stamp_dc(&mut ctx, Some(x));
                }
                // Ground node constraint
                mna.builder.insert(gnd, gnd, 1.0);
                let (ap, ai, ax) = mna.builder.finalize();
                (ap, ai, ax, mna.rhs, mna.builder.n)
            }, self.solver.as_mut());

            total_iterations += result.iterations;

            match result.reason {
                crate::newton::NewtonExitReason::Converged => {
                    sweep_solutions.push(x.clone());
                }
                crate::newton::NewtonExitReason::MaxIters => {
                    final_status = RunStatus::MaxIters;
                    final_message = Some(format!("Failed to converge at sweep point {}", sweep_val));
                    break;
                }
                crate::newton::NewtonExitReason::SolverFailure => {
                    final_status = RunStatus::Failed;
                    final_message = Some(format!("Solver failure at sweep point {}", sweep_val));
                    break;
                }
            }
        }

        // For compatibility, set solution to the last sweep point solution
        let solution = sweep_solutions.last().cloned().unwrap_or_default();

        RunResult {
            id: RunId(0),
            analysis: AnalysisType::Dc,
            status: final_status,
            iterations: total_iterations,
            node_names: self.circuit.nodes.id_to_name.clone(),
            solution,
            message: final_message,
            sweep_var: Some(source.to_string()),
            sweep_values,
            sweep_solutions,
            tran_times: Vec::new(),
            tran_solutions: Vec::new(),
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
