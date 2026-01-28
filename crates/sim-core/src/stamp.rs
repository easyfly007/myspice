use crate::circuit::{DeviceKind, Instance};
use crate::mna::StampContext;
use std::collections::HashMap;

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
            DeviceKind::E => stamp_vcvs(ctx, &self.instance),
            DeviceKind::G => stamp_vccs(ctx, &self.instance),
            DeviceKind::F => stamp_cccs(ctx, &self.instance),
            DeviceKind::H => stamp_ccvs(ctx, &self.instance),
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
/// nodes: [out+, out-, in+, in-]
fn stamp_vcvs(ctx: &mut StampContext, inst: &Instance) -> Result<(), StampError> {
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

/// Voltage Controlled Current Source (VCCS)
/// Iout = G * Vin where G is the transconductance
/// nodes: [out+, out-, in+, in-]
fn stamp_vccs(ctx: &mut StampContext, inst: &Instance) -> Result<(), StampError> {
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

/// Current Controlled Current Source (CCCS)
/// Iout = F * Icontrol where F is the current gain
/// nodes: [out+, out-], control: name of controlling voltage source
fn stamp_cccs(ctx: &mut StampContext, inst: &Instance) -> Result<(), StampError> {
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

/// Current Controlled Voltage Source (CCVS)
/// Vout = H * Icontrol where H is the transresistance
/// nodes: [out+, out-], control: name of controlling voltage source
fn stamp_ccvs(ctx: &mut StampContext, inst: &Instance) -> Result<(), StampError> {
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
