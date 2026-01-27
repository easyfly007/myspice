//! BSIM3 DC Evaluation
//!
//! Main entry point for BSIM MOSFET DC analysis.
//! Computes drain current (Ids) and small-signal parameters (gm, gds, gmbs)
//! using the modular physics functions.
//!
//! ## DC Current Model
//!
//! The drain current is calculated differently depending on operating region:
//!
//! **Cutoff (Vgs < Vth)**:
//! - Ids ≈ 0 (subthreshold leakage in full model)
//!
//! **Linear (Vds < Vdsat)**:
//! - Ids = W/L * ueff * Cox * [(Vgs-Vth)*Vds - Vds^2/2]
//!
//! **Saturation (Vds >= Vdsat)**:
//! - Ids = W/L * ueff * Cox * Vdsat^2/2 * CLM_factor
//!
//! ## Small-Signal Parameters
//!
//! - gm = dIds/dVgs (transconductance)
//! - gds = dIds/dVds (output conductance)
//! - gmbs = dIds/dVbs (body transconductance)
//!
//! ## MNA Stamping
//!
//! For Newton-Raphson iteration, the linearized current is:
//! ```text
//! i_ds = gm*(vgs-VGS) + gds*(vds-VDS) + gmbs*(vbs-VBS) + IDS
//!      = gm*vgs + gds*vds + gmbs*vbs + (IDS - gm*VGS - gds*VDS - gmbs*VBS)
//!      = gm*vgs + gds*vds + gmbs*vbs + ieq
//! ```

use super::params::{BsimParams, EPSILON_OX, K_BOLTZMANN, Q_ELECTRON};
use super::types::{BsimOutput, MosRegion, MosType};
use super::threshold::calculate_vth;
use super::mobility::calculate_mobility;
use super::channel::{calculate_vdsat, calculate_clm_factor, calculate_rds};

/// Minimum conductance for numerical stability [S]
const GMIN: f64 = 1e-12;

/// Main BSIM DC evaluation function
///
/// Computes drain current and all small-signal parameters needed for
/// MNA matrix stamping.
///
/// # Arguments
/// * `params` - BSIM3 model parameters
/// * `w` - Device width [m]
/// * `l` - Device length [m]
/// * `vd` - Drain voltage [V]
/// * `vg` - Gate voltage [V]
/// * `vs` - Source voltage [V]
/// * `vb` - Bulk/body voltage [V]
/// * `temp` - Temperature [K]
///
/// # Returns
/// * `BsimOutput` containing Ids, gm, gds, gmbs, ieq, region, vth_eff
///
/// # Example
/// ```ignore
/// let params = BsimParams::nmos_default();
/// let output = evaluate_bsim_dc(&params, 1e-6, 1e-6, 1.8, 1.5, 0.0, 0.0, 300.15);
/// println!("Ids = {} A", output.ids);
/// println!("gm = {} S", output.gm);
/// ```
pub fn evaluate_bsim_dc(
    params: &BsimParams,
    w: f64,
    l: f64,
    vd: f64,
    vg: f64,
    vs: f64,
    vb: f64,
    temp: f64,
) -> BsimOutput {
    // Handle PMOS by flipping voltage signs
    let (vd_int, vg_int, vs_int, vb_int, sign) = match params.mos_type {
        MosType::Nmos => (vd, vg, vs, vb, 1.0),
        MosType::Pmos => (-vs, -vg, -vd, -vb, -1.0), // Swap D/S and negate
    };

    // Terminal voltages (internal, after PMOS flip)
    let mut vgs = vg_int - vs_int;
    let mut vds = vd_int - vs_int;
    let vbs = vb_int - vs_int;

    // Source/drain swap for negative Vds (reverse mode)
    let reversed = vds < 0.0;
    if reversed {
        vds = -vds;
        vgs = vg_int - vd_int; // Vgd becomes effective Vgs
    }

    // Effective dimensions
    let leff = params.leff(l);
    let weff = params.weff(w);

    // Oxide capacitance per unit area
    let cox = EPSILON_OX / params.tox;

    // Thermal voltage
    let vt = K_BOLTZMANN * temp / Q_ELECTRON;

    // ========================================
    // Step 1: Threshold Voltage
    // ========================================
    let (vth, dvth_dvbs) = calculate_vth(params, vbs, vds, leff, weff, temp);

    // Gate overdrive
    let vgst = vgs - vth;

    // ========================================
    // Step 2: Operating Region Determination
    // ========================================
    let region;
    let mut ids: f64;
    let mut gm: f64;
    let mut gds: f64;
    let gmbs: f64;

    if vgst <= 0.0 {
        // ========================================
        // Cutoff Region (with subthreshold)
        // ========================================
        region = MosRegion::Cutoff;

        // Subthreshold current (weak inversion)
        // Ids = I0 * exp((Vgs - Vth) / (n * Vt)) * (1 - exp(-Vds/Vt))
        let n = params.nfactor.max(1.0);
        let i0 = weff / leff * params.u0 * 1e-4 * cox * vt * vt * (n - 1.0);

        let exp_vgst = (vgst / (n * vt)).exp();
        let exp_vds = (-vds / vt).exp();

        // Subthreshold current
        ids = i0 * exp_vgst * (1.0 - exp_vds);
        ids = ids.max(0.0);

        // Small-signal parameters in subthreshold
        gm = ids / (n * vt);
        gds = i0 * exp_vgst * exp_vds / vt;
        gmbs = -gm * dvth_dvbs;

        // Ensure minimum conductance
        gds = gds.max(GMIN);
        gm = gm.max(GMIN * 0.01);

    } else {
        // ========================================
        // Step 3: Mobility Calculation
        // ========================================
        let ueff = calculate_mobility(params, vgs, vbs, vth, leff, temp);

        // ========================================
        // Step 4: Saturation Voltage
        // ========================================
        let (vdsat, dvdsat_dvgs) = calculate_vdsat(params, vgs, vth, ueff, leff);

        // ========================================
        // Step 5: Drain Current Calculation
        // ========================================
        // Beta factor: W/L * ueff * Cox
        let ueff_m2 = ueff * 1e-4; // cm^2/V/s to m^2/V/s
        let beta = weff / leff * ueff_m2 * cox;

        if vds < vdsat {
            // ========================================
            // Linear Region
            // ========================================
            region = MosRegion::Linear;

            // Ids = beta * [(Vgst - Vds/2) * Vds]
            ids = beta * (vgst * vds - 0.5 * vds * vds);

            // gm = dIds/dVgs = beta * Vds
            gm = beta * vds;

            // gds = dIds/dVds = beta * (Vgst - Vds)
            gds = beta * (vgst - vds);
            gds = gds.max(GMIN);

            // gmbs = dIds/dVbs = -gm * dVth/dVbs
            gmbs = -gm * dvth_dvbs;

        } else {
            // ========================================
            // Saturation Region
            // ========================================
            region = MosRegion::Saturation;

            // Channel length modulation
            let (clm_factor, dclm_dvds) = calculate_clm_factor(params, vds, vdsat, leff, ueff);

            // Saturation current: Ids = beta * Vdsat^2 / 2 * CLM
            let ids_sat = 0.5 * beta * vdsat * vdsat;
            ids = ids_sat * clm_factor;

            // gm = dIds/dVgs
            // = d/dVgs [beta * Vdsat^2/2 * CLM]
            // = beta * Vdsat * dVdsat/dVgs * CLM
            gm = beta * vdsat * dvdsat_dvgs * clm_factor;

            // gds = dIds/dVds (from CLM)
            // = Ids_sat * dCLM/dVds
            gds = ids_sat * dclm_dvds;
            gds = gds.max(GMIN);

            // DIBL contribution to gds
            // gds_dibl ≈ gm * ETA0
            let gds_dibl = gm * params.eta0;
            gds += gds_dibl;

            // gmbs = dIds/dVbs = -gm * dVth/dVbs (Vdsat depends on Vth)
            // Plus contribution from Vdsat dependence on Vth
            gmbs = -gm * dvth_dvbs;
        }
    }

    // Ensure positive current
    ids = ids.max(0.0);

    // ========================================
    // Source/Drain Series Resistance
    // ========================================
    let rds = calculate_rds(params, weff, temp);
    if rds > 0.0 && ids > 0.0 {
        // Simplified Rds effect: reduce effective gds
        // Full model would iterate on Vds_int
        let v_rds = ids * rds;
        if v_rds < vds * 0.5 {
            // Only apply if Rds drop is small
            gds = gds / (1.0 + rds * gds);
        }
    }

    // ========================================
    // Handle source/drain reversal
    // ========================================
    if reversed {
        // In reverse mode, gm acts on Vgd instead of Vgs
        // For stamping purposes, we keep the same form
        // The caller will handle node mapping
    }

    // Apply sign for PMOS
    ids *= sign;

    // ========================================
    // Calculate equivalent current for MNA
    // ========================================
    // ieq = Ids - gm*Vgs - gds*Vds - gmbs*Vbs
    // This is the DC offset for linearized current source
    let vgs_orig = vg - vs;
    let vds_orig = vd - vs;
    let vbs_orig = vb - vs;

    let ieq = ids - gm * vgs_orig - gds * vds_orig - gmbs * vbs_orig;

    BsimOutput {
        ids,
        gm,
        gds,
        gmbs,
        ieq,
        region,
        vth_eff: vth,
    }
}

/// Simplified Level 1 MOSFET evaluation
///
/// For backward compatibility with simple models.
/// Uses only VTH0, KP (or BETA), and LAMBDA parameters.
pub fn evaluate_level1_dc(
    vth0: f64,
    beta: f64,
    lambda: f64,
    w: f64,
    l: f64,
    vd: f64,
    vg: f64,
    vs: f64,
    _vb: f64,
    is_pmos: bool,
) -> BsimOutput {
    // Handle PMOS
    let (vd_int, vg_int, vs_int, sign) = if is_pmos {
        (-vs, -vg, -vd, -1.0)
    } else {
        (vd, vg, vs, 1.0)
    };

    let mut vgs = vg_int - vs_int;
    let mut vds = vd_int - vs_int;

    // Source/drain swap
    if vds < 0.0 {
        vds = -vds;
        vgs = vg_int - vd_int;
    }

    let vth = if is_pmos { -vth0.abs() } else { vth0.abs() };
    let beta_eff = beta * w / l;

    let region;
    let ids;
    let gm;
    let gds;

    if vgs <= vth {
        // Cutoff
        region = MosRegion::Cutoff;
        ids = 0.0;
        gm = 0.0;
        gds = GMIN;
    } else if vds < vgs - vth {
        // Linear
        region = MosRegion::Linear;
        ids = beta_eff * ((vgs - vth) * vds - 0.5 * vds * vds);
        gm = beta_eff * vds;
        gds = (beta_eff * ((vgs - vth) - vds)).max(GMIN);
    } else {
        // Saturation
        region = MosRegion::Saturation;
        ids = 0.5 * beta_eff * (vgs - vth).powi(2) * (1.0 + lambda * vds);
        gm = beta_eff * (vgs - vth) * (1.0 + lambda * vds);
        gds = (0.5 * beta_eff * (vgs - vth).powi(2) * lambda).max(GMIN);
    }

    let ids_signed = ids * sign;

    let vgs_orig = vg - vs;
    let vds_orig = vd - vs;
    let ieq = ids_signed - gm * vgs_orig - gds * vds_orig;

    BsimOutput {
        ids: ids_signed,
        gm,
        gds,
        gmbs: 0.0, // Level 1 ignores body effect on current
        ieq,
        region,
        vth_eff: vth,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nmos_cutoff() {
        let params = BsimParams::nmos_default();
        // Vg = 0.3V is well below Vth0 = 0.7V, should be in cutoff
        // Use longer channel for cleaner behavior
        let output = evaluate_bsim_dc(&params, 10e-6, 10e-6, 1.0, 0.2, 0.0, 0.0, 300.15);
        assert_eq!(output.region, MosRegion::Cutoff, "Expected cutoff, got {:?}, vth_eff={}", output.region, output.vth_eff);
        assert!(output.ids.abs() < 1e-6); // Very small current
    }

    #[test]
    fn test_nmos_linear() {
        let params = BsimParams::nmos_default();
        // Vgs = 1.5V > Vth (~0.7V), Vds = 0.1V < Vgs - Vth
        let output = evaluate_bsim_dc(&params, 1e-6, 1e-6, 0.1, 1.5, 0.0, 0.0, 300.15);
        assert_eq!(output.region, MosRegion::Linear, "Expected linear, got {:?}", output.region);
        assert!(output.ids > 0.0);
    }

    #[test]
    fn test_nmos_saturation() {
        let params = BsimParams::nmos_default();
        // Vgs = 1.2V, Vds = 2.0V >> Vgs - Vth, clearly saturation
        let output = evaluate_bsim_dc(&params, 1e-6, 1e-6, 2.0, 1.2, 0.0, 0.0, 300.15);
        assert_eq!(output.region, MosRegion::Saturation, "Expected saturation, got {:?}, vth_eff={}", output.region, output.vth_eff);
        assert!(output.ids > 0.0);
    }

    #[test]
    fn test_pmos_saturation() {
        let params = BsimParams::pmos_default();
        let output = evaluate_bsim_dc(&params, 1e-6, 1e-6, 0.0, 0.0, 1.8, 1.8, 300.15);
        // PMOS: Vgs = 0 - 1.8 = -1.8V, should be on
        // Vds = 0 - 1.8 = -1.8V
        assert!(output.ids < 0.0); // Current flows out of drain
    }

    #[test]
    fn test_ids_increases_with_vgs() {
        let params = BsimParams::nmos_default();
        let out1 = evaluate_bsim_dc(&params, 1e-6, 1e-6, 1.8, 1.0, 0.0, 0.0, 300.15);
        let out2 = evaluate_bsim_dc(&params, 1e-6, 1e-6, 1.8, 1.5, 0.0, 0.0, 300.15);
        let out3 = evaluate_bsim_dc(&params, 1e-6, 1e-6, 1.8, 2.0, 0.0, 0.0, 300.15);
        assert!(out2.ids > out1.ids);
        assert!(out3.ids > out2.ids);
    }

    #[test]
    fn test_ids_increases_with_width() {
        let params = BsimParams::nmos_default();
        let out1 = evaluate_bsim_dc(&params, 1e-6, 1e-6, 1.8, 1.5, 0.0, 0.0, 300.15);
        let out2 = evaluate_bsim_dc(&params, 2e-6, 1e-6, 1.8, 1.5, 0.0, 0.0, 300.15);
        assert!((out2.ids / out1.ids - 2.0).abs() < 0.2); // Should roughly double
    }

    #[test]
    fn test_level1_compatibility() {
        let out = evaluate_level1_dc(0.7, 1e-3, 0.02, 1e-6, 1e-6, 1.8, 1.5, 0.0, 0.0, false);
        assert_eq!(out.region, MosRegion::Saturation);
        assert!(out.ids > 0.0);
    }
}
