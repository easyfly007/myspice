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

## 测试结构

- crates/sim-core/tests: Rust 单元与集成测试
- crates/sim-devices/tests: 器件模型单元测试
- crates/sim-api/tests: API 层单元测试
- crates/sim-cli/tests: CLI 单元测试
- tests/fixtures/netlists: 网表 fixture 用例
- tools/ai-agent/tests: Python CLI/代理测试

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
