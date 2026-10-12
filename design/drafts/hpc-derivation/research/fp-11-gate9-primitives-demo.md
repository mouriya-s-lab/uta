# fp-11：闸门 9 demo——原语中间层值不值得做

状态：一手实测，闭合。**定位**：`hpc-derivation-subsystem.md` §8.1 的中间层假设（decision-log F7）在写进设计前必须先由 demo 回答两件事——**开发体验简洁多少、相比标量快多少**（维护者要求）。本文合并三个独立代理的同口径实验：原语集 demo（附录 `fp-11-appendix-prims.md`）、零优化普通写法基线（`fp-11-appendix-naive.md`）、对齐/cache line 对照（`fp-11-appendix-align.md`）。全部 Rust、Apple M4 aarch64、stable rustc 1.95.0、release `opt-level=3, lto=true`；真实 SPY 日线 8465 行（SHA-256 `eeee8593…`）；golden 同 fp-10（1e-9）。标注 **[OBSERVED]** / **[INFERENCE]**。

---

## 1. 摘要

- **值得做，附一个条件。** 用词汇取自 pandas/numpy 的原语集（`rolling/ewm/shift/where/linreg` + 标量逃逸口）写三个 Pine 级指标：**作者代码 115 行 vs 手写 pulp 425 行（降 73%）vs ndarray 手写 268 行（降 57%）**；七指标全部逐点匹配 golden、validity 位对齐、计时区零分配。**[OBSERVED]**
- **性能**（同一进程同一次运行，四实现并排）：SuperTrend 原语版 52.8 µs——比手写 pulp（72.3）**快** 27%；Squeeze 142.5 µs = 手写 pulp 的 1.25×；**Ichimoku 206 µs = 手写 pulp 的 2.23×，超过 1.5× 闸门**。原因单一：它完全由 rolling max/min 主导，纯标量单调队列在 9/26/52 的小窗口上打不过显式 NEON。**[OBSERVED]**
- **条件**：`rolling.min/max` 是唯一需要在原语**内部**用 SIMD 的 kernel（作者面不变）；加上它 Ichimoku 预期回到 ~1.0–1.2× **[INFERENCE，未实测]**。其余原语纯标量 + autovec 即可。
- **零优化普通写法**（`Vec` + 索引 + 朴素 O(n·w) 窗口 + 中间量分配 + NaN）：release 下 Ichimoku 643 µs、Squeeze 412 µs（比原语版慢 3.1× / 2.9×），SuperTrend 58 µs（反而不慢——递推主导，无可优化）；debug 构建 1.2 / 11.9 / 10.6 ms（16–61×）。**[OBSERVED]**
- **对齐/cache line**：64 B 对齐 vs 起点错位 8/24/40 B vs 普通 `Vec`，3 指标 × 2 实现 × 7 pass，差异全部在 ±2% 噪声内（24 组对照 19 组区间相交，5 组不相交且方向不一致）。**对齐在这个数据规模上测不出收益或惩罚。** **[OBSERVED]**

---

## 2. 主表：四种写法并排（µs/call，中位数；同机同数据同次运行）

| 指标 | 零优化普通写法 | 原语集（作者不碰 SIMD） | ndarray 手写 | 手写 pulp NEON | 直白标量 oracle | debug 零优化 |
|---|---:|---:|---:|---:|---:|---:|
| SuperTrend(10,3) | 58.3 (9 allocs) | **52.8** | 50.7 | 72.3 | 72.9 | 1211 |
| Ichimoku(9,26,52) | 642.9 (12 allocs) | 206.0 | 226.8 | **92.6** | 329.9 | 11912 |
| Squeeze Momentum | 412.3 (14 allocs) | 142.5 | 114.3 | **113.6** | 173.2 | 10602 |
| 作者 LOC（三指标合计） | 182 | **115** | 268 | 425 | — | — |

三份实现的 golden 与 validity 全部通过；除零优化版外分配均为 (0,0)。[OBSERVED]

读法：
- **原语层的价值在窗口类指标**：Ichimoku/Squeeze 相对零优化写法快 3×，相对直白标量快 1.2–1.6×——来源是 O(n) 算法（增量窗口、单调队列、linreg 代数递推），不是 SIMD。
- **递推类指标没有优化空间**：SuperTrend 四种写法 50–73 µs，零优化版甚至比 pulp 版快；手写 SIMD 的 head/tail 样板反而是负担。印证 E11。
- **显式 SIMD 唯一显著赢的地方**：朴素 rolling max/min（Ichimoku pulp 92.6 vs 单调队列 206）。fp-10 猜"O(n) 队列可能反超 SIMD"——**被否证**（M4、窗口 ≤52）。

---

## 3. 开发体验

| 指标 | 原语集 | ndarray 手写 | pulp 手写 | 逃逸口（原语集） |
|---|---:|---:|---:|---|
| SuperTrend | 43 | 62 | 114 | `up/dn/trend` 状态机 ~18 行（强制）；TR[0] 位图修正 1 行 |
| Ichimoku | 36 | 78 | 99 | **无** |
| Squeeze | 36 | 128 | 212 | `sqz: i8` 三态分类 ~6 行（输出是 i8 列） |

LOC 口径：非空非注释非属性行，含签名/输出准备/状态机，不含 harness/oracle；原语 crate 不计入作者 LOC。预注册阈值"降 ≥25–30% 才算显著"——实测 57%/73%。[OBSERVED]

三份 SuperTrend 并排（全文见附录 prims §A）：状态机段三者几乎相同（命题 8：不可消除）；差距全在窗口/递推的表达——原语版 `shift` + `ewise3` + `ewm(Wilder(10))` 三行拿到 TR 与 ATR；ndarray 手拼三个 offset 视图 + `Zip` + 手写 `rma10`；pulp 手写 `WithSimd` 拆 head/tail + `simd.*` + 手写 tail。

写作摩擦（实记，附录 prims §4.4）：`Col`（视图 + 位图）可 `Copy` 复用，`ColMut` 每个输出内联重建，没有 pulp 那种多列 lifetime 拼装；**最高风险处是 `ewm` seed 语义**——seed 必须按"第 n 个有效样本"计数而非物理下标，否则 RSI（diff 派生列有效起点偏一）首值错位；冻结后 EMA/RSI/ATR 一次通过。temp 列显式传参使签名偏长（demo 未收进 scratch struct，保守多算了原语版的行）。

**原语 crate 自身成本**：428 行（validity 位图、`Col/ColMut`、Neumaier 累加、Workspace、rolling{sum,mean,var,std,min,max}、ewm、shift/diff、linreg、ewise/map、cumsum/reduce）。摊到七指标 61 行/指标。这是选 (b) 的代价：`min_periods`/ddof/seed/`gap_policy`/validity 传播实现一次并成为契约。

---

## 4. NEON 逐 kernel 与写法纪律

| kernel | 算术 `.2d` | autovec | 未命中原因 |
|---|---|---|---|
| `ewise2/3/4`（elementwise/where，经 `Zip`） | 8 × `fadd.2d` | **是** | — |
| `rolling.mean/std` | 0 | 否 | 归约重结合被禁 + 循环携带（单 Neumaier 累加器增量加减） |
| `rolling.max/min` | 0 | 否 | 循环携带 + 数据依赖分支（单调队列） |
| `ewm` | 0 | 否 | 递推 `y_t = f(y_{t-1})` |
| `linreg` | 0 | 否 | O(1) 代数递推 |

结论 [OBSERVED]：只有 elementwise 被 autovec；其余是**设计使然的标量递推**，靠算法拿速度。数据只有 16 B 分配器对齐，`ewise` 仍 autovec 成 `.2d`——**对齐不是 autovec 前提**（与 AlignControl 结论一致）。

---

## 5. 对齐与 debug 量级（附录 align）

| 指标 | 64 B 对齐 scalar / pulp | +8 B | +24 B | +40 B | 普通 Vec | debug（opt-level=0）scalar / pulp |
|---|---|---|---|---|---|---|
| SuperTrend | 72.9 / 72.6 | 0.987 / 0.969 | 0.986 / 0.983 | 0.992 / 0.984 | 0.993 / 0.993 | 352 / 1126 |
| Ichimoku | 390 / 92.2 | 1.000 / 1.011 | 1.003 / 1.003 | 0.851 / 1.001 | 1.003 / 1.001 | 7001 / 8982 |
| Squeeze | 172.8 / 113.7 | 1.002 / 0.997 | 1.001 / 0.999 | 1.000 / 0.999 | 0.999 / 0.997 | 3480 / 8692 |

（比值相对 64 B 对齐；Ichimoku scalar 在本次运行呈 331/390 双峰，属机器状态，不是对齐效应。）debug 构建 5–20× 慢（scalar）到 12–90× 慢（pulp/零优化），只作量级参照。

---

## 6. 对闸门 9 判据的裁定

判据（§9 闸门 9）："比手写 pulp 慢 >1.5× 或 LOC 未显著下降 → 回退 (a1) 纯 `ndarray` + 作者手写。"

- LOC：**通过**（57–73%）。
- 性能：SuperTrend 0.73×、Squeeze 1.25× **通过**；Ichimoku 2.23× **不通过**，稳定（7 pass 205–207 µs），无实现错误，根因是单一原语 `rolling.min/max`。

**裁定：中间层值得做**；把 `rolling.min/max` 列为唯一允许内部 SIMD kernel 的原语（§8.1 已预留的阀门），其余纯标量 + autovec。Ichimoku 在加内部 SIMD 后是否落进 1.5× → 作为闸门 9 的补测项。若维护者裁定原语层不许任何 SIMD，则 rolling-extreme 密集指标显式接受 ~2× 手写代价。

---

## 7. 未覆盖

- [需实测] `rolling.min/max` 内部 SIMD 版及其对 Ichimoku 的实际效果。
- [需实测] 中间列物化的内存流量占比（原语版逐原语物化 temp 列，pulp 融合循环——prims/pulp 差异含此项）。
- [需实测] 人造 gap × `gap_policy` 端到端（SPY 无缺口，`ewm` 的 Hold/Reset/Missing 分支未触发）。
- [未覆盖] x86 AVX2 实机。

---

## 8. 路径

- 原语 crate `/Users/mouriya/Ext/tmp/fp10/gate9/uta-prims/`；demo harness `gate9/demo/`（`baselines.rs` 为 fp-10 scalar/pulp/ndarray 逐字移植）；`gate9/run.log`；汇编 `gate9/demo.otool.txt`。
- 零优化 crate `gate9/naive/`；对齐 crate `gate9/align/`。
- 数据 `/Users/mouriya/Ext/tmp/fp10/data/spy_daily.csv`。
- 三份原始子报告：`fp-11-appendix-{prims,naive,align}.md`。
