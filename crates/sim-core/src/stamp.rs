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
            DeviceKind::C => Ok(()), // 电容在 DC 下开路
            _ => Ok(()), // TODO: 完善其他器件 stamp
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
    if let Some(x) = x {
        let va = x.get(a).copied().unwrap_or(0.0);
        let vb = x.get(b).copied().unwrap_or(0.0);
        let vd = va - vb;
        let isat = 1e-14;
        let vt = 0.02585;
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
    let gmin = if ctx.gmin > 0.0 { ctx.gmin } else { 1e-12 };
    if let Some(x) = x {
        let vd = x.get(drain).copied().unwrap_or(0.0);
        let vg = x.get(gate).copied().unwrap_or(0.0);
        let vs = x.get(source).copied().unwrap_or(0.0);
        let vgs = vg - vs;
        let vds = vd - vs;
        let vth = 1.0;
        let beta = 1e-3;
        let lambda = 0.0;
        let (id, gm, gds) = if vgs <= vth {
            (0.0, 0.0, gmin)
        } else if vds < vgs - vth {
            let id = beta * ((vgs - vth) * vds - 0.5 * vds * vds);
            let gm = beta * vds;
            let gds = beta * ((vgs - vth) - vds).max(0.0);
            (id, gm, gds.max(gmin))
        } else {
            let id = 0.5 * beta * (vgs - vth) * (vgs - vth) * (1.0 + lambda * vds);
            let gm = beta * (vgs - vth) * (1.0 + lambda * vds);
            let gds = 0.5 * beta * (vgs - vth) * (vgs - vth) * lambda;
            (id, gm, gds.max(gmin))
        };
        let ieq = id - gm * vgs - gds * vds;
        ctx.add(drain, drain, gds);
        ctx.add(source, source, gds);
        ctx.add(drain, source, -gds);
        ctx.add(source, drain, -gds);
        ctx.add(drain, gate, gm);
        ctx.add(drain, source, -gm);
        ctx.add(source, gate, -gm);
        ctx.add(source, source, gm);
        ctx.add_rhs(drain, -ieq);
        ctx.add_rhs(source, ieq);
        return Ok(());
    }
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
