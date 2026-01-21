#[derive(Debug)]
pub enum SolverError {
    AnalyzeFailed,
    FactorFailed,
    SolveFailed,
}

pub trait LinearSolver {
    fn prepare(&mut self, n: usize);
    fn analyze(&mut self, ap: &[i64], ai: &[i64]) -> Result<(), SolverError>;
    fn factor(&mut self, ap: &[i64], ai: &[i64], ax: &[f64]) -> Result<(), SolverError>;
    fn solve(&mut self, rhs: &mut [f64]) -> Result<(), SolverError>;
    fn reset_pattern(&mut self);
}

pub struct KluSolver {
    pub n: usize,
    pub enabled: bool,
    last_ap_len: usize,
    last_ai_len: usize,
    #[cfg(feature = "klu")]
    symbolic: *mut klu_sys::klu_symbolic,
    #[cfg(feature = "klu")]
    numeric: *mut klu_sys::klu_numeric,
    #[cfg(feature = "klu")]
    common: klu_sys::klu_common,
}

impl KluSolver {
    pub fn new(n: usize) -> Self {
        let mut solver = Self {
            n,
            enabled: cfg!(feature = "klu"),
            last_ap_len: 0,
            last_ai_len: 0,
            #[cfg(feature = "klu")]
            symbolic: std::ptr::null_mut(),
            #[cfg(feature = "klu")]
            numeric: std::ptr::null_mut(),
            #[cfg(feature = "klu")]
            common: klu_sys::klu_common { status: 0 },
        };
        #[cfg(feature = "klu")]
        unsafe {
            klu_sys::klu_defaults(&mut solver.common as *mut klu_sys::klu_common);
        }
        solver
    }

    pub fn analyze(&mut self, ap: &[i64], ai: &[i64]) -> Result<(), SolverError> {
        if !self.enabled {
            return Err(SolverError::AnalyzeFailed);
        }
        if !self.symbolic.is_null()
            && self.last_ap_len == ap.len()
            && self.last_ai_len == ai.len()
        {
            return Ok(());
        }
        #[cfg(feature = "klu")]
        unsafe {
            if !self.symbolic.is_null() {
                klu_sys::klu_free_symbolic(&mut self.symbolic, &mut self.common);
            }
            self.symbolic = klu_sys::klu_analyze(
                self.n as i32,
                ap.as_ptr(),
                ai.as_ptr(),
                &mut self.common,
            );
            if self.symbolic.is_null() {
                return Err(SolverError::AnalyzeFailed);
            }
        }
        self.last_ap_len = ap.len();
        self.last_ai_len = ai.len();
        Ok(())
    }

    pub fn factor(&mut self, ap: &[i64], ai: &[i64], ax: &[f64]) -> Result<(), SolverError> {
        if !self.enabled {
            return Err(SolverError::FactorFailed);
        }
        #[cfg(feature = "klu")]
        unsafe {
            if !self.numeric.is_null() {
                klu_sys::klu_free_numeric(&mut self.numeric, &mut self.common);
            }
            self.numeric = klu_sys::klu_factor(
                ap.as_ptr(),
                ai.as_ptr(),
                ax.as_ptr(),
                self.symbolic,
                &mut self.common,
            );
            if self.numeric.is_null() {
                return Err(SolverError::FactorFailed);
            }
        }
        Ok(())
    }

    pub fn solve(&mut self, _rhs: &mut [f64]) -> Result<(), SolverError> {
        if !self.enabled {
            return Err(SolverError::SolveFailed);
        }
        #[cfg(feature = "klu")]
        unsafe {
            let ok = klu_sys::klu_solve(
                self.symbolic,
                self.numeric,
                self.n as i32,
                1,
                _rhs.as_mut_ptr(),
                &mut self.common,
            );
            if ok == 0 {
                return Err(SolverError::SolveFailed);
            }
        }
        Ok(())
    }

    pub fn reset_pattern(&mut self) {
        if !self.enabled {
            return;
        }
        #[cfg(feature = "klu")]
        unsafe {
            if !self.symbolic.is_null() {
                klu_sys::klu_free_symbolic(&mut self.symbolic, &mut self.common);
            }
            if !self.numeric.is_null() {
                klu_sys::klu_free_numeric(&mut self.numeric, &mut self.common);
            }
            self.symbolic = std::ptr::null_mut();
            self.numeric = std::ptr::null_mut();
        }
        self.last_ap_len = 0;
        self.last_ai_len = 0;
    }
}

impl LinearSolver for KluSolver {
    fn prepare(&mut self, n: usize) {
        if n != self.n {
            self.reset_pattern();
        }
        self.n = n;
    }

    fn analyze(&mut self, ap: &[i64], ai: &[i64]) -> Result<(), SolverError> {
        KluSolver::analyze(self, ap, ai)
    }

    fn factor(&mut self, ap: &[i64], ai: &[i64], ax: &[f64]) -> Result<(), SolverError> {
        KluSolver::factor(self, ap, ai, ax)
    }

    fn solve(&mut self, rhs: &mut [f64]) -> Result<(), SolverError> {
        KluSolver::solve(self, rhs)
    }

    fn reset_pattern(&mut self) {
        KluSolver::reset_pattern(self)
    }
}

impl LinearSolver for KluSolver {
    fn analyze(&mut self, ap: &[i64], ai: &[i64]) -> Result<(), SolverError> {
        KluSolver::analyze(self, ap, ai)
    }

    fn factor(&mut self, ap: &[i64], ai: &[i64], ax: &[f64]) -> Result<(), SolverError> {
        KluSolver::factor(self, ap, ai, ax)
    }

    fn solve(&mut self, rhs: &mut [f64]) -> Result<(), SolverError> {
        KluSolver::solve(self, rhs)
    }
}

pub fn debug_dump_solver() {
    println!("solver: klu solver stub");
}

impl Drop for KluSolver {
    fn drop(&mut self) {
        self.reset_pattern();
    }
}

#[cfg(feature = "klu")]
#[allow(non_camel_case_types)]
mod klu_sys {
    #[repr(C)]
    pub struct klu_symbolic;
    #[repr(C)]
    pub struct klu_numeric;
    #[repr(C)]
    pub struct klu_common {
        pub status: i32,
    }

    #[link(name = "klu")]
    extern "C" {
        pub fn klu_defaults(common: *mut klu_common) -> i32;
        pub fn klu_analyze(
            n: i32,
            ap: *const i64,
            ai: *const i64,
            common: *mut klu_common,
        ) -> *mut klu_symbolic;
        pub fn klu_factor(
            ap: *const i64,
            ai: *const i64,
            ax: *const f64,
            symbolic: *mut klu_symbolic,
            common: *mut klu_common,
        ) -> *mut klu_numeric;
        pub fn klu_solve(
            symbolic: *mut klu_symbolic,
            numeric: *mut klu_numeric,
            n: i32,
            nrhs: i32,
            b: *mut f64,
            common: *mut klu_common,
        ) -> i32;
        pub fn klu_free_symbolic(symbolic: *mut *mut klu_symbolic, common: *mut klu_common);
        pub fn klu_free_numeric(numeric: *mut *mut klu_numeric, common: *mut klu_common);
    }
}
