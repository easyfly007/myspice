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

#[derive(Debug)]
pub struct DenseSolver {
    pub n: usize,
    lu: Vec<f64>,
    pivots: Vec<usize>,
}

impl DenseSolver {
    pub fn new(n: usize) -> Self {
        Self {
            n,
            lu: vec![0.0; n * n],
            pivots: (0..n).collect(),
        }
    }

    fn ensure_capacity(&mut self, n: usize) {
        if self.n != n {
            self.n = n;
            self.lu.resize(n * n, 0.0);
            self.pivots = (0..n).collect();
        }
    }

    fn build_dense(&mut self, ap: &[i64], ai: &[i64], ax: &[f64]) -> Result<(), SolverError> {
        let n = self.n;
        if ap.len() != n + 1 {
            return Err(SolverError::AnalyzeFailed);
        }
        self.lu.fill(0.0);
        for col in 0..n {
            let start = ap[col] as usize;
            let end = ap[col + 1] as usize;
            for idx in start..end {
                let row = ai[idx] as usize;
                if row < n {
                    self.lu[row * n + col] += ax[idx];
                }
            }
        }
        Ok(())
    }

    fn factorize(&mut self) -> Result<(), SolverError> {
        let n = self.n;
        for i in 0..n {
            self.pivots[i] = i;
        }
        for k in 0..n {
            let mut pivot = k;
            let mut max_val = self.lu[k * n + k].abs();
            for i in (k + 1)..n {
                let val = self.lu[i * n + k].abs();
                if val > max_val {
                    max_val = val;
                    pivot = i;
                }
            }
            if max_val == 0.0 {
                return Err(SolverError::FactorFailed);
            }
            if pivot != k {
                for j in 0..n {
                    self.lu.swap(k * n + j, pivot * n + j);
                }
                self.pivots.swap(k, pivot);
            }
            let pivot_val = self.lu[k * n + k];
            for i in (k + 1)..n {
                let factor = self.lu[i * n + k] / pivot_val;
                self.lu[i * n + k] = factor;
                for j in (k + 1)..n {
                    self.lu[i * n + j] -= factor * self.lu[k * n + j];
                }
            }
        }
        Ok(())
    }
}

impl LinearSolver for DenseSolver {
    fn prepare(&mut self, n: usize) {
        self.ensure_capacity(n);
    }

    fn analyze(&mut self, _ap: &[i64], _ai: &[i64]) -> Result<(), SolverError> {
        Ok(())
    }

    fn factor(&mut self, ap: &[i64], ai: &[i64], ax: &[f64]) -> Result<(), SolverError> {
        self.build_dense(ap, ai, ax)?;
        self.factorize()
    }

    fn solve(&mut self, rhs: &mut [f64]) -> Result<(), SolverError> {
        let n = self.n;
        if rhs.len() != n {
            return Err(SolverError::SolveFailed);
        }
        let mut b = vec![0.0; n];
        for i in 0..n {
            b[i] = rhs[self.pivots[i]];
        }
        for i in 0..n {
            let mut sum = b[i];
            for j in 0..i {
                sum -= self.lu[i * n + j] * b[j];
            }
            b[i] = sum;
        }
        for i in (0..n).rev() {
            let mut sum = b[i];
            for j in (i + 1)..n {
                sum -= self.lu[i * n + j] * rhs[j];
            }
            let diag = self.lu[i * n + i];
            if diag == 0.0 {
                return Err(SolverError::SolveFailed);
            }
            rhs[i] = sum / diag;
        }
        Ok(())
    }

    fn reset_pattern(&mut self) {}
}

#[derive(Debug)]
pub struct DefaultSolver {
    inner: SolverImpl,
}

#[derive(Debug)]
enum SolverImpl {
    #[cfg(feature = "klu")]
    Klu(KluSolver),
    Dense(DenseSolver),
}

impl DefaultSolver {
    pub fn new(n: usize) -> Self {
        let inner = if cfg!(feature = "klu") {
            #[cfg(feature = "klu")]
            {
                SolverImpl::Klu(KluSolver::new(n))
            }
            #[cfg(not(feature = "klu"))]
            {
                SolverImpl::Dense(DenseSolver::new(n))
            }
        } else {
            SolverImpl::Dense(DenseSolver::new(n))
        };
        Self { inner }
    }
}

impl LinearSolver for DefaultSolver {
    fn prepare(&mut self, n: usize) {
        match &mut self.inner {
            #[cfg(feature = "klu")]
            SolverImpl::Klu(solver) => solver.prepare(n),
            SolverImpl::Dense(solver) => solver.prepare(n),
        }
    }

    fn analyze(&mut self, ap: &[i64], ai: &[i64]) -> Result<(), SolverError> {
        match &mut self.inner {
            #[cfg(feature = "klu")]
            SolverImpl::Klu(solver) => solver.analyze(ap, ai),
            SolverImpl::Dense(solver) => solver.analyze(ap, ai),
        }
    }

    fn factor(&mut self, ap: &[i64], ai: &[i64], ax: &[f64]) -> Result<(), SolverError> {
        match &mut self.inner {
            #[cfg(feature = "klu")]
            SolverImpl::Klu(solver) => solver.factor(ap, ai, ax),
            SolverImpl::Dense(solver) => solver.factor(ap, ai, ax),
        }
    }

    fn solve(&mut self, rhs: &mut [f64]) -> Result<(), SolverError> {
        match &mut self.inner {
            #[cfg(feature = "klu")]
            SolverImpl::Klu(solver) => solver.solve(rhs),
            SolverImpl::Dense(solver) => solver.solve(rhs),
        }
    }

    fn reset_pattern(&mut self) {
        match &mut self.inner {
            #[cfg(feature = "klu")]
            SolverImpl::Klu(solver) => solver.reset_pattern(),
            SolverImpl::Dense(solver) => solver.reset_pattern(),
        }
    }
}

pub struct KluSolver {
    pub n: usize,
    pub enabled: bool,
    last_ap: Vec<i64>,
    last_ai: Vec<i64>,
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
            last_ap: Vec::new(),
            last_ai: Vec::new(),
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
}

impl LinearSolver for KluSolver {
    fn prepare(&mut self, n: usize) {
        if n != self.n {
            self.reset_pattern();
        }
        self.n = n;
    }

    fn analyze(&mut self, ap: &[i64], ai: &[i64]) -> Result<(), SolverError> {
        if !self.enabled {
            return Err(SolverError::AnalyzeFailed);
        }
        #[cfg(feature = "klu")]
        {
            if !self.symbolic.is_null() && self.last_ap == ap && self.last_ai == ai {
                return Ok(());
            }
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
        self.last_ap = ap.to_vec();
        self.last_ai = ai.to_vec();
        Ok(())
    }

    fn factor(&mut self, ap: &[i64], ai: &[i64], ax: &[f64]) -> Result<(), SolverError> {
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

    fn solve(&mut self, rhs: &mut [f64]) -> Result<(), SolverError> {
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
                rhs.as_mut_ptr(),
                &mut self.common,
            );
            if ok == 0 {
                return Err(SolverError::SolveFailed);
            }
        }
        Ok(())
    }

    fn reset_pattern(&mut self) {
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
        self.last_ap.clear();
        self.last_ai.clear();
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
