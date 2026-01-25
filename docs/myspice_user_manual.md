# MySpice 用户手册

本文档面向使用 MySpice 的用户，介绍基本使用流程、命令行模式以及如何读取仿真输出。当前版本以 DC 与 TRAN 的基本流程为目标，随着功能完善会持续更新。

## 1. 快速开始

### 1.1 准备 netlist

MySpice 使用 SPICE 风格网表文件作为输入。示例:

```
* Basic DC example
V1 in 0 DC 1
R1 in out 1k
R2 out 0 2k
.op
.end
```

保存为 `example.cir`。

### 1.2 运行仿真

当前的二进制入口为 `sim-cli`:

```
sim-cli example.cir
```

运行成功会打印解析信息。后续版本将输出实际仿真结果。

---

## 2. 命令行模式 (Binary Mode)

### 2.1 基本语法

```
sim-cli <netlist>
```

- `<netlist>`: SPICE 网表文件路径  
- 网表必须包含 `.end`

### 2.3 当前解析能力说明

当前 parser 为 skeleton 版本，具备以下能力:

- 行级解析与续行拼接
- 识别注释行与空行
- 识别控制语句与器件语句
- 抽取器件节点与值/模型 (简化规则)
- 参数按 `key=value` 形式抽取
- 识别 `.param` / `.model` / `.subckt` (基础字段)
- 部分器件节点与字段的基础校验
- MOS 支持 3 节点（隐式 bulk=0）
- 支持 `.include` 递归读取 (基础版)
- 支持子电路基础展开与参数映射 (简单覆盖规则)
- 支持基础参数表达式求值 (加减乘除与括号，支持单位后缀)
- 受控源 E/G/F/H 支持 POLY 语法解析与系数校验
- POLY 控制节点/控制源数量校验
- 支持嵌套子电路基础展开
- 支持参数表达式函数: max/min/abs/if，并支持一元负号与幂运算 (^)
- 电压/电流源支持 DC 关键字取值
- 电压/电流源支持波形关键字（PULSE/SIN/AC 等）
- 参数支持逗号分隔，model 括号参数解析
- 波形关键字缺参校验
- 支持子电路内部 `.param` 作用域

注意:

- 节点数量与字段语义尚未严格校验
- 参数表达式与更复杂的作用域规则将在后续阶段完善

### 2.2 批量运行 (spice-datasets)

项目支持批量运行 `spice-datasets` 中的网表，用于 smoke test:

```
python tests/run_spice_datasets.py
```

执行后会输出通过率，例如:

```
total=50 passed=50 failed=0 passrate=100.00%
```

---

## 3. 仿真结果查看

当前阶段已支持基础求解输出（默认 Dense，KLU 可选）。未来输出将支持:

- OP/DC/TRAN 的节点电压与器件电流
- 波形数据查询与导出
- PSF 文本格式输出

### 3.1 结果解读建议 (未来支持)

- OP: 查看静态工作点电压与电流
- DC Sweep: 查看参数扫描曲线
- TRAN: 查看时域波形

---

## 4. 常见问题

### 4.1 提示缺少 .end

确保网表末尾有 `.end` 行。

### 4.2 无输出结果

当前版本已支持基础求解输出。若无输出，通常是求解失败或网表语法不完整，请先检查错误提示。

---

## 5. 版本说明

手册会与项目阶段同步更新。后续将补充:

- 仿真结果字段说明
- 交互式 API 使用方法
- CLI 与 AI 代理的使用示例
- KLU 求解器的使用与依赖说明
- 仿真引擎核心结构说明 (Circuit/MNA/Solver)
- 仿真引擎骨架的使用与调试说明

当前进展:

- 已加入 DC 基础 stamp（R/I/V/D/MOS）
- MNA 骨架与单元测试已覆盖
- KLU 接口为 stub，需链接 SuiteSparse 后启用
- Newton 迭代与收敛控制骨架已加入
- gmin/source stepping 基础接入
- 二极管/MOS 非线性线性化（简化模型）
- TRAN 骨架入口（时间步循环）
- TRAN 电容/电感等效 stamp
- TRAN 自适应步长与误差估计骨架
- TRAN 非线性器件 Newton 迭代
- ResultStore 接入与仿真结果输出
- PSF 文本输出（基础格式）
- TRAN 收敛失败时回退 gmin/source stepping
- TRAN 加权误差估计

## 6. KLU 求解器依赖说明

MySpice 的线性求解器规划为 SuiteSparse 的 KLU。当前阶段仅完成规划与接口设计，正式启用时需要:

- 安装 SuiteSparse（包含 KLU）
- 在 Windows 环境准备预编译库或本地编译产物

### 6.1 安装流程（草案）

#### Windows（推荐）

1) 安装 Visual Studio Build Tools（包含 C/C++ 工具链）
2) 获取 SuiteSparse 预编译包或自行编译
3) 配置环境变量或在构建系统中指定库路径

建议路径约定:

- 头文件: `C:\libs\suitesparse\include`
- 库文件: `C:\libs\suitesparse\lib`

#### Linux

1) 使用系统包管理器安装 SuiteSparse  
   - Ubuntu 示例: `sudo apt install libsuitesparse-dev`
2) 确认 `klu.h` 与库文件可被构建系统找到

#### macOS

1) 使用 Homebrew 安装  
   - `brew install suite-sparse`
2) 确认库路径可被 Rust 构建系统访问

### 6.2 构建系统配置（草案）

建议采用 `build.rs` + 环境变量的方式自动发现 KLU。

环境变量约定（示例）:

- `SUITESPARSE_DIR`：SuiteSparse 根目录  
- `KLU_INCLUDE_DIR`：头文件目录  
- `KLU_LIB_DIR`：库文件目录  

构建系统行为（建议）:

1) 优先读取 `KLU_INCLUDE_DIR` / `KLU_LIB_DIR`
2) 若未设置，则尝试 `SUITESPARSE_DIR/include` 与 `SUITESPARSE_DIR/lib`
3) 最后尝试系统默认路径

示例（Windows PowerShell）:

```
$env:SUITESPARSE_DIR="C:\libs\suitesparse"
```

示例（Linux/macOS）:

```
export SUITESPARSE_DIR=/usr/local
```