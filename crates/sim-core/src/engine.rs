use crate::analysis::{
    estimate_error_weighted, AnalysisPlan, ErrorEstimate, TimeStepConfig, TimeStepState,
};
use crate::circuit::{AcSweepType, Circuit};
use crate::complex_mna::ComplexMnaBuilder;
use crate::complex_solver::create_complex_solver;
use crate::mna::MnaBuilder;
use crate::result_store::{AnalysisType, ResultStore, RunId, RunResult, RunStatus};
use crate::solver::{create_solver, LinearSolver, SolverType};
use crate::stamp::{update_transient_state, DeviceStamp, InstanceStamp, TransientState};
use crate::newton::{debug_dump_newton_with_tag, run_newton_with_stepping, NewtonConfig};
use num_complex::Complex64;

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
            crate::circuit::AnalysisCmd::Ac { .. } => {
                let _ = self.run_ac_result_from_plan(plan);
            }
            _ => self.run_dc(),
        }
    }

    pub fn run_with_store(&mut self, plan: &AnalysisPlan, store: &mut ResultStore) -> RunId {
        let result = match &plan.cmd {
            crate::circuit::AnalysisCmd::Tran { .. } => self.run_tran_result(),
            crate::circuit::AnalysisCmd::Dc { source, start, stop, step } => {
                self.run_dc_sweep_result(source, *start, *stop, *step)
            }
            crate::circuit::AnalysisCmd::Ac { sweep_type, points, fstart, fstop } => {
                self.run_ac_result(*sweep_type, *points, *fstart, *fstop)
            }
            _ => self.run_dc_result(AnalysisType::Op),
        };
        store.add_run(result)
    }

    fn run_ac_result_from_plan(&mut self, plan: &AnalysisPlan) -> RunResult {
        match &plan.cmd {
            crate::circuit::AnalysisCmd::Ac { sweep_type, points, fstart, fstop } => {
                self.run_ac_result(*sweep_type, *points, *fstart, *fstop)
            }
            _ => self.run_dc_result(AnalysisType::Op),
        }
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
            ac_frequencies: Vec::new(),
            ac_solutions: Vec::new(),
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
            let gnd = self.circuit.nodes.gnd_id.0;
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
                // 固定地节点，避免矩阵奇异
                mna.builder.insert(gnd, gnd, 1.0);
                let (ap, ai, ax) = mna.builder.finalize();
                (ap, ai, ax, mna.rhs, mna.builder.n)
            }, self.solver.as_mut());

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
            sweep_var: None,
            sweep_values: Vec::new(),
            sweep_solutions: Vec::new(),
            ac_frequencies: Vec::new(),
            ac_solutions: Vec::new(),
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
                ac_frequencies: Vec::new(),
                ac_solutions: Vec::new(),
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
            ac_frequencies: Vec::new(),
            ac_solutions: Vec::new(),
        }
    }

    /// Run AC (small-signal frequency-domain) analysis.
    ///
    /// This performs:
    /// 1. DC operating point to linearize nonlinear devices
    /// 2. Build complex admittance matrix Y(jω) at each frequency
    /// 3. Solve Y·V = I for complex node voltages
    /// 4. Store magnitude (dB) and phase (degrees) results
    fn run_ac_result(
        &mut self,
        sweep_type: AcSweepType,
        points: usize,
        fstart: f64,
        fstop: f64,
    ) -> RunResult {
        // Step 1: Run DC operating point
        let dc_result = self.run_dc_result(AnalysisType::Op);
        if !matches!(dc_result.status, RunStatus::Converged) {
            return RunResult {
                id: RunId(0),
                analysis: AnalysisType::Ac,
                status: dc_result.status,
                iterations: dc_result.iterations,
                node_names: self.circuit.nodes.id_to_name.clone(),
                solution: Vec::new(),
                message: Some("DC operating point failed".to_string()),
                sweep_var: None,
                sweep_values: Vec::new(),
                sweep_solutions: Vec::new(),
                ac_frequencies: Vec::new(),
                ac_solutions: Vec::new(),
            };
        }
        let dc_solution = dc_result.solution;

        // Step 2: Generate frequency points
        let frequencies = generate_frequency_points(sweep_type, points, fstart, fstop);
        if frequencies.is_empty() {
            return RunResult {
                id: RunId(0),
                analysis: AnalysisType::Ac,
                status: RunStatus::Failed,
                iterations: 0,
                node_names: self.circuit.nodes.id_to_name.clone(),
                solution: Vec::new(),
                message: Some("No frequency points generated".to_string()),
                sweep_var: None,
                sweep_values: Vec::new(),
                sweep_solutions: Vec::new(),
                ac_frequencies: Vec::new(),
                ac_solutions: Vec::new(),
            };
        }

        let node_count = self.circuit.nodes.id_to_name.len();
        let gnd = self.circuit.nodes.gnd_id.0;
        let mut complex_solver = create_complex_solver();
        complex_solver.prepare(node_count);

        let mut ac_frequencies = Vec::with_capacity(frequencies.len());
        let mut ac_solutions = Vec::with_capacity(frequencies.len());
        let mut final_status = RunStatus::Converged;
        let mut final_message = None;

        // Step 3: For each frequency, build and solve the complex MNA system
        for freq in frequencies {
            let omega = 2.0 * std::f64::consts::PI * freq;

            // Build complex MNA matrix
            let mut mna = ComplexMnaBuilder::new(node_count);

            for inst in &self.circuit.instances.instances {
                let stamp = InstanceStamp {
                    instance: inst.clone(),
                };
                let mut ctx = mna.context(omega);
                if let Err(_) = stamp.stamp_ac(&mut ctx, &dc_solution) {
                    // Skip devices that fail to stamp (e.g., missing values)
                    continue;
                }
            }

            // Ground node constraint
            mna.builder.insert(gnd, gnd, Complex64::new(1.0, 0.0));

            let (ap, ai, ax) = mna.builder.finalize();
            let n = mna.builder.n;
            complex_solver.prepare(n);

            let mut x = vec![Complex64::new(0.0, 0.0); n];

            if !complex_solver.solve(&ap, &ai, &ax, &mna.rhs, &mut x) {
                final_status = RunStatus::Failed;
                final_message = Some(format!("AC solve failed at frequency {} Hz", freq));
                break;
            }

            // Convert complex solution to magnitude (dB) and phase (degrees)
            let mut freq_solution = Vec::with_capacity(node_count);
            for i in 0..node_count {
                let v = x[i];
                let mag = v.norm();
                // Convert magnitude to dB (20*log10), handle zero case
                let mag_db = if mag > 1e-30 {
                    20.0 * mag.log10()
                } else {
                    -600.0 // Very small value in dB
                };
                let phase_deg = v.arg() * 180.0 / std::f64::consts::PI;
                freq_solution.push((mag_db, phase_deg));
            }

            ac_frequencies.push(freq);
            ac_solutions.push(freq_solution);
        }

        RunResult {
            id: RunId(0),
            analysis: AnalysisType::Ac,
            status: final_status,
            iterations: ac_frequencies.len(),
            node_names: self.circuit.nodes.id_to_name.clone(),
            solution: dc_solution,
            message: final_message,
            sweep_var: None,
            sweep_values: Vec::new(),
            sweep_solutions: Vec::new(),
            ac_frequencies,
            ac_solutions,
        }
    }
}

/// Generate frequency points for AC sweep.
fn generate_frequency_points(sweep_type: AcSweepType, points: usize, fstart: f64, fstop: f64) -> Vec<f64> {
    if points == 0 || fstart <= 0.0 || fstop <= 0.0 || fstart >= fstop {
        return Vec::new();
    }

    match sweep_type {
        AcSweepType::Dec => {
            // Logarithmic sweep with N points per decade
            let decades = (fstop / fstart).log10();
            let total_points = (points as f64 * decades).ceil() as usize + 1;
            let log_start = fstart.log10();
            let log_stop = fstop.log10();
            let log_step = (log_stop - log_start) / (total_points.saturating_sub(1).max(1)) as f64;

            (0..total_points)
                .map(|i| 10_f64.powf(log_start + i as f64 * log_step))
                .collect()
        }
        AcSweepType::Oct => {
            // Logarithmic sweep with N points per octave
            let octaves = (fstop / fstart).log2();
            let total_points = (points as f64 * octaves).ceil() as usize + 1;
            let log_start = fstart.ln();
            let log_stop = fstop.ln();
            let log_step = (log_stop - log_start) / (total_points.saturating_sub(1).max(1)) as f64;

            (0..total_points)
                .map(|i| (log_start + i as f64 * log_step).exp())
                .collect()
        }
        AcSweepType::Lin => {
            // Linear sweep with N total points
            let step = (fstop - fstart) / (points.saturating_sub(1).max(1)) as f64;
            (0..points)
                .map(|i| fstart + i as f64 * step)
                .collect()
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
