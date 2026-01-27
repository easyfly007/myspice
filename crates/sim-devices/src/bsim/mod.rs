//! BSIM MOSFET Model Implementation
//!
//! This module implements BSIM3 (Level 49) DC model for SPICE simulation.
//! BSIM3 is the industry-standard model for 90nm+ CMOS processes.
//!
//! ## Module Structure
//!
//! - `params`: Model parameters (BsimParams) with defaults
//! - `types`: Enums and output structures (MosType, MosRegion, BsimOutput)
//! - `threshold`: Threshold voltage calculation with body effect, SCE, DIBL
//! - `mobility`: Mobility degradation with field and temperature effects
//! - `channel`: Vdsat, CLM, and output conductance calculations
//! - `evaluate`: Main DC evaluation entry point
//!
//! ## Usage
//!
//! ```ignore
//! use sim_devices::bsim::{BsimParams, evaluate_bsim_dc, MosType};
//!
//! // Create NMOS parameters
//! let params = BsimParams::nmos_default();
//!
//! // Evaluate DC operating point
//! let output = evaluate_bsim_dc(
//!     &params,
//!     1e-6,    // W = 1um
//!     100e-9,  // L = 100nm
//!     1.8,     // Vd
//!     1.2,     // Vg
//!     0.0,     // Vs
//!     0.0,     // Vb
//!     300.15,  // T = 27C
//! );
//!
//! println!("Ids = {:.3e} A", output.ids);
//! println!("Region: {:?}", output.region);
//! ```
//!
//! ## Supported Model Levels
//!
//! | Level | Model | Status |
//! |-------|-------|--------|
//! | 1 | Level 1 (Shichman-Hodges) | Supported via `evaluate_level1_dc` |
//! | 49 | BSIM3v3 | Core DC supported |
//! | 54 | BSIM4 | Future work |
//!
//! ## References
//!
//! - BSIM3v3.3 Manual, UC Berkeley Device Group
//! - Y. Cheng, C. Hu, "MOSFET Modeling & BSIM3 User's Guide"
//! - W. Liu, "MOSFET Models for SPICE Simulation"

pub mod params;
pub mod types;
pub mod threshold;
pub mod mobility;
pub mod channel;
pub mod evaluate;

// Re-export commonly used items
pub use params::BsimParams;
pub use types::{MosType, MosRegion, BsimOutput, BsimState};
pub use evaluate::{evaluate_bsim_dc, evaluate_level1_dc};

use std::collections::HashMap;

/// Build BsimParams from a parameter HashMap
///
/// Extracts BSIM parameters from the netlist parameter map.
/// Uses defaults for any unspecified parameters.
///
/// # Arguments
/// * `params` - HashMap of parameter name -> value string
/// * `level` - Model level (1, 49, 54)
/// * `is_pmos` - True for PMOS device
///
/// # Returns
/// * `BsimParams` with extracted values
pub fn build_bsim_params(
    params: &HashMap<String, String>,
    level: u32,
    is_pmos: bool,
) -> BsimParams {
    let mut p = if is_pmos {
        BsimParams::pmos_default()
    } else {
        BsimParams::nmos_default()
    };

    p.level = level;

    // Helper to parse parameter value
    let get_param = |keys: &[&str]| -> Option<f64> {
        for key in keys {
            let key_lower = key.to_ascii_lowercase();
            if let Some(value) = params.get(&key_lower) {
                if let Some(num) = parse_number(value) {
                    return Some(num);
                }
            }
        }
        None
    };

    // Threshold voltage parameters
    if let Some(v) = get_param(&["vth0", "vto", "vth"]) {
        p.vth0 = v;
    }
    if let Some(v) = get_param(&["k1"]) {
        p.k1 = v;
    }
    if let Some(v) = get_param(&["k2"]) {
        p.k2 = v;
    }
    if let Some(v) = get_param(&["dvt0"]) {
        p.dvt0 = v;
    }
    if let Some(v) = get_param(&["dvt1"]) {
        p.dvt1 = v;
    }
    if let Some(v) = get_param(&["dvt2"]) {
        p.dvt2 = v;
    }
    if let Some(v) = get_param(&["eta0"]) {
        p.eta0 = v;
    }
    if let Some(v) = get_param(&["dsub"]) {
        p.dsub = v;
    }
    if let Some(v) = get_param(&["nlx"]) {
        p.nlx = v;
    }
    if let Some(v) = get_param(&["nfactor"]) {
        p.nfactor = v;
    }

    // Mobility parameters
    if let Some(v) = get_param(&["u0", "uo"]) {
        p.u0 = v;
    }
    if let Some(v) = get_param(&["ua"]) {
        p.ua = v;
    }
    if let Some(v) = get_param(&["ub"]) {
        p.ub = v;
    }
    if let Some(v) = get_param(&["uc"]) {
        p.uc = v;
    }
    if let Some(v) = get_param(&["vsat"]) {
        p.vsat = v;
    }
    if let Some(v) = get_param(&["a0"]) {
        p.a0 = v;
    }
    if let Some(v) = get_param(&["ags"]) {
        p.ags = v;
    }

    // Output conductance parameters
    if let Some(v) = get_param(&["pclm"]) {
        p.pclm = v;
    }
    if let Some(v) = get_param(&["pdiblc1"]) {
        p.pdiblc1 = v;
    }
    if let Some(v) = get_param(&["pdiblc2"]) {
        p.pdiblc2 = v;
    }
    if let Some(v) = get_param(&["pdiblcb"]) {
        p.pdiblcb = v;
    }
    if let Some(v) = get_param(&["drout"]) {
        p.drout = v;
    }

    // Geometry parameters
    if let Some(v) = get_param(&["tox"]) {
        p.tox = v;
    }
    if let Some(v) = get_param(&["lint"]) {
        p.lint = v;
    }
    if let Some(v) = get_param(&["wint"]) {
        p.wint = v;
    }

    // Parasitic resistance
    if let Some(v) = get_param(&["rdsw"]) {
        p.rdsw = v;
    }
    if let Some(v) = get_param(&["rsh"]) {
        p.rsh = v;
    }

    // Temperature parameters
    if let Some(v) = get_param(&["tnom"]) {
        p.tnom = v + 273.15; // Convert C to K if given in C
    }
    if let Some(v) = get_param(&["ute"]) {
        p.ute = v;
    }
    if let Some(v) = get_param(&["kt1"]) {
        p.kt1 = v;
    }
    if let Some(v) = get_param(&["kt1l"]) {
        p.kt1l = v;
    }
    if let Some(v) = get_param(&["kt2"]) {
        p.kt2 = v;
    }

    // Capacitance parameters (for future use)
    if let Some(v) = get_param(&["cgso"]) {
        p.cgso = v;
    }
    if let Some(v) = get_param(&["cgdo"]) {
        p.cgdo = v;
    }
    if let Some(v) = get_param(&["cgbo"]) {
        p.cgbo = v;
    }

    p
}

/// Parse a number with optional SI suffix
fn parse_number(s: &str) -> Option<f64> {
    let lower = s.to_ascii_lowercase();
    let trimmed = lower.trim();

    // Check for SI suffixes
    let (num_str, multiplier) = if trimmed.ends_with("meg") {
        (&trimmed[..trimmed.len() - 3], 1e6)
    } else if trimmed.ends_with("mil") {
        (&trimmed[..trimmed.len() - 3], 25.4e-6)
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

    num_str.parse::<f64>().ok().map(|n| n * multiplier)
        .or_else(|| trimmed.parse::<f64>().ok())
}

/// Route to appropriate model evaluation based on level
///
/// # Arguments
/// * `params` - BSIM parameters
/// * `w` - Device width [m]
/// * `l` - Device length [m]
/// * `vd`, `vg`, `vs`, `vb` - Terminal voltages [V]
/// * `temp` - Temperature [K]
///
/// # Returns
/// * `BsimOutput` from the appropriate model
pub fn evaluate_mos(
    params: &BsimParams,
    w: f64,
    l: f64,
    vd: f64,
    vg: f64,
    vs: f64,
    vb: f64,
    temp: f64,
) -> BsimOutput {
    match params.level {
        1 => {
            // Level 1: Simple Shichman-Hodges model
            // Extract basic parameters
            let vth0 = params.vth0;
            let lambda = 0.02; // Default CLM for Level 1
            let beta = params.u0 * 1e-4 * params.cox(); // Beta from mobility and Cox

            evaluate_level1_dc(
                vth0,
                beta,
                lambda,
                w, l,
                vd, vg, vs, vb,
                params.mos_type == MosType::Pmos,
            )
        }
        49 | 54 => {
            // BSIM3 (49) or BSIM4 (54) - use full model
            evaluate_bsim_dc(params, w, l, vd, vg, vs, vb, temp)
        }
        _ => {
            // Default to BSIM3 for unknown levels
            evaluate_bsim_dc(params, w, l, vd, vg, vs, vb, temp)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_params_empty() {
        let params = HashMap::new();
        let p = build_bsim_params(&params, 49, false);
        assert_eq!(p.level, 49);
        assert_eq!(p.mos_type, MosType::Nmos);
    }

    #[test]
    fn test_build_params_with_values() {
        let mut params = HashMap::new();
        params.insert("vth0".to_string(), "0.5".to_string());
        params.insert("u0".to_string(), "400".to_string());
        params.insert("tox".to_string(), "2n".to_string());

        let p = build_bsim_params(&params, 49, false);
        assert!((p.vth0 - 0.5).abs() < 0.001);
        assert!((p.u0 - 400.0).abs() < 0.1);
        assert!((p.tox - 2e-9).abs() < 1e-12);
    }

    #[test]
    fn test_parse_number_suffixes() {
        assert!((parse_number("1.5").unwrap() - 1.5).abs() < 1e-10);
        assert!((parse_number("1n").unwrap() - 1e-9).abs() < 1e-15);
        assert!((parse_number("1u").unwrap() - 1e-6).abs() < 1e-12);
        assert!((parse_number("10k").unwrap() - 1e4).abs() < 1e-6);
        assert!((parse_number("2.5meg").unwrap() - 2.5e6).abs() < 1.0);
    }

    #[test]
    fn test_evaluate_mos_level1() {
        let params = BsimParams {
            level: 1,
            ..BsimParams::nmos_default()
        };
        let out = evaluate_mos(&params, 1e-6, 1e-6, 1.8, 1.5, 0.0, 0.0, 300.15);
        assert!(out.ids > 0.0);
    }

    #[test]
    fn test_evaluate_mos_level49() {
        let params = BsimParams {
            level: 49,
            ..BsimParams::nmos_default()
        };
        let out = evaluate_mos(&params, 1e-6, 1e-6, 1.8, 1.5, 0.0, 0.0, 300.15);
        assert!(out.ids > 0.0);
    }
}
