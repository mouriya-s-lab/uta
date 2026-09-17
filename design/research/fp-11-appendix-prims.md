# 闸门 9：中间层原语集实证（uta-prims demo）

> 本文是 `hpc-derivation-subsystem.md` §9 闸门 9 / §8.1（decision-log F7）的一手实测证据，供 Main 合并进 `fp-11-gate9-primitives-demo.md`。全部本机 M4/aarch64、stable rustc 1.95.0、release（`opt-level=3, lto=true, codegen-units=1, panic=abort`）实测。标注 **[OBSERVED]**（本机亲测/汇编逐行看到）/ **[INFERENCE]**（据源码/结果推断）。产物落 `/Users/mouriya/Ext/tmp/fp10/gate9/`，不落 /tmp、不触本仓库构建。

## 0. 摘要（首行直接回答）

**值得做，但有一个明确例外。** 用词汇取自 pandas/numpy 的一小组原语（rolling/ewm/shift/where/linreg + 标量逃逸口）重写三个 Pine 级指标：**调用侧代码相对手写 pulp 降 73%、相对 ndarray 手写降 57%（三指标合计 115 行 vs 425 / 268）**，全部逐点匹配 fp-10 golden（1e-9）、validity 位对齐、计时区零分配 (0,0)。性能上 **SuperTrend 0.73×（比手写 pulp 还快）、Squeeze 1.25×（过 1.5× 闸门）——只有 Ichimoku 2.23×（超 1.5×，不过闸门）**，原因是它唯一依赖 rolling max/min，而纯标量单调队列在 M4 的小窗口（9/26/52）上打不过 pulp 的显式 NEON。这恰是 §8.1 已预留内部 pulp 的那一个原语（"仅当实测超过 autovec 才在原语内部用 pulp"）。**结论：中间层假设成立——LOC 显著下降、绝大多数指标性能≤手写；把 rolling min/max 作为唯一需要内部 SIMD 的原语即可让 Ichimoku 也过闸门。**

## 1. 方法与口径（同机同编译器同数据，四实现并排重跑）

- **不复用 fp-10 历史 µs**。scalar / 手写 pulp / ndarray 三个基线全部从 fp-10 源码逐字移植到同一 harness（`demo/src/baselines.rs`，即 `scalar_*`/`pulp_*`/`nd_*`），与 prims 消费**完全相同**的 SPY 输入、产生相同完整输出、都过 `black_box`，在**同一进程同一次运行**里重测。[OBSERVED]
- **数据**：`/Users/mouriya/Ext/tmp/fp10/data/spy_daily.csv`，SHA-256 `eeee8593…`，8465 行。golden = fp-10 §4.1 定点值，全部命中（见 §2）。
- **计时**：`bench_once` = 20 warmup + 300 iter 中位数；prims 复杂三指标再取 **7 个独立 pass 的中位数并报 min/max**（阈值附近看分布）。counting global allocator，`probe()` 包一次调用测分配。
- **oracle 独立**：golden oracle = 直白标量 `scalar_*`（朴素每窗重算，不复用任何原语/代数推导），prims 全数组对它断言 `|a-b| ≤ 1e-9 + 1e-9·|b|` **且** validity 位图逐位相等；复杂三指标另对 fp-10 golden 定点断言。断言不通过即 panic。[OBSERVED]
- **不用 `#[inline(never)]` 污染 prims**：被测的 `prims_*` 是普通函数，内部原语（uta-prims）正常内联/优化；汇编观察另用只读 `#[no_mangle]` 薄 wrapper（`asm_*`），不参与计时。

## 2. 正确性：七指标全过 golden + validity（allocs (0,0)）

运行日志 `/Users/mouriya/Ext/tmp/fp10/gate9/run.log`，末行 `ALL GOLDEN + VALIDITY (bit-for-bit) ASSERTIONS PASSED`。[OBSERVED]

| 指标 | prims 定点值（idx 100/1000/5000/8464） | golden | 匹配 |
|---|---|---|---|
| SuperTrend val | 45.315821/73.348225/137.373727/775.277032 | 同 | ✓ (trend −1/1/1/−1) |
| Ichimoku tenkan | 44.703125/75.070312/140.959999/761.815002 | 同 | ✓ |
| — kijun | 44.828125/74.632812/139.209999/764.484985 | 同 | ✓ |
| — senkouB(raw) | 44.546875/73.742188/140.930001/754.234985 | 同 | ✓ |
| Squeeze val (fv=38) | −0.632299/1.777589/3.481440/−9.281345 | 同 | ✓ (sqz 位对齐) |
| SMA20/EMA14/RSI14/BB20 | 对 scalar oracle 全数组 1e-9 | — | ✓ |

validity 位图与 oracle 的 `!isnan` 逐位相等（`first_valid`：ST 9、Ichi tenkan 8/kijun 25/senkouB 51、Squeeze 38、SMA 19、EMA 13、RSI 14、BB 19），计时区分配全部 **(0,0)**。[OBSERVED]

## 3. 性能主表（µs/call，同机同数据同次运行）

复杂三指标 prims 取 7-pass 中位；scalar/pulp/ndarray 取 300-iter 中位。判据 = prims/pulp。

| 指标 | prims µs (min..max) | scalar | pulp(手写NEON) | ndarray | **prims/pulp** | prims/ndarray | scalar/prims | allocs |
|---|---|---|---|---|---|---|---|---|
| **SuperTrend(10,3)** | 52.79 (52.4..53.1) | 72.9 | 72.3 | 50.7 | **0.730** ✅ | 1.042 | 1.381 | (0,0) |
| **Ichimoku(9,26,52)** | 206.0 (205.5..206.9) | 329.9 | 92.6 | 226.8 | **2.225** ❌ | 0.909 | 1.601 | (0,0) |
| **Squeeze Momentum** | 142.5 (142.2..142.7) | 173.2 | 113.6 | 114.3 | **1.254** ✅ | 1.246 | 1.215 | (0,0) |

warm-up（prims vs scalar oracle，非闸门；补偿求和 + 位图传播的固定开销使其略慢于裸标量）：

| 指标 | prims µs | scalar µs | scalar/prims |
|---|---|---|---|
| SMA20 | 16.2 | 12.0 | 0.743 |
| EMA14 | 16.1 | 12.8 | 0.798 |
| RSI14 | 52.8 | 36.3 | 0.688 |
| BB20 | 49.4 | 43.1 | 0.873 |

**要点** [OBSERVED]：
- **SuperTrend 递推状态机：prims 0.73× 手写 pulp**——比 pulp 还快。因为 SuperTrend 的向量化余地≈0（fp-10 已证≈1.0×），pulp 的 `WithSimd` 拆 head/tail 反而增加开销；prims 走干净标量 + autovec 的 ewise（TR/hl2）就够，逃逸口的状态机是纯标量。
- **Squeeze 混合：prims 1.25×**——窗口统计（mean/std/rolling max-min）+ linreg + 逃逸口分类，均在 1.5× 内。
- **Ichimoku：prims 2.23×，超闸门**——它 100% 由 rolling max/min 主导；纯标量单调队列在 9/26/52 的小窗口上打不过 pulp 的暴力 SIMD（见 §5）。但 prims 仍 0.91× ndarray（ndarray 是朴素 O(n·w) 标量），即 O(n) 队列确实比朴素标量快，只是**没有反超 SIMD**——这正面否证了 fp-10 §4.2 "换单调队列 O(n) 可能反超 SIMD" 的猜测（对 M4 这些窗口尺寸为假）。

## 4. 开发体验（核心交付）

### 4.1 调用侧 LOC（统一机械规则：非空非注释非属性行，含签名/输出准备/状态机；排除 harness/oracle/测试；原语 crate 单列）

| 指标 | prims（用原语集） | ndarray 手写 | pulp 手写 |
|---|---|---|---|
| SuperTrend | **43** | 62 | 114 |
| Ichimoku | **36** | 78 | 99 |
| Squeeze | **36** | 212 | 212 |
| **合计** | **115** | **268** | **425** |
| 相对 prims 降幅 | — | prims 降 **57%** | prims 降 **73%** |

> pulp 合计 = 各指标的 `WithSimd` struct + impl + wrapper + 其专属 helper（ST 的 `supertrend_state`/`tr_at`、Ichi 的 `rolling_mid_simd`、Squeeze 内联 linreg + `LINREG_X`）。ndarray 合计 = `nd_*` + 其专属 helper（`rma10`/`rolling_extreme`/`mean20`/`stdev20`/`linreg20`）。prims 只数 `prims_*` 函数体（含多参数签名与逃逸口），**generic 原语实现不计入用户 LOC**。此口径为本次重算，数值低于 fp-10 报的 185/189/322（后者含空行/花括号/驱动），维护者已要求统一重算而非沿用。[OBSERVED]

**预注册阈值**（编码前定，见 mentor 校准）：复杂三指标合计 LOC 降 ≥25–30% 才算"显著下降"。实测 57%（vs ndarray）/ 73%（vs pulp），**决定性通过**。

### 4.2 原语 crate 自身成本（另列，摊销）

`uta-prims/src/lib.rs` = **428 代码行**（非空非注释）。这是可复用底座（validity 位图 + Col/ColMut + Neumaier 累加器 + Workspace + rolling{sum,mean,var,std,min,max} + ewm + shift/diff + linreg + ewise2/3/4/map1 + cumsum/reduce）。摊到三复杂指标 = 143 行/指标；摊到全部 7 指标 = 61 行/指标。**这是选 (b) 而非 (a1) 让作者手写一切的代价**：validity 传播 / `min_periods` / ddof / seed / `gap_policy` 语义由子系统实现一次并成为契约，作者不重写。

各原语 crate LOC（供参考）：linreg 57、extreme(deque) 40、ewm 39、moments(rolling 核) 29、ewise4 23、Neu 17、Mask 15、shift 15、cumsum 15、std/var 各 15、rolling mean/sum 各 13/12。

### 4.3 逃逸口清单（[OBSERVED]）

| 指标 | 逃逸口 | 行数 | 性质 |
|---|---|---|---|
| SuperTrend | `up/dn/trend` loop-carried 状态机 `for` 循环 | ~18 | **强制**（命题 8：任何纯向量词汇都无法表达） |
| SuperTrend | TR[0]=H−L 边界（`trv.copy_from_slice(hv)` 修 validity） | 1 | 边界折叠（NaN-max 已让值正确，仅补位图） |
| Squeeze | `sqz: i8` 三态分类 `for` 循环 | ~6 | 分类输出是 i8 列，非 f64，不进 where 原语 |
| **Ichimoku** | **无** | 0 | 纯 `rolling.max/min` + `ewise2` + `shift` 组合 |

Ichimoku 零逃逸口是 (b) 的最好例证：多窗口 rolling max/min + 位移完全由原语组合表达；SuperTrend 则印证了逃逸口不可消除。

### 4.4 写作摩擦（[OBSERVED]，编码时实记）

1. **Col/ColMut 绑值+位图**：`Col` 派生 `Copy`（内含 `ArrayView1` + `&[u64]`），同一输入列可无成本复用（`rolling(close,20).mean(...)` 后再 `.std(...)`）；`ColMut` 含 `&mut` 不能 Copy，每个输出 `ColMut::new(buf, mask)` 内联重建（零分配）。相对 pulp 的 `WithSimd` 多输出 struct 随列数变宽、每列手写 head/tail，这里没有 lifetime 拼装摩擦。
2. **借用拆分**：原语内部 `out.data.as_slice_mut()` 与 `out.valid` 必须走**独立字段**访问才能同时可变借用（方法 `slice_mut(&mut self)` 会整体借用 → E0499）。是实现者一次性摩擦，不外泄给作者。
3. **ewm seed 语义（最高风险处，mentor 预警）**：seed 按**第 n 个有效样本**计数，而非物理下标 n−1。这样同一 `Wilder(14)` 同时适配 close（有效从 0，seed 落 idx13）与 diff 派生的 gain/loss（有效从 1，seed 落 idx14）——RSI 首值在 14 不是 13，正因第一个 diff 使有效起点偏一。冻结该语义后 EMA/RSI/ATR 一次通过。
4. **temp 列线程化**：prims 签名里显式传 temp 列（ST 的 pc/tr/atr/hl2、Squeeze 的 basis/sd/range/…）+ 各自位图，参数很多。真实 SDK 可把这些收进一个 scratch/output struct 减少签名行；本 demo **未收**（保守，多算 prims 的行），即便如此 prims 仍大幅领先。
5. **Workspace**：rolling min/max 的单调队列需一块可复用 `usize` scratch（`Workspace::new(N)`，容量≥列长），计时区零分配。作者一次构造、跨调用复用，是唯一显式 scratch。

## 5. NEON 逐 kernel 记录（写法纪律，[OBSERVED] 汇编 `demo.otool.txt`）

Rust 无 fast-math、LLVM 不重结合浮点归约——凡 loop-carried 或单累加器归约的原语**本就应是标量**，看到 `.2d` 也要区分是算术还是 `movi.2d`（寄存器清零）。逐 kernel（`otool -tvV` 定位单态化符号）：

| kernel | 算术 `.2d` | 标量 `d` fp | 是否 autovec | 未命中原因 | 写法 |
|---|---|---|---|---|---|
| `ewise2/3/4`（elementwise/where） | **8 `fadd.2d`** | — | **是** | — | ndarray `Zip`（无边界检查、无归约、无循环携带） |
| `Rolling::mean` | 0 | 16 | 否 | **归约重结合被禁 + 循环携带**（单 Neumaier 累加器增量加/减） | 索引 `x[i]`/`x[j]` + `bit_get` |
| `Rolling::std` | 0 | 36 | 否 | 同上（两条 Neumaier：sum 与 sum_sq） | 同上 |
| `Rolling::max/min` | 0 | 1 (`fcmp`) | 否 | **循环携带 + 数据依赖分支**（单调队列） | 索引 + deque |
| `ewm` | 0（2 处 `movi.2d` 是清零） | 6 | 否 | **循环携带递推** `y_t=f(y_{t-1})` | 顺序标量 |
| `linreg` | 0（5 处 `movi.2d` 是清零） | 33 | 否 | **循环携带 O(1) 递推** `Sxy(i)=Sxy(i-1)−Sy(i)+w·y_i` | 顺序标量 |

**结论**：只有 elementwise（经 `Zip`）autovec 成 `fadd.2d`；rolling sum/mean/std、ewm、linreg 全部是**设计使然的标量递推**，非 bug、非边界检查所致——它们靠 O(n) 算法（增量窗口 / 单调队列 / 代数递推）拿速度，不靠 SIMD。这与 §8.1 "窗口向量化、递归标量" 的分工一致。

**边界检查处理**：ewise 走 `Zip`/迭代器，无逐元素边界检查（也因此能 autovec）；rolling/deque 用索引 `x[i]`，`extreme` 前置 `assert!(ws.idx.len()>=n)`，但即便消掉边界检查这些循环也不会向量化（归约重结合/循环携带是根因，不是边界检查）。

**对齐观察**：输入/输出列是普通 `Vec<f64>`，系统分配器只给 **16 B 对齐**（非 64 B cache line）。`ewise2` 仍 autovec 成 `fadd.2d` → **对齐不是 aarch64 autovec 的前提**（NEON 容忍非对齐 load/store）；64 B 对齐的收益需另测（已交 AlignControl 代理）。

## 6. 对闸门 9 判据的裁定

判据（§9 闸门 9）："原语集若比 fp-10 手写 pulp 版慢超过 1.5× **或** LOC 未显著下降 → 回到 (a1) 纯 ndarray + 作者手写。" 预注册细化（mentor）：**逐指标**任一稳定 >1.5× 即性能失败；LOC 同口径合计降不足阈值即简化失败。

- **LOC 判据：通过（决定性）**。合计降 57%/73%，远超 25–30% 阈值；(0,0) 分配；validity 位对齐。
- **性能判据：三缺一**。SuperTrend 0.73×（且快过 pulp）、Squeeze 1.25×——过；**Ichimoku 2.23×——不过**（7-pass 稳定 205–207µs，非噪声，无实现 bug：golden + 位图全过，deque 已是竞态 O(n) 实现、去 modulo、单 `if` 前驱逐出、is_max 分支外提）。

**裁定**：中间层假设**基本成立、值得做**——它以 57–73% 的作者 LOC 降幅换来 2/3 复杂指标性能≤手写 pulp（SuperTrend 反超）。唯一失败点 Ichimoku 完全归因于 rolling max/min 这一个原语：纯标量单调队列在 M4 小窗口上是 pulp 显式 NEON 的 ~2.2×。这**不是**推翻 (b) 的证据，而是精确指向 §8.1 已写的那条阀门——"rolling min/max…仅当实测超过 autovec 才在原语内部用 pulp，永不暴露给作者"。**建议**：把 rolling min/max 作为唯一给内部 pulp/SIMD kernel 的原语（作者面不变），其余全部纯标量 + autovec；据 fp-10 pulp Ichimoku 92µs [INFERENCE]，届时 Ichimoku prims 应落到 ~1.0–1.2× pulp，全三指标过闸门。若维护者坚持"原语层不许有任何 SIMD"，则 Ichimoku 类 rolling-extreme 密集指标须显式接受 ~2× 手写代价，或该类指标走 (a1) 手写。

## 7. 未覆盖 / 待实测

- [未覆盖] **零优化普通写法基线 + 64B 对齐 sweep + debug 量级**：已按维护者要求拆给 NaiveBaseline / AlignControl 两代理（同一 harness 口径），本文不含。
- [需实测] **rolling min/max 内部 pulp 版**：本 demo 全程未在任何原语内部用 SIMD（纯看惯用写法 + autovec 值多少）；给 rolling max/min 加内部 pulp 后 Ichimoku 能否落进 1.5× 未实测（§6 建议）。
- [需实测] **中间流复杂度/多 pass 内存流量**：prims 逐原语物化中间列（Ichimoku 用 hi/lo/mid_ab 三 temp 复用；Squeeze 用 basis/sd/range/range_ma/hi/lo/source 七 temp），而手写 pulp 融合循环。prims/pulp 差异同时衡量了抽象边界与内存流量——这是该中间层的真实成本，非"没 SIMD"单因；每指标 pass 数/temp 列数已随代码可数，未单独 profile 内存带宽占比。
- [需实测] **warm-up 补偿求和开销**：prims 4 个 warm-up 比裸标量慢 0.69–0.87×（Neumaier + 位图）；若某些原语可安全退回非补偿求和仍过 1e-9，可缩小该开销，未扫描。
- [未覆盖] **x86 AVX2**：本机仅 aarch64，`ewise` autovec 的 AVX2 f64×4 收益 [INFERENCE] 未实测。
- [未覆盖] **人造 gap × gap_policy 端到端**：本 demo validity 全有效（raw SPY 无缺口），ewm 遇中途 invalid 的 Hold/Reset/Missing 分支已实现但未被 SPY 数据触发。

## 8. 路径清单（[OBSERVED]，可复现）

- 原语 crate：`/Users/mouriya/Ext/tmp/fp10/gate9/uta-prims/`（`src/lib.rs` 428 行）
- demo harness：`/Users/mouriya/Ext/tmp/fp10/gate9/demo/`（`src/main.rs` 计时/断言/allocator、`src/baselines.rs` fp-10 scalar/pulp/ndarray 逐字移植、`src/prims_impl.rs` 用户代码 + `asm_*` 汇编探针）
- 构建/运行：`CARGO_TARGET_DIR=/Users/mouriya/Ext/tmp/fp10/gate9/target cargo build --release`（在 demo/），运行二进制 `.../gate9/target/release/gate9-demo`
- 运行日志：`/Users/mouriya/Ext/tmp/fp10/gate9/run.log`
- 汇编：`/Users/mouriya/Ext/tmp/fp10/gate9/demo.otool.txt`（`otool -tvV`）

## 附录 A. 三份 SuperTrend 并排（关键段，[OBSERVED] 源码原文）

### A.1 原语集（`prims_impl.rs`，43 行；rolling/ewm/ewise + 状态机逃逸口）

```rust
pub fn prims_supertrend(h,hv, l,lv, c,cv, st,stv, trend, pc,pcv, tr,trv, atr,atrv, hl2,hl2v) -> Written {
    let n = c.len();
    // TR = max(H-L,|H-prevC|,|L-prevC|); TR[0]=H-L (NaN-max folds the boundary)
    shift(Col::new(c,cv), 1, ColMut::new(pc,pcv));
    ewise3(Col::new(h,hv),Col::new(l,lv),Col::new(pc,pcv), ColMut::new(tr,trv),
           |h,l,pc| (h-l).max((h-pc).abs()).max((l-pc).abs()));
    trv.copy_from_slice(hv);                                  // TR 每根都有效
    ewm(Col::new(tr,trv), 1.0/10.0, Seed::Wilder(10), ColMut::new(atr,atrv));  // ATR=RMA(TR,10)
    ewise2(Col::new(h,hv),Col::new(l,lv), ColMut::new(hl2,hl2v), |h,l| (h+l)*0.5);
    // ESCAPE: loop-carried up/down/trend 状态机
    st.fill(NAN); trend[..n].fill(0); stv.fill(0);
    let (mut pu, mut pd, mut pt) = (NAN, NAN, 1i8);
    for i in 9..n {
        let up = hl2[i]-3.0*atr[i];  let dn = hl2[i]+3.0*atr[i];
        let (u,d,t) = if i==9 {(up,dn,1)} else {
            let u = if c[i-1]>pu {up.max(pu)} else {up};
            let d = if c[i-1]<pd {dn.min(pd)} else {dn};
            let t = if pt==-1 && c[i]>pd {1} else if pt==1 && c[i]<pu {-1} else {pt};
            (u,d,t)
        };
        pu=u; pd=d; pt=t; trend[i]=t; st[i]=if t==1 {u} else {d};
        stv[i>>6] |= 1u64 << (i&63);
    }
    Written { len:n, first_valid:9 }
}
```

### A.2 ndarray 手写（`baselines.rs`，62 行含 `rma10`；TR 用 `Zip`，RMA/状态机手写标量）

```rust
fn nd_supertrend(h,l,c, mut st, mut trend_out, tr:&mut[f64], atr:&mut[f64]) -> Written {
    st.fill(NAN); trend_out.fill(0); tr[0]=h[0]-l[0];
    { let hv=ArrayView::from_shape((n-1,), &h.slice(s![1..]).to_slice().unwrap()).unwrap();
      let lv=...; let pv=...;                       // 三个手拼 offset 视图
      let mut tv=ArrayViewMut::from_shape((n-1,), &mut tr[1..]).unwrap();
      Zip::from(&mut tv).and(&hv).and(&lv).and(&pv).for_each(|t,&hh,&ll,&pp|
          *t=(hh-ll).max((hh-pp).abs()).max((ll-pp).abs())); }
    rma10(tr, atr);                                 // 手写 Wilder RMA
    let (mut up_prev,mut dn_prev,mut trend)=(0.0,0.0,1i8);
    for i in 9..n { /* 同 A.1 的状态机，手写 */ }
    Written{len:n,first_valid:9}
}
// + fn rma10(tr,atr){ reset(atr); let mut s=0.0; for i in 0..=9{s+=tr[i];} atr[9]=s/10.0;
//                     for i in 10..n{atr[i]=(atr[i-1]*9.0+tr[i])/10.0;} }
```

### A.3 pulp 手写（`baselines.rs`，114 行含 struct+WithSimd+wrapper+`supertrend_state`/`tr_at`；作者写 SIMD）

```rust
struct SuperTrendOp<'a,'b,'c,'d,'e,'f>{ high:&'a[f64], low:&'b[f64], close:&'c[f64],
    atr:&'d mut[f64], supertrend:&'e mut[f64], trend_output:&'f mut[i8] }
impl<...> WithSimd for SuperTrendOp<...> {
    fn with_simd<S:Simd>(self, simd:S) -> Written {
        // TR 显式向量化：拆 head/tail，手调 simd.sub/abs/max
        let (high_head,high_tail)=S::as_simd_f64s(&high[1..n]);
        let (low_head,low_tail)=S::as_simd_f64s(&low[1..n]);
        let (pc_head,pc_tail)=S::as_simd_f64s(&close[..n-1]);
        let (atr_head,atr_tail)=S::as_mut_simd_f64s(&mut atr[1..n]);
        for (((dst,h),l),pc) in atr_head.iter_mut().zip(...) {
            let range=simd.sub_f64s(*h,*l);
            let hg=simd.abs_f64s(simd.sub_f64s(*h,*pc));
            let lg=simd.abs_f64s(simd.sub_f64s(*l,*pc));
            *dst=simd.max_f64s(range, simd.max_f64s(hg,lg));
        }
        for (((dst,h),l),pc) in atr_tail.iter_mut().zip(...) { *dst=tr_at(*h,*l,*pc); }  // 手写 tail
        // RMA seed + Wilder 递推（标量）… 然后 supertrend_state(...)（45 行独立状态机）
    }
}
fn pulp_supertrend(...) -> Written { Arch::new().dispatch(SuperTrendOp{...}) }  // 10 行转发
```

**并排要点**：三者的状态机段几乎等价（都是手写标量 `for`，命题 8 不可消除）；差别全在**窗口/递推的表达**——prims 用 `shift`+`ewise3`+`ewm` 三行拿到 TR 与 ATR，ndarray 手拼 offset 视图 + `Zip` + 手写 `rma10`，pulp 手写 `WithSimd` 拆 head/tail + `simd.*` 原语 + 手写 tail。这是 43 vs 62 vs 114 行差距的来源，也是"作者不碰 SIMD"在 prims 下成立、在 pulp 下不成立的直接体现。
