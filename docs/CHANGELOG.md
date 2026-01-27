# MySpice 开发日志 (Changelog)

本文档记录 MySpice 项目的开发进展和计划。

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

1. **AC 分析实现**
   - 添加频率响应分析支持
   - 实现器件的 AC 模型 (电容、电感的频域阻抗)
   - 复数矩阵求解器

2. **POLY 语法支持**
   - 完善受控源的 POLY 多项式语法
   - 支持多组控制节点/电流

3. **更多输出格式**
   - JSON 格式导出
   - CSV 格式导出
   - ngspice raw 格式兼容

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

### 待解决
- [ ] `spice_datasets_runner` 测试因权限问题失败 (环境问题)
- [ ] POLY 语法的受控源尚未完全支持
- [ ] 缺少 AC 分析的器件模型
- [ ] DC sweep PSF 输出格式支持

---

## 贡献者

- Claude Code (AI 辅助开发)
