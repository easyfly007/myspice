use crate::circuit::{DeviceKind, Instance, PolySpec};
use crate::complex_mna::ComplexStampContext;
use crate::mna::StampContext;
use num_complex::Complex64;
use std::collections::HashMap;

/// Evaluate a POLY(n) polynomial and its derivatives
///
/// For POLY(1): f(x) = c0 + c1*x + c2*x² + c3*x³ + ...
/// For POLY(2): f(x1,x2) = c0 + c1*x1 + c2*x2 + c3*x1*x2 + c4*x1² + c5*x2² + ...
///
/// Returns (value, partial_derivatives)
fn evaluate_poly(poly: &PolySpec, inputs: &[f64]) -> (f64, Vec<f64>) {
    let n = poly.degree;
    let coeffs = &poly.coeffs;

    if n == 0 || inputs.len() < n {
        return (coeffs.first().copied().unwrap_or(0.0), vec![0.0; n]);
    }

    match n {
        1 => {
            // POLY(1): f(x) = c0 + c1*x + c2*x² + c3*x³ + ...
            let x = inputs[0];
            let mut value = 0.0;
            let mut x_power = 1.0;

            // Compute value
            for &c in coeffs.iter() {
                value += c * x_power;
                x_power *= x;
            }

            // Compute derivative: df/dx = c1 + 2*c2*x + 3*c3*x² + ...
            let mut deriv = 0.0;
            let mut x_power = 1.0;
            for (i, &c) in coeffs.iter().enumerate().skip(1) {
                deriv += c * (i as f64) * x_power;
                x_power *= x;
            }

            (value, vec![deriv])
        }
        2 => {
            // POLY(2): f(x1,x2) = c0 + c1*x1 + c2*x2 + c3*x1*x2 + c4*x1² + c5*x2² + c6*x1²*x2 + ...
            // Simplified: only use up to cross term for now
            let x1 = inputs[0];
            let x2 = inputs[1];

            let c0 = coeffs.get(0).copied().unwrap_or(0.0);
            let c1 = coeffs.get(1).copied().unwrap_or(0.0);
            let c2 = coeffs.get(2).copied().unwrap_or(0.0);
            let c3 = coeffs.get(3).copied().unwrap_or(0.0);
            let c4 = coeffs.get(4).copied().unwrap_or(0.0);
            let c5 = coeffs.get(5).copied().unwrap_or(0.0);

            // f = c0 + c1*x1 + c2*x2 + c3*x1*x2 + c4*x1² + c5*x2²
            let value = c0 + c1*x1 + c2*x2 + c3*x1*x2 + c4*x1*x1 + c5*x2*x2;

            // df/dx1 = c1 + c3*x2 + 2*c4*x1
            let deriv1 = c1 + c3*x2 + 2.0*c4*x1;

            // df/dx2 = c2 + c3*x1 + 2*c5*x2
            let deriv2 = c2 + c3*x1 + 2.0*c5*x2;

            (value, vec![deriv1, deriv2])
        }
        _ => {
            // General case: linear terms only for simplicity
            // f = c0 + c1*x1 + c2*x2 + c3*x3 + ...
            let c0 = coeffs.get(0).copied().unwrap_or(0.0);
            let mut value = c0;
            let mut derivs = Vec::with_capacity(n);

            for i in 0..n {
                let c = coeffs.get(i + 1).copied().unwrap_or(0.0);
                let x = inputs.get(i).copied().unwrap_or(0.0);
                value += c * x;
                derivs.push(c);
            }

            (value, derivs)
        }
    }
}

#[derive(Debug, Clone)]
pub enum StampError {
    MissingValue,
    InvalidNodes,
}

pub trait DeviceStamp {
    fn stamp_dc(&self, ctx: &mut StampContext, x: Option<&[f64]>) -> Result<(), StampError>;
    fn stamp_tran(
        &self,
        ctx: &mut StampContext,
        x: Option<&[f64]>,
        dt: f64,
        state: &mut TransientState,
    ) -> Result<(), StampError>;
    fn stamp_ac(
        &self,
        ctx: &mut ComplexStampContext,
        dc_solution: &[f64],
    ) -> Result<(), StampError>;
}

#[derive(Debug, Clone)]
pub struct InstanceStamp {
    pub instance: Instance,
}

impl DeviceStamp for InstanceStamp {
    fn stamp_dc(&self, ctx: &mut StampContext, x: Option<&[f64]>) -> Result<(), StampError> {
        match self.instance.kind {
            DeviceKind::R => stamp_resistor(ctx, &self.instance),
            DeviceKind::I => stamp_current(ctx, &self.instance),
            DeviceKind::V => stamp_voltage(ctx, &self.instance),
            DeviceKind::D => stamp_diode(ctx, &self.instance, x),
            DeviceKind::M => stamp_mos(ctx, &self.instance, x),
            DeviceKind::L => stamp_inductor_dc(ctx, &self.instance),
            DeviceKind::C => Ok(()), // Capacitor is open circuit in DC
            DeviceKind::E => stamp_vcvs(ctx, &self.instance, x),
            DeviceKind::G => stamp_vccs(ctx, &self.instance, x),
            DeviceKind::F => stamp_cccs(ctx, &self.instance, x),
            DeviceKind::H => stamp_ccvs(ctx, &self.instance, x),
            DeviceKind::X => Ok(()), // Subcircuit instances are already expanded
        }
    }

    fn stamp_tran(
        &self,
        ctx: &mut StampContext,
        x: Option<&[f64]>,
        dt: f64,
        state: &mut TransientState,
    ) -> Result<(), StampError> {
        match self.instance.kind {
            DeviceKind::C => stamp_capacitor_tran(ctx, &self.instance, x, dt, state),
            DeviceKind::L => stamp_inductor_tran(ctx, &self.instance, x, dt, state),
            _ => self.stamp_dc(ctx, x),
        }
    }

    fn stamp_ac(
        &self,
        ctx: &mut ComplexStampContext,
        dc_solution: &[f64],
    ) -> Result<(), StampError> {
        match self.instance.kind {
            DeviceKind::R => stamp_resistor_ac(ctx, &self.instance),
            DeviceKind::C => stamp_capacitor_ac(ctx, &self.instance),
            DeviceKind::L => stamp_inductor_ac(ctx, &self.instance),
            DeviceKind::V => stamp_voltage_ac(ctx, &self.instance),
            DeviceKind::I => stamp_current_ac(ctx, &self.instance),
            DeviceKind::D => stamp_diode_ac(ctx, &self.instance, dc_solution),
            DeviceKind::M => stamp_mos_ac(ctx, &self.instance, dc_solution),
            DeviceKind::E => stamp_vcvs_ac(ctx, &self.instance, dc_solution),
            DeviceKind::G => stamp_vccs_ac(ctx, &self.instance, dc_solution),
            DeviceKind::F => stamp_cccs_ac(ctx, &self.instance, dc_solution),
            DeviceKind::H => stamp_ccvs_ac(ctx, &self.instance, dc_solution),
            DeviceKind::X => Ok(()), // Subcircuit instances are already expanded
        }
    }
}

fn stamp_resistor(ctx: &mut StampContext, inst: &Instance) -> Result<(), StampError> {
    if inst.nodes.len() != 2 {
        return Err(StampError::InvalidNodes);
    }
    let value = inst
        .value
        .as_deref()
        .and_then(parse_number_with_suffix)
        .ok_or(StampError::MissingValue)?;
    let g = 1.0 / value;
    let a = inst.nodes[0].0;
    let b = inst.nodes[1].0;
    ctx.add(a, a, g);
    ctx.add(b, b, g);
    ctx.add(a, b, -g);
    ctx.add(b, a, -g);
    Ok(())
}

fn stamp_current(ctx: &mut StampContext, inst: &Instance) -> Result<(), StampError> {
    if inst.nodes.len() != 2 {
        return Err(StampError::InvalidNodes);
    }
    let value = inst
        .value
        .as_deref()
        .and_then(parse_number_with_suffix)
        .ok_or(StampError::MissingValue)?;
    let value = value * ctx.source_scale;
    let a = inst.nodes[0].0;
    let b = inst.nodes[1].0;
    ctx.add_rhs(a, -value);
    ctx.add_rhs(b, value);
    Ok(())
}

fn stamp_voltage(ctx: &mut StampContext, inst: &Instance) -> Result<(), StampError> {
    if inst.nodes.len() != 2 {
        return Err(StampError::InvalidNodes);
    }
    let value = inst
        .value
        .as_deref()
        .and_then(parse_number_with_suffix)
        .ok_or(StampError::MissingValue)?;
    let value = value * ctx.source_scale;
    let a = inst.nodes[0].0;
    let b = inst.nodes[1].0;
    let k = ctx.allocate_aux(&inst.name);
    ctx.add(a, k, 1.0);
    ctx.add(b, k, -1.0);
    ctx.add(k, a, 1.0);
    ctx.add(k, b, -1.0);
    ctx.add_rhs(k, value);
    Ok(())
}

fn stamp_diode(
    ctx: &mut StampContext,
    inst: &Instance,
    x: Option<&[f64]>,
) -> Result<(), StampError> {
    if inst.nodes.len() != 2 {
        return Err(StampError::InvalidNodes);
    }
    let a = inst.nodes[0].0;
    let b = inst.nodes[1].0;
    let gmin = if ctx.gmin > 0.0 { ctx.gmin } else { 1e-12 };
    let isat = param_value(&inst.params, &["is"]).unwrap_or(1e-14);
    let emission = param_value(&inst.params, &["n", "nj"]).unwrap_or(1.0);
    let vt = 0.02585 * emission;
    if let Some(x) = x {
        let va = x.get(a).copied().unwrap_or(0.0);
        let vb = x.get(b).copied().unwrap_or(0.0);
        let vd = va - vb;
        let exp_vd = (vd / vt).exp();
        let id = isat * (exp_vd - 1.0);
        let gd = (isat / vt) * exp_vd;
        let g = gd.max(gmin);
        let ieq = id - gd * vd;
        ctx.add(a, a, g);
        ctx.add(b, b, g);
        ctx.add(a, b, -g);
        ctx.add(b, a, -g);
        ctx.add_rhs(a, -ieq);
        ctx.add_rhs(b, ieq);
        return Ok(());
    }
    ctx.add(a, a, gmin);
    ctx.add(b, b, gmin);
    ctx.add(a, b, -gmin);
    ctx.add(b, a, -gmin);
    Ok(())
}

fn stamp_mos(ctx: &mut StampContext, inst: &Instance, x: Option<&[f64]>) -> Result<(), StampError> {
    if inst.nodes.len() < 4 {
        return Err(StampError::InvalidNodes);
    }
    let drain = inst.nodes[0].0;
    let gate = inst.nodes[1].0;
    let source = inst.nodes[2].0;
    let bulk = inst.nodes[3].0;
    let gmin = if ctx.gmin > 0.0 { ctx.gmin } else { 1e-12 };

    // Parse model level (default to 49 for BSIM3)
    let level = param_value(&inst.params, &["level"]).unwrap_or(49.0) as u32;

    // Determine NMOS/PMOS from model type
    let is_pmos = if let Some(t) = inst.params.get("type") {
        let t_lower = t.to_ascii_lowercase();
        t_lower.contains("pmos") || t_lower == "p"
    } else if inst.params.contains_key("pmos") {
        true
    } else {
        false
    };

    // Build BSIM parameters from instance params
    let params = sim_devices::bsim::build_bsim_params(&inst.params, level, is_pmos);

    // Get device dimensions
    let w = param_value(&inst.params, &["w"]).unwrap_or(1e-6);
    let l = param_value(&inst.params, &["l"]).unwrap_or(1e-6);

    // Temperature (default 27C = 300.15K)
    let temp = param_value(&inst.params, &["temp"]).unwrap_or(300.15);

    // BSIM4: Stress parameters (SA/SB distance to STI)
    let sa = param_value(&inst.params, &["sa"]).unwrap_or(0.0);
    let sb = param_value(&inst.params, &["sb"]).unwrap_or(0.0);

    if let Some(x) = x {
        let vd = x.get(drain).copied().unwrap_or(0.0);
        let vg = x.get(gate).copied().unwrap_or(0.0);
        let vs = x.get(source).copied().unwrap_or(0.0);
        let vb = x.get(bulk).copied().unwrap_or(0.0);

        // Use BSIM4 evaluator for Level 54, BSIM3 for others
        if level == 54 {
            // BSIM4: Full evaluation with stress and additional currents
            let output = sim_devices::bsim::evaluate_mos_bsim4(
                &params, w, l, vd, vg, vs, vb, temp, sa, sb
            );

            let gm = output.base.gm;
            let gds = output.base.gds.max(gmin);
            let gmbs = output.base.gmbs;
            let ieq = output.base.ieq;

            // Stamp gds (output conductance between drain and source)
            ctx.add(drain, drain, gds);
            ctx.add(source, source, gds);
            ctx.add(drain, source, -gds);
            ctx.add(source, drain, -gds);

            // Stamp gm (transconductance: current controlled by Vgs)
            ctx.add(drain, gate, gm);
            ctx.add(drain, source, -gm);
            ctx.add(source, gate, -gm);
            ctx.add(source, source, gm);

            // Stamp gmbs (body transconductance: current controlled by Vbs)
            if gmbs.abs() > gmin * 0.01 {
                ctx.add(drain, bulk, gmbs);
                ctx.add(drain, source, -gmbs);
                ctx.add(source, bulk, -gmbs);
                ctx.add(source, source, gmbs);
            }

            // Stamp equivalent current source for Ids
            ctx.add_rhs(drain, -ieq);
            ctx.add_rhs(source, ieq);

            // BSIM4: Substrate current (impact ionization)
            // Isub flows from drain to bulk
            if output.isub.abs() > gmin && output.gsub > gmin * 0.01 {
                // Stamp gsub (substrate conductance)
                ctx.add(drain, drain, output.gsub);
                ctx.add(bulk, bulk, output.gsub);
                ctx.add(drain, bulk, -output.gsub);
                ctx.add(bulk, drain, -output.gsub);

                // Equivalent current for Isub
                let isub_eq = output.isub - output.gsub * (vd - vb);
                ctx.add_rhs(drain, -isub_eq);
                ctx.add_rhs(bulk, isub_eq);
            }

            // BSIM4: Gate tunneling currents
            // Igs flows from gate to source
            if output.igs.abs() > gmin && output.gigs > gmin * 0.01 {
                ctx.add(gate, gate, output.gigs);
                ctx.add(source, source, output.gigs);
                ctx.add(gate, source, -output.gigs);
                ctx.add(source, gate, -output.gigs);

                let igs_eq = output.igs - output.gigs * (vg - vs);
                ctx.add_rhs(gate, -igs_eq);
                ctx.add_rhs(source, igs_eq);
            }

            // Igd flows from gate to drain
            if output.igd.abs() > gmin && output.gigd > gmin * 0.01 {
                ctx.add(gate, gate, output.gigd);
                ctx.add(drain, drain, output.gigd);
                ctx.add(gate, drain, -output.gigd);
                ctx.add(drain, gate, -output.gigd);

                let igd_eq = output.igd - output.gigd * (vg - vd);
                ctx.add_rhs(gate, -igd_eq);
                ctx.add_rhs(drain, igd_eq);
            }

            return Ok(());
        }

        // BSIM3 or Level 1: Use standard evaluator
        let output = sim_devices::bsim::evaluate_mos(
            &params, w, l, vd, vg, vs, vb, temp
        );

        let gm = output.gm;
        let gds = output.gds.max(gmin);
        let gmbs = output.gmbs;
        let ieq = output.ieq;

        // Stamp gds (output conductance between drain and source)
        ctx.add(drain, drain, gds);
        ctx.add(source, source, gds);
        ctx.add(drain, source, -gds);
        ctx.add(source, drain, -gds);

        // Stamp gm (transconductance: current controlled by Vgs)
        ctx.add(drain, gate, gm);
        ctx.add(drain, source, -gm);
        ctx.add(source, gate, -gm);
        ctx.add(source, source, gm);

        // Stamp gmbs (body transconductance: current controlled by Vbs)
        if gmbs.abs() > gmin * 0.01 {
            ctx.add(drain, bulk, gmbs);
            ctx.add(drain, source, -gmbs);
            ctx.add(source, bulk, -gmbs);
            ctx.add(source, source, gmbs);
        }

        // Stamp equivalent current source
        ctx.add_rhs(drain, -ieq);
        ctx.add_rhs(source, ieq);
        return Ok(());
    }

    // Initial guess: add small conductance for convergence
    ctx.add(drain, drain, gmin);
    ctx.add(source, source, gmin);
    ctx.add(drain, source, -gmin);
    ctx.add(source, drain, -gmin);
    Ok(())
}

pub fn debug_dump_stamp(instance: &Instance) {
    println!(
        "stamp: name={} kind={:?} nodes={} value={:?}",
        instance.name,
        instance.kind,
        instance.nodes.len(),
        instance.value
    );
}

pub fn update_transient_state(instances: &[Instance], x: &[f64], state: &mut TransientState) {
    for inst in instances {
        match inst.kind {
            DeviceKind::C => {
                if inst.nodes.len() == 2 {
                    let a = inst.nodes[0].0;
                    let b = inst.nodes[1].0;
                    let va = x.get(a).copied().unwrap_or(0.0);
                    let vb = x.get(b).copied().unwrap_or(0.0);
                    state.cap_voltage.insert(inst.name.clone(), va - vb);
                }
            }
            DeviceKind::L => {
                if let Some(aux) = state.ind_aux.get(&inst.name) {
                    if let Some(current) = x.get(*aux).copied() {
                        state.ind_current.insert(inst.name.clone(), current);
                    }
                }
            }
            _ => {}
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct TransientState {
    pub cap_voltage: HashMap<String, f64>,
    pub ind_current: HashMap<String, f64>,
    pub ind_aux: HashMap<String, usize>,
}

fn parse_number_with_suffix(token: &str) -> Option<f64> {
    let lower = token.to_ascii_lowercase();
    let trimmed = lower.trim();
    let (num_str, multiplier) = if trimmed.ends_with("meg") {
        (&trimmed[..trimmed.len() - 3], 1e6)
    } else {
        let (value_part, suffix) = trimmed.split_at(trimmed.len().saturating_sub(1));
        match suffix {
            "f" => (value_part, 1e-15),
            "p" => (value_part, 1e-12),
            "n" => (value_part, 1e-9),
            "u" => (value_part, 1e-6),
            "m" => (value_part, 1e-3),
            "k" => (value_part, 1e3),
            "g" => (value_part, 1e9),
            "t" => (value_part, 1e12),
            _ => (trimmed, 1.0),
        }
    };

    if let Ok(num) = num_str.parse::<f64>() {
        Some(num * multiplier)
    } else {
        None
    }
}

fn param_value(params: &HashMap<String, String>, keys: &[&str]) -> Option<f64> {
    for key in keys {
        let key = key.to_ascii_lowercase();
        if let Some(value) = params.get(&key) {
            if let Some(num) = parse_number_with_suffix(value).or_else(|| value.parse().ok()) {
                return Some(num);
            }
        }
    }
    None
}

fn stamp_capacitor_tran(
    ctx: &mut StampContext,
    inst: &Instance,
    x: Option<&[f64]>,
    dt: f64,
    state: &mut TransientState,
) -> Result<(), StampError> {
    if inst.nodes.len() != 2 {
        return Err(StampError::InvalidNodes);
    }
    let c = inst
        .value
        .as_deref()
        .and_then(parse_number_with_suffix)
        .ok_or(StampError::MissingValue)?;
    let g = c / dt;
    let a = inst.nodes[0].0;
    let b = inst.nodes[1].0;
    let v_prev = *state.cap_voltage.get(&inst.name).unwrap_or(&0.0);
    let ieq = g * v_prev;
    ctx.add(a, a, g);
    ctx.add(b, b, g);
    ctx.add(a, b, -g);
    ctx.add(b, a, -g);
    ctx.add_rhs(a, -ieq);
    ctx.add_rhs(b, ieq);
    let _ = x;
    Ok(())
}

fn stamp_inductor_tran(
    ctx: &mut StampContext,
    inst: &Instance,
    x: Option<&[f64]>,
    dt: f64,
    state: &mut TransientState,
) -> Result<(), StampError> {
    if inst.nodes.len() != 2 {
        return Err(StampError::InvalidNodes);
    }
    let l = inst
        .value
        .as_deref()
        .and_then(parse_number_with_suffix)
        .ok_or(StampError::MissingValue)?;
    let a = inst.nodes[0].0;
    let b = inst.nodes[1].0;
    let k = *state
        .ind_aux
        .entry(inst.name.clone())
        .or_insert_with(|| ctx.allocate_aux(&inst.name));
    let g = -(l / dt);
    let i_prev = *state.ind_current.get(&inst.name).unwrap_or(&0.0);
    ctx.add(a, k, 1.0);
    ctx.add(b, k, -1.0);
    ctx.add(k, a, 1.0);
    ctx.add(k, b, -1.0);
    ctx.add(k, k, g);
    ctx.add_rhs(k, g * i_prev);
    let _ = x;
    Ok(())
}

fn stamp_inductor_dc(ctx: &mut StampContext, inst: &Instance) -> Result<(), StampError> {
    if inst.nodes.len() != 2 {
        return Err(StampError::InvalidNodes);
    }
    let gshort = 1e9;
    let a = inst.nodes[0].0;
    let b = inst.nodes[1].0;
    ctx.add(a, a, gshort);
    ctx.add(b, b, gshort);
    ctx.add(a, b, -gshort);
    ctx.add(b, a, -gshort);
    Ok(())
}

/// Voltage Controlled Voltage Source (VCVS)
/// Vout = E * Vin where E is the gain
/// nodes: [out+, out-, in+, in-] for simple case
/// nodes: [out+, out-] with POLY for polynomial case
fn stamp_vcvs(ctx: &mut StampContext, inst: &Instance, x: Option<&[f64]>) -> Result<(), StampError> {
    // Check for POLY syntax
    if let Some(ref poly) = inst.poly {
        return stamp_vcvs_poly(ctx, inst, poly, x);
    }

    // Simple linear case: Vout = gain * Vin
    if inst.nodes.len() != 4 {
        return Err(StampError::InvalidNodes);
    }
    let gain = inst
        .value
        .as_deref()
        .and_then(parse_number_with_suffix)
        .ok_or(StampError::MissingValue)?;

    let out_p = inst.nodes[0].0;
    let out_n = inst.nodes[1].0;
    let in_p = inst.nodes[2].0;
    let in_n = inst.nodes[3].0;

    // Allocate auxiliary variable for output current
    let k = ctx.allocate_aux(&inst.name);

    // KCL at output nodes: I flows from out+ to out-
    ctx.add(out_p, k, 1.0);
    ctx.add(out_n, k, -1.0);

    // Constitutive relation: V(out+) - V(out-) = E * (V(in+) - V(in-))
    ctx.add(k, out_p, 1.0);
    ctx.add(k, out_n, -1.0);
    ctx.add(k, in_p, -gain);
    ctx.add(k, in_n, gain);

    Ok(())
}

/// VCVS with POLY syntax
/// Vout = f(Vin1, Vin2, ...) where f is a polynomial
fn stamp_vcvs_poly(
    ctx: &mut StampContext,
    inst: &Instance,
    poly: &PolySpec,
    x: Option<&[f64]>,
) -> Result<(), StampError> {
    if inst.nodes.len() < 2 {
        return Err(StampError::InvalidNodes);
    }

    let out_p = inst.nodes[0].0;
    let out_n = inst.nodes[1].0;

    // Allocate auxiliary variable for output current
    let k = ctx.allocate_aux(&inst.name);

    // KCL at output nodes
    ctx.add(out_p, k, 1.0);
    ctx.add(out_n, k, -1.0);

    // Get control voltages from solution vector
    let mut control_voltages: Vec<f64> = Vec::with_capacity(poly.degree);
    if let Some(x) = x {
        for &(pos, neg) in &poly.control_nodes {
            let v_pos = x.get(pos).copied().unwrap_or(0.0);
            let v_neg = x.get(neg).copied().unwrap_or(0.0);
            control_voltages.push(v_pos - v_neg);
        }
    } else {
        control_voltages.resize(poly.degree, 0.0);
    }

    // Evaluate polynomial and derivatives
    let (f_value, derivs) = evaluate_poly(poly, &control_voltages);

    // Check if purely linear (only c0 and c1..cn terms, no higher order)
    let is_linear = poly.coeffs.len() <= poly.degree + 1;

    if is_linear {
        // Linear case: V(out) = c0 + c1*V1 + c2*V2 + ...
        // Stamp directly into matrix
        ctx.add(k, out_p, 1.0);
        ctx.add(k, out_n, -1.0);

        // Stamp the constant term
        let c0 = poly.coeffs.first().copied().unwrap_or(0.0);
        ctx.add_rhs(k, c0);

        // Stamp the linear coefficients
        for (i, &(pos, neg)) in poly.control_nodes.iter().enumerate() {
            let c = poly.coeffs.get(i + 1).copied().unwrap_or(0.0);
            ctx.add(k, pos, -c);
            ctx.add(k, neg, c);
        }
    } else {
        // Nonlinear case: use Newton-Raphson linearization
        // f(x) ≈ f(x0) + f'(x0) * (x - x0)
        // V(out) = f(V1, V2, ...) is linearized as:
        // V(out) = f(V1_0, V2_0, ...) + df/dV1 * (V1 - V1_0) + df/dV2 * (V2 - V2_0) + ...
        // Rearranging: V(out) - df/dV1 * V1 - df/dV2 * V2 - ... = f(x0) - df/dV1 * V1_0 - ...

        ctx.add(k, out_p, 1.0);
        ctx.add(k, out_n, -1.0);

        // Stamp derivatives as coefficients for control nodes
        for (i, &(pos, neg)) in poly.control_nodes.iter().enumerate() {
            if let Some(&deriv) = derivs.get(i) {
                ctx.add(k, pos, -deriv);
                ctx.add(k, neg, deriv);
            }
        }

        // RHS: f(x0) - sum(df/dVi * Vi_0)
        let mut rhs = f_value;
        for (i, &v) in control_voltages.iter().enumerate() {
            if let Some(&deriv) = derivs.get(i) {
                rhs -= deriv * v;
            }
        }
        ctx.add_rhs(k, rhs);
    }

    Ok(())
}

/// Voltage Controlled Current Source (VCCS)
/// Iout = G * Vin where G is the transconductance
/// nodes: [out+, out-, in+, in-] for simple case
/// nodes: [out+, out-] with POLY for polynomial case
fn stamp_vccs(ctx: &mut StampContext, inst: &Instance, x: Option<&[f64]>) -> Result<(), StampError> {
    // Check for POLY syntax
    if let Some(ref poly) = inst.poly {
        return stamp_vccs_poly(ctx, inst, poly, x);
    }

    // Simple linear case
    if inst.nodes.len() != 4 {
        return Err(StampError::InvalidNodes);
    }
    let gm = inst
        .value
        .as_deref()
        .and_then(parse_number_with_suffix)
        .ok_or(StampError::MissingValue)?;

    let out_p = inst.nodes[0].0;
    let out_n = inst.nodes[1].0;
    let in_p = inst.nodes[2].0;
    let in_n = inst.nodes[3].0;

    // Current flows from out+ to out-, controlled by V(in+) - V(in-)
    // I = G * (V(in+) - V(in-))
    ctx.add(out_p, in_p, gm);
    ctx.add(out_p, in_n, -gm);
    ctx.add(out_n, in_p, -gm);
    ctx.add(out_n, in_n, gm);

    Ok(())
}

/// VCCS with POLY syntax
/// Iout = f(Vin1, Vin2, ...) where f is a polynomial
fn stamp_vccs_poly(
    ctx: &mut StampContext,
    inst: &Instance,
    poly: &PolySpec,
    x: Option<&[f64]>,
) -> Result<(), StampError> {
    if inst.nodes.len() < 2 {
        return Err(StampError::InvalidNodes);
    }

    let out_p = inst.nodes[0].0;
    let out_n = inst.nodes[1].0;

    // Get control voltages from solution vector
    let mut control_voltages: Vec<f64> = Vec::with_capacity(poly.degree);
    if let Some(x) = x {
        for &(pos, neg) in &poly.control_nodes {
            let v_pos = x.get(pos).copied().unwrap_or(0.0);
            let v_neg = x.get(neg).copied().unwrap_or(0.0);
            control_voltages.push(v_pos - v_neg);
        }
    } else {
        control_voltages.resize(poly.degree, 0.0);
    }

    // Evaluate polynomial and derivatives
    let (f_value, derivs) = evaluate_poly(poly, &control_voltages);

    // Check if purely linear
    let is_linear = poly.coeffs.len() <= poly.degree + 1;

    if is_linear {
        // Linear case: I = c0 + c1*V1 + c2*V2 + ...
        // Stamp constant term as equivalent current source
        let c0 = poly.coeffs.first().copied().unwrap_or(0.0);
        ctx.add_rhs(out_p, -c0);
        ctx.add_rhs(out_n, c0);

        // Stamp the linear coefficients (transconductances)
        for (i, &(pos, neg)) in poly.control_nodes.iter().enumerate() {
            let gm = poly.coeffs.get(i + 1).copied().unwrap_or(0.0);
            ctx.add(out_p, pos, gm);
            ctx.add(out_p, neg, -gm);
            ctx.add(out_n, pos, -gm);
            ctx.add(out_n, neg, gm);
        }
    } else {
        // Nonlinear case: Newton-Raphson linearization
        // I = f(V1, V2, ...) linearized as:
        // I = f(x0) + df/dV1 * (V1 - V1_0) + df/dV2 * (V2 - V2_0) + ...

        // Stamp derivatives as transconductances
        for (i, &(pos, neg)) in poly.control_nodes.iter().enumerate() {
            if let Some(&deriv) = derivs.get(i) {
                ctx.add(out_p, pos, deriv);
                ctx.add(out_p, neg, -deriv);
                ctx.add(out_n, pos, -deriv);
                ctx.add(out_n, neg, deriv);
            }
        }

        // Equivalent current source: I_eq = f(x0) - sum(df/dVi * Vi_0)
        let mut i_eq = f_value;
        for (i, &v) in control_voltages.iter().enumerate() {
            if let Some(&deriv) = derivs.get(i) {
                i_eq -= deriv * v;
            }
        }
        ctx.add_rhs(out_p, -i_eq);
        ctx.add_rhs(out_n, i_eq);
    }

    Ok(())
}

/// Current Controlled Current Source (CCCS)
/// Iout = F * Icontrol where F is the current gain
/// nodes: [out+, out-], control: name of controlling voltage source
fn stamp_cccs(ctx: &mut StampContext, inst: &Instance, x: Option<&[f64]>) -> Result<(), StampError> {
    // Check for POLY syntax
    if let Some(ref poly) = inst.poly {
        return stamp_cccs_poly(ctx, inst, poly, x);
    }

    // Simple linear case
    if inst.nodes.len() != 2 {
        return Err(StampError::InvalidNodes);
    }
    let gain = inst
        .value
        .as_deref()
        .and_then(parse_number_with_suffix)
        .ok_or(StampError::MissingValue)?;

    let out_p = inst.nodes[0].0;
    let out_n = inst.nodes[1].0;

    // Get the controlling voltage source's auxiliary variable
    let control_name = inst.control.as_ref().ok_or(StampError::MissingValue)?;
    let control_aux = ctx
        .aux
        .name_to_id
        .get(control_name)
        .copied()
        .ok_or(StampError::MissingValue)?;
    let k_control = ctx.node_count + control_aux;

    // Current flows from out+ to out-, controlled by current through controlling source
    // I = F * I_control
    ctx.add(out_p, k_control, gain);
    ctx.add(out_n, k_control, -gain);

    Ok(())
}

/// CCCS with POLY syntax
/// Iout = f(I1, I2, ...) where f is a polynomial of control currents
fn stamp_cccs_poly(
    ctx: &mut StampContext,
    inst: &Instance,
    poly: &PolySpec,
    x: Option<&[f64]>,
) -> Result<(), StampError> {
    if inst.nodes.len() < 2 {
        return Err(StampError::InvalidNodes);
    }

    let out_p = inst.nodes[0].0;
    let out_n = inst.nodes[1].0;

    // Get auxiliary variable indices for control sources
    let mut control_aux_indices: Vec<usize> = Vec::with_capacity(poly.degree);
    for source_name in &poly.control_sources {
        if let Some(&aux_id) = ctx.aux.name_to_id.get(source_name) {
            control_aux_indices.push(ctx.node_count + aux_id);
        } else {
            // Control source not yet stamped - this shouldn't happen if ordering is correct
            return Err(StampError::MissingValue);
        }
    }

    // Get control currents from solution vector
    let mut control_currents: Vec<f64> = Vec::with_capacity(poly.degree);
    if let Some(x) = x {
        for &aux_idx in &control_aux_indices {
            let i = x.get(aux_idx).copied().unwrap_or(0.0);
            control_currents.push(i);
        }
    } else {
        control_currents.resize(poly.degree, 0.0);
    }

    // Evaluate polynomial and derivatives
    let (f_value, derivs) = evaluate_poly(poly, &control_currents);

    // Check if purely linear
    let is_linear = poly.coeffs.len() <= poly.degree + 1;

    if is_linear {
        // Linear case: I = c0 + c1*I1 + c2*I2 + ...
        let c0 = poly.coeffs.first().copied().unwrap_or(0.0);
        ctx.add_rhs(out_p, -c0);
        ctx.add_rhs(out_n, c0);

        // Stamp linear coefficients (current gains)
        for (i, &aux_idx) in control_aux_indices.iter().enumerate() {
            let gain = poly.coeffs.get(i + 1).copied().unwrap_or(0.0);
            ctx.add(out_p, aux_idx, gain);
            ctx.add(out_n, aux_idx, -gain);
        }
    } else {
        // Nonlinear case: Newton-Raphson linearization
        // Stamp derivatives as gains
        for (i, &aux_idx) in control_aux_indices.iter().enumerate() {
            if let Some(&deriv) = derivs.get(i) {
                ctx.add(out_p, aux_idx, deriv);
                ctx.add(out_n, aux_idx, -deriv);
            }
        }

        // Equivalent current source
        let mut i_eq = f_value;
        for (i, &curr) in control_currents.iter().enumerate() {
            if let Some(&deriv) = derivs.get(i) {
                i_eq -= deriv * curr;
            }
        }
        ctx.add_rhs(out_p, -i_eq);
        ctx.add_rhs(out_n, i_eq);
    }

    Ok(())
}

/// Current Controlled Voltage Source (CCVS)
/// Vout = H * Icontrol where H is the transresistance
/// nodes: [out+, out-], control: name of controlling voltage source
fn stamp_ccvs(ctx: &mut StampContext, inst: &Instance, x: Option<&[f64]>) -> Result<(), StampError> {
    // Check for POLY syntax
    if let Some(ref poly) = inst.poly {
        return stamp_ccvs_poly(ctx, inst, poly, x);
    }

    // Simple linear case
    if inst.nodes.len() != 2 {
        return Err(StampError::InvalidNodes);
    }
    let gain = inst
        .value
        .as_deref()
        .and_then(parse_number_with_suffix)
        .ok_or(StampError::MissingValue)?;

    let out_p = inst.nodes[0].0;
    let out_n = inst.nodes[1].0;

    // Get the controlling voltage source's auxiliary variable
    let control_name = inst.control.as_ref().ok_or(StampError::MissingValue)?;
    let control_aux = ctx
        .aux
        .name_to_id
        .get(control_name)
        .copied()
        .ok_or(StampError::MissingValue)?;
    let k_control = ctx.node_count + control_aux;

    // Allocate auxiliary variable for output current
    let k = ctx.allocate_aux(&inst.name);

    // KCL at output nodes
    ctx.add(out_p, k, 1.0);
    ctx.add(out_n, k, -1.0);

    // Constitutive relation: V(out+) - V(out-) = H * I_control
    ctx.add(k, out_p, 1.0);
    ctx.add(k, out_n, -1.0);
    ctx.add(k, k_control, -gain);

    Ok(())
}

/// CCVS with POLY syntax
/// Vout = f(I1, I2, ...) where f is a polynomial of control currents
fn stamp_ccvs_poly(
    ctx: &mut StampContext,
    inst: &Instance,
    poly: &PolySpec,
    x: Option<&[f64]>,
) -> Result<(), StampError> {
    if inst.nodes.len() < 2 {
        return Err(StampError::InvalidNodes);
    }

    let out_p = inst.nodes[0].0;
    let out_n = inst.nodes[1].0;

    // Allocate auxiliary variable for output current
    let k = ctx.allocate_aux(&inst.name);

    // KCL at output nodes
    ctx.add(out_p, k, 1.0);
    ctx.add(out_n, k, -1.0);

    // Get auxiliary variable indices for control sources
    let mut control_aux_indices: Vec<usize> = Vec::with_capacity(poly.degree);
    for source_name in &poly.control_sources {
        if let Some(&aux_id) = ctx.aux.name_to_id.get(source_name) {
            control_aux_indices.push(ctx.node_count + aux_id);
        } else {
            return Err(StampError::MissingValue);
        }
    }

    // Get control currents from solution vector
    let mut control_currents: Vec<f64> = Vec::with_capacity(poly.degree);
    if let Some(x) = x {
        for &aux_idx in &control_aux_indices {
            let i = x.get(aux_idx).copied().unwrap_or(0.0);
            control_currents.push(i);
        }
    } else {
        control_currents.resize(poly.degree, 0.0);
    }

    // Evaluate polynomial and derivatives
    let (f_value, derivs) = evaluate_poly(poly, &control_currents);

    // Check if purely linear
    let is_linear = poly.coeffs.len() <= poly.degree + 1;

    if is_linear {
        // Linear case: V(out) = c0 + c1*I1 + c2*I2 + ...
        ctx.add(k, out_p, 1.0);
        ctx.add(k, out_n, -1.0);

        // Stamp constant term
        let c0 = poly.coeffs.first().copied().unwrap_or(0.0);
        ctx.add_rhs(k, c0);

        // Stamp linear coefficients (transresistances)
        for (i, &aux_idx) in control_aux_indices.iter().enumerate() {
            let h = poly.coeffs.get(i + 1).copied().unwrap_or(0.0);
            ctx.add(k, aux_idx, -h);
        }
    } else {
        // Nonlinear case: Newton-Raphson linearization
        ctx.add(k, out_p, 1.0);
        ctx.add(k, out_n, -1.0);

        // Stamp derivatives as transresistances
        for (i, &aux_idx) in control_aux_indices.iter().enumerate() {
            if let Some(&deriv) = derivs.get(i) {
                ctx.add(k, aux_idx, -deriv);
            }
        }

        // RHS: f(x0) - sum(df/dIi * Ii_0)
        let mut rhs = f_value;
        for (i, &curr) in control_currents.iter().enumerate() {
            if let Some(&deriv) = derivs.get(i) {
                rhs -= deriv * curr;
            }
        }
        ctx.add_rhs(k, rhs);
    }

    Ok(())
}

// ============================================================================
// AC Small-Signal Stamping Functions
// ============================================================================

/// Resistor AC stamping: Y = G = 1/R (real admittance)
fn stamp_resistor_ac(ctx: &mut ComplexStampContext, inst: &Instance) -> Result<(), StampError> {
    if inst.nodes.len() != 2 {
        return Err(StampError::InvalidNodes);
    }
    let value = inst
        .value
        .as_deref()
        .and_then(parse_number_with_suffix)
        .ok_or(StampError::MissingValue)?;
    let g = 1.0 / value;
    let a = inst.nodes[0].0;
    let b = inst.nodes[1].0;
    ctx.add_real(a, a, g);
    ctx.add_real(b, b, g);
    ctx.add_real(a, b, -g);
    ctx.add_real(b, a, -g);
    Ok(())
}

/// Capacitor AC stamping: Y = jωC (imaginary admittance)
fn stamp_capacitor_ac(ctx: &mut ComplexStampContext, inst: &Instance) -> Result<(), StampError> {
    if inst.nodes.len() != 2 {
        return Err(StampError::InvalidNodes);
    }
    let c = inst
        .value
        .as_deref()
        .and_then(parse_number_with_suffix)
        .ok_or(StampError::MissingValue)?;
    let y = ctx.omega * c; // jωC has imaginary part ωC
    let a = inst.nodes[0].0;
    let b = inst.nodes[1].0;
    ctx.add_imag(a, a, y);
    ctx.add_imag(b, b, y);
    ctx.add_imag(a, b, -y);
    ctx.add_imag(b, a, -y);
    Ok(())
}

/// Inductor AC stamping: Y = 1/(jωL) = -j/(ωL)
/// Uses auxiliary variable for inductor current
fn stamp_inductor_ac(ctx: &mut ComplexStampContext, inst: &Instance) -> Result<(), StampError> {
    if inst.nodes.len() != 2 {
        return Err(StampError::InvalidNodes);
    }
    let l = inst
        .value
        .as_deref()
        .and_then(parse_number_with_suffix)
        .ok_or(StampError::MissingValue)?;
    let a = inst.nodes[0].0;
    let b = inst.nodes[1].0;

    // Allocate auxiliary variable for inductor current
    let k = ctx.allocate_aux(&inst.name);

    // KCL at nodes: current flows from a to b
    ctx.add_real(a, k, 1.0);
    ctx.add_real(b, k, -1.0);

    // Constitutive relation: V(a) - V(b) = jωL * I
    ctx.add_real(k, a, 1.0);
    ctx.add_real(k, b, -1.0);
    ctx.add_imag(k, k, -ctx.omega * l); // -jωL

    Ok(())
}

/// Voltage source AC stamping with AC magnitude and phase
fn stamp_voltage_ac(ctx: &mut ComplexStampContext, inst: &Instance) -> Result<(), StampError> {
    if inst.nodes.len() != 2 {
        return Err(StampError::InvalidNodes);
    }
    let a = inst.nodes[0].0;
    let b = inst.nodes[1].0;

    // Allocate auxiliary variable for source current
    let k = ctx.allocate_aux(&inst.name);

    // KCL at nodes
    ctx.add_real(a, k, 1.0);
    ctx.add_real(b, k, -1.0);

    // Constitutive relation: V(a) - V(b) = Vac
    ctx.add_real(k, a, 1.0);
    ctx.add_real(k, b, -1.0);

    // AC excitation: Vac = ac_mag * exp(j * ac_phase)
    let ac_mag = inst.ac_mag.unwrap_or(0.0);
    let ac_phase_deg = inst.ac_phase.unwrap_or(0.0);
    let ac_phase_rad = ac_phase_deg * std::f64::consts::PI / 180.0;
    let vac = Complex64::from_polar(ac_mag, ac_phase_rad);
    ctx.add_rhs(k, vac);

    Ok(())
}

/// Current source AC stamping with AC magnitude and phase
fn stamp_current_ac(ctx: &mut ComplexStampContext, inst: &Instance) -> Result<(), StampError> {
    if inst.nodes.len() != 2 {
        return Err(StampError::InvalidNodes);
    }
    let a = inst.nodes[0].0;
    let b = inst.nodes[1].0;

    // AC excitation: Iac = ac_mag * exp(j * ac_phase)
    let ac_mag = inst.ac_mag.unwrap_or(0.0);
    let ac_phase_deg = inst.ac_phase.unwrap_or(0.0);
    let ac_phase_rad = ac_phase_deg * std::f64::consts::PI / 180.0;
    let iac = Complex64::from_polar(ac_mag, ac_phase_rad);

    // Current flows from a to b (out of a, into b)
    ctx.add_rhs(a, -iac);
    ctx.add_rhs(b, iac);

    Ok(())
}

/// Diode AC stamping: linearized small-signal conductance from DC operating point
fn stamp_diode_ac(
    ctx: &mut ComplexStampContext,
    inst: &Instance,
    dc_solution: &[f64],
) -> Result<(), StampError> {
    if inst.nodes.len() != 2 {
        return Err(StampError::InvalidNodes);
    }
    let a = inst.nodes[0].0;
    let b = inst.nodes[1].0;

    let gmin = 1e-12;
    let isat = param_value(&inst.params, &["is"]).unwrap_or(1e-14);
    let emission = param_value(&inst.params, &["n", "nj"]).unwrap_or(1.0);
    let vt = 0.02585 * emission;

    let va = dc_solution.get(a).copied().unwrap_or(0.0);
    let vb = dc_solution.get(b).copied().unwrap_or(0.0);
    let vd = va - vb;

    // Small-signal conductance gd = dId/dVd = (Is/Vt) * exp(Vd/Vt)
    let exp_vd = (vd / vt).exp();
    let gd = (isat / vt) * exp_vd;
    let g = gd.max(gmin);

    ctx.add_real(a, a, g);
    ctx.add_real(b, b, g);
    ctx.add_real(a, b, -g);
    ctx.add_real(b, a, -g);

    Ok(())
}

/// MOSFET AC stamping: linearized small-signal model from DC operating point
fn stamp_mos_ac(
    ctx: &mut ComplexStampContext,
    inst: &Instance,
    dc_solution: &[f64],
) -> Result<(), StampError> {
    if inst.nodes.len() < 4 {
        return Err(StampError::InvalidNodes);
    }
    let drain = inst.nodes[0].0;
    let gate = inst.nodes[1].0;
    let source = inst.nodes[2].0;
    let bulk = inst.nodes[3].0;
    let gmin = 1e-12;

    // Parse model level
    let level = param_value(&inst.params, &["level"]).unwrap_or(49.0) as u32;

    // Determine NMOS/PMOS
    let is_pmos = if let Some(t) = inst.params.get("type") {
        let t_lower = t.to_ascii_lowercase();
        t_lower.contains("pmos") || t_lower == "p"
    } else if inst.params.contains_key("pmos") {
        true
    } else {
        false
    };

    // Build BSIM parameters
    let params = sim_devices::bsim::build_bsim_params(&inst.params, level, is_pmos);

    let w = param_value(&inst.params, &["w"]).unwrap_or(1e-6);
    let l = param_value(&inst.params, &["l"]).unwrap_or(1e-6);
    let temp = param_value(&inst.params, &["temp"]).unwrap_or(300.15);

    let vd = dc_solution.get(drain).copied().unwrap_or(0.0);
    let vg = dc_solution.get(gate).copied().unwrap_or(0.0);
    let vs = dc_solution.get(source).copied().unwrap_or(0.0);
    let vb = dc_solution.get(bulk).copied().unwrap_or(0.0);

    // Get small-signal parameters from DC operating point
    let output = sim_devices::bsim::evaluate_mos(&params, w, l, vd, vg, vs, vb, temp);

    let gm = output.gm;
    let gds = output.gds.max(gmin);
    let gmbs = output.gmbs;

    // Stamp gds (output conductance between drain and source)
    ctx.add_real(drain, drain, gds);
    ctx.add_real(source, source, gds);
    ctx.add_real(drain, source, -gds);
    ctx.add_real(source, drain, -gds);

    // Stamp gm (transconductance: current controlled by Vgs)
    ctx.add_real(drain, gate, gm);
    ctx.add_real(drain, source, -gm);
    ctx.add_real(source, gate, -gm);
    ctx.add_real(source, source, gm);

    // Stamp gmbs (body transconductance: current controlled by Vbs)
    if gmbs.abs() > gmin * 0.01 {
        ctx.add_real(drain, bulk, gmbs);
        ctx.add_real(drain, source, -gmbs);
        ctx.add_real(source, bulk, -gmbs);
        ctx.add_real(source, source, gmbs);
    }

    Ok(())
}

/// VCVS AC stamping (frequency-independent)
/// For POLY, uses linearized small-signal model from DC operating point
fn stamp_vcvs_ac(
    ctx: &mut ComplexStampContext,
    inst: &Instance,
    dc_solution: &[f64],
) -> Result<(), StampError> {
    // Check for POLY syntax
    if let Some(ref poly) = inst.poly {
        return stamp_vcvs_poly_ac(ctx, inst, poly, dc_solution);
    }

    // Simple linear case
    if inst.nodes.len() != 4 {
        return Err(StampError::InvalidNodes);
    }
    let gain = inst
        .value
        .as_deref()
        .and_then(parse_number_with_suffix)
        .ok_or(StampError::MissingValue)?;

    let out_p = inst.nodes[0].0;
    let out_n = inst.nodes[1].0;
    let in_p = inst.nodes[2].0;
    let in_n = inst.nodes[3].0;

    let k = ctx.allocate_aux(&inst.name);

    ctx.add_real(out_p, k, 1.0);
    ctx.add_real(out_n, k, -1.0);
    ctx.add_real(k, out_p, 1.0);
    ctx.add_real(k, out_n, -1.0);
    ctx.add_real(k, in_p, -gain);
    ctx.add_real(k, in_n, gain);

    Ok(())
}

/// VCVS POLY AC stamping - linearized around DC operating point
fn stamp_vcvs_poly_ac(
    ctx: &mut ComplexStampContext,
    inst: &Instance,
    poly: &PolySpec,
    dc_solution: &[f64],
) -> Result<(), StampError> {
    if inst.nodes.len() < 2 {
        return Err(StampError::InvalidNodes);
    }

    let out_p = inst.nodes[0].0;
    let out_n = inst.nodes[1].0;
    let k = ctx.allocate_aux(&inst.name);

    ctx.add_real(out_p, k, 1.0);
    ctx.add_real(out_n, k, -1.0);
    ctx.add_real(k, out_p, 1.0);
    ctx.add_real(k, out_n, -1.0);

    // Get control voltages from DC solution
    let mut control_voltages: Vec<f64> = Vec::with_capacity(poly.degree);
    for &(pos, neg) in &poly.control_nodes {
        let v_pos = dc_solution.get(pos).copied().unwrap_or(0.0);
        let v_neg = dc_solution.get(neg).copied().unwrap_or(0.0);
        control_voltages.push(v_pos - v_neg);
    }

    // Evaluate derivatives at DC operating point
    let (_, derivs) = evaluate_poly(poly, &control_voltages);

    // Stamp the linearized gains
    for (i, &(pos, neg)) in poly.control_nodes.iter().enumerate() {
        let gain = derivs.get(i).copied().unwrap_or(0.0);
        ctx.add_real(k, pos, -gain);
        ctx.add_real(k, neg, gain);
    }

    Ok(())
}

/// VCCS AC stamping (frequency-independent)
fn stamp_vccs_ac(
    ctx: &mut ComplexStampContext,
    inst: &Instance,
    dc_solution: &[f64],
) -> Result<(), StampError> {
    // Check for POLY syntax
    if let Some(ref poly) = inst.poly {
        return stamp_vccs_poly_ac(ctx, inst, poly, dc_solution);
    }

    // Simple linear case
    if inst.nodes.len() != 4 {
        return Err(StampError::InvalidNodes);
    }
    let gm = inst
        .value
        .as_deref()
        .and_then(parse_number_with_suffix)
        .ok_or(StampError::MissingValue)?;

    let out_p = inst.nodes[0].0;
    let out_n = inst.nodes[1].0;
    let in_p = inst.nodes[2].0;
    let in_n = inst.nodes[3].0;

    ctx.add_real(out_p, in_p, gm);
    ctx.add_real(out_p, in_n, -gm);
    ctx.add_real(out_n, in_p, -gm);
    ctx.add_real(out_n, in_n, gm);

    Ok(())
}

/// VCCS POLY AC stamping - linearized around DC operating point
fn stamp_vccs_poly_ac(
    ctx: &mut ComplexStampContext,
    inst: &Instance,
    poly: &PolySpec,
    dc_solution: &[f64],
) -> Result<(), StampError> {
    if inst.nodes.len() < 2 {
        return Err(StampError::InvalidNodes);
    }

    let out_p = inst.nodes[0].0;
    let out_n = inst.nodes[1].0;

    // Get control voltages from DC solution
    let mut control_voltages: Vec<f64> = Vec::with_capacity(poly.degree);
    for &(pos, neg) in &poly.control_nodes {
        let v_pos = dc_solution.get(pos).copied().unwrap_or(0.0);
        let v_neg = dc_solution.get(neg).copied().unwrap_or(0.0);
        control_voltages.push(v_pos - v_neg);
    }

    // Evaluate derivatives at DC operating point
    let (_, derivs) = evaluate_poly(poly, &control_voltages);

    // Stamp the linearized transconductances
    for (i, &(pos, neg)) in poly.control_nodes.iter().enumerate() {
        let gm = derivs.get(i).copied().unwrap_or(0.0);
        ctx.add_real(out_p, pos, gm);
        ctx.add_real(out_p, neg, -gm);
        ctx.add_real(out_n, pos, -gm);
        ctx.add_real(out_n, neg, gm);
    }

    Ok(())
}

/// CCCS AC stamping (frequency-independent)
fn stamp_cccs_ac(
    ctx: &mut ComplexStampContext,
    inst: &Instance,
    dc_solution: &[f64],
) -> Result<(), StampError> {
    // Check for POLY syntax
    if let Some(ref poly) = inst.poly {
        return stamp_cccs_poly_ac(ctx, inst, poly, dc_solution);
    }

    // Simple linear case
    if inst.nodes.len() != 2 {
        return Err(StampError::InvalidNodes);
    }
    let gain = inst
        .value
        .as_deref()
        .and_then(parse_number_with_suffix)
        .ok_or(StampError::MissingValue)?;

    let out_p = inst.nodes[0].0;
    let out_n = inst.nodes[1].0;

    let control_name = inst.control.as_ref().ok_or(StampError::MissingValue)?;
    let control_aux = ctx
        .aux
        .name_to_id
        .get(control_name)
        .copied()
        .ok_or(StampError::MissingValue)?;
    let k_control = ctx.node_count + control_aux;

    ctx.add_real(out_p, k_control, gain);
    ctx.add_real(out_n, k_control, -gain);

    Ok(())
}

/// CCCS POLY AC stamping - linearized around DC operating point
fn stamp_cccs_poly_ac(
    ctx: &mut ComplexStampContext,
    inst: &Instance,
    poly: &PolySpec,
    dc_solution: &[f64],
) -> Result<(), StampError> {
    if inst.nodes.len() < 2 {
        return Err(StampError::InvalidNodes);
    }

    let out_p = inst.nodes[0].0;
    let out_n = inst.nodes[1].0;

    // Get auxiliary variable indices for control sources
    let mut control_aux_indices: Vec<usize> = Vec::with_capacity(poly.degree);
    for source_name in &poly.control_sources {
        if let Some(&aux_id) = ctx.aux.name_to_id.get(source_name) {
            control_aux_indices.push(ctx.node_count + aux_id);
        } else {
            return Err(StampError::MissingValue);
        }
    }

    // Get control currents from DC solution
    let mut control_currents: Vec<f64> = Vec::with_capacity(poly.degree);
    for &aux_idx in &control_aux_indices {
        let i = dc_solution.get(aux_idx).copied().unwrap_or(0.0);
        control_currents.push(i);
    }

    // Evaluate derivatives at DC operating point
    let (_, derivs) = evaluate_poly(poly, &control_currents);

    // Stamp the linearized gains
    for (i, &aux_idx) in control_aux_indices.iter().enumerate() {
        let gain = derivs.get(i).copied().unwrap_or(0.0);
        ctx.add_real(out_p, aux_idx, gain);
        ctx.add_real(out_n, aux_idx, -gain);
    }

    Ok(())
}

/// CCVS AC stamping (frequency-independent)
fn stamp_ccvs_ac(
    ctx: &mut ComplexStampContext,
    inst: &Instance,
    dc_solution: &[f64],
) -> Result<(), StampError> {
    // Check for POLY syntax
    if let Some(ref poly) = inst.poly {
        return stamp_ccvs_poly_ac(ctx, inst, poly, dc_solution);
    }

    // Simple linear case
    if inst.nodes.len() != 2 {
        return Err(StampError::InvalidNodes);
    }
    let gain = inst
        .value
        .as_deref()
        .and_then(parse_number_with_suffix)
        .ok_or(StampError::MissingValue)?;

    let out_p = inst.nodes[0].0;
    let out_n = inst.nodes[1].0;

    let control_name = inst.control.as_ref().ok_or(StampError::MissingValue)?;
    let control_aux = ctx
        .aux
        .name_to_id
        .get(control_name)
        .copied()
        .ok_or(StampError::MissingValue)?;
    let k_control = ctx.node_count + control_aux;

    let k = ctx.allocate_aux(&inst.name);

    ctx.add_real(out_p, k, 1.0);
    ctx.add_real(out_n, k, -1.0);
    ctx.add_real(k, out_p, 1.0);
    ctx.add_real(k, out_n, -1.0);
    ctx.add_real(k, k_control, -gain);

    Ok(())
}

/// CCVS POLY AC stamping - linearized around DC operating point
fn stamp_ccvs_poly_ac(
    ctx: &mut ComplexStampContext,
    inst: &Instance,
    poly: &PolySpec,
    dc_solution: &[f64],
) -> Result<(), StampError> {
    if inst.nodes.len() < 2 {
        return Err(StampError::InvalidNodes);
    }

    let out_p = inst.nodes[0].0;
    let out_n = inst.nodes[1].0;
    let k = ctx.allocate_aux(&inst.name);

    ctx.add_real(out_p, k, 1.0);
    ctx.add_real(out_n, k, -1.0);
    ctx.add_real(k, out_p, 1.0);
    ctx.add_real(k, out_n, -1.0);

    // Get auxiliary variable indices for control sources
    let mut control_aux_indices: Vec<usize> = Vec::with_capacity(poly.degree);
    for source_name in &poly.control_sources {
        if let Some(&aux_id) = ctx.aux.name_to_id.get(source_name) {
            control_aux_indices.push(ctx.node_count + aux_id);
        } else {
            return Err(StampError::MissingValue);
        }
    }

    // Get control currents from DC solution
    let mut control_currents: Vec<f64> = Vec::with_capacity(poly.degree);
    for &aux_idx in &control_aux_indices {
        let i = dc_solution.get(aux_idx).copied().unwrap_or(0.0);
        control_currents.push(i);
    }

    // Evaluate derivatives at DC operating point
    let (_, derivs) = evaluate_poly(poly, &control_currents);

    // Stamp the linearized transresistances
    for (i, &aux_idx) in control_aux_indices.iter().enumerate() {
        let h = derivs.get(i).copied().unwrap_or(0.0);
        ctx.add_real(k, aux_idx, -h);
    }

    Ok(())
}
