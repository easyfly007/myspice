# MySpice

一个基于 Rust 的 SPICE 仿真器项目，目标是先支持 DC + 基本器件（含 MOSFET），并在架构上预留 BSIM 模型、交互式 API、AI 代理与可视化界面的扩展能力。

## 目标与阶段

- 阶段 A: DC + 基础器件 + MOSFET（BSIM 结构占位）
- 阶段 B: 小规模到大规模网表的性能扩展
- 阶段 C: 多种输出格式（PSF 文本、ngspice raw、PSF/FSDB 等）

## 项目架构概览

```
┌─────────────────────────────────────────────────────────────────────┐
│                           sim-cli (应用层)                           │
│                        命令行工具入口                                 │
├─────────────────────────────────────────────────────────────────────┤
│                           sim-api (API 层)                          │
│                schema.rs │ session_api.rs │ http.rs                 │
├─────────────────────────────────────────────────────────────────────┤
│                          sim-core (核心层)                           │
│  ┌──────────┬──────────┬──────────┬──────────┬──────────┐          │
│  │ netlist  │ circuit  │  engine  │  newton  │ result   │          │
│  │ 网表解析 │ 电路结构 │ 仿真引擎 │ 非线性   │ 结果存储 │          │
│  ├──────────┼──────────┼──────────┴──────────┼──────────┤          │
│  │   mna    │  stamp   │      solver        │   psf    │          │
│  │ 矩阵构建 │ 器件贡献 │     线性求解       │ 结果输出 │          │
│  └──────────┴──────────┴────────────────────┴──────────┘          │
├─────────────────────────────────────────────────────────────────────┤
│                        sim-devices (基础层)                          │
│         passive.rs │ source.rs │ diode.rs │ mosfet.rs              │
└─────────────────────────────────────────────────────────────────────┘
```

### 分层说明

| 层级 | Crate | 职责 |
|------|-------|------|
| 应用层 | sim-cli | 命令行工具入口 |
| API 层 | sim-api | HTTP API 服务、Schema 定义 |
| 核心层 | sim-core | 网表解析、MNA 构建、求解、结果管理 |
| 基础层 | sim-devices | 器件模型（R/C/L/V/I/D/MOS） |

### 依赖关系

```
sim-cli
  └─> sim-api
        └─> sim-core
              └─> sim-devices
```

## 核心模块详解

### sim-core 模块结构

```
sim-core/src/
├── lib.rs           # 模块导出
├── netlist.rs       # 网表解析（AST、子电路展开、参数替换）
├── circuit.rs       # 电路数据结构（节点表、模型表、实例表）
├── topology.rs      # 拓扑分析（占位）
├── mna.rs           # MNA 矩阵构建（SparseBuilder、AuxVarTable）
├── stamp.rs         # 器件 Stamp（DC/TRAN 模式）
├── solver.rs        # 线性求解器（DenseSolver、KluSolver）
├── newton.rs        # Newton 迭代（gmin/source stepping）
├── engine.rs        # 仿真引擎（DC、TRAN 分析）
├── analysis.rs      # 分析配置（时间步控制、误差估计）
├── result_store.rs  # 结果存储管理
├── psf.rs           # PSF 格式输出
└── session.rs       # 会话管理
```

### 模块功能说明

| 模块 | 核心结构/函数 | 功能 |
|------|--------------|------|
| `netlist.rs` | `parse_netlist()`, `elaborate_netlist()` | 解析网表、展开子电路、参数替换 |
| `circuit.rs` | `Circuit`, `NodeTable`, `AnalysisCmd` | 电路中间表示 |
| `mna.rs` | `MnaBuilder`, `SparseBuilder`, `AuxVarTable` | 构建 MNA 稀疏矩阵 |
| `stamp.rs` | `DeviceStamp` trait, `InstanceStamp` | 各器件对矩阵的贡献 |
| `solver.rs` | `LinearSolver` trait, `DenseSolver`, `KluSolver` | 线性方程组求解 |
| `newton.rs` | `run_newton_with_stepping()` | 非线性迭代收敛 |
| `engine.rs` | `Engine`, `run_dc_result()`, `run_tran_result()` | 执行 DC/TRAN 仿真 |
| `result_store.rs` | `ResultStore`, `RunResult` | 管理仿真结果 |

### 关键 Trait 设计

```rust
// 线性求解器接口 (solver.rs)
pub trait LinearSolver {
    fn prepare(&mut self, n: usize);
    fn analyze(&mut self, ap: &[i64], ai: &[i64]) -> Result<(), SolverError>;
    fn factor(&mut self, ap: &[i64], ai: &[i64], ax: &[f64]) -> Result<(), SolverError>;
    fn solve(&mut self, rhs: &mut [f64]) -> Result<(), SolverError>;
    fn reset_pattern(&mut self);
}

// 器件 Stamp 接口 (stamp.rs)
pub trait DeviceStamp {
    fn stamp_dc(&self, ctx: &mut StampContext, solution: Option<&[f64]>) -> Result<(), String>;
    fn stamp_tran(&self, ctx: &mut StampContext, solution: Option<&[f64]>, 
                  dt: f64, state: &mut TransientState) -> Result<(), String>;
}
```

## 数据流向

```
网表文件 (.cir)
    │
    ▼
┌───────────────────────────────────┐
│ netlist::parse_netlist()          │  解析为 AST
│ netlist::elaborate_netlist()      │  展开子电路、参数替换
└───────────────────────────────────┘
    │
    ▼
┌───────────────────────────────────┐
│ circuit::Circuit                  │  构建电路数据结构
│ (NodeTable, InstanceTable, ...)   │  节点/实例/模型表
└───────────────────────────────────┘
    │
    ▼
┌───────────────────────────────────┐
│ engine::Engine                    │  仿真引擎
└───────────────────────────────────┘
    │
    ├─── DC 分析 ───────────────────────────────────┐
    │                                                │
    │   ┌─────────────────────────────────────────┐ │
    │   │ 1. mna::MnaBuilder 构建稀疏矩阵         │ │
    │   │ 2. stamp::InstanceStamp 贡献器件方程    │ │
    │   │ 3. solver::DenseSolver 分解求解         │ │
    │   │ 4. newton::run_newton_with_stepping    │ │
    │   │    └── gmin stepping → source stepping │ │
    │   └─────────────────────────────────────────┘ │
    │                                                │
    ├─── TRAN 分析 ──────────────────────────────────┤
    │                                                │
    │   ┌─────────────────────────────────────────┐ │
    │   │ 时间步循环:                             │ │
    │   │   1. 构建 MNA (含 C/L 等效)             │ │
    │   │   2. Newton 迭代求解                    │ │
    │   │   3. 误差估计 + 自适应步长              │ │
    │   │   4. 更新瞬态状态                       │ │
    │   └─────────────────────────────────────────┘ │
    │                                                │
    ▼                                                │
┌───────────────────────────────────┐               │
│ result_store::ResultStore         │ ◄─────────────┘
│ (RunResult, 节点电压/电流)        │
└───────────────────────────────────┘
    │
    ▼
┌───────────────────────────────────┐
│ psf::write_psf_text()             │  输出结果文件
└───────────────────────────────────┘
```

## 当前实现状态

### 已完成功能

| 模块 | 状态 | 说明 |
|------|------|------|
| 网表解析 | ✅ 完成 | 支持子电路、参数替换、include、表达式求值 |
| DC 仿真 | ✅ 完成 | Newton 迭代 + gmin/source stepping |
| TRAN 仿真 | ✅ 完成 | 自适应步长、加权误差估计 |
| AC 仿真 | ✅ 完成 | 小信号频域分析，支持 DEC/OCT/LIN 扫描 |
| 器件模型 | ✅ 完成 | R/C/L/V/I/D/MOS 的 stamp 实现，BSIM3/BSIM4 完整支持 |
| 求解器 | ✅ 完成 | DenseSolver 实现，KLU 接口可选，复数求解器 |
| 结果输出 | ✅ 完成 | PSF 文本格式（含 DC/TRAN/AC 导出、精度控制） |
| API 服务 | 🔄 最小可用 | 已支持 OP 运行与结果查询 |
| CLI | ✅ 完成 | 完整帮助信息、版本、分析类型选择、PSF 导出、精度控制 |

### 待完善功能

- 语义展开的完整规则（更复杂的参数表达式、作用域边界）
- 受控源高级语法（POLY 细节、多项式参数）
- API 服务实现与交互模式

## 交互式模式与 API

交互模式下，仿真器启动后只做前端解析与拓扑构建，不自动开始仿真。进程常驻并通过 API 提供电路信息访问。用户可通过 API 触发仿真，仿真完成后进程不退出，结果可继续查询。

### 状态机

```
Parsed -> Elaborated -> Ready -> Running -> Completed
```

### ResultStore 结构

- `run_id`: 每次仿真唯一标识
- `analysis_type`: OP / DC sweep / TRAN
- `metadata`: 迭代次数、收敛信息、时间戳
- `data`: 节点电压、器件电流、扫描曲线等

### API 端点（草案）

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

## 操作指南

### CLI 使用方法

```
sim-cli <NETLIST> [OPTIONS]

OPTIONS:
    -h, --help              显示帮助信息
    -V, --version           显示版本信息
    -o, --psf <PATH>        导出 PSF 文本文件
    -a, --analysis <TYPE>   分析类型: op, dc, tran, ac (默认: 从网表或 op)
    --dc-source <NAME>      DC 扫描源名称
    --dc-start <VALUE>      DC 扫描起始值
    --dc-stop <VALUE>       DC 扫描终止值
    --dc-step <VALUE>       DC 扫描步长
    --ac-sweep <TYPE>       AC 扫描类型: dec, oct, lin (默认: dec)
    --ac-points <N>         AC 每十倍频/倍频程点数或总点数 (默认: 10)
    --ac-fstart <FREQ>      AC 起始频率 Hz (默认: 1)
    --ac-fstop <FREQ>       AC 终止频率 Hz (默认: 1e6)
    --precision <N>         输出精度 (1-15 有效数字, 默认: 6)
```

### 1) 运行 CLI（最小 OP 示例）

```
cargo run -p sim-cli -- tests/fixtures/netlists/basic_dc.cir
```

输出示例（节点电压）:

```
run status: Converged iterations=2
V(0) = -5.418302e-20
V(in) = 1.000000e0
V(out) = 6.666667e-1
```

### 2) CLI 输出 PSF 文本

```
cargo run -p sim-cli -- tests/fixtures/netlists/basic_dc.cir --psf /tmp/basic_dc.psf
```

PSF 输出格式示例:

```
PSF_TEXT
# Generated by MySpice v0.1.0
# Date: 2026-01-27T06:31:24Z

[Header]
analysis = Op
status = Converged
iterations = 2

[Signals]
V(0)
V(in)
V(out)

[Values]
V(0)    -5.418302e-20
V(in)   1.000000e0
V(out)  6.666667e-1
```

### 2.1) CLI 指定分析类型

```
cargo run -p sim-cli -- tests/fixtures/netlists/basic_dc.cir --analysis op
```

### 2.2) DC 扫描并导出 PSF

```
cargo run -p sim-cli -- tests/fixtures/netlists/basic_dc.cir --analysis dc \
  --dc-source V1 --dc-start 0 --dc-stop 1 --dc-step 0.1 --psf /tmp/sweep.psf
```

### 2.3) 控制输出精度

```
cargo run -p sim-cli -- tests/fixtures/netlists/basic_dc.cir --precision 3
```

### 3) 启动 API 服务

```
cargo run -p sim-api -- --addr 127.0.0.1:3000
```

### 4) 使用 netlist 字符串触发 OP

```
curl -X POST http://127.0.0.1:3000/v1/run/op \
  -H "Content-Type: application/json" \
  -d "{\"netlist\":\"V1 in 0 DC 1\\nR1 in out 1k\\nR2 out 0 2k\\n.op\\n.end\\n\"}"
```

### 5) 使用文件路径触发 OP

```
curl -X POST http://127.0.0.1:3000/v1/run/op \
  -H "Content-Type: application/json" \
  -d "{\"path\":\"tests/fixtures/netlists/basic_dc.cir\"}"
```

说明:
- `path` 只能访问当前工作目录内的文件（建议在仓库根目录启动服务）
- 返回结果包含 `run_id`、节点名与电压数组

### 6) 触发 DC 扫描

```
curl -X POST http://127.0.0.1:3000/v1/run/dc \
  -H "Content-Type: application/json" \
  -d "{\"netlist\":\".param V=0\\nV1 in 0 DC 0\\nR1 in 0 1k\\n.dc V1 0 1 0.1\\n.end\\n\"}"
```

也可以显式传入扫描参数:

```
curl -X POST http://127.0.0.1:3000/v1/run/dc \
  -H "Content-Type: application/json" \
  -d "{\"path\":\"tests/fixtures/netlists/basic_dc.cir\",\"source\":\"V1\",\"start\":0,\"stop\":1,\"step\":0.1}"
```

### 7) 触发 TRAN 分析

```
curl -X POST http://127.0.0.1:3000/v1/run/tran \
  -H "Content-Type: application/json" \
  -d "{\"netlist\":\"V1 in 0 DC 1\\nR1 in 0 1k\\n.tran 1e-6 1e-5\\n.end\\n\"}"
```

### 8) 查询运行记录与导出 PSF

```
curl http://127.0.0.1:3000/v1/runs
curl http://127.0.0.1:3000/v1/runs/0
```

```
curl -X POST http://127.0.0.1:3000/v1/runs/0/export \
  -H "Content-Type: application/json" \
  -d "{\"path\":\"/tmp/run0.psf\"}"
```

### 9) 查询电路结构

```
curl http://127.0.0.1:3000/v1/summary
curl http://127.0.0.1:3000/v1/nodes
```

## AI 交互与 CLI

交互式界面优先做 CLI，并由 AI 代理决定是否调用仿真器 API。

### 架构

- 仿真器内核: Rust 常驻进程 (sim-api)
- CLI + AI 代理: Python (`tools/ai-agent/`)
- 通讯方式: 本地 HTTP (localhost:3000)

### AI Agent 安装与使用

```bash
# 安装 AI Agent
cd tools/ai-agent
pip install ".[ai]"

# 启动 API 服务器
cargo run -p sim-api -- --addr 127.0.0.1:3000

# 设置 API Key
export ANTHROPIC_API_KEY=your-key

# 启动 AI 交互模式
myspice-agent

# 或直接运行模拟
myspice-agent op circuit.cir
myspice-agent dc circuit.cir -s V1 --start 0 --stop 5 --step 0.5
```

AI 代理通过工具调用协议访问 API，获取电路与仿真结果信息，并以自然语言反馈。

详见 `tools/ai-agent/README.md` 获取完整文档。

## Netlist 前端语法支持

### 已支持

- 注释行: 以 `*` 开头
- 续行: 以 `+` 开头
- 语句: `.title` `.include` `.param` `.model` `.subckt` `.ends` `.op` `.dc` `.tran` `.end`
- 器件: R C L V I D M E G F H X
- 参数: `param=expr`，单位后缀 f p n u m k meg g t
- 子电路: `.subckt` / X 实例化
- 表达式: `+ - * / ^ ( )` 与函数 `max/min/abs/if`
- 受控源: E/G/F/H 基础 POLY 语法
- .model: 模型定义解析与实例绑定（D/M 读取基础参数）

### 暂不支持

- `.lib` `.if/.elseif/.else/.endif`
- `.measure` `.plot` `.print` (可解析后忽略)
- `.alter` `.step` `.temp`
- 行为源 B 元件、传输线等扩展器件

## MOSFET / BSIM 模型支持

已完成 BSIM3 (Level 49) 和 BSIM4 (Level 54) DC 模型实现:

| 模型 | Level | 状态 | 说明 |
|------|-------|------|------|
| Level 1 | 1 | ✅ 完成 | Shichman-Hodges 简化模型 |
| BSIM3v3 | 49 | ✅ 完成 | 完整 DC 模型，50+ 参数 |
| BSIM4 | 54 | ✅ 完成 | 增强模型，含应力效应、衬底电流、栅隧穿 |

### BSIM4 特有功能

- **应力效应**: SA/SB 参数控制 STI 距离对迁移率和阈值电压的影响
- **衬底电流**: ALPHA0/BETA0 参数建模冲击电离
- **栅隧穿电流**: JTSS/JTSD 参数建模栅氧隧穿

详见 `docs/bsim4_model.md` 获取完整参数参考。

## Solver 规划（KLU）

仿真引擎将直接使用 SuiteSparse 的 KLU 作为稀疏线性求解器。

### KLU 调用链

```
1) 初始化:
   klu_defaults(&mut common)
   klu_analyze(n, Ap, Ai, &mut common)

2) 每次迭代:
   更新 Ax
   klu_factor(Ap, Ai, Ax, symbolic, &mut common)
   klu_solve(symbolic, numeric, n, 1, b, &mut common)

3) 结构变化时:
   重新 klu_analyze
```

### 稀疏矩阵构建（CSC）

- `n`: 矩阵维度
- `Ap: Vec<i64>`（列指针，长度 n+1）
- `Ai: Vec<i64>`（行索引）
- `Ax: Vec<f64>`（数值）

### 构建集成

- 使用 `--features klu` 启用 KLU
- 需要设置 `KLU_LIB_DIR` 或 `SUITESPARSE_DIR`

## 目录结构

```
myspice/
├── Cargo.toml                    # Workspace 配置
├── README.md                     # 项目文档
├── crates/
│   ├── sim-core/                 # 仿真核心
│   │   ├── Cargo.toml
│   │   ├── build.rs              # KLU 构建脚本
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── netlist.rs        # 网表解析
│   │   │   ├── circuit.rs        # 电路结构
│   │   │   ├── topology.rs       # 拓扑分析
│   │   │   ├── mna.rs            # MNA 构建
│   │   │   ├── stamp.rs          # 器件 Stamp
│   │   │   ├── solver.rs         # 线性求解
│   │   │   ├── newton.rs         # 非线性迭代
│   │   │   ├── engine.rs         # 仿真引擎
│   │   │   ├── analysis.rs       # 分析配置
│   │   │   ├── result_store.rs   # 结果存储
│   │   │   ├── psf.rs            # PSF 输出
│   │   │   └── session.rs        # 会话管理
│   │   └── tests/                # 单元测试
│   │
│   ├── sim-devices/              # 器件模型
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── model.rs          # 模型 trait
│   │   │   ├── passive.rs        # R/C/L
│   │   │   ├── source.rs         # V/I
│   │   │   ├── diode.rs          # 二极管
│   │   │   └── mosfet.rs         # MOSFET
│   │   └── tests/
│   │
│   ├── sim-api/                  # API 层
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── schema.rs         # API Schema
│   │   │   ├── session_api.rs    # Session API
│   │   │   └── http.rs           # HTTP 服务
│   │   └── tests/
│   │
│   └── sim-cli/                  # CLI 工具
│       ├── Cargo.toml
│       ├── src/
│       │   └── main.rs
│       └── tests/
│
├── tests/
│   ├── fixtures/netlists/        # 测试网表
│   │   ├── basic_dc.cir
│   │   ├── include_parent.cir
│   │   └── include_child.cir
│   └── run_spice_datasets.py     # 批量测试脚本
│
├── tools/
│   ├── ai-agent/                 # Python AI 代理
│   │   ├── pyproject.toml        # 包配置
│   │   ├── README.md             # 使用文档
│   │   ├── myspice_agent/        # Python 包
│   │   │   ├── __init__.py
│   │   │   ├── cli.py            # CLI 入口
│   │   │   ├── client.py         # HTTP 客户端
│   │   │   ├── agent.py          # AI 代理
│   │   │   ├── tools.py          # LLM 工具定义
│   │   │   ├── formatters.py     # 格式化工具
│   │   │   ├── config.py         # 配置管理
│   │   │   └── prompts.py        # 系统提示词
│   │   └── tests/
│   └── gui/                      # GUI（待实现）
│       └── README.md
│
└── docs/                         # 文档
    ├── myspice_user_manual.md
    └── solver_klu_plan.md
```

## 测试结构

- `crates/sim-core/tests`: Rust 单元与集成测试
- `crates/sim-devices/tests`: 器件模型单元测试
- `crates/sim-api/tests`: API 层单元测试
- `crates/sim-cli/tests`: CLI 单元测试
- `tests/fixtures/netlists`: 网表 fixture 用例
- `tests/run_spice_datasets.py`: 运行 spice-datasets 的批量 smoke 测试
- `tools/ai-agent/tests`: Python CLI/代理测试
- 外部数据集: `../spice-datasets` (测试用例引用)

### 运行测试

```bash
# 在 WSL 或 Linux 环境中
cargo test --workspace

# 排除需要外部数据集的测试
cargo test --workspace --exclude sim-cli
```

## 里程碑计划

### W0: 项目脚手架与目录结构 ✅
目标: 建立 workspace 和基础模块边界，打通最小构建链路。  
验收标准: 能编译通过，核心 crate 可被独立引用。  
交付物: 目录结构、基础配置、最小可运行入口。

### W1: 网表解析与核心数据结构 ✅
目标: 完成 SPICE 网表解析、语义展开入口与基础器件数据结构，形成仿真核心流程骨架。  
验收标准: 能解析典型网表并生成展开后的实例清单与拓扑信息。  
交付物: 解析器、符号表、基础器件结构与核心流程文档。

### W2: DC 仿真引擎与 MOSFET 框架 ✅
目标: 打通 DC 仿真核心能力，完成 MOSFET modeling 框架、交互 API 与 ResultStore。
验收标准: 支持基础器件 DC 仿真并输出结构化结果，MOSFET 接口可被调用。
交付物: DC 求解链路、BSIM3v3 完整实现、API 原型与结果存储模块。

### W3: CLI 与交互体验 + 波形输出 🔄 (进行中)
目标: 完成 CLI + AI 代理的交互流程，提供结果查询与波形输出能力。
验收标准: CLI 能驱动仿真与查询结果，支持 PSF 文本输出。
交付物: CLI 程序、交互协议、PSF 文本输出实现与示例用例。

**已完成 (2026-01-27 ~ 2026-01-29):**
- ✅ CLI 帮助信息 (`--help`, `-h`)
- ✅ 版本信息 (`--version`, `-V`)
- ✅ TRAN 分析结果输出到终端
- ✅ PSF 格式改进（时间戳、节信息、对齐列）
- ✅ DC 扫描 PSF 导出支持
- ✅ 精度控制 (`--precision <N>`)
- ✅ TRAN 波形数据时序存储（`tran_times`, `tran_solutions`）

**待完成:**
- [ ] AI 代理集成
- [ ] 更多输出格式（JSON、CSV）

### W4: 首版优化与稳定性完善 ⏳
目标: 强化性能、稳定性与错误诊断，完成首版可用性打磨。  
验收标准: 典型小规模网表稳定运行，错误报告清晰，运行性能可接受。  
交付物: 性能优化与诊断改进、回归用例与阶段总结。

## 下一步 Todo

### 已完成
- [x] 参数表达式升级（函数、条件）
- [x] 子电路内 `.param` 局部作用域 + 嵌套子电路
- [x] BSIM3v3 完整实现（DC 模型，1952 行代码，23 个单元测试）
- [x] CLI 帮助与版本信息 (`--help`, `--version`)
- [x] PSF 格式改进（时间戳、节信息、对齐列）
- [x] DC 扫描 PSF 导出
- [x] 输出精度控制 (`--precision`)
- [x] TRAN 分析结果终端输出
- [x] BSIM4 (Level 54) 完整实现（衬底电流、应力效应、栅隧穿）
- [x] TRAN 波形时序存储（多点数据 + PSF 导出）
- [x] AC 小信号频域分析（复数 MNA、DEC/OCT/LIN 扫描、PSF 导出）

### 进行中
- [x] AI 代理集成与交互协议 ✅ 已完成 (2026-02-02)

### 后续计划
- [ ] 更完善的受控源语法（POLY 细节）
- [ ] API 服务完善
- [ ] 大规模网表性能优化
- [ ] GUI 界面开发

## GUI 规划（后续阶段）

推荐使用 PySide6 (Qt for Python):

- 跨平台成熟、控件丰富
- 适合复杂布局与波形/图形显示

GUI 组件建议:

- 命令输入面板 (Command Panel)
- AI 输出/日志面板 (Chat/Log Panel)
- 电路示意图面板 (Schematic Viewer)
- 结果与表格面板 (Result Panel)

### 基于 Netlist 的 Schematic 显示

示意图不是传统设计级 schematic，而是从 netlist 自动生成的拓扑视图:

```
Netlist -> 拓扑图 -> 自动布局 -> Qt 绘制
```

布局建议:
- 优先使用 Graphviz/dot 生成坐标
- 小规模电路可用简化布局算法

绘制建议:
- QGraphicsView / QGraphicsScene
- 器件符号与连线分别作为图元

## BSIM 模型文档

详见 `docs/bsim_model.md`（包含 BSIM 模型概念、当前支持参数、简化计算与 stamp 说明）。

## 更新日志

### 2026-01-31: AC 小信号分析
- 实现完整 AC 频域分析功能
- 支持 DEC（十倍频程）、OCT（倍频程）、LIN（线性）频率扫描
- 所有器件 AC 小信号模型（R/C/L/V/I/D/M/E/G/F/H）
- 复数 MNA 矩阵构建与复数 LU 求解器
- CLI 支持 `--ac-sweep`、`--ac-points`、`--ac-fstart`、`--ac-fstop` 选项
- AC 结果 PSF 导出（幅度 dB + 相位度）
- 修复复数求解器重复条目求和问题
- 详见 `docs/ac_analysis.md`

### 2026-01-29: TRAN 波形存储
- 实现 TRAN 分析波形时序存储功能
- 新增 `tran_times` 和 `tran_solutions` 字段存储每个时间点的解
- 初始 DC 工作点计算（t=0）
- 自适应时间步进，存储所有接受的时间点
- PSF 波形输出支持（`write_psf_tran()`）
- 3 个新测试用例验证波形存储功能
- 详见 `docs/tran_waveform_plan.md`

### 2026-01-27: BSIM4 (Level 54) 模型
- 完整实现 BSIM4 DC 模型（~600 行新代码）
- 支持 55+ BSIM4 特有参数
- 衬底电流（冲击电离）：ALPHA0/1, BETA0/1
- 布局应力效应：SA, SB, KU0, KVTH0
- 栅隧穿电流：JTSS, JTSD, VTSS, VTSD
- 25 个新单元测试（共 52 个 BSIM 测试）
- 完整参数参考文档 `docs/bsim4_model.md`

### 2026-01-27: CLI & 输出优化
- 新增 `--help` / `-h` 帮助信息
- 新增 `--version` / `-V` 版本显示
- 新增 `--precision <N>` 精度控制（1-15 有效数字）
- 改进 PSF 输出格式（时间戳、节信息、对齐列）
- 支持 DC 扫描 PSF 导出（之前不支持）
- TRAN 分析结果输出到终端

### 2026-01-26: BSIM3v3 模型
- 完整实现 BSIM3v3 DC 模型（1952 行代码）
- 支持 50+ BSIM3 参数
- 23 个单元测试覆盖所有计算模块
- 支持 NMOS/PMOS 自动处理

### 2026-01-25: .model 支持
- 解析 `.model` 语句
- 模型参数与实例绑定

### 2026-01-24: CLI 分析类型
- 支持 `--analysis` 选择分析类型
- DC 扫描参数命令行指定
