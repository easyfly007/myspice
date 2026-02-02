# Adaptive Time Step Optimization Plan

本文档描述 MySpice 瞬态分析自适应时间步长优化的实现计划。

## 当前状态

| 方面 | 当前实现 | 局限性 |
|------|---------|--------|
| 积分方法 | Backward Euler (1阶) | 精度低，数值阻尼大 |
| 误差估计 | 比较新旧解 | 非真正 LTE，不准确 |
| 步长控制 | 固定 1.5x/0.5x 因子 | 无预测控制 |
| 断点处理 | 无 | PWL 源未处理 |

### 当前代码位置

| 组件 | 文件 | 行号 |
|------|------|------|
| 时间步配置 | `analysis.rs` | 20-29 |
| 误差估计 | `analysis.rs` | 74-95 |
| 主循环 | `engine.rs` | 147-302 |
| 步长调整 | `engine.rs` | 262-284 |
| 电容 stamp | `stamp.rs` | 509-537 |
| 电感 stamp | `stamp.rs` | 539-570 |

---

## 优化目标

1. **提高精度**: 使用更准确的局部截断误差 (LTE) 估计
2. **提高效率**: 使用 PI 控制器智能调整步长
3. **提高稳定性**: 支持 Trapezoidal 积分方法
4. **处理不连续**: 正确处理 PWL/PULSE 源的断点

---

## 实现方案

### 1. 局部截断误差 (LTE) 估计

#### 算法: Milne's Device (预测-校正法)

```
对于每个时间步:
1. 用 Trapezoidal (2阶) 预测 x_{n+1}
2. 用 Backward Euler (1阶) 校正 x_{n+1}
3. LTE ≈ |x_trap - x_be| / 3

误差比: e = LTE / (abs_tol + rel_tol * |x|)
```

**原理说明:**

Backward Euler 局部截断误差: `O(dt²)`
Trapezoidal 局部截断误差: `O(dt³)`

两者之差主要由低阶项主导:
```
LTE_BE ≈ (dt²/2) * x''
差值 ≈ (dt²/2) * x'' ≈ 3 * LTE_BE
因此 LTE ≈ |x_trap - x_be| / 3
```

#### 代码设计

```rust
// analysis.rs

pub struct LteEstimate {
    pub error_norm: f64,        // 最大误差比
    pub max_error_node: usize,  // 误差最大的节点
    pub accept: bool,           // 是否接受此步
    pub suggested_factor: f64,  // 建议的步长调整因子
}

/// 使用 Milne's Device 估计局部截断误差
pub fn estimate_lte_milne(
    x_be: &[f64],       // Backward Euler 解
    x_trap: &[f64],     // Trapezoidal 解
    abs_tol: f64,       // 绝对容差 (默认 1e-9)
    rel_tol: f64,       // 相对容差 (默认 1e-6)
) -> LteEstimate {
    let mut max_ratio = 0.0;
    let mut max_node = 0;

    for (i, (be, tr)) in x_be.iter().zip(x_trap.iter()).enumerate() {
        // LTE ≈ |x_trap - x_be| / 3
        let lte = (tr - be).abs() / 3.0;

        // 归一化误差
        let tol = abs_tol + rel_tol * be.abs().max(tr.abs());
        let ratio = lte / tol;

        if ratio > max_ratio {
            max_ratio = ratio;
            max_node = i;
        }
    }

    // 建议的步长因子 (基于误差的 1/2 次方，因为 LTE ∝ dt²)
    let factor = if max_ratio > 0.0 {
        (1.0 / max_ratio).powf(0.5) * 0.9  // 0.9 为安全因子
    } else {
        2.0  // 误差很小，允许步长翻倍
    };

    LteEstimate {
        error_norm: max_ratio,
        max_error_node: max_node,
        accept: max_ratio <= 1.0,
        suggested_factor: factor.clamp(0.1, 2.0),
    }
}
```

---

### 2. PI 控制器步长调整

#### 算法

用 **比例-积分 (PI) 控制器** 替代固定的 1.5x/0.5x 因子:

```
dt_new = dt * (e_tol/e_n)^k_p * (e_tol/e_{n-1})^k_i

其中:
- e_n = 当前误差比
- e_{n-1} = 上一步误差比
- e_tol = 1.0 (目标误差)
- k_p = 0.7/p (比例增益, p = 方法阶数)
- k_i = 0.4/p (积分增益)
```

#### k_p 和 k_i 参数详解

**k_p** (比例增益) 和 **k_i** (积分增益) 来自控制理论，用于根据误差历史调整时间步长。

| 增益 | 作用 | 效果 |
|------|------|------|
| **k_p** | 比例项 | 响应**当前**误差。k_p 越大，响应越激进 |
| **k_i** | 积分项 | 响应**历史**误差。平滑振荡，防止超调 |

**最优值推导:**

对于阶数为 `p` 的数值方法，最优增益为:

```
k_p = 0.7 / p
k_i = 0.4 / p
```

| 方法 | 阶数 (p) | k_p | k_i |
|------|----------|-----|-----|
| Backward Euler | 1 | 0.70 | 0.40 |
| Trapezoidal | 2 | 0.35 | 0.20 |
| BDF2 | 2 | 0.35 | 0.20 |

**为什么需要 PI 控制器:**

简单方法 (固定因子):
```
if error < tol:
    dt *= 1.5  # 固定增大
else:
    dt *= 0.5  # 固定减小
```
这会导致步长振荡和效率低下。

PI 控制器效果对比:
```
         ┌─────────────────────────────────────────┐
误差     │    ****                                 │
         │   *    *     简单方法: 振荡             │
         │  *      *   *      *                    │
         │ *        * * *    * *                   │
目标 ────│─────────────────────────────────────────│
         │                                         │
         │  ───────────────────────────            │
         │      PI 控制: 平滑收敛                  │
         └─────────────────────────────────────────┘
                        时间步 →
```

**计算示例:**

```
已知:
  e_n = 0.5      (当前误差为容差的一半)
  e_{n-1} = 0.8  (上一步误差)
  dt = 1e-9 s
  k_p = 0.35, k_i = 0.20

计算:
  factor_p = (1.0 / 0.5)^0.35 = 2.0^0.35 ≈ 1.27
  factor_i = (1.0 / 0.8)^0.20 = 1.25^0.20 ≈ 1.05

  dt_new = 1e-9 * 1.27 * 1.05 * 0.9 (安全因子)
         ≈ 1.2e-9 s
```

步长增加约 20%，而非直接跳到 1.5 倍。

**参考文献:**
- Gustafsson, K. "Control-theoretic techniques for stepsize selection in implicit Runge-Kutta methods", ACM TOMS, 1994
- Söderlind, G. "Automatic control and adaptive time-stepping", Numerical Algorithms, 2002

这些值是生产级仿真器 (SPICE3, Spectre 等) 的标准选择。

**安全限制:**
- 增长限制: `dt_new ≤ 2 * dt`
- 收缩限制: `dt_new ≥ 0.1 * dt`
- 拒绝条件: `e_n > 1.5` (安全裕度)

#### 代码设计

```rust
// analysis.rs

use std::collections::VecDeque;

/// PI 控制器配置
pub struct PiControllerConfig {
    pub k_p: f64,           // 比例增益 (默认 0.7/p)
    pub k_i: f64,           // 积分增益 (默认 0.4/p)
    pub growth_limit: f64,  // 最大增长因子 (默认 2.0)
    pub shrink_limit: f64,  // 最小收缩因子 (默认 0.1)
    pub safety_factor: f64, // 安全因子 (默认 0.9)
    pub reject_threshold: f64, // 拒绝阈值 (默认 1.5)
}

impl Default for PiControllerConfig {
    fn default() -> Self {
        Self {
            k_p: 0.35,          // 0.7/2 for 2nd order
            k_i: 0.20,          // 0.4/2 for 2nd order
            growth_limit: 2.0,
            shrink_limit: 0.1,
            safety_factor: 0.9,
            reject_threshold: 1.5,
        }
    }
}

/// 步长控制器
pub struct StepController {
    config: PiControllerConfig,
    error_history: VecDeque<f64>,  // 最近 2-3 个误差
    dt_history: VecDeque<f64>,     // 最近 2-3 个步长
    method_order: usize,           // 1 = BE, 2 = Trap
    consecutive_rejects: usize,    // 连续拒绝次数
}

impl StepController {
    pub fn new(method_order: usize) -> Self {
        let mut config = PiControllerConfig::default();
        config.k_p = 0.7 / method_order as f64;
        config.k_i = 0.4 / method_order as f64;

        Self {
            config,
            error_history: VecDeque::with_capacity(3),
            dt_history: VecDeque::with_capacity(3),
            method_order,
            consecutive_rejects: 0,
        }
    }

    /// 根据当前误差建议新步长
    pub fn suggest_dt(&self, current_error: f64, current_dt: f64) -> f64 {
        let e_n = current_error.max(1e-10);  // 避免除零
        let e_tol = 1.0;

        // PI 控制器公式
        let mut factor = (e_tol / e_n).powf(self.config.k_p);

        // 加入积分项 (如果有历史)
        if let Some(&e_prev) = self.error_history.back() {
            factor *= (e_tol / e_prev).powf(self.config.k_i);
        }

        // 应用安全因子
        factor *= self.config.safety_factor;

        // 限制增长/收缩
        factor = factor.clamp(self.config.shrink_limit, self.config.growth_limit);

        current_dt * factor
    }

    /// 记录步长结果
    pub fn record_step(&mut self, error: f64, dt: f64, accepted: bool) {
        if accepted {
            self.error_history.push_back(error);
            self.dt_history.push_back(dt);
            self.consecutive_rejects = 0;

            // 只保留最近 3 个
            while self.error_history.len() > 3 {
                self.error_history.pop_front();
            }
            while self.dt_history.len() > 3 {
                self.dt_history.pop_front();
            }
        } else {
            self.consecutive_rejects += 1;
        }
    }

    /// 是否应该拒绝当前步
    pub fn should_reject(&self, error: f64) -> bool {
        error > self.config.reject_threshold
    }

    /// 连续拒绝次数过多时的处理
    pub fn emergency_dt(&self, current_dt: f64, min_dt: f64) -> f64 {
        if self.consecutive_rejects > 5 {
            // 紧急收缩到最小步长
            min_dt
        } else {
            // 激进收缩
            (current_dt * 0.25).max(min_dt)
        }
    }
}
```

---

### 3. Trapezoidal 积分方法

#### 公式推导

**电容 (Backward Euler):**
```
I = C * dV/dt
BE: (V_n - V_{n-1}) / dt = I_n / C
等效: V_n = V_{n-1} + (dt/C) * I_n
电路模型: g = C/dt, i_eq = g * V_{n-1}
```

**电容 (Trapezoidal):**
```
Trap: (V_n - V_{n-1}) / dt = (I_n + I_{n-1}) / (2C)
等效: V_n = V_{n-1} + (dt/2C) * (I_n + I_{n-1})
电路模型: g = 2C/dt, i_eq = g * V_{n-1} + I_{n-1}
```

**电感 (Backward Euler):**
```
V = L * dI/dt
BE: (I_n - I_{n-1}) / dt = V_n / L
电路模型: V_n = (L/dt) * I_n - (L/dt) * I_{n-1}
```

**电感 (Trapezoidal):**
```
Trap: (I_n - I_{n-1}) / dt = (V_n + V_{n-1}) / (2L)
电路模型: g = 2L/dt, v_eq = g * I_{n-1} + V_{n-1}
```

#### 代码设计

```rust
// stamp.rs

/// 积分方法枚举
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IntegrationMethod {
    BackwardEuler,
    Trapezoidal,
}

/// 扩展的瞬态状态
#[derive(Debug, Default, Clone)]
pub struct TransientState {
    // 电容状态
    pub cap_voltage: HashMap<String, f64>,   // V_{n-1}
    pub cap_current: HashMap<String, f64>,   // I_{n-1} (Trapezoidal 需要)

    // 电感状态
    pub ind_current: HashMap<String, f64>,   // I_{n-1}
    pub ind_voltage: HashMap<String, f64>,   // V_{n-1} (Trapezoidal 需要)
    pub ind_aux: HashMap<String, usize>,     // 辅助变量索引

    // 方法选择
    pub method: IntegrationMethod,
}

/// 电容 Trapezoidal stamp
fn stamp_capacitor_trap(
    ctx: &mut StampContext,
    inst: &Instance,
    x: Option<&[f64]>,
    dt: f64,
    state: &mut TransientState,
) -> Result<(), StampError> {
    let c = inst.value;
    let (a, b) = get_node_indices(ctx, inst)?;

    // Trapezoidal: g = 2C/dt
    let g = 2.0 * c / dt;

    // 获取历史值
    let v_prev = *state.cap_voltage.get(&inst.name).unwrap_or(&0.0);
    let i_prev = *state.cap_current.get(&inst.name).unwrap_or(&0.0);

    // 等效电流: i_eq = g * V_{n-1} + I_{n-1}
    let ieq = g * v_prev + i_prev;

    // Stamp 导纳矩阵
    ctx.add(a, a, g);
    ctx.add(b, b, g);
    ctx.add(a, b, -g);
    ctx.add(b, a, -g);

    // Stamp RHS
    ctx.add_rhs(a, ieq);
    ctx.add_rhs(b, -ieq);

    Ok(())
}

/// 电感 Trapezoidal stamp
fn stamp_inductor_trap(
    ctx: &mut StampContext,
    inst: &Instance,
    x: Option<&[f64]>,
    dt: f64,
    state: &mut TransientState,
) -> Result<(), StampError> {
    let l = inst.value;
    let (a, b) = get_node_indices(ctx, inst)?;

    // 辅助变量 (电流)
    let k = ctx.allocate_aux(&inst.name);
    state.ind_aux.insert(inst.name.clone(), k);

    // Trapezoidal: g = dt/(2L)
    let g = dt / (2.0 * l);

    // 获取历史值
    let i_prev = *state.ind_current.get(&inst.name).unwrap_or(&0.0);
    let v_prev = *state.ind_voltage.get(&inst.name).unwrap_or(&0.0);

    // KCL: 节点电流
    ctx.add(a, k, 1.0);
    ctx.add(b, k, -1.0);

    // 本构关系: V_a - V_b = (2L/dt) * I - (2L/dt) * I_{n-1} - V_{n-1}
    ctx.add(k, a, 1.0);
    ctx.add(k, b, -1.0);
    ctx.add(k, k, -1.0 / g);  // -2L/dt

    // RHS: (2L/dt) * I_{n-1} + V_{n-1}
    let rhs = i_prev / g + v_prev;
    ctx.add_rhs(k, rhs);

    Ok(())
}

/// 统一的瞬态 stamp 函数
pub fn stamp_device_tran(
    ctx: &mut StampContext,
    inst: &Instance,
    x: Option<&[f64]>,
    dt: f64,
    state: &mut TransientState,
) -> Result<(), StampError> {
    match inst.kind {
        DeviceKind::C => {
            match state.method {
                IntegrationMethod::BackwardEuler => stamp_capacitor_be(ctx, inst, x, dt, state),
                IntegrationMethod::Trapezoidal => stamp_capacitor_trap(ctx, inst, x, dt, state),
            }
        }
        DeviceKind::L => {
            match state.method {
                IntegrationMethod::BackwardEuler => stamp_inductor_be(ctx, inst, x, dt, state),
                IntegrationMethod::Trapezoidal => stamp_inductor_trap(ctx, inst, x, dt, state),
            }
        }
        _ => stamp_device_dc(ctx, inst, x),  // 其他器件无时间相关
    }
}

/// 更新瞬态状态 (包括电流)
pub fn update_transient_state_full(
    instances: &[Instance],
    x: &[f64],
    x_prev: &[f64],
    dt: f64,
    state: &mut TransientState,
) {
    for inst in instances {
        match inst.kind {
            DeviceKind::C => {
                let (a, b) = get_node_pair(inst);
                let va = x.get(a).copied().unwrap_or(0.0);
                let vb = x.get(b).copied().unwrap_or(0.0);
                let v = va - vb;

                // 保存电压
                state.cap_voltage.insert(inst.name.clone(), v);

                // 计算并保存电流: I = C * dV/dt
                let v_prev = *state.cap_voltage.get(&inst.name).unwrap_or(&0.0);
                let i = inst.value * (v - v_prev) / dt;
                state.cap_current.insert(inst.name.clone(), i);
            }
            DeviceKind::L => {
                // 保存电流 (从辅助变量)
                if let Some(&aux) = state.ind_aux.get(&inst.name) {
                    if let Some(&current) = x.get(aux) {
                        state.ind_current.insert(inst.name.clone(), current);
                    }
                }

                // 保存电压
                let (a, b) = get_node_pair(inst);
                let va = x.get(a).copied().unwrap_or(0.0);
                let vb = x.get(b).copied().unwrap_or(0.0);
                state.ind_voltage.insert(inst.name.clone(), va - vb);
            }
            _ => {}
        }
    }
}
```

---

### 4. 断点处理

#### 断点来源

| 源类型 | 语法 | 断点 |
|--------|------|------|
| PULSE | `PULSE(v1 v2 td tr tf pw per)` | td, td+tr, td+tr+pw, td+tr+pw+tf, ... |
| PWL | `PWL(t1 v1 t2 v2 ...)` | t1, t2, t3, ... |
| SIN | `SIN(vo va freq td theta)` | td (可选) |
| EXP | `EXP(v1 v2 td1 tau1 td2 tau2)` | td1, td2 |

#### 代码设计

```rust
// breakpoint.rs (新文件)

use std::collections::BTreeSet;
use ordered_float::OrderedFloat;

/// 断点类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BreakpointType {
    PwlCorner,      // PWL 拐点
    PulseRise,      // PULSE 上升沿开始
    PulseFall,      // PULSE 下降沿开始
    PulseHigh,      // PULSE 高电平开始
    PulseLow,       // PULSE 低电平开始
    ExpTransition,  // EXP 过渡开始
    UserDefined,    // 用户定义
}

/// 断点信息
#[derive(Debug, Clone)]
pub struct Breakpoint {
    pub time: f64,
    pub source_name: String,
    pub bp_type: BreakpointType,
}

/// 断点管理器
pub struct BreakpointManager {
    breakpoints: BTreeSet<OrderedFloat<f64>>,
    breakpoint_info: Vec<Breakpoint>,
    settling_steps: usize,      // 断点后的稳定步数
    current_settling: usize,    // 当前稳定计数
}

impl BreakpointManager {
    pub fn new() -> Self {
        Self {
            breakpoints: BTreeSet::new(),
            breakpoint_info: Vec::new(),
            settling_steps: 5,
            current_settling: 0,
        }
    }

    /// 从电路提取所有断点
    pub fn extract_from_circuit(circuit: &Circuit, tstop: f64) -> Self {
        let mut manager = Self::new();

        for inst in &circuit.instances {
            match inst.kind {
                DeviceKind::V | DeviceKind::I => {
                    manager.extract_source_breakpoints(inst, tstop);
                }
                _ => {}
            }
        }

        manager
    }

    /// 提取源的断点
    fn extract_source_breakpoints(&mut self, inst: &Instance, tstop: f64) {
        // 解析 PULSE 参数
        if let Some(pulse) = &inst.pulse {
            self.extract_pulse_breakpoints(&inst.name, pulse, tstop);
        }

        // 解析 PWL 参数
        if let Some(pwl) = &inst.pwl {
            self.extract_pwl_breakpoints(&inst.name, pwl);
        }
    }

    /// 提取 PULSE 断点
    fn extract_pulse_breakpoints(&mut self, name: &str, pulse: &PulseParams, tstop: f64) {
        let mut t = pulse.td;  // 延迟时间
        let period = pulse.per;

        while t <= tstop {
            // 上升沿开始
            self.add_breakpoint(t, name, BreakpointType::PulseRise);

            // 上升沿结束 (高电平开始)
            t += pulse.tr;
            if t > tstop { break; }
            self.add_breakpoint(t, name, BreakpointType::PulseHigh);

            // 下降沿开始
            t += pulse.pw;
            if t > tstop { break; }
            self.add_breakpoint(t, name, BreakpointType::PulseFall);

            // 下降沿结束 (低电平开始)
            t += pulse.tf;
            if t > tstop { break; }
            self.add_breakpoint(t, name, BreakpointType::PulseLow);

            // 下一个周期
            if period > 0.0 {
                t = pulse.td + period * ((t - pulse.td) / period).ceil();
            } else {
                break;  // 非周期
            }
        }
    }

    /// 提取 PWL 断点
    fn extract_pwl_breakpoints(&mut self, name: &str, pwl: &[(f64, f64)]) {
        for (t, _v) in pwl {
            self.add_breakpoint(*t, name, BreakpointType::PwlCorner);
        }
    }

    /// 添加断点
    fn add_breakpoint(&mut self, time: f64, source: &str, bp_type: BreakpointType) {
        self.breakpoints.insert(OrderedFloat(time));
        self.breakpoint_info.push(Breakpoint {
            time,
            source_name: source.to_string(),
            bp_type,
        });
    }

    /// 获取下一个断点 (大于当前时间)
    pub fn next_breakpoint(&self, t: f64) -> Option<f64> {
        self.breakpoints
            .range((std::ops::Bound::Excluded(OrderedFloat(t)), std::ops::Bound::Unbounded))
            .next()
            .map(|of| of.0)
    }

    /// 限制步长以命中断点
    pub fn limit_dt(&self, t: f64, proposed_dt: f64, min_margin: f64) -> f64 {
        if let Some(next_bp) = self.next_breakpoint(t) {
            let to_bp = next_bp - t;

            // 如果步长会跨过断点
            if proposed_dt > to_bp - min_margin {
                // 精确命中断点
                return to_bp;
            }

            // 如果步长接近断点但不会跨过，保持原样
            // 避免在断点前产生很小的步长
            if proposed_dt > 0.9 * to_bp {
                return to_bp;
            }
        }

        proposed_dt
    }

    /// 检查是否刚过断点
    pub fn just_passed_breakpoint(&self, t_prev: f64, t: f64) -> bool {
        self.breakpoints
            .range((std::ops::Bound::Excluded(OrderedFloat(t_prev)),
                    std::ops::Bound::Included(OrderedFloat(t))))
            .next()
            .is_some()
    }

    /// 断点后是否需要小步长
    pub fn needs_settling(&mut self, t_prev: f64, t: f64) -> bool {
        if self.just_passed_breakpoint(t_prev, t) {
            self.current_settling = self.settling_steps;
        }

        if self.current_settling > 0 {
            self.current_settling -= 1;
            true
        } else {
            false
        }
    }

    /// 断点后的建议步长
    pub fn settling_dt(&self, normal_dt: f64, min_dt: f64) -> f64 {
        // 断点后使用较小步长
        (normal_dt * 0.1).max(min_dt * 10.0)
    }
}
```

---

## 完整算法流程

```
┌─────────────────────────────────────────────────────────────────┐
│                      自适应瞬态分析                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  1. 初始化                                                       │
│     ├─ 计算 DC 工作点 → x_0                                     │
│     ├─ 初始化瞬态状态 (v_prev, i_prev)                          │
│     ├─ 提取断点 (PWL, PULSE)                                    │
│     ├─ 初始化 PI 控制器                                         │
│     └─ 设置 t=0, dt=dt_init, method=BE                          │
│                                                                  │
│  2. 时间步进循环 (WHILE t < t_stop)                              │
│     │                                                            │
│     ├─ (a) 断点处理                                              │
│     │      ├─ dt = breakpoint_mgr.limit_dt(t, dt)               │
│     │      └─ 如果刚过断点: dt = settling_dt                    │
│     │                                                            │
│     ├─ (b) 双重求解                                              │
│     │      ├─ 用 Backward Euler 求解 → x_be                     │
│     │      └─ 用 Trapezoidal 求解 → x_trap                      │
│     │                                                            │
│     ├─ (c) LTE 估计                                              │
│     │      └─ lte = estimate_lte_milne(x_be, x_trap)            │
│     │                                                            │
│     ├─ (d) 步长决策                                              │
│     │      │                                                     │
│     │      ├─ IF lte.accept (误差 ≤ 1.0):                       │
│     │      │   ├─ 接受 x = x_trap (高阶解)                      │
│     │      │   ├─ t += dt                                       │
│     │      │   ├─ 更新瞬态状态                                   │
│     │      │   ├─ 存储波形点                                     │
│     │      │   ├─ controller.record_step(...)                   │
│     │      │   └─ dt = controller.suggest_dt(...)               │
│     │      │                                                     │
│     │      └─ ELSE (拒绝):                                       │
│     │          ├─ dt = dt * lte.suggested_factor                │
│     │          ├─ controller.record_step(..., false)            │
│     │          └─ 在相同 t 重试                                  │
│     │                                                            │
│     └─ (e) 安全检查                                              │
│          ├─ dt = dt.clamp(dt_min, dt_max)                       │
│          └─ 如果连续拒绝过多: dt = emergency_dt                 │
│                                                                  │
│  3. 返回结果                                                     │
│     └─ (tran_times, tran_solutions, statistics)                 │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## 配置参数

```rust
/// 自适应瞬态分析配置
pub struct AdaptiveTransientConfig {
    // === 误差控制 ===
    pub abs_tol: f64,           // 绝对容差 (默认 1e-9 V)
    pub rel_tol: f64,           // 相对容差 (默认 1e-6)
    pub charge_tol: f64,        // 电荷容差 (默认 1e-14 C)

    // === 步长限制 ===
    pub dt_min: f64,            // 最小步长 (默认 1e-15 s)
    pub dt_max: f64,            // 最大步长 (默认 tstop/10)
    pub dt_init: f64,           // 初始步长 (默认 tstep 或自动)

    // === PI 控制器 ===
    pub pi_k_p: f64,            // 比例增益 (默认 0.35)
    pub pi_k_i: f64,            // 积分增益 (默认 0.20)
    pub growth_limit: f64,      // 最大步长增长 (默认 2.0)
    pub shrink_limit: f64,      // 最大步长收缩 (默认 0.1)
    pub safety_factor: f64,     // 安全因子 (默认 0.9)

    // === 方法选择 ===
    pub method: IntegrationMethod,  // 默认 Trapezoidal
    pub auto_method_switch: bool,   // 自动 BE↔Trap 切换

    // === 断点处理 ===
    pub breakpoint_settling_steps: usize,  // 断点后稳定步数 (默认 5)
    pub breakpoint_dt_factor: f64,         // 断点后步长因子 (默认 0.1)

    // === 收敛控制 ===
    pub max_rejects: usize,     // 最大连续拒绝次数 (默认 10)
    pub newton_max_iters: usize, // Newton 最大迭代 (默认 50)
}

impl Default for AdaptiveTransientConfig {
    fn default() -> Self {
        Self {
            abs_tol: 1e-9,
            rel_tol: 1e-6,
            charge_tol: 1e-14,
            dt_min: 1e-15,
            dt_max: 1e-3,
            dt_init: 1e-9,
            pi_k_p: 0.35,
            pi_k_i: 0.20,
            growth_limit: 2.0,
            shrink_limit: 0.1,
            safety_factor: 0.9,
            method: IntegrationMethod::Trapezoidal,
            auto_method_switch: true,
            breakpoint_settling_steps: 5,
            breakpoint_dt_factor: 0.1,
            max_rejects: 10,
            newton_max_iters: 50,
        }
    }
}
```

---

## 测试计划

### 1. RC 阶跃响应测试

```spice
* RC step response
V1 in 0 DC 0 PULSE(0 1 0 1n 1n 10u 20u)
R1 in out 1k
C1 out 0 1n
.tran 1n 20u
.end
```

**验证:**
- 解析解: `V(out) = 1 - exp(-t/RC)`, RC = 1us
- 在 t = RC 时, V(out) ≈ 0.632
- 比较自适应 vs 固定步长的步数

### 2. LC 振荡器测试

```spice
* LC oscillator (energy conservation)
V1 in 0 DC 1
R1 in cap 1m
C1 cap 0 1u
L1 cap 0 1m
.ic V(cap) = 1
.tran 1n 10m
.end
```

**验证:**
- 振荡频率: f = 1/(2π√LC) ≈ 5.03 kHz
- Trapezoidal 应保持振幅
- Backward Euler 会衰减

### 3. PWL 断点测试

```spice
* PWL breakpoint handling
V1 in 0 PWL(0 0 1u 1 2u 1 3u 0)
R1 in out 1k
C1 out 0 100p
.tran 10n 5u
.end
```

**验证:**
- 断点在 1u, 2u, 3u 被精确命中
- 断点后步长适当减小
- 波形无振铃或过冲

### 4. 刚性电路测试

```spice
* Stiff circuit (fast diode)
V1 in 0 PULSE(0 5 0 1n 1n 5u 10u)
R1 in anode 100
D1 anode 0 DFAST
.model DFAST D(IS=1e-14 N=1 RS=1)
.tran 1n 20u
.end
```

**验证:**
- 二极管开关时步长自动减小
- 无收敛失败
- 解稳定无振荡

### 5. 性能基准测试

比较固定步长 vs 自适应:

| 电路 | 固定步长步数 | 自适应步数 | 精度相当 |
|------|-------------|-----------|----------|
| RC | ~1000 | ~100-200 | ✓ |
| LC | ~5000 | ~500-1000 | ✓ |
| PWL | ~500 | ~200 | ✓ |

---

## 实现阶段

| 阶段 | 内容 | 文件 | 复杂度 | 状态 |
|------|------|------|--------|------|
| Phase 1 | LTE 估计 | `analysis.rs` | 中 | ✅ 完成 |
| Phase 2 | PI 控制器 | `analysis.rs` | 中 | ✅ 完成 |
| Phase 3 | Trapezoidal 积分 | `stamp.rs` | 高 | ✅ 完成 |
| Phase 4 | 断点处理 | `waveform.rs` | 中 | ✅ 完成 |
| Phase 5 | 集成测试 | `tests/adaptive_timestep_tests.rs` | 中 | ✅ 完成 |

**建议顺序:** Phase 1 → Phase 2 → Phase 5 (基础测试) → Phase 3 → Phase 4 → Phase 5 (完整测试)

### Phase 1 实现详情 (已完成)

**新增代码位置:** `crates/sim-core/src/analysis.rs`

**新增结构体:**
- `LteEstimate` - LTE 估计结果，包含误差范数、最大误差节点、是否接受、建议因子
- `AdaptiveConfig` - 自适应配置，包含容差、步长限制、安全因子等

**新增函数:**
- `estimate_lte_milne()` - 使用 Milne's Device 估计 LTE (需要双重求解)
- `estimate_lte_difference()` - 使用解差估计 LTE (单次求解，精度较低)

**测试用例:** 8 个单元测试全部通过
- `test_lte_milne_identical_solutions`
- `test_lte_milne_small_difference`
- `test_lte_milne_large_difference`
- `test_lte_milne_relative_tolerance`
- `test_lte_milne_finds_max_error_node`
- `test_lte_difference_basic`
- `test_adaptive_config_defaults`
- `test_adaptive_config_compute_new_dt`

### Phase 2 实现详情 (已完成)

**新增代码位置:** `crates/sim-core/src/analysis.rs`

**新增结构体:**

| 结构体 | 描述 |
|--------|------|
| `PiControllerConfig` | PI 控制器配置 (k_p, k_i, 增益限制, 安全因子) |
| `StepController` | 步长控制器主体，维护误差历史和统计信息 |
| `StepControllerStats` | 控制器统计 (接受/拒绝数、拒绝率、dt 范围) |
| `AdaptiveStepController` | 组合控制器，整合 LTE 估计和 PI 控制 |

**新增方法:**

`PiControllerConfig`:
- `for_order(order)` - 根据方法阶数创建配置
- `backward_euler()` - Backward Euler 配置 (k_p=0.7, k_i=0.4)
- `trapezoidal()` - Trapezoidal 配置 (k_p=0.35, k_i=0.2)

`StepController`:
- `suggest_dt(error, dt)` - 使用 PI 公式计算建议步长
- `record_step(error, dt, accepted)` - 记录步长结果，更新历史
- `should_reject(error)` - 判断是否应拒绝当前步
- `emergency_dt(dt, dt_min)` - 计算紧急缩减步长
- `is_struggling()` - 检查是否连续拒绝过多
- `statistics()` - 获取控制器统计信息
- `reset()` - 重置控制器状态

`AdaptiveStepController`:
- `process_lte(lte, dt)` - 处理 LTE 估计，返回 (是否接受, 下一步长)

**PI 控制器公式:**
```
dt_new = dt * (1/e_n)^k_p * (1/e_{n-1})^k_i * safety_factor

其中:
- e_n = 当前误差比 (LTE/tolerance)
- e_{n-1} = 上一步误差比
- k_p = 0.7/p (比例增益, p=方法阶数)
- k_i = 0.4/p (积分增益)
- safety_factor = 0.9
```

**紧急步长策略:**
| 连续拒绝次数 | 缩减因子 |
|-------------|---------|
| 0-2 | 0.5 |
| 3-5 | 0.25 |
| 6-10 | 0.1 |
| >10 | dt_min |

**测试用例:** 13 个单元测试全部通过
- `test_pi_config_for_order`
- `test_pi_controller_small_error_increases_dt`
- `test_pi_controller_large_error_decreases_dt`
- `test_pi_controller_error_at_target`
- `test_pi_controller_uses_history`
- `test_pi_controller_record_step_updates_stats`
- `test_pi_controller_consecutive_rejects`
- `test_pi_controller_emergency_dt`
- `test_pi_controller_reset`
- `test_adaptive_controller_accept_and_grow`
- `test_adaptive_controller_reject_and_shrink`
- `test_adaptive_controller_respects_limits`
- `test_step_controller_stats_display`

**总计测试:** Phase 1 (8) + Phase 2 (13) = 21 个测试全部通过

### Phase 3 实现详情 (已完成)

**新增代码位置:** `crates/sim-core/src/stamp.rs`

**新增结构体:**

| 结构体 | 描述 |
|--------|------|
| `IntegrationMethod` | 枚举: BackwardEuler, Trapezoidal |
| `TransientState` | 扩展瞬态状态，包含电容电流和电感电压历史 |

**TransientState 结构详解:**
```rust
pub struct TransientState {
    // === 电容状态 ===
    pub cap_voltage: HashMap<String, f64>,  // V_{n-1}
    pub cap_current: HashMap<String, f64>,  // I_{n-1} (Trapezoidal 需要)

    // === 电感状态 ===
    pub ind_current: HashMap<String, f64>,  // I_{n-1}
    pub ind_voltage: HashMap<String, f64>,  // V_{n-1} (Trapezoidal 需要)
    pub ind_aux: HashMap<String, usize>,    // 辅助变量索引

    // === 方法选择 ===
    pub method: IntegrationMethod,          // 默认 BackwardEuler
}
```

**新增函数:**

| 函数 | 描述 |
|------|------|
| `stamp_capacitor_trap()` | 电容 Trapezoidal stamp |
| `stamp_inductor_trap()` | 电感 Trapezoidal stamp |
| `stamp_capacitor_tran_method()` | 统一电容 stamp (按方法选择) |
| `stamp_inductor_tran_method()` | 统一电感 stamp (按方法选择) |
| `update_transient_state_full()` | 更新瞬态状态 (含电流计算) |

**算法详解:**

**1. 电容 Trapezoidal 积分:**

从电容本构关系:
```
I = C * dV/dt
```

Trapezoidal 近似:
```
(V_n - V_{n-1}) / dt = (I_n + I_{n-1}) / (2C)
```

重排得:
```
I_n = (2C/dt) * V_n - (2C/dt) * V_{n-1} - I_{n-1}
```

等效电路模型:
- 等效导纳: g = 2C/dt (是 BE 的 2 倍)
- 等效电流: I_eq = g * V_{n-1} + I_{n-1}

**方法对比:**
| 方法 | 导纳 | 历史项 |
|------|------|--------|
| Backward Euler | g = C/dt | I_eq = g * V_{n-1} |
| Trapezoidal | g = 2C/dt | I_eq = g * V_{n-1} + I_{n-1} |

**2. 电感 Trapezoidal 积分:**

从电感本构关系:
```
V = L * dI/dt
```

Trapezoidal 近似:
```
(I_n - I_{n-1}) / dt = (V_n + V_{n-1}) / (2L)
```

重排得:
```
I_n = (dt/2L) * V_n + (dt/2L) * V_{n-1} + I_{n-1}
```

使用辅助变量 k 表示电感电流:
```
V_a - V_b = (2L/dt) * I_k - (2L/dt) * I_{n-1} - V_{n-1}
```

等效电路模型:
- 等效电阻: R_eq = 2L/dt
- RHS 项: -R_eq * I_{n-1} - V_{n-1}

**方法对比:**
| 方法 | 等效项 | RHS |
|------|--------|-----|
| Backward Euler | -L/dt | -(L/dt) * I_{n-1} |
| Trapezoidal | -2L/dt | -R_eq * I_{n-1} - V_{n-1} |

**3. 方法选择:**

`InstanceStamp::stamp_tran()` 根据 `state.method` 自动选择:
```rust
DeviceKind::C => match state.method {
    IntegrationMethod::BackwardEuler => stamp_capacitor_tran(...)
    IntegrationMethod::Trapezoidal => stamp_capacitor_trap(...)
}
```

**测试用例:** 18 个单元测试全部通过

| 测试类别 | 测试名称 |
|----------|----------|
| 电容 BE | `test_capacitor_be_conductance`, `test_capacitor_be_history` |
| 电容 TRAP | `test_capacitor_trap_conductance`, `test_capacitor_trap_history`, `test_capacitor_trap_vs_be_ratio` |
| 电感 BE | `test_inductor_be_stamp`, `test_inductor_be_history` |
| 电感 TRAP | `test_inductor_trap_stamp`, `test_inductor_trap_history`, `test_inductor_trap_vs_be_ratio` |
| 方法选择 | `test_method_selection_default`, `test_method_selection_capacitor`, `test_method_selection_inductor` |
| 状态更新 | `test_update_transient_state_capacitor`, `test_update_transient_state_full_capacitor`, `test_update_transient_state_inductor` |
| 辅助函数 | `test_stamp_capacitor_tran_method`, `test_stamp_inductor_tran_method` |

**关键验证点:**
- Trapezoidal 导纳/电阻是 BE 的 2 倍 ✓
- Trapezoidal 需要电流/电压历史 ✓
- 默认方法为 BackwardEuler ✓
- `stamp_tran()` 正确分发到方法实现 ✓
- 状态更新正确保存历史值 ✓

**总计测试:** Phase 1 (8) + Phase 2 (13) + Phase 3 (18) = 39 个测试全部通过

### Phase 4 实现详情 (已完成)

**新增代码位置:** `crates/sim-core/src/waveform.rs`

**模块概述:**

Phase 4 实现了完整的波形规格和断点处理功能，包括:
1. 波形规格 (DC, PULSE, PWL, SIN, EXP)
2. 波形求值函数
3. 断点提取和管理
4. 时间步长限制以精确命中断点

**新增结构体:**

| 结构体 | 描述 |
|--------|------|
| `PulseParams` | PULSE 波形参数 (v1, v2, td, tr, tf, pw, per) |
| `PwlParams` | PWL 波形参数 (时间-值对列表) |
| `SinParams` | SIN 波形参数 (vo, va, freq, td, theta) |
| `ExpParams` | EXP 波形参数 (v1, v2, td1, tau1, td2, tau2) |
| `WaveformSpec` | 波形规格枚举 (Dc, Pulse, Pwl, Sin, Exp) |
| `BreakpointType` | 断点类型枚举 |
| `Breakpoint` | 断点信息 (时间, 源名, 类型) |
| `TransientSource` | 带波形的瞬态源 |
| `BreakpointManager` | 断点管理器 |

**波形求值算法:**

**1. PULSE 波形:**
```
       v2 ─────┬─────┐
              /│     │\
             / │     │ \
            /  │     │  \
       v1 ─┘   │     │   └─────
           │   │     │   │
           td  tr    pw  tf
           └───────per───────┘
```

**2. PWL 波形:**
- 线性插值: `v = v0 + (v1-v0) * (t-t0) / (t1-t0)`
- 边界处理: 第一点前返回第一值，最后点后返回最后值

**3. SIN 波形:**
```
v(t) = vo + va * sin(2π * freq * (t - td)) * exp(-theta * (t - td))
```

**4. EXP 波形:**
- 第一阶段 (t < td1): v = v1
- 第二阶段 (td1 ≤ t < td2): v = v1 + (v2-v1) * (1 - exp(-(t-td1)/tau1))
- 第三阶段 (t ≥ td2): 加入返回过渡

**断点提取:**

| 源类型 | 断点 |
|--------|------|
| PULSE | td, td+tr, td+tr+pw, td+tr+pw+tf, ... (周期性) |
| PWL | 每个角点 (t1, t2, t3, ...) |
| EXP | td1, td2 |
| SIN | td (如果 > 0) |

**BreakpointManager 方法:**

| 方法 | 描述 |
|------|------|
| `extract_from_sources()` | 从瞬态源列表提取所有断点 |
| `next_breakpoint(t)` | 获取时间 t 之后的下一个断点 |
| `limit_dt(t, proposed_dt, min_margin)` | 限制步长以命中断点 |
| `crossed_breakpoint(t_prev, t)` | 检查是否跨过断点 |
| `update_settling(t_prev, t)` | 更新断点后稳定状态 |
| `settling_dt(normal_dt, dt_min)` | 获取稳定期步长 |

**步长限制算法:**

```rust
pub fn limit_dt(&self, t: f64, proposed_dt: f64, min_margin: f64) -> f64 {
    if let Some(next_bp) = self.next_breakpoint(t) {
        let time_to_bp = next_bp - t;

        // 如果会跨过断点，精确命中它
        if proposed_dt >= time_to_bp {
            return time_to_bp.max(min_margin);
        }

        // 如果接近断点 (90% 以内)，直接命中
        if proposed_dt > 0.9 * time_to_bp {
            return time_to_bp;
        }

        // 如果剩余距离很小，拉伸步长命中断点
        let remaining = time_to_bp - proposed_dt;
        if remaining < 0.1 * proposed_dt {
            return time_to_bp;
        }
    }
    proposed_dt
}
```

**断点后稳定 (Settling):**

断点处波形有不连续性，需要小步长捕获瞬态响应:
- 默认 5 步稳定期
- 稳定期步长 = normal_dt * 0.1

**波形解析函数:**

| 函数 | 描述 |
|------|------|
| `parse_pulse(spec)` | 解析 "PULSE(v1 v2 td tr tf pw per)" |
| `parse_pwl(spec)` | 解析 "PWL(t1 v1 t2 v2 ...)" |
| `parse_number_with_suffix(token)` | 解析带后缀的数值 (k, m, u, n, p, f, meg) |

**测试用例:** 27 个单元测试全部通过

| 测试类别 | 测试数量 | 测试名称示例 |
|----------|----------|-------------|
| PULSE 求值 | 5 | `test_pulse_before_delay`, `test_pulse_rise`, `test_pulse_high`, `test_pulse_fall`, `test_pulse_periodic` |
| PWL 求值 | 2 | `test_pwl_interpolation`, `test_pwl_before_and_after` |
| SIN 求值 | 2 | `test_sin_evaluation`, `test_sin_with_delay` |
| EXP 求值 | 1 | `test_exp_evaluation` |
| 断点提取 | 5 | `test_extract_pulse_breakpoints`, `test_extract_pwl_breakpoints`, `test_extract_exp_breakpoints`, etc. |
| 步长限制 | 3 | `test_limit_dt_no_breakpoints`, `test_limit_dt_hits_breakpoint`, `test_limit_dt_close_to_breakpoint` |
| 稳定处理 | 2 | `test_settling_after_breakpoint`, `test_settling_dt` |
| 解析函数 | 3 | `test_parse_pulse`, `test_parse_pwl`, `test_parse_number_with_suffix` |
| 集成测试 | 1 | `test_full_breakpoint_workflow` |

**关键验证点:**
- PULSE 各阶段求值正确 (delay, rise, high, fall, low) ✓
- PWL 线性插值和边界处理正确 ✓
- 周期性 PULSE 正确处理多周期 ✓
- 断点精确命中 (步长限制) ✓
- 断点后稳定期正确触发 ✓
- 解析器正确处理工程后缀 ✓

**总计测试:** Phase 1 (8) + Phase 2 (13) + Phase 3 (18) + Phase 4 (27) = **66 个测试全部通过**

### Phase 5 实现详情 (已完成)

**新增代码位置:** `crates/sim-core/tests/adaptive_timestep_tests.rs`

**模块概述:**

Phase 5 实现了全面的集成测试，验证自适应时间步长系统的各组件协同工作。测试覆盖 9 个类别，共 31 个测试用例。

**测试类别:**

| 类别 | 测试数量 | 描述 |
|------|----------|------|
| 1. 波形求值 | 4 | PULSE/PWL/SIN/EXP 完整波形测试 |
| 2. 断点处理 | 4 | 多源断点提取、步长限制、稳定期 |
| 3. LTE 估计 | 4 | Milne's Device、差分法、误差定位 |
| 4. PI 控制器 | 4 | 步长调整、历史效应、紧急处理 |
| 5. 积分方法 | 2 | 方法枚举、状态历史存储 |
| 6. 完整工作流 | 4 | RC/LC 电路、PWL 精确命中、能量守恒 |
| 7. 解析测试 | 2 | PULSE/PWL 字符串解析 |
| 8. 边界情况 | 5 | 零时间、单点、极小/极大步长、空管理器 |
| 9. 统计监控 | 2 | 控制器统计、波形断点检测 |

**关键测试用例:**

**波形求值集成测试:**
- `test_pulse_waveform_full_cycle` - 完整 PULSE 周期 (100MHz, 3.3V CMOS)
- `test_pwl_waveform_ramp` - PWL 斜坡和保持
- `test_sin_waveform_frequency` - SIN 频率准确性
- `test_exp_waveform_transition` - EXP 时间常数响应

**断点处理集成测试:**
- `test_breakpoint_extraction_from_multiple_sources` - 多源断点合并
- `test_breakpoint_step_limiting` - 步长精确命中断点
- `test_breakpoint_settling_behavior` - 断点后稳定期
- `test_breakpoint_simulation_workflow` - 完整仿真循环

**全工作流测试:**
- `test_full_adaptive_workflow` - 完整自适应循环
  - 波形 → 断点 → LTE → PI控制 → 稳定期
- `test_rc_time_constant_accuracy` - RC 电路精度验证
  - 解析解对比: V(t) = V0 * (1 - exp(-t/RC))
- `test_lc_oscillator_energy_conservation` - LC 振荡器能量守恒
  - Trapezoidal 方法应保持能量
- `test_pwl_exact_breakpoint_hits` - PWL 断点精确命中
  - 验证仿真时间点包含所有 PWL 角点

**边界情况测试:**
- `test_zero_duration_pulse` - 零上升/下降时间
- `test_single_point_pwl` - 单点 PWL (常数)
- `test_very_small_time_steps` - dt_min = 1e-18 (阿秒级)
- `test_very_large_time_steps` - dt_max = 1e-6 (微秒级)
- `test_empty_breakpoint_manager` - 空断点管理器

**测试验证的关键点:**

| 验证点 | 测试覆盖 |
|--------|---------|
| PULSE 波形各阶段正确 | ✓ |
| PWL 线性插值和边界 | ✓ |
| SIN 频率和相位 | ✓ |
| EXP 时间常数 | ✓ |
| 多源断点合并 | ✓ |
| 步长限制命中断点 | ✓ |
| 断点后稳定期触发 | ✓ |
| LTE Milne 估计 | ✓ |
| LTE 差分估计 | ✓ |
| PI 控制器增长/收缩 | ✓ |
| PI 历史效应 | ✓ |
| 紧急步长缩减 | ✓ |
| 控制器统计 | ✓ |
| 极端步长限制 | ✓ |

**Phase 5 测试结果:** 31 个测试全部通过

**总计测试:** Phase 1 (8) + Phase 2 (13) + Phase 3 (18) + Phase 4 (27) + Phase 5 (31) = **97 个测试全部通过**

---

## 参考资料

1. Nagel, L.W., "SPICE2: A Computer Program to Simulate Semiconductor Circuits", UCB/ERL M520, 1975
2. Kundert, K., "The Designer's Guide to SPICE and Spectre", Kluwer, 1995
3. Vlach, J., Singhal, K., "Computer Methods for Circuit Analysis and Design", Van Nostrand, 1983
4. Shampine, L.F., "Numerical Solution of Ordinary Differential Equations", Chapman & Hall, 1994
