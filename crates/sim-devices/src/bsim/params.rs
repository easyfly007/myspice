//! BSIM MOSFET model parameters
//!
//! Contains the BsimParams structure with all BSIM3 model parameters
//! and their default values for NMOS and PMOS devices.

use super::types::MosType;

/// Physical constants
pub const EPSILON_SI: f64 = 11.7 * 8.854e-12; // Silicon permittivity [F/m]
pub const EPSILON_OX: f64 = 3.9 * 8.854e-12; // Oxide permittivity [F/m]
pub const Q_ELECTRON: f64 = 1.602e-19; // Electron charge [C]
pub const K_BOLTZMANN: f64 = 1.381e-23; // Boltzmann constant [J/K]
pub const T_NOMINAL: f64 = 300.15; // Nominal temperature [K] (27C)

/// BSIM3 Model Parameters
///
/// This structure contains the key parameters for BSIM3 (Level 49) model.
/// Parameters are grouped by their physical function:
///
/// - Model selection: level, mos_type
/// - Threshold voltage: vth0, k1, k2, dvt0, dvt1, dvt2, eta0, dsub
/// - Mobility: u0, ua, ub, uc, vsat
/// - Short-channel effects: pclm, pdiblc1, pdiblc2
/// - Geometry: tox, lint, wint
/// - Parasitic: rdsw
/// - Temperature: tnom, ute, kt1
#[derive(Debug, Clone)]
pub struct BsimParams {
    // ============ Model Selection ============
    /// Model level: 1=Level1, 49=BSIM3, 54=BSIM4
    pub level: u32,
    /// Device type: NMOS or PMOS
    pub mos_type: MosType,

    // ============ Threshold Voltage ============
    /// Zero-bias threshold voltage [V]
    /// Physical meaning: Gate voltage needed to create inversion layer at Vbs=0
    pub vth0: f64,
    /// First-order body effect coefficient [V^0.5]
    /// Physical meaning: How Vth increases with reverse body bias (sqrt dependence)
    pub k1: f64,
    /// Second-order body effect coefficient [dimensionless]
    /// Physical meaning: Correction to first-order body effect
    pub k2: f64,
    /// Short-channel effect coefficient for Vth [dimensionless]
    /// Physical meaning: Controls Vth roll-off with channel length
    pub dvt0: f64,
    /// Short-channel effect exponent [dimensionless]
    /// Physical meaning: Exponential decay rate of SCE with length
    pub dvt1: f64,
    /// Body-bias coefficient for short-channel effect [1/V]
    /// Physical meaning: How body bias affects SCE
    pub dvt2: f64,
    /// DIBL (Drain-Induced Barrier Lowering) coefficient [dimensionless]
    /// Physical meaning: Vth reduction due to Vds
    pub eta0: f64,
    /// DIBL exponent [dimensionless]
    /// Physical meaning: Length dependence of DIBL effect
    pub dsub: f64,
    /// Narrow width effect coefficient [dimensionless]
    pub nlx: f64,
    /// Subthreshold swing coefficient [dimensionless]
    pub nfactor: f64,

    // ============ Mobility ============
    /// Low-field mobility [cm^2/V/s]
    /// Physical meaning: Carrier mobility without field degradation
    pub u0: f64,
    /// First-order mobility degradation coefficient [m/V]
    /// Physical meaning: Linear reduction of mobility with vertical field
    pub ua: f64,
    /// Second-order mobility degradation coefficient [(m/V)^2]
    /// Physical meaning: Quadratic mobility degradation with vertical field
    pub ub: f64,
    /// Body-bias mobility degradation coefficient [m/V^2]
    /// Physical meaning: How body bias affects mobility degradation
    pub uc: f64,
    /// Saturation velocity [m/s]
    /// Physical meaning: Maximum carrier velocity under high lateral field
    pub vsat: f64,
    /// Mobility reduction factor due to Rds [dimensionless]
    pub a0: f64,
    /// Gate-bias dependent Rds parameter [dimensionless]
    pub ags: f64,
    /// Source/drain resistance gate bias coefficient [1/V]
    pub prwg: f64,
    /// Source/drain resistance body bias coefficient [1/V^0.5]
    pub prwb: f64,

    // ============ Short-channel/Output Conductance ============
    /// Channel length modulation coefficient [dimensionless]
    /// Physical meaning: Controls increase of Ids with Vds in saturation
    pub pclm: f64,
    /// DIBL output resistance coefficient 1 [dimensionless]
    pub pdiblc1: f64,
    /// DIBL output resistance coefficient 2 [dimensionless]
    pub pdiblc2: f64,
    /// DIBL body bias coefficient [1/V]
    pub pdiblcb: f64,
    /// Drain-induced threshold shift coefficient [dimensionless]
    pub drout: f64,
    /// Subthreshold output conductance parameter [dimensionless]
    pub pscbe1: f64,
    /// Subthreshold output conductance exponent [V/m]
    pub pscbe2: f64,
    /// Substrate current body effect coefficient [1/V]
    pub alpha0: f64,
    /// Substrate current DIBL coefficient [V]
    pub beta0: f64,

    // ============ Geometry ============
    /// Gate oxide thickness [m]
    pub tox: f64,
    /// Channel length offset for Leff calculation [m]
    /// Leff = L - 2*LINT
    pub lint: f64,
    /// Channel width offset for Weff calculation [m]
    /// Weff = W - 2*WINT
    pub wint: f64,
    /// Minimum channel length for model validity [m]
    pub lmin: f64,
    /// Minimum channel width for model validity [m]
    pub wmin: f64,
    /// Effective length scaling parameter [dimensionless]
    pub lln: f64,
    /// Effective length scaling reference [m]
    pub lw: f64,
    /// Effective length scaling parameter [dimensionless]
    pub lwn: f64,
    /// Effective width scaling parameter [dimensionless]
    pub wln: f64,
    /// Effective width scaling reference [m]
    pub ww: f64,
    /// Effective width scaling parameter [dimensionless]
    pub wwn: f64,

    // ============ Parasitic Resistance ============
    /// Source/drain sheet resistance per unit width [ohm*um]
    /// Total Rds = RDSW / Weff
    pub rdsw: f64,
    /// Gate resistance per unit width [ohm*um]
    pub rsh: f64,

    // ============ Temperature ============
    /// Nominal temperature for parameter extraction [K]
    pub tnom: f64,
    /// Mobility temperature exponent [dimensionless]
    /// u(T) = u0 * (T/Tnom)^UTE
    pub ute: f64,
    /// Vth temperature coefficient [V]
    /// Vth(T) = Vth0 + KT1 * (T/Tnom - 1)
    pub kt1: f64,
    /// Vth temperature coefficient (length dependence) [V*m]
    pub kt1l: f64,
    /// Vth temperature coefficient (body bias) [V]
    pub kt2: f64,
    /// Saturation velocity temperature coefficient [m/s/K]
    pub at: f64,
    /// RDSW temperature coefficient [1/K]
    pub prt: f64,

    // ============ Capacitance (for future AC/transient) ============
    /// Gate-source overlap capacitance per unit width [F/m]
    pub cgso: f64,
    /// Gate-drain overlap capacitance per unit width [F/m]
    pub cgdo: f64,
    /// Gate-bulk overlap capacitance per unit width [F/m]
    pub cgbo: f64,
    /// Junction capacitance parameter [F/m^2]
    pub cj: f64,
    /// Junction sidewall capacitance [F/m]
    pub cjsw: f64,
    /// Junction built-in potential [V]
    pub pb: f64,
    /// Junction sidewall built-in potential [V]
    pub pbsw: f64,
    /// Junction grading coefficient [dimensionless]
    pub mj: f64,
    /// Junction sidewall grading coefficient [dimensionless]
    pub mjsw: f64,

    // ============ Flicker Noise (for future noise analysis) ============
    /// Flicker noise coefficient A [dimensionless]
    pub kf: f64,
    /// Flicker noise exponent [dimensionless]
    pub af: f64,
    /// Flicker noise frequency exponent [dimensionless]
    pub ef: f64,
}

impl Default for BsimParams {
    fn default() -> Self {
        Self::nmos_default()
    }
}

impl BsimParams {
    /// Create NMOS default parameters
    pub fn nmos_default() -> Self {
        BsimParams {
            // Model Selection
            level: 49,
            mos_type: MosType::Nmos,

            // Threshold Voltage
            vth0: 0.7,
            k1: 0.5,
            k2: 0.0,
            dvt0: 2.2,
            dvt1: 0.53,
            dvt2: -0.032,
            eta0: 0.08,
            dsub: 0.56,
            nlx: 1.74e-7,
            nfactor: 1.0,

            // Mobility
            u0: 500.0,    // cm^2/V/s for NMOS
            ua: 2.25e-9,  // m/V
            ub: 5.87e-19, // (m/V)^2
            uc: -4.65e-11,
            vsat: 1.5e5,  // m/s
            a0: 1.0,
            ags: 0.2,
            prwg: 0.0,
            prwb: 0.0,

            // Short-channel/Output Conductance
            pclm: 1.3,
            pdiblc1: 0.39,
            pdiblc2: 0.0086,
            pdiblcb: -0.1,
            drout: 0.56,
            pscbe1: 4.24e8,
            pscbe2: 1.0e-5,
            alpha0: 0.0,
            beta0: 30.0,

            // Geometry
            tox: 1.5e-8,  // 15nm
            lint: 0.0,
            wint: 0.0,
            lmin: 0.0,
            wmin: 0.0,
            lln: 1.0,
            lw: 0.0,
            lwn: 1.0,
            wln: 1.0,
            ww: 0.0,
            wwn: 1.0,

            // Parasitic
            rdsw: 0.0,
            rsh: 0.0,

            // Temperature
            tnom: T_NOMINAL,
            ute: -1.5,
            kt1: -0.11,
            kt1l: 0.0,
            kt2: 0.022,
            at: 3.3e4,
            prt: 0.0,

            // Capacitance
            cgso: 0.0,
            cgdo: 0.0,
            cgbo: 0.0,
            cj: 5.0e-4,
            cjsw: 5.0e-10,
            pb: 1.0,
            pbsw: 1.0,
            mj: 0.5,
            mjsw: 0.33,

            // Noise
            kf: 0.0,
            af: 1.0,
            ef: 1.0,
        }
    }

    /// Create PMOS default parameters
    pub fn pmos_default() -> Self {
        let mut params = Self::nmos_default();
        params.mos_type = MosType::Pmos;
        params.vth0 = -0.7;      // Negative for PMOS
        params.u0 = 150.0;       // Lower mobility for holes
        params.ute = -1.0;       // Different temp coefficient
        params.kt1 = -0.08;
        params
    }

    /// Calculate oxide capacitance per unit area [F/m^2]
    pub fn cox(&self) -> f64 {
        EPSILON_OX / self.tox
    }

    /// Calculate effective channel length [m]
    pub fn leff(&self, l: f64) -> f64 {
        (l - 2.0 * self.lint).max(1e-9)
    }

    /// Calculate effective channel width [m]
    pub fn weff(&self, w: f64) -> f64 {
        (w - 2.0 * self.wint).max(1e-9)
    }

    /// Calculate thermal voltage at given temperature [V]
    pub fn vt(&self, temp: f64) -> f64 {
        K_BOLTZMANN * temp / Q_ELECTRON
    }
}
