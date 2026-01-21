# MySpice

一个基于 Rust 的 SPICE 仿真器项目，目标是先支持 DC + 基本器件（含 MOSFET），并在架构上预留 BSIM 模型、交互式 API、AI 代理与可视化界面的扩展能力。

## 目标与阶段

- 阶段 A: DC + 基础器件 + MOSFET（BSIM 结构占位）
- 阶段 B: 小规模到大规模网表的性能扩展
- 阶段 C: 多种输出格式（PSF 文本、ngspice raw、PSF/FSDB 等）

## 总体架构

- 前端解析层: SPICE 网表解析、语义展开、参数替换
- 核心仿真层: 拓扑构建、MNA 方程生成、非线性求解
- 器件模型层: R/C/L/源/二极管/MOSFET，BSIM 作为可插拔模型
- 结果与输出层: 结果缓存与格式化输出
- 交互与 API 层: 常驻会话、查询电路/结果、触发仿真
- 交互 UI/AI 层: CLI 与 AI 代理；未来可扩展 GUI

## 交互式模式与 API

交互模式下，仿真器启动后只做前端解析与拓扑构建，不自动开始仿真。进程常驻并通过 API 提供电路信息访问。用户可通过 API 触发仿真，仿真完成后进程不退出，结果可继续查询。

状态机建议:

Parsed -> Elaborated -> Ready -> Running -> Completed

ResultStore:

- run_id: 每次仿真唯一标识
- analysis_type: OP / DC sweep / TRAN
- metadata: 迭代次数、收敛信息、时间戳
- data: 节点电压、器件电流、扫描曲线等

### API 端点建议 (草案)

电路结构查询:

- GET /v1/summary
- GET /v1/nodes
- GET /v1/devices
- GET /v1/devices/{id}
- GET /v1/models
- GET /v1/models/{name}
- GET /v1/subckts
- GET /v1/subckts/{name}
- GET /v1/topology
- GET /v1/validate

仿真控制与结果访问:

- POST /v1/run/op
- POST /v1/run/dc
- POST /v1/run/tran
- GET /v1/runs
- GET /v1/runs/{run_id}
- GET /v1/runs/{run_id}/signals
- GET /v1/runs/{run_id}/waveform?signal=V(n001)
- GET /v1/runs/{run_id}/op
- GET /v1/runs/{run_id}/dc
- GET /v1/runs/{run_id}/tran
- POST /v1/runs/{run_id}/export

## AI 交互与 CLI

交互式界面优先做 CLI，并由 AI 代理决定是否调用仿真器 API。推荐方案:

- 仿真器内核: Rust 常驻进程
- CLI + AI 代理: Python
- 通讯方式: 本地 HTTP 或 IPC

AI 代理通过工具调用协议访问 API，获取电路与仿真结果信息，并以自然语言反馈。

## GUI 规划与框架选择

未来需要 GUI 时，推荐使用 PySide6 (Qt for Python):

- 跨平台成熟、控件丰富
- 适合复杂布局与波形/图形显示

GUI 组件建议:

- 命令输入面板 (Command Panel)
- AI 输出/日志面板 (Chat/Log Panel)
- 电路示意图面板 (Schematic Viewer)
- 结果与表格面板 (Result Panel)

## 基于 Netlist 的 Schematic 显示

示意图不是传统设计级 schematic，而是从 netlist 自动生成的拓扑视图:

Netlist -> 拓扑图 -> 自动布局 -> Qt 绘制

布局建议:

- 优先使用 Graphviz/dot 生成坐标
- 小规模电路可用简化布局算法

绘制建议:

- QGraphicsView / QGraphicsScene
- 器件符号与连线分别作为图元

## Netlist 前端语法支持边界 (阶段 A)

支持:

- 注释行: 以 * 开头
- 续行: 以 + 开头
- 语句: .title .include .param .model .subckt .ends .op .dc .tran .end
- 器件: R C L V I D M E G F H
- 参数: param=expr，单位后缀 f p n u m k meg g t
- 子电路: .subckt / X 实例化

暂不支持:

- .lib .if/.elseif/.else/.endif
- .measure .plot .print (可解析后忽略)
- .alter .step .temp
- 行为源 B 元件、传输线等扩展器件

## MOSFET / BSIM 架构预留

- 模型参数与实例参数分离
- 模型计算核心独立，输入电压，输出电流与导数
- 支持 BSIM3/BSIM4 的接口占位，后续填充公式实现

## 输出格式规划

- 首先支持 PSF 文本格式
- 逐步扩展: ngspice raw / PSF / FSDB

## 当前进度

已完成:

- 项目 workspace 与基础 crate 架构
- CLI 二进制框架与批量测试脚本
- Netlist parser skeleton (行级解析 + 语句识别 + 节点/参数抽取 + 基础校验)
- .include 读取、.param 基础替换、.subckt 基础展开与子电路参数映射
- 参数表达式基础求值与受控源语法补充
- 嵌套子电路基础展开
- 基础测试用例与 spice-datasets 引用
- 仿真引擎骨架（Circuit/MNA/Stamp/Engine/KLU stub）
- DC 基础 stamp（R/I/V/D/MOS）与 MNA 骨架测试
- Newton 迭代与收敛控制骨架
- gmin/source stepping 基础接入
- 二极管/MOS 非线性线性化（简化模型）
- TRAN 骨架入口（时间步循环）
- TRAN 电容/电感等效 stamp
- TRAN 自适应步长与误差估计骨架
- TRAN 非线性器件 Newton 迭代
- TRAN 收敛失败时回退 gmin/source stepping
- TRAN 加权误差估计
- ResultStore 接入与仿真结果输出
- PSF 文本输出（基础格式）
- Solver 复用与 Newton 统一模块接入

待完善:

- 语义展开的完整规则 (更复杂的参数表达式、作用域边界)
- 设备级字段解析的完整规则 (受控源高级语法)
- 求解器与实际仿真结果输出（默认 Dense，KLU 可选）
- API 服务实现与交互模式

## Netlist Parser 现状

已实现:

- 行级解析：注释、空行、续行合并
- 控制语句：.title/.include/.param/.model/.subckt/.ends/.op/.dc/.tran/.end
- 器件识别：R/C/L/V/I/D/M/E/G/F/H/X
- 字段抽取：节点、model、value、params
- 基础校验：R/C/L/V/I/D/M/E/G/F/H 的节点/字段检查
- MOS 支持 3 节点（隐式 bulk=0）
- 受控源基础：E/G/F/H 支持 POLY 语法解析与系数校验
- 受控源 POLY 控制节点/控制源数量校验
- .include 递归读取与循环检测
- .param 参数替换（全局 + 子电路局部覆盖 + 子电路内部 .param）
- 电压/电流源支持 DC 关键字取值
- 电压/电流源支持波形关键字（PULSE/SIN/AC 等）
- 参数支持逗号分隔，model 括号参数解析
- 表达式求值：+ - * / ( ) 与单位后缀 (含 meg)
- 表达式函数：max/min/abs/if，支持一元负号与幂运算 (^)
- 子电路展开：X 实例展开、嵌套子电路递归展开

待完善:

- 语义展开完整规则（更复杂表达式/作用域）
- 受控源高级语法（POLY 细节、多项式参数）
- 设备字段解析的全面语法覆盖

## Netlist Parser 下一步

- 完善受控源高级语法（POLY 细节、多项式参数）
- 完整语义展开规则（更复杂表达式与作用域边界）
- 设备字段解析的全面语法覆盖与错误诊断完善

## 仿真引擎下一步

- 定义仿真核心数据结构（节点表、实例表、模型表）
- MNA 方程构建骨架与 stamp 接口
- DC 求解流程骨架（Newton 迭代、收敛判据）
- TRAN 时间步管理与积分器接口（BE/TR 占位）
- 结果采集与 ResultStore 对接

## 仿真引擎规划（Core）

核心结构:

- Circuit IR: NodeTable / InstanceTable / ModelTable / AnalysisCmd
- MNA Builder: SparseBuilder + StampContext + AuxVarTable
- Solver: KLU 封装（analyze/factor/solve）
- Analysis: DC / TRAN 引擎骨架
- Result: ResultStore + 波形容器

## Solver 规划（KLU）

仿真引擎将直接使用 SuiteSparse 的 KLU 作为稀疏线性求解器。

### KLU 调用链

1) 初始化:
- `klu_defaults(&mut common)`
- `klu_analyze(n, Ap, Ai, &mut common)`

2) 每次迭代:
- 更新 `Ax`
- `klu_factor(Ap, Ai, Ax, symbolic, &mut common)`
- `klu_solve(symbolic, numeric, n, 1, b, &mut common)`

3) 结构变化时:
- 重新 `klu_analyze`

### 稀疏矩阵构建（CSC）

- `n`: 矩阵维度
- `Ap: Vec<i64>`（列指针，长度 n+1）
- `Ai: Vec<i64>`（行索引）
- `Ax: Vec<f64>`（数值）

### SparseBuilder 结构草案

- `n: usize`
- `col_entries: Vec<Vec<(usize, f64)>>`（按列收集）
- `finalize() -> (Ap, Ai, Ax)`
- `clear_values()`（保留结构，仅重置数值）

### KluSolver 接口草案

- `new(n, ap, ai)`
- `analyze()`（结构变化时）
- `factor(ax)`
- `solve(b)`（原地）
- `reset_pattern()`

### 依赖说明

- 依赖 SuiteSparse（KLU）
- Windows 环境需要预编译或本地构建 SuiteSparse

### 构建集成

- 使用 `--features klu` 启用 KLU
- 需要设置 `KLU_LIB_DIR` 或 `SUITESPARSE_DIR`

## 下一步 Todo

- [x] 参数表达式升级（函数、条件）
- [ ] 更完善的受控源语法
- [x] 子电路内 `.param` 局部作用域 + 嵌套子电路

## 当前目录结构

当前已落地的 workspace 结构如下:

- crates/sim-core: 仿真核心 (解析、拓扑、MNA、求解器、会话、结果存储)
  - src/netlist.rs
  - src/topology.rs
  - src/mna.rs
  - src/solver.rs
  - src/session.rs
  - src/result_store.rs
  - tests/netlist_parse.rs
  - tests/dc_smoke.rs
  - tests/netlist_tests.rs
  - tests/topology_tests.rs
  - tests/mna_tests.rs
  - tests/solver_tests.rs
  - tests/session_tests.rs
  - tests/result_store_tests.rs
- crates/sim-devices: 器件模型库与 BSIM 占位
  - src/model.rs
  - src/passive.rs
  - src/source.rs
  - src/diode.rs
  - src/mosfet.rs
  - tests/model_tests.rs
  - tests/passive_tests.rs
  - tests/source_tests.rs
  - tests/diode_tests.rs
  - tests/mosfet_tests.rs
- crates/sim-api: API 服务层 (schema/session_api/http 占位)
  - src/schema.rs
  - src/session_api.rs
  - src/http.rs
  - tests/schema_tests.rs
  - tests/session_api_tests.rs
  - tests/http_tests.rs
- crates/sim-cli: CLI 启动器
  - src/main.rs
  - tests/cli_tests.rs
- tests/fixtures/netlists: 网表 fixture
  - basic_dc.cir
- tools/ai-agent: Python AI 代理与 CLI 交互
  - cli.py
  - tests/test_cli_smoke.py
- tools/gui: PySide6 GUI (后续阶段)
  - README.md
- docs: 项目文档
  - myspice_user_manual.md
  - solver_klu_plan.md

## 测试结构

- crates/sim-core/tests: Rust 单元与集成测试
- crates/sim-devices/tests: 器件模型单元测试
- crates/sim-api/tests: API 层单元测试
- crates/sim-cli/tests: CLI 单元测试
- tests/fixtures/netlists: 网表 fixture 用例
- tests/run_spice_datasets.py: 运行 spice-datasets 的批量 smoke 测试并输出 passrate
- tools/ai-agent/tests: Python CLI/代理测试
 - 外部数据集: `../spice-datasets` (测试用例引用)

## 里程碑计划

### W0: 项目脚手架与目录结构
目标: 建立 workspace 和基础模块边界，打通最小构建链路。  
验收标准: 能编译通过，核心 crate 可被独立引用。  
交付物: 目录结构、基础配置、最小可运行入口。

### W1: 网表解析与核心数据结构
目标: 完成 SPICE 网表解析、语义展开入口与基础器件数据结构，形成仿真核心流程骨架。  
验收标准: 能解析典型网表并生成展开后的实例清单与拓扑信息。  
交付物: 解析器、符号表、基础器件结构与核心流程文档。

### W2: DC 仿真引擎与 MOSFET 框架
目标: 打通 DC 仿真核心能力，完成 MOSFET modeling 框架、交互 API 与 ResultStore。  
验收标准: 支持基础器件 DC 仿真并输出结构化结果，MOSFET 接口可被调用。  
交付物: DC 求解链路、MOSFET 接口占位、API 原型与结果存储模块。

### W3: CLI 与交互体验 + 波形输出
目标: 完成 CLI + AI 代理的交互流程，提供结果查询与波形输出能力。  
验收标准: CLI 能驱动仿真与查询结果，支持 PSF 文本输出。  
交付物: CLI 程序、交互协议、PSF 文本输出实现与示例用例。

### W4: 首版优化与稳定性完善
目标: 强化性能、稳定性与错误诊断，完成首版可用性打磨。  
验收标准: 典型小规模网表稳定运行，错误报告清晰，运行性能可接受。  
交付物: 性能优化与诊断改进、回归用例与阶段总结。
