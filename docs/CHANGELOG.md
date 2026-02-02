# MySpice 开发日志 (Changelog)

本文档记录 MySpice 项目的开发进展和计划。

---

## 2026-02-02 - KLU 稀疏求解器完整实现

### 已完成

#### KLU Sparse Solver FFI Bindings

完整实现 SuiteSparse KLU 稀疏求解器的 FFI 绑定和 Rust 封装。

**功能特性：**

1. **完整 FFI 绑定**
   - 完整的 `klu_common` 结构体定义（所有控制参数和统计字段）
   - 64 位索引支持（`klu_l_*` 系列函数）
   - KLU 状态码定义和错误信息转换
   - 支持 `klu_rcond` 条件数估计

2. **高效重因子化 (Refactorization)**
   - 当稀疏模式不变时自动使用 `klu_refactor`
   - 比完整因子化快约 3 倍
   - 自动跟踪因子化和重因子化次数

3. **增强的错误处理**
   - 详细的错误类型：`SingularMatrix`, `IllConditioned`, `InvalidMatrix`, `KluError`
   - KLU 状态码到错误消息的映射
   - 条件数监控和警告

4. **配置选项**
   - `set_pivot_tolerance(tol)`: 设置主元容差 (0.001-1.0)
   - `set_ordering(method)`: 选择排序算法 (AMD/COLAMD/Natural)
   - `set_btf(enable)`: 启用/禁用块三角分解

5. **统计信息**
   - 因子化/重因子化计数
   - L/U 因子非零元素数
   - 内存使用量
   - 浮点运算数

**跨平台构建支持：**

- Linux: 自动检测系统安装的 SuiteSparse
- macOS: 支持 Homebrew 安装
- Windows: 支持 vcpkg 和手动构建
- 静态/动态链接选项 (`KLU_STATIC=1`)

**环境变量：**
```bash
SUITESPARSE_DIR=/path/to/suitesparse  # 根目录
KLU_LIB_DIR=/path/to/lib              # 库目录
KLU_INCLUDE_DIR=/path/to/include      # 头文件目录
KLU_STATIC=1                          # 静态链接
```

**使用示例：**
```bash
# Linux
sudo apt-get install libsuitesparse-dev
cargo build --features klu

# macOS
brew install suite-sparse
export SUITESPARSE_DIR=$(brew --prefix suite-sparse)
cargo build --features klu
```

**新增文件：**
- `docs/klu_solver.md` - 完整文档（安装、API、性能调优）
- `crates/sim-core/tests/klu_tests.rs` - 单元测试

**修改文件：**
- `crates/sim-core/src/solver.rs` - 完整 FFI 和 KluSolver 实现
- `crates/sim-core/build.rs` - 跨平台构建配置

**代码统计：**
- solver.rs: ~680 行（含完整 FFI 和实现）
- klu_tests.rs: ~475 行（含 KLU 特定测试）
- klu_solver.md: ~400 行文档

---

## 2026-02-01 - DC Sweep PSF 输出格式修复

### 已完成

#### DC Sweep 输出格式修复

修复 DC Sweep 分析的输出格式问题，确保所有格式 (PSF/Raw/JSON/CSV) 正确输出扫描结果。

**修复问题：**

1. **扫描值未正确应用**
   - 问题：DC sweep 时电压源值固定不变
   - 原因：`run_dc_sweep` 使用 `AnalysisCmd::Dc` 触发引擎内部的完整扫描，忽略了手动设置的值
   - 修复：改用 `AnalysisCmd::Op` 进行单点分析，由 CLI 控制扫描循环

2. **PSF 格式列对齐问题**
   - 问题：数据列数多于表头列数（包含了分支电流）
   - 修复：输出数据时按 node_names 索引取值，与表头保持一致

**验证测试：**
```
* DC Sweep test - Resistor divider
V1 in 0 DC 0
R1 in out 1k
R2 out 0 2k
.dc V1 0 5 1
.end

# 预期结果：V(out) = V(in) * 2/3
V1=0 → V(in)=0, V(out)=0
V1=1 → V(in)=1, V(out)=0.667
V1=2 → V(in)=2, V(out)=1.333
V1=3 → V(in)=3, V(out)=2.0
V1=4 → V(in)=4, V(out)=2.667
V1=5 → V(in)=5, V(out)=3.333
```

**修改文件：**
- `crates/sim-cli/src/main.rs` - 使用 Op 分析代替 Dc 分析进行单点扫描
- `crates/sim-core/src/psf.rs` - 修复列数对齐问题

---

## 2026-02-01 - JSON/CSV 输出格式支持

### 已完成

#### JSON/CSV 格式导出

实现 JSON 和 CSV 格式输出，方便与其他工具集成和数据处理。

**功能特性：**
- 支持所有分析类型：OP、DC sweep、TRAN、AC
- JSON 格式：结构化数据，包含元信息和数据数组
- CSV 格式：标准逗号分隔，兼容 Excel、Python pandas 等工具

**CLI 使用：**
```bash
# JSON 格式输出
sim-cli circuit.cir -o output.json -f json

# CSV 格式输出
sim-cli circuit.cir -o output.csv -f csv

# 其他格式
sim-cli circuit.cir -o output.psf           # PSF (默认)
sim-cli circuit.cir -o output.raw -f raw    # ngspice raw
```

**JSON 格式示例 (DC sweep)：**
```json
{
  "format": "myspice-json",
  "version": "0.1.0",
  "analysis": "Dc",
  "sweep_source": "V1",
  "points": 6,
  "variables": [
    {"name": "V1", "type": "sweep"},
    {"name": "in", "type": "voltage"},
    {"name": "out", "type": "voltage"}
  ],
  "data": [
    [0.0, 5.0, 3.333333],
    [1.0, 5.0, 3.333333]
  ]
}
```

**CSV 格式示例 (DC sweep)：**
```csv
V1,V(in),V(out)
0.000000e0,5.000000e0,3.333333e0
1.000000e0,5.000000e0,3.333333e0
```

**新增文件：**
- `crates/sim-core/src/json_export.rs` - JSON 格式写入函数
- `crates/sim-core/src/csv_export.rs` - CSV 格式写入函数

**API:**
```rust
use sim_core::{json_export, csv_export};

// JSON 导出
json_export::write_json_op(&run, &path, precision)?;
json_export::write_json_sweep(source, sweep_values, node_names, results, &path, precision)?;
json_export::write_json_tran(times, node_names, solutions, &path, precision)?;
json_export::write_json_ac(frequencies, node_names, ac_solutions, &path, precision)?;

// CSV 导出
csv_export::write_csv_op(&run, &path, precision)?;
csv_export::write_csv_sweep(source, sweep_values, node_names, results, &path, precision)?;
csv_export::write_csv_tran(times, node_names, solutions, &path, precision)?;
csv_export::write_csv_ac(frequencies, node_names, ac_solutions, &path, precision)?;
```

### 代码统计
- 新增文件: 2 (json_export.rs, csv_export.rs)
- 修改文件: 2 (lib.rs, main.rs)

---

## 2026-02-01 - POLY 多项式受控源仿真支持

### 已完成

#### POLY 语法完整仿真支持

实现受控源 (E/G/F/H) 的 POLY 多项式语法完整仿真，支持非线性多项式关系和多输入依赖。

**功能特性：**
- POLY(1) 单输入多项式：支持任意阶 (c0 + c1*x + c2*x² + c3*x³ + ...)
- POLY(2) 双输入多项式：支持到交叉项 (c0 + c1*x1 + c2*x2 + c3*x1*x2 + c4*x1² + c5*x2²)
- POLY(n) 多输入：支持线性项组合
- DC 分析：使用 Newton-Raphson 迭代求解非线性方程
- AC 分析：在 DC 工作点计算小信号导数进行线性化

**支持的器件：**
- E (VCVS) - 电压控制电压源
- G (VCCS) - 电压控制电流源
- F (CCCS) - 电流控制电流源
- H (CCVS) - 电流控制电压源

**典型应用：**
```spice
* 乘法器: Vout = Va × Vb
E_mult out 0 POLY(2) a 0 b 0 0 0 0 1.0

* 平方器: Vout = Vin²
E_sq out 0 POLY(1) in 0 0 0 1.0

* 加法器: Vout = 2×Va + 3×Vb
E_add out 0 POLY(2) a 0 b 0 0 2.0 3.0
```

**技术实现：**
- 在 `circuit.rs` 中新增 `PolySpec` 结构体
- 在 `Instance` 结构体中添加 `poly` 字段
- 在 `stamp.rs` 中实现 `evaluate_poly()` 函数计算多项式值和偏导数
- 为 E/G/F/H 器件添加 `stamp_*_poly()` 和 `stamp_*_poly_ac()` 函数

**修改文件：**
- `crates/sim-core/src/circuit.rs` - 添加 PolySpec 结构体
- `crates/sim-core/src/netlist.rs` - 构建 POLY 规格
- `crates/sim-core/src/stamp.rs` - 多项式评估和 stamp 函数
- `docs/myspice_user_manual.md` - 更新文档

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

1. **更多输出格式**
   - ~~JSON 格式导出~~ ✓ 已完成
   - ~~CSV 格式导出~~ ✓ 已完成
   - ~~ngspice raw 格式兼容~~ ✓ 已完成
   - ~~POLY 语法支持~~ ✓ 已完成

### 中优先级

2. ~~**KLU 稀疏求解器集成**~~ ✓ 已完成 (2026-02-02)
   - ~~完成 KLU 库的 FFI 绑定~~
   - ~~大规模电路性能优化~~

3. **瞬态分析改进**
   - 自适应时间步长优化
   - 断点处理 (PWL 波形)

4. **AI 代理集成**
   - 完善 `tools/ai-agent/` 功能
   - 交互式电路分析

### 低优先级

5. **GUI 实现**
   - PySide6 界面开发
   - 波形显示

6. **噪声分析**
   - 器件噪声模型
   - 噪声传递函数

---

## 版本历史

| 日期 | 版本 | 主要变更 |
|------|------|----------|
| 2026-02-02 | - | **KLU 稀疏求解器完整实现** |
| 2026-02-01 | - | **DC Sweep PSF 输出格式修复** |
| 2026-02-01 | - | **JSON/CSV 输出格式支持** |
| 2026-02-01 | - | **POLY 多项式受控源仿真支持** |
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
- [x] POLY 语法的受控源完整仿真支持
- [x] DC sweep PSF 输出格式问题

### 待解决
- [ ] `spice_datasets_runner` 测试因权限问题失败 (环境问题)

---

## 贡献者

- Claude Code (AI 辅助开发)
