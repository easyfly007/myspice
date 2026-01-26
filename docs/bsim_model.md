# BSIM 模型说明（MySpice 版本）

## 1. BSIM 是什么
BSIM（Berkeley Short-channel IGFET Model）是由 UC Berkeley BSIM 研究组维护的 MOSFET 紧凑模型系列。  
BSIM4 是 BSIM3 的后继版本，面向深亚微米工艺，覆盖短沟道效应、迁移率退化、速度饱和、DIBL、寄生电阻电容等多种物理效应，并成为工业界主流 SPICE MOS 模型之一。  
参考：BSIM4 模型简介与官方发布页面。  
来源：https://bsim.berkeley.edu/models/bsim4/

> 本文档是 **MySpice 当前实现的“简化版 BSIM 参数支持”** 说明。  
> MySpice 目前只实现了 **极小子集**，用于打通 `.model` → 参数 → stamp 的流程；并非完整 BSIM3/BSIM4。

---

## 2. MySpice 当前支持范围

### 2.1 已支持的模型参数（极简）

#### 二极管（D）
| 参数 | 含义 | 当前用途 |
|------|------|----------|
| `IS` | 反向饱和电流 | 用于 `I = IS (exp(Vd/Vt) - 1)` |
| `N` / `NJ` | 结理想因子 | 修正热电压 `Vt = 0.02585 * N` |

#### MOS（M）
| 参数 | 含义 | 当前用途 |
|------|------|----------|
| `VTH` / `VTO` | 阈值电压 | 控制导通/截止区分 |
| `BETA` / `KP` | 转导参数 | 影响 Id、gm、gds |
| `LAMBDA` | 沟道长度调制 | 影响饱和区输出电导 |

> 注意：这些参数属于“教学/原型级别”，并非 BSIM4/BSIM3 的完整物理建模。

### 2.2 参数合并规则
1. `.model` 参数作为默认值  
2. 器件实例参数覆盖 `.model` 参数  

例如：
```
.model NMOS NMOS VTO=0.7 KP=1e-3
M1 d g s b NMOS VTO=0.65
```
最终 `M1.VTO = 0.65`，`KP = 1e-3`。

---

## 3. 目前的 MOS 简化计算模型

> 这不是 BSIM4，仅是教学级 MOS 方程，用于打通流程。

设：
- `Vgs = Vg - Vs`
- `Vds = Vd - Vs`
- `Vth = VTO` 或 `VTH`
- `beta = KP` 或 `BETA`
- `lambda = LAMBDA`

### 3.1 区域划分
1. **截止区**：`Vgs <= Vth`  
   - `Id = 0`  
   - `gm = 0`  
   - `gds = gmin`

2. **线性区**：`Vds < Vgs - Vth`  
   - `Id = beta * ((Vgs - Vth) * Vds - 0.5 * Vds^2)`
   - `gm = beta * Vds`
   - `gds = beta * (Vgs - Vth - Vds)`（并限制 `gds >= gmin`）

3. **饱和区**：`Vds >= Vgs - Vth`
   - `Id = 0.5 * beta * (Vgs - Vth)^2 * (1 + lambda * Vds)`
   - `gm = beta * (Vgs - Vth) * (1 + lambda * Vds)`
   - `gds = 0.5 * beta * (Vgs - Vth)^2 * lambda`（并限制 `gds >= gmin`）

---

## 4. Stamp（线性化）流程

### 4.1 MOS Stamp（线性化导数）
当前在 `stamp.rs` 中使用 **小信号等效**：
```
Id ≈ Id0 + gm * (Vg - Vs) + gds * (Vd - Vs)
```
在 MNA 中体现为：
- 在 `drain/source` 节点加入 `gds` 导纳
- 在 `gate` 控制项加入 `gm`
- 在 RHS 中加入等效电流 `ieq = Id - gm*Vgs - gds*Vds`

### 4.2 二极管 Stamp
基于 Shockley 模型：
```
Id = IS (exp(Vd/Vt) - 1)
gd = (IS/Vt) * exp(Vd/Vt)
```
在 MNA 中以 `gd` 线性化，并加入 RHS 等效电流 `ieq`。

---

## 5. 完整 BSIM4/BSIM3 的参数与物理效应（概览）
完整 BSIM4 模型包含大量参数与效应分组，主要包括：

- **阈值电压与体效应**：`VTH0/K1/K2/DVT0/DVT1/DVT2/ETA0 ...`
- **迁移率退化**：`U0/UA/UB/UC/VMAX ...`
- **短沟道效应与 DIBL**：`ETA0/ETAB/DSUB ...`
- **寄生电阻/电容**：`RDSW/CGSO/CGDO/CGBO ...`
- **温度模型**：`TNOM/UTE/KT1/KT2 ...`
- **噪声模型**：`NOIA/NOIB/NOIC ...`
- **模型选择开关**：`MOBMOD/CAPMOD/NOIMOD ...`

完整参数表可参考 BSIM4 官方/教材文档（篇幅巨大）。  
参考（BSIM4 参数分组与说明）：
https://people.ece.ubc.ca/robertor/Links_files/Files/ICCAP-2008-doc/icmdl/icmdl048.html

---

## 6. 未来计划（建议路线）
1. **短期**：扩展 MOS 参数（`GAMMA/PHI/TOX/U0/VSAT/...`）  
2. **中期**：支持 BSIM3 或 BSIM4 的核心 DC 方程  
3. **长期**：完整 CV/噪声/温度模型 + 参数 binning  

---

## 7. 当前实现位置（代码导航）
- `.model` 解析与合并：`sim-core/src/netlist.rs`
- MOS/二极管 stamp：`sim-core/src/stamp.rs`
- 参数解析（字符串 → 数值）：`parse_number_with_suffix`

---

## 8. 免责声明
本项目当前实现的是 **教学级简化 MOS 模型**，用于打通流程与验证架构。  
要达到工业级 BSIM4/BSIM3 精度，需要引入完整方程与参数集，以及更严格的数值稳定性处理。  
请勿将当前模型用于真实工艺的精确仿真。
