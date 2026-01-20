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
- analysis_type: OP / DC sweep
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
- GET /v1/runs
- GET /v1/runs/{run_id}
- GET /v1/runs/{run_id}/signals
- GET /v1/runs/{run_id}/waveform?signal=V(n001)
- GET /v1/runs/{run_id}/op
- GET /v1/runs/{run_id}/dc
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
- 语句: .title .include .param .model .subckt .ends .op .dc .end
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

## 目录结构建议

建议以 workspace 形式组织:

- crates/sim-core: 仿真核心 (解析、拓扑、MNA、求解器)
- crates/sim-devices: 器件模型库与 BSIM 占位
- crates/sim-api: API 服务层 (HTTP/IPC)
- crates/sim-cli: CLI 启动器与交互模式
- tools/ai-agent: Python AI 代理与 CLI 交互
- tools/gui: PySide6 GUI (后续阶段)
- docs: 设计文档与规范

## 里程碑建议

- M0: 项目脚手架与基本目录结构
- M1: Netlist 解析/展开 + 基础器件数据结构
- M2: DC 求解最小链路 (R/L/C/源/二极管)
- M3: MOSFET 接口占位 + BSIM 参数注册
- M4: 交互式 API + ResultStore
- M5: CLI + AI 代理 (Python)
- M6: PSF 文本输出
- M7: 性能扩展 (稀疏求解器、内存优化)
- M8: GUI 原型 + Netlist Schematic
*** End Patch
