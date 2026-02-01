# MySpice 开发日志 (Changelog)

本文档记录 MySpice 项目的开发进展和计划。

---

## 2026-02-01 - Ngspice Raw Format 输出支持

### 已完成

#### Ngspice Raw 格式支持 (raw.rs)

实现 ngspice raw 文件格式输出，兼容 ngspice、LTspice、gwave 等波形查看器。

**功能特性：**
- 支持所有分析类型的 raw 格式输出：
  - Operating Point (OP)
  - DC transfer characteristic (DC sweep)
  - Transient Analysis (TRAN)
  - AC Analysis (complex data)
- ASCII 格式输出，便于调试和兼容性
- 自动过滤地节点 (node "0")

**CLI 更新：**
```bash
# 新增 --format / -f 选项
sim-cli circuit.cir -o output.raw -f raw

# 保持 PSF 格式为默认
sim-cli circuit.cir -o output.psf        # PSF format (default)
sim-cli circuit.cir -o output.raw -f raw # Raw format
```

**新增文件：**
- `crates/sim-core/src/raw.rs` - Raw 格式写入函数
- `crates/sim-core/tests/raw_tests.rs` - 格式测试
- `docs/ngspice_raw_format.md` - 格式文档

**API:**
```rust
use sim_core::raw;
raw::write_raw_op(&run, &path, precision)?;
raw::write_raw_sweep(source, sweep_values, node_names, results, &path, precision)?;
raw::write_raw_tran(times, node_names, solutions, &path, precision)?;
raw::write_raw_ac(frequencies, node_names, ac_solutions, &path, precision)?;
```

### 代码统计
- 新增文件: 3 (raw.rs, raw_tests.rs, ngspice_raw_format.md)
- 修改文件: 2 (lib.rs, main.rs)
- 新增测试: 5

---

## 2026-01-31 - AC 小信号频域分析实现

### 已完成

#### AC 分析功能 (engine.rs, stamp.rs, complex_mna.rs, complex_solver.rs)

实现完整的 AC 小信号频域分析功能，计算电路的频率响应。

**功能特性：**
- 支持三种频率扫描类型：
  - DEC: 每十倍频程 N 个点（对数扫描）
  - OCT: 每倍频程 N 个点（对数扫描）
  - LIN: 总共 N 个点（线性扫描）
- 在 DC 工作点处线性化非线性器件
- 复数 MNA 矩阵构建与求解
- 输出幅度（dB）和相位（度）

**器件 AC 模型：**

| 器件 | AC 导纳/行为 |
|------|-------------|
| R | Y = G = 1/R（实数） |
| C | Y = jωC（纯虚数） |
| L | Y = 1/(jωL)（使用辅助变量） |
| V | 辅助变量 + AC 幅度∠相位 激励 |
| I | RHS 注入 AC 幅度∠相位 |
| D | DC 工作点线性化 gd |
| M | DC 工作点 gm, gds, gmbs |
| E/G/F/H | 与 DC 相同（频率无关） |

**数据结构更新：**
```rust
// result_store.rs
pub enum AnalysisType {
    Op, Dc, Tran, Ac,  // 新增 Ac
}

pub struct RunResult {
    // ... 现有字段 ...
    pub ac_frequencies: Vec<f64>,           // 频率点
    pub ac_solutions: Vec<Vec<(f64, f64)>>, // (幅度_dB, 相位_度)
}
```

**网表语法：**
```spice
.AC DEC 10 1 1MEG      * 10 points per decade from 1 Hz to 1 MHz
.AC OCT 5 100 10K      * 5 points per octave from 100 Hz to 10 kHz
.AC LIN 100 1K 10K     * 100 points linearly from 1 kHz to 10 kHz

V1 in 0 DC 0 AC 1 45   * 1V magnitude, 45 degree phase
```

**CLI 选项：**
```bash
sim-cli circuit.cir -a ac --ac-sweep dec --ac-points 10 \
    --ac-fstart 1 --ac-fstop 1meg --psf output.psf
```

**验证测试（RC 低通滤波器）：**
- R=1kΩ, C=1µF, 截止频率 fc=159.15 Hz
- 1 Hz: -0.000171 dB, -0.36°（理论: ~0 dB, ~0°）✓
- 159 Hz: -3.006 dB, -44.97°（理论: -3 dB, -45°）✓
- 1 MHz: -75.96 dB, -89.99°（理论: -76 dB, -90°）✓

**Bug 修复：**
- 修复 ComplexDenseSolver 中重复矩阵条目覆盖而非求和的问题

### 代码统计
- 修改文件: 7 (netlist.rs, result_store.rs, stamp.rs, engine.rs, complex_solver.rs, main.rs, 测试文件)
- 新增代码: ~400 行
- 新增 AC 相关测试: 验证通过

---

## 2026-01-27 - DC Sweep 分析实现

### 已完成

#### DC Sweep 功能 (engine.rs, result_store.rs)

实现完整的 DC 扫描分析功能，支持对电压源或电流源进行参数扫描。

**功能特性：**
- 支持正向和反向扫描（start < stop 或 start > stop）
- 自动计算扫描点，避免浮点累积误差
- 使用前一扫描点的解作为下一点的初始猜测（continuation method）
- 支持单点扫描（start == stop）

**数据结构更新：**
`RunResult` 新增字段：
```rust
pub sweep_var: Option<String>,      // 扫描变量名 (如 "V1")
pub sweep_values: Vec<f64>,          // 扫描点值
pub sweep_solutions: Vec<Vec<f64>>,  // 每个扫描点的解向量
```

**使用示例：**
```spice
* DC sweep example
V1 in 0 DC 0
R1 in out 1k
R2 out 0 2k
.dc V1 0 5 0.5
.end
```

**新增测试：**
- `dc_sweep_resistor_divider` - 电阻分压器扫描验证
- `dc_sweep_negative_range` - 负电压范围扫描
- `dc_sweep_fine_step` - 细步长扫描精度测试
- `dc_sweep_single_point` - 单点扫描

### 代码统计
- 修改文件: 4 (engine.rs, result_store.rs, psf_tests.rs, result_store_tests.rs)
- 新增文件: 2 (dc_sweep_tests.rs, dc_sweep.cir)
- 新增测试: 4
- 新增代码: ~120 行

---

## 2026-01-27 - 代码质量改进与功能完善

### 已完成

#### 1. 修复编译器警告 (solver.rs)
- 移除 `KluSolver::new()` 中不必要的 `mut` 修饰符
- 为 KLU 功能禁用时未使用的参数添加 `#[allow(unused_variables)]` 属性
- 优化了 KLU 和非 KLU 构建路径的代码结构

#### 2. 清理死代码 (netlist.rs)
- 移除了未使用的 `expand_subckt_instance` 函数
- 该功能已被更完善的 `expand_subckt_instance_recursive` 函数替代

#### 3. 完善子电路展开 (netlist.rs)
- 子电路内的 `.model` 语句现在会被正确提取和处理
- 新增 `subckt_models` 字段到 `ElaboratedNetlist` 结构
- 更新 `expand_subckt_instance_recursive` 函数以收集子电路内的模型定义
- 更新 `build_circuit` 函数以使用提取的子电路模型
- 子电路内的模型名称会自动添加实例前缀以避免命名冲突

#### 4. 实现受控源器件 Stamp (stamp.rs)
新增四种受控源的 MNA stamp 实现：

| 器件 | 类型 | 描述 |
|------|------|------|
| E | VCVS | 电压控制电压源 (Voltage Controlled Voltage Source) |
| G | VCCS | 电压控制电流源 (Voltage Controlled Current Source) |
| F | CCCS | 电流控制电流源 (Current Controlled Current Source) |
| H | CCVS | 电流控制电压源 (Current Controlled Voltage Source) |

- X (子电路实例) 的 stamp 现在返回 Ok(()) 因为子电路已在展开阶段处理

#### 5. 新增测试用例
为受控源器件添加了单元测试：
- `vcvs_stamp_basic` - 测试 VCVS 基本功能
- `vccs_stamp_basic` - 测试 VCCS 基本功能
- `cccs_stamp_requires_control_source` - 测试 CCCS 与控制源的交互
- `ccvs_stamp_requires_control_source` - 测试 CCVS 与控制源的交互
- `subcircuit_instance_stamp_is_noop` - 验证子电路实例 stamp 为空操作

### 代码统计
- 修改文件: 3 (netlist.rs, solver.rs, stamp.rs)
- 新增测试: 5
- 编译警告: 0 (从 6 个减少到 0)

---

## 下一步计划 (Next Steps)

### 高优先级

1. **POLY 语法支持**
   - 完善受控源的 POLY 多项式语法
   - 支持多组控制节点/电流

3. **更多输出格式**
   - JSON 格式导出
   - CSV 格式导出
   - ~~ngspice raw 格式兼容~~ ✓ 已完成

### 中优先级

4. **KLU 稀疏求解器集成**
   - 完成 KLU 库的 FFI 绑定
   - 大规模电路性能优化

5. **瞬态分析改进**
   - 自适应时间步长优化
   - 断点处理 (PWL 波形)

6. **AI 代理集成**
   - 完善 `tools/ai-agent/` 功能
   - 交互式电路分析

### 低优先级

7. **GUI 实现**
   - PySide6 界面开发
   - 波形显示

8. **噪声分析**
   - 器件噪声模型
   - 噪声传递函数

---

## 版本历史

| 日期 | 版本 | 主要变更 |
|------|------|----------|
| 2026-02-01 | - | **Ngspice Raw 格式输出支持** |
| 2026-01-31 | - | **AC 小信号频域分析实现** |
| 2026-01-27 | - | **DC Sweep 分析实现** |
| 2026-01-27 | - | 代码质量改进、受控源实现、子电路模型支持 |
| 2026-01-27 | - | BSIM4 支持 |
| 2026-01-26 | - | CLI 文档完善 |
| 2026-01-25 | - | BSIM3 支持 |

---

## 技术债务 (Technical Debt)

### 已解决
- [x] solver.rs 编译警告
- [x] netlist.rs 死代码警告
- [x] 子电路内 .model 语句不被处理
- [x] 受控源 (E/G/F/H) 未实现 stamp
- [x] DC sweep 仅解析未实现
- [x] AC 分析的器件模型 (R/C/L/V/I/D/M/E/G/F/H)

### 待解决
- [ ] `spice_datasets_runner` 测试因权限问题失败 (环境问题)
- [ ] POLY 语法的受控源尚未完全支持
- [ ] DC sweep PSF 输出格式支持

---

## 贡献者

- Claude Code (AI 辅助开发)
