# fp-10：数组中间层（原生 op 作者写列/数组表达式，向量化交给中间层）一手调查

状态：一手调查，闭合。**定位**：为 `hpc-derivation-subsystem.md`（可选、独立于核心的行情派生高性能计算子系统）补充"原生 op 作者不写 SIMD、只写 numpy 式列/数组表达式，向量化由中间层负责"这一裁决所需的一手证据；不做 UTA 设计，不下"库层(a) / 算子层(b)"的最终裁决。**服务对象**：子系统 §6.3 原生 op 执行、§7 作者面、§8.1 算法层。**上游延伸**：fp-09（已排除 VectorTA 与"作者直接调用的 TA 指标库"）。**证据标注**：**[OBSERVED]**（本机 aarch64 亲测/源码逐行看到）/ **[INFERENCE]**（据源码推断的 x86/跨 OS 行为或用户体验）/ **[需实测]**（本调查未证）。

---

## 1. 摘要（≤10 行）

- **最接近"numpy 式 + 作者不碰 SIMD + 满足 UTA 借入外部列/写调用方输出列 ABI"的原生 Rust 库是 `ndarray`**：它是唯一同时做到零拷贝借用外部 `&[f64]`（含自定义 stride）为视图、把结果写进调用方预填充 `&mut [f64]`（`allocs=(0,0)`、指针 identity 不变）、且词汇最接近 NumPy 的候选。**最大缺口**：`ndarray` 没有库级 SIMD、没有 `rolling/scan/ewm/where/validity` 词汇——窗口与递归都要作者手写 loop，向量化只能靠 LLVM autovec；它是"numpy 式底座"，不是"numpy 式 + 向量化中间层"。[OBSERVED]
- **一条贯穿全篇的关键事实**：本机 M4 上，release 默认的**"标量"基线本身已被 LLVM autovec 成 NEON**（elementwise/dense 窗口出现 `fadd.2d`/`fmul.2d`/`fmla.2d`）。因此在 aarch64 首要平台上，"作者写惯用 Rust 数组表达式、编译器自动向量化"对**可向量化的 elementwise/密集窗口**基本成立；显式 SIMD（pulp）的**增量**收益按指标不同：SuperTrend 递推≈1.0x、Squeeze 混合≈1.36x、Ichimoku 朴素 rolling max/min 3.3x（但换单调队列 O(n) 标量可能反超）。这正面支持维护者"不追求全向量化、递归标量即可"的判断。[OBSERVED]
- **失败边界很清楚**：`arrow-rs`、`polars`、`candle`、`burn`（列式/张量引擎）以及 `numrs`/`numrs2` 都**过不了往返闸门**——它们的 kernel 返回新数组/新 `Series`/新 `Tensor`，没有"写调用方 `&mut [f64]`"的 ABI；polars 复杂指标比标量慢 1.5–2.3x 且带分配。[OBSERVED]
- **`faer`（借 `pulp`）** 的"窗口/归约向量化 + 递归标量"分工最自然、过闸门，但它是**线性代数词汇**且要作者写 `WithSimd` SIMD kernel——与"作者不碰 SIMD"直接冲突。`pulp` 本身不是 numpy 中间层，是稳定 Rust 的 runtime-dispatch SIMD **后端**，只能作为方案(b)的底座。[OBSERVED]
- **放置裁决所需的两面证据都已就位（不裁决）**：(a) 库层——没有任何 Rust 库同时满足 ABI 与 pandas 式 rolling/ewm 词汇（ndarray 满足 ABI 但词汇裸、polars 有词汇但违 ABI）；(b) 算子层——一个词汇取自 numpy/pandas 的闭合原语集（`rolling/ewm/where/reduce/cumsum/mask` + pulp 派发）能同时满足 ABI、隐藏 SIMD、贴近认知，但 SuperTrend 类状态机必须留一个 scalar/custom 逃逸口，`shift`/`linreg` 落在最小集合之外。
- **词汇距离**按维护者标准 6 降为软指标：不作淘汰门；polars 最接近 pandas（`rolling`/`ewm`），ndarray 最接近 NumPy（broadcast/map/windows），pulp/faer/nalgebra 是 SIMD/线代词汇。

---

## 2. 范围与方法

- **只读设计来源**：`hpc-derivation-subsystem.md` 全文、`research/fp-09-...md`、`uta-core-design.md` §0.2/§5.3；本仓库其他文件未读。
- **子系统事实**（约束调查）：段是**列式**（一次快照每字段一条**对齐连续列** `&[f64]`/`&[i64]` + validity 位图，fp-09 §3.0 裁决），只读共享内存借出；输出段 = 调用方提供的**可写列 + 位图**；op 在独立进程，批量 ABI `compute(&Window, &mut OutputColumns) -> Written{len, first_valid}`；50 ms 预算；stable Rust 优先。因此中间层借入的是**连续 `&[f64]`**；自定义 stride 只作能力探针。
- **维护者裁决口径**（本篇纳入）：(1) 不追求全向量化——评每候选"窗口/归约向量化、递归标量但不变慢"的分工是否自然、EMA/RSI 递归/scan 是否顺手；(2) 词汇距离为软指标，不淘汰；(3) 只测 f64 列，不测 i64 定点，目标 ISA = **aarch64 NEON（首要）+ x86 AVX2（第二）**，AVX-512 仅备注；(4) **aarch64 NEON 本机实测是主评判**，仅 x86 SIMD/aarch64 标量的库明确降级，横向表按 NEON 实际收益读；(5) 实验模拟真实开发——真实行情数据 + PineScript 级复杂指标 + 开发难易度记录；(6) 高级用户用最强 AI 写程序，词汇非 numpy 不淘汰(b)，但记学习成本；(7) 实现/计时/汇编全部 Rust release，Python 仅作 golden 一次性核对；重产物落独立盘 `/Users/mouriya/Ext/tmp/fp10/`。
- **决定性往返闸门**（mentor 校准，强于"能编译"）：构造 offset/strided 外部输入视图 → 执行一次 elementwise/where → 写入**预填充的调用方自有输出 slice** → 断言数值正确 + 输入/输出指针 identity 不变 + 分配计数；validity 位图给实测或明写"unsupported"。判负即停止 bench，记失败边界，不造 adapter。
- **本机**：`rustc 1.95.0`，`aarch64-apple-darwin`，Apple M4，release `opt-level=3, lto=true`。计时 warm-up 后 100–300 iter 中位数、消费输出防 DCE、counting allocator 排除输入/输出预建。NEON 证据：`otool -tvV` grep `fmla|fadd.*\.2d|fmul.*\.2d|ldp q`，仅候选符号命中才归因；NEON 收益要求标量对照算法/validity/输出/编译一致仅向量路径不同。
- **子调查段落**（完整 ①–⑨、往返日志、bench、逐条来源）：`local://fp10-ndarrayfaer.md`、`local://fp10-arrowpolars.md`、`local://fp10-pulp.md`、`local://fp10-placement.md`；本文是其权威综合。

---

## 3. 候选逐库 ①–⑨（commit-pinned；"未找到"如实标注）

分级：**过闸门可作中间层底座** / **过闸门但非 numpy 中间层（SIMD 后端/线代）** / **闸门失败**。所有路径相对各库 pinned clone（commit 见 §10）。

### 3.1 `ndarray` 0.17.2（`2cf23d6a`）+ `ndarray-stats` 0.7.0（`67d0292f`）+ `ndarray-npy` 0.10.0（`e403bc75`）——**过闸门可作底座**

- ① 通用 N 维数组，`MIT OR Apache-2.0`（`ndarray/Cargo.toml:1-17,37-75`）；stats 是统计扩展（`ndarray-stats/src/lib.rs:1-39`，无指标）；npy 只做 `.npy/.npz` I/O（Cargo 未声明 license，`ndarray-npy/Cargo.toml:1-27`）。
- ② **PASS**。`ArrayView::from_shape` 借 `&[A]` + 自定义 stride（`src/impl_views/constructors.rs:18-67`），`ArrayViewMut::from_shape`（`:121-170`），裸指针 `from_shape_ptr`（`:69-118`）；`Zip::from(&mut out).and(&in)` 写调用方输出。实测 `input_ptr_identity=true output_ptr_identity=true values=[...2,4,6,8] allocs=(0,0) validity=unsupported`。
- ③ **无库级 SIMD/runtime dispatch**；`Zip` 只承诺 lockstep、同布局利于编译器向量化（`src/zip/mod.rs:117-140`）；可选 BLAS 仅矩阵乘。简单指标候选符号无 NEON 命中；复杂 Ichimoku/Squeeze 候选符号有 `fadd.2d`/`fmul.2d` 但标量参考同样被 autovec，不独占归因。[OBSERVED]
- ④ broadcast（`src/dimension/broadcast.rs:4-9,44-80`）；elementwise `mapv`/`Zip`（`src/impl_methods.rs:2990-3105`）；fold 归约（`:3130`）；`windows`/`windows_with_stride` 视图迭代器非 rolling 聚合（`:1443-1496`）；`cumsum/scan`、`where`、null/validity 全**未找到**；mask 仅布尔索引（`src/doc/ndarray_for_numpy_users/mod.rs:44-47`）。
- ⑤ **无 scan/ewm**；EMA/RSI 只能在 view 上写显式 loop。
- ⑥ 依赖中等（num-traits/num-complex/matrixmultiply/rawpointer，rayon/BLAS 可选）；stride 安全构造内部检查，裸指针构造是显式 unsafe。
- ⑦ 作者只承诺"同布局利于编译器向量化"，不承诺 SIMD/TA/validity/scan。
- ⑧ 窗口/归约用 `Zip`/fold + autovec 较自然，但无库级 vector kernel；递归手写 scalar，实测 EMA 反不慢（254 vs 339 µs）、RSI 近似——是编译器/写法结果，不是 scan 原语。
- ⑨ 词汇**中等偏近**：broadcast/map/windows/mask-index 近；`ufunc`/`where`/`cumsum`/`rolling`/`ewm`/validity 缺。

### 3.2 `faer` 0.24.4（`0539947f`，重点 `pulp`）——**过闸门但是 SIMD/线代词汇，非 numpy 中间层**

- ① 纯 Rust 线性代数库，`MIT`，MSRV 1.84（`faer/Cargo.toml:1-35`）；无 TA/时间序列 API。
- ② **PASS**。`ColRef::from_raw_parts(ptr,n,stride)`/`ColMut::from_raw_parts_mut`（`src/col/colref.rs:57-101`、`colmut.rs:59-104`），矩阵 view 带 stride（`matref.rs:187-251,299-327`）；`faer::zip!` 写调用方输出。实测 `input_ptr_identity=true output_ptr_identity=true allocs=(0,0) validity=unsupported`。
- ③ **显式 `pulp::Simd`/`SimdCtx`**（`src/utils/simd.rs:1-16,46-88,156-198`），归约用 `WithSimd`/`dispatch!`（`src/linalg/reductions/sum.rs:1-29`），矩阵乘 `arch.dispatch(...)` runtime 选架构（`src/linalg/matmul/mod.rs:209-238`）。复杂候选 `fa_squeeze`/`fa_ichimoku` 命中 `fadd.2d`/`fmul.2d`，`fa_supertrend` 仅 `ldp q`；但标量参考同被 autovec，不独占归因。[OBSERVED]
- ④ broadcast **未找到**（zip 要求可配对布局）；elementwise `zip!`/`map`；pulp 归约；`window/rolling`、`cumsum/scan`、`where`、mask、validity 全**未找到**。
- ⑤ 无 scan/rolling/ewm；EMA/RSI 手写 loop-carried scalar；`pulp::Simd` 用于独立 lane loop/reduction，不能平行化时间递归。
- ⑥ 依赖较重（`pulp=0.22.2`、rayon/linalg）；裸指针 view 的 alias/alignment/lifetime 由调用者承担。
- ⑦ 定位线性代数库，不承诺 pandas/TA/rolling/scan/validity/外部 output-column。
- ⑧ **四个过闸门者中"窗口/归约走 SIMD、递归保持 scalar"最自然**：`WithSimd`/`dispatch!`/zip/reduction 边界清晰；实测 EMA 154 vs 338 µs（0.457x）、RSI 1.007x、复杂三项不慢——证明"递归 scalar 但不变慢"成立。
- ⑨ 词汇**远**：map/zip/fold/sum 近；broadcast/where/mask/rolling/cumsum/ewm/validity 缺；AI 需把线代布局词汇转成列式时间序列循环。

### 3.3 `nalgebra` 0.35.0（`5f927f6c`）——**过闸门但矩阵词汇、SIMD 不自然**

- ① 成熟泛型线性代数，`Apache-2.0`，Rust 1.89（`Cargo.toml:1-19,25-53`）。
- ② **PASS**。`MatrixView::from_slice_with_strides`（`src/base/construction_view.rs:7-60`）、mutable（`:156-230,285-326`）；`MatrixViewMut` 索引写。实测 `allocs=(0,0) validity=unsupported`。
- ③ SIMD 靠 `simba` 泛型 trait（`src/base/componentwise.rs:258-283`、`matrix_simba.rs:1-56`），**无 pulp 式 runtime dispatcher**，主要靠泛型/autovec[INFERENCE]；候选符号无 NEON 命中。
- ④ broadcast/where/mask/rolling/cumsum/validity **未找到**；有 componentwise/`zip_map`（`src/base/ops.rs`）、fold 归约（`:460-535`）。
- ⑤ 无 scan/ewm；EMA/RSI 手写。
- ⑥ simba/matrixmultiply/num-traits 依赖 + 大量泛型单态化较重；view 构造含 unsafe。
- ⑦ 源码注释：可用 slice 时直接 slice 常快于泛型矩阵抽象（`src/base/ops.rs:460-535`）。
- ⑧ SIMD 分工**不自然**：无列式窗口 dispatch，简单 SMA/BBands 显式索引循环显著慢于同 harness 标量。
- ⑨ 词汇**远**（矩阵而非 pandas 列）。

### 3.4 `mdarray` 0.8.0（`8eddbfc3`）——**过闸门但 unsafe 面最大、词汇偏新**

- ① row-major 多维 Array/View + expression iteration，`MIT OR Apache-2.0`，依赖极轻（`Cargo.toml:1-20`）；API 偏实验性、无 LTS。
- ② **PASS（unsafe 面最大）**。`StridedMapping::new`（`src/mapping.rs:264-278`）、`DView::new_unchecked(ptr,mapping)`（`src/view.rs:267-276`）、`Slice::assign`（`src/slice.rs:28-65`）；`output.assign(expr::map(&in,...))` 写调用方 storage。实测 `allocs=(0,0) validity=unsupported`。
- ③ **无 SIMD/dispatch/NEON kernel**；lib 文档只说内层 expression 可被编译器向量化（`src/lib.rs:119-125`）；候选符号无命中。
- ④ broadcast（`src/lib.rs:21-115`）、expression `map`/`zip`（`src/expr/adapters.rs:89-123`）、fold（`src/expr/mod.rs:16-49`）；`rolling`/`cumsum/scan`/`where`/mask/validity **未找到**。
- ⑤ 无 scan/rolling/ewm；EMA/RSI 手写 scalar。
- ⑥ 依赖最轻、编译负担低；裸指针 view 依赖 `new_unchecked` 前置条件（地址/布局/寿命/alias）。
- ⑦ 仅承诺"内层循环 can be vectorized by the compiler"，无 SIMD/target 保证。
- ⑧ expression map/zip 对 elementwise 直观，但窗口只能靠 LLVM；递归无 scan 语法。
- ⑨ 词汇**中等**（表达式近、时间序列远）。

### 3.5 `pulp` 0.22.3（`5eb07fd7`）——**过闸门（连续列），但是 SIMD 后端不是中间层**

- ① "safe abstraction over SIMD"，`MIT`（`pulp/Cargo.toml:1-10`）；SIMD 原语 + runtime 架构派发，无金融/列式模型。
- ② **连续列 PASS**：`as_simd_f64s`/`as_mut_simd_f64s` 把连续 slice 拆 SIMD head + scalar tail，底层 `split_slice` 仅重解释指针不复制（`src/lib.rs:551-580,3175-3208`）；实测连续/offset `allocs=(0,0) validity=ok(caller 维护)`。**真 stride**：无 stride API，只能 scalar 手写下标（记边界，不判负——真实场景是连续列）。
- ③ `WithSimd` + `as_mut_simd_f64s` + head/tail + `splat/mul/add`（`README:27-63`）；AArch64 `Neon` `f64s=f64x2`、`vfmaq_f64`（`src/aarch64.rs:274-306,1047-1060,2717-2721`），`Arch::new` 运行期选 Neon（`:3359-3385`）；x86 V3 `f64x4`（`src/x86/v3.rs:346-370`）[INFERENCE]、V4 `f64x8`（`src/x86/v4.rs:404-422`，AVX-512 备注）。显式 kernel 命中 `fsub/fabd/fmax/fminnm/fmaxnm/fadd/fmul/fmla.2d`（`disassembly.txt:22812-28496`）。**关键**：默认"标量"基线也被 LLVM autovec 成 `.2d`（`disassembly.txt:27984-28496`）。[OBSERVED]
- ④ `splat_f64s`(broadcast 异名)、`add/sub/mul/div_f64s`(elementwise)、`reduce_sum/max/min_f64s`(归约)、`select_f64s`(where 异名)；`rolling`/`cumsum/scan`/`ewm`/validity 全**未找到**（`src/lib.rs:540-548,659-669,1297-1312,1369-1394`）。
- ⑤ **无 scan/rolling/ewm**；SuperTrend 的 RMA/state 全显式标量循环。
- ⑥ 依赖不重（bytemuck/cfg-if/libm/num-complex/paste/reborrow），公共面 safe、内部 unsafe 集中（`from_raw_parts`）；harness fresh build ~5.4s。
- ⑦ README：输出/尾部/生命周期由调用者组织，作者要写 SIMD 泛型；不承诺 rolling/scan/validity/金融指标。
- ⑧ 作为**后端**能自然表达"窗口向量化、递归标量"；但作为中间层不够高层——作者要拆 head/tail、维护 `S::f64s`、调 `simd.*`，**违反"作者不碰 SIMD"**。
- ⑨ primitive 词汇**中等偏远**、高层分析词汇**远**；本质是 SIMD DSL。

### 3.6 `arrow-rs` 60.0.0（`ef1fa157`）——**闸门失败（无写外部输出）**

- ① Apache Arrow 官方 Rust；`arrow-array/arrow-arith/arrow-select/arrow-ord/arrow-buffer`，`Apache-2.0 AND MIT`（`arrow-array/Cargo.toml:18-35`）。
- ② **FAIL**：连续 offset 借入 PASS（`Buffer::from_custom_allocation`、`ScalarBuffer::new`，`arrow-buffer/src/buffer/{immutable.rs:140-175,scalar.rs:66-107}`）；stride **未找到**；**输出失败**——`arrow_arith::arity::unary` 返回新 `PrimitiveArray`（`arrow-array/src/array/primitive_array.rs:677-702,927-990`、`arrow-arith/src/arity.rs:28-83`），`unary_mut` 只在输入 buffer 唯一时就地改**输入**，不接受调用方 `&mut [f64]`；实测 caller output 保持 NaN、kernel 指针不同、`allocs=(2,0)`。validity **支持**（`BooleanBuffer`/`NullBuffer`，`arrow-buffer/src/buffer/null.rs:22-43`）——六候选中最接近 UTA 位图模型。
- ③ arithmetic 源码原话"can leverage SIMD"但"**Currently no runtime detection is provided**"（`arrow-arith/src/arithmetic.rs:18-23`），需 build 期 `target-feature`；非 pulp 式 runtime dispatch。复杂实验 Arrow wrapper 无 f64 NEON 算术（只调 scalar）。[OBSERVED]/x86[INFERENCE]
- ④ broadcast(scalar+array)/`unary`/`binary`/`filter`/aggregate/validity 有；`where`(名 if-else)、`rolling`、`ewm`、通用 scan **未找到**（有 `cumulative` 但非本 crate 主线）。
- ⑤ 无命名 scan/rolling/EMA/RSI；作者手写顺序 loop 且仍 materialize 新数组。
- ⑥ 探针 lock 64 包；LTO 首编 ~12.8s；外部 allocation/FFI 是 unsafe 面。
- ⑦ "no runtime detection"、`ScalarBuffer` 只表连续 byte slice——两条最关键限制。
- ⑧ elementwise/归约自然，窗口不在 kernel 层→分工不自然；无 scan/rolling/外部输出 API。
- ⑨ 词汇**中等偏远**（Arrow/PrimitiveArray/NullBuffer）；validity 语义清晰。

### 3.7 `polars` 0.55.2（`d7488c71`）——**闸门失败（新 Series），但 pandas 词汇最近**

- ① Rust DataFrame/query engine；`polars-core`/`polars-arrow`/`polars-compute`（"Private compute kernels"，`polars-compute/Cargo.toml:1-10`）/`polars-ops`，`MIT AND Apache-2.0`。
- ② **FAIL**：连续 offset 借入 PASS（`SharedStorage::from_slice_unchecked`，`polars-buffer/src/{buffer.rs:10-18,99-137,storage.rs:144-170}`）；stride **未找到**；**输出失败**——`try_as_mut_slice` 仅 Vec backing（`storage.rs:347-359`），外部 backing 走新 Vec + 新 `PrimitiveArray`（`polars-compute/src/arity.rs:44-72`）；实测 caller output NaN、`allocs=(2,0)`。validity **支持**（`Option<Bitmap>`）。
- ③ `simd` feature 依赖 nightly `portable_simd`（`polars-arrow/src/lib.rs:11-14`、`polars-compute/src/float_sum.rs`），stable 默认不开；rolling kernel 是窗口状态标量循环；某 if-then-else kernel 明注"Auto-generated SIMD was slower on ARM"退 scalar（placement §S32）。复杂实验 polars_squeeze 有 `fadd.2d`（release 对 dense loop 生成），SuperTrend/Ichimoku 无。[OBSERVED]
- ④ **`rolling_mean/min/max/var`** 有（`polars-compute/src/rolling/no_nulls/{mean.rs,moment.rs}`）；**`ewm_mean`/`EwmMeanState`** 有（`polars-compute/src/ewm/mean.rs:21-89`、`polars-ops/src/series/ops/ewm.rs`）；broadcast/mask/validity 有；`cumsum/generic scan`、`where`(有 when-then) 视路径。
- ⑤ **有顺序 scan（`EwmMeanState`）**——四列式引擎中唯一；但 seed/adjust/ignore_na 是通用 EWM，不等于 oracle EMA(14) idx13 seed，也不表达 Wilder RSI/SuperTrend；复杂实验 polars_supertrend 退回 scalar。
- ⑥ 探针 lock 103 包；LTO 首编 ~24.7s；rolling 用 `unsafe get_unchecked`。复杂 Ichimoku `allocs=(47,17)`、Squeeze `(38,11)`——新数组/chunk 开销显著。
- ⑦ `polars-compute` 自称 private kernels；`simd` 需 nightly；`Buffer` clone 活着时 abort——是内部执行引擎不是外部 output ABI。
- ⑧ **窗口/归约分工最自然**（rolling_* 现成），EWM 表达度中等；但 materialization 违 ABI，复杂 Squeeze 慢 2.305x；"不变慢"未被实测支持。
- ⑨ 词汇**四者最近**（`rolling`/`ewm`/broadcast/Bitmap），但仍 Polars/Arrow 术语；"最近"≠过 ABI 闸门。

### 3.8 `candle-core` 0.11.0（`31f35b14`）——**闸门失败（输入即复制）**

- ① minimalist ML/tensor framework，`MIT OR Apache-2.0`（`candle-core/Cargo.toml:1-60`）。
- ② **FAIL**：`Tensor::from_slice`→`storage_from_slice`→拥有型 `CpuStorage`（`tensor.rs:551-572`、`cpu_backend/mod.rs:3069-3082`），实测 `copied=true`；stride **未找到**；`affine` 返回新 Tensor（`tensor.rs:773-791`），无调用方 sink；validity **unsupported**。
- ③ CPU SIMD 仅 `target_feature` 编译（`cpu/mod.rs:65-84`），NEON `CurrentCpu` 是 `float32x4_t`、`fmla` 在 fp16（`cpu/neon.rs:8-40,152-153`），**无 f64 指标 kernel**。[OBSERVED]/x86[INFERENCE]
- ④ broadcast(`Layout::broadcast_as`)、elementwise、`where_cond`、reduce 有；`rolling`/`cumsum/scan`/`ewm`/validity **未找到**。
- ⑤ 无 scan/rolling/ewm；须转 host slice 手写再重建 Tensor（已违零拷贝）。
- ⑥ 探针 lock 143 包；LTO 首编 ~28s；CPU backend 大量 unsafe，但外部构造仍复制。
- ⑦ 定位 ML/GPU；`from_slice` 即创建 Tensor 而非 borrowed view。
- ⑧ 通用 dense tensor SIMD 自然，窗口/递归无 primitives，分工不自然；gate 负。
- ⑨ tensor 词汇近 numpy，pandas 时间序列词汇远。

### 3.9 `burn` 0.21.0 ndarray backend（`546cacb5`，+ `macerator` v0.3.4）——**闸门失败（COW + 无输出 sink）**

- ① tensor + DL framework，可切换 backend，`MIT OR Apache-2.0`；`burn-ndarray` 默认 `std,simd,multi-threads`（`burn-ndarray/Cargo.toml:17-48`）。
- ② **FAIL**：`NdArrayStorage::from_borrowed(Bytes,shape)` 能零拷贝借**连续** bytes（实测 `borrowed=true`，`storage.rs:66-105`），但要求 contiguous row-major、stride **未找到**；`FloatTensorOps::float_add_scalar`/`float_into_data` 返回新 Tensor/TensorData，**无调用方 `&mut [f64]`**，borrowed 存储 COW 首次修改即复制（`storage.rs:113-176,217-231`）；实测 caller output NaN、`allocs=(6,0)`；validity **unsupported**。
- ③ `macerator` 对 aarch64 做 runtime dispatch，f64 `NeonFma` 与 x86 V2/V3/V4（`macerator-0.3.4/src/backend/{arch.rs:19-83,aarch64.rs:157-190}`）；但 borrowed COW 会复制。[OBSERVED aarch64 源码]/x86[INFERENCE]
- ④ broadcast/`mask_where`/`mask_fill`/reduce 有；固定 `cumsum/cumprod/cummin/cummax`（`burn-backend/.../tensor.rs:897-937`）但非任意 scan；`rolling`/`ewm`/validity **未找到**。
- ⑤ cumsum 不能替代 seed/状态 scan；EMA/RSI 须 host loop 或大量 tensor op + COW 复制。
- ⑥ 探针 lock 274 包（最重）；LTO 首编 ~22.7s；含 burn-autodiff/cubecl/rayon/macerator，对纯 f64 列式指标显著偏重。
- ⑦ storage 自述"Copy-on-Write storage for zero-copy tensor **loading**"、borrowed→owned 会 copy——设计目标是模型/mmap 输入加载，非可写外部列输出。
- ⑧ elementwise/broadcast/mask SIMD 最自然（f64 NEON runtime dispatch），但 window/recurrence 不自然，cumsum≠scan；gate 负、COW/输出 materialization 先构成成本。
- ⑨ broadcast/mask/cumsum 近 numpy，pandas 时间序列远。

### 3.10 `numrs` 0.2.0（`bdb812f3`）/ `numrs2` 0.4.1（`ee569fa6`）/ `ndarray-simd`——**闸门失败 / 不存在**

- `numrs`：早期 owning `Vec` 库，`Vector{pub data:Vec<T>}`（`src/vector.rs:24-30`），无 view/stride/lifetime，算术新建结果；gate **FAIL**（`input_materialized=true output_caller_slice=false`）。①–⑨ 除基本算术外全**未找到**，词汇距 NumPy**极远**。
- `numrs2`：宣传 NumPy alternative（README 约 262k 行、Arc COW、SIMD/NEON/AVX、Arrow zero-copy）；但 `Array` 是 `Arc<scirs2_core::ndarray::Array>`（`src/array/core.rs:37-64`），`from_vec_shape` 消费 owned Vec，公开无从外部 `&[f64]` 直接建 view 的入口，`map_to` 输出是 `&mut Array<T>` 非调用方 slice（`operations_optimized.rs:106-125`），`where_cond`/非 contiguous `map` 会 `to_vec` materialize（`array_ops/conditional.rs:214-256`、`array/operations.rs:181-200`）；gate **FAIL**（`allocs=(2,0)`）。④ broadcast/`where_cond`/mask 有；`cumsum`/rolling/ewm/validity **未找到**。**词汇近但存储/输出契约远**，依赖最重，不能因表面词汇过标准。
- `ndarray-simd`：crates.io `q=ndarray-simd` 返回 `[]`，**未找到**该包（不把 `std::simd`/ndarray autovec 误当独立库）。

---

## 4. 本机实测（Rust release，Apple M4 aarch64；标量为 LLVM-autovec 后基线）

### 4.1 数据与 oracle

- 真实 SPY 日线 OHLCV，8465 行（1993-01-29 → 2026-09-16），622 KB，SHA-256 `eeee85933f3b26b783ee7dfae2aaa0d59db6d0205b6ec54d18fbbcc828da96f4`；Yahoo chart API（Stooq 被 JS 挑战挡）；`/Users/mouriya/Ext/tmp/fp10/data/spy_daily.csv`。用 raw High/Low/Close。
- 复杂指标严格对齐 Pine：**SuperTrend(10,3)**（ATR 递推 + 方向状态机）、**Ichimoku(9,26,52)**（多窗口 rolling max/min + 位移）、**Squeeze Momentum[LazyBear]**（BB/KC 挤压 + linreg20 动量）；契约见 `local://fp10-experiment-spec.md`。
- **独立 golden（父 agent Python 一次性核对，所有 Rust 候选逐点 1e-9 匹配）**，下标 `[100,1000,5000,8464]`：SuperTrend `first_valid=9` = `45.315821/73.348225/137.373727/775.277032`，trend `-1/1/1/-1`；Ichimoku tenkan `44.703125/75.070312/140.959999/761.815002`、kijun `44.828125/74.632812/139.209999/764.484985`、senkouB(未位移) `44.546875/73.742188/140.930001/754.234985`；Squeeze `val first_valid=38`（分量 19，linreg 套派生序列再 +19）= `-0.632299/1.777589/3.481440/-9.281345`。

### 4.2 复杂指标主表（µs/call，SPY 8465，各候选 vs 各自 LLVM-autovec 标量基线）

> 跨候选**绝对 µs 不可直接比**（各 agent 独立 harness/写法，标量基线本身不同）；**同候选内 candidate/scalar 比值**才可比。allocs 为计时区操作期，均 `(0,0)` 除 polars。

| 候选（过闸门者 + pulp 后端） | SuperTrend µs (比值) | Ichimoku µs (比值) | Squeeze µs (比值) | 外部输出零分配 | NEON 命中 |
|---|---|---|---|---|---|
| **ndarray**（autovec） | 65.084 (1.302x) | 227.750 (0.984x) | 111.750 (0.987x) | 是 (0,0) | Ichimoku/Squeeze `fadd/fmul.2d`；ST 仅 `ldp q` |
| **faer**（autovec，pulp 可用） | 63.959 (0.978x) | 227.916 (1.000x) | 110.709 (1.001x) | 是 (0,0) | 同上 |
| **pulp**（显式手写 NEON kernel） | 65.708 (1.013x) | **92.709 (3.308x)** | 100.916 (1.364x) | 是 (0,0) | ST TR `fsub/fabd/fmax.2d`；Ichimoku `fminnm/fmaxnm.2d`；Squeeze `fmla.2d` |
| polars（新 Series，**违 ABI**） | 119.167 (1.004x) | 443.833 (1.571x)↓ | 371.208 (2.305x)↓ | 否，allocs Ichi(47,17)/Sqz(38,11) | Squeeze `fadd.2d`；ST/Ichi 无 |
| arrow-rs（薄 wrapper+scalar，**违 ABI**） | 123.208 (1.038x) | 286.875 (1.015x) | 162.875 (1.011x) | 否 | wrapper 无算术 NEON（调 scalar） |

要点：
- **pulp 的 Ichimoku 3.308x 是唯一的大幅显式-SIMD 收益**，因 LLVM 不 autovec 朴素 rolling max/min；但**该标量是 naive O(n·w)，换单调队列 O(n) 可能反超 SIMD**（pulp agent 明注）。ndarray/faer 的 Ichimoku≈1.0x 正因它们也用了能被 autovec 的写法。
- SuperTrend（递推状态机）在所有候选 ≈1.0x——**递归标量不变慢**，正是维护者标准 1。
- polars 复杂路径**慢 1.5–2.3x 且分配**（新数组/chunk），是列式引擎作算法层的真实代价。

### 4.3 LLVM autovec 对照（pulp harness，`-C llvm-args=-vectorize-loops=false -vectorize-slp=false`）

| 指标 | 默认 scalar | 禁 autovec scalar (比) | 默认 pulp | 禁 autovec pulp (比) |
|---|---:|---:|---:|---:|
| SuperTrend | 66.458 | 66.625 (1.003x) | 65.708 | 67.000 (1.020x) |
| Ichimoku | 307.875 | 307.500 (0.999x) | 92.709 | 144.417 (1.558x) |
| Squeeze | 137.709 | 157.125 (1.141x) | 100.916 | 118.625 (1.175x) |

- **默认"标量"基线已含 NEON**：Squeeze 简单 range/linreg 有 ~14% autovec 贡献；Ichimoku 朴素 rolling 不被 autovec（0.999x）。`-C target-feature=-neon` 无法产生可运行 AArch64 二进制（NEON 是架构必备，exit 101）——故没有"真无 NEON"对照。这条决定性支持：**aarch64 上 elementwise/密集窗口靠 autovec 已拿到 NEON，作者写惯用数组表达式即可，不必写 SIMD**；显式 SIMD 只在 autovec 覆盖不到（朴素 rolling max/min）时有大收益，而那又可能被更好的标量算法抵消。

### 4.4 四简单指标（warm-up 对照，100k synthetic f64，非主表）

各候选 SMA20/EMA14/RSI14/BB20 均过 oracle（1e-9、warm-up NaN、first_valid）。100k synthetic f64，candidate/scalar（同 harness 各自标量基线），allocs 均 `(0,0)`、写外部输出零额外分配：

| 候选 | SMA(20) | EMA(14) | RSI(14) | BBands(20) |
|---|---|---|---|---|
| ndarray | 144.792µs (1.232x) | 254.375µs (0.750x) | 423.542µs (0.995x) | 762.125µs (1.485x) |
| faer | 121.000µs (1.018x) | 154.500µs (**0.457x**) | 422.541µs (1.007x) | 708.709µs (1.369x) |
| nalgebra | 219.500µs (1.815x) | 336.417µs (0.958x) | 528.125µs (1.223x) | 1021.500µs (1.947x) |
| mdarray | 131.875µs (1.116x) | 342.083µs (0.984x) | 421.583µs (0.981x) | 1335.583µs (1.140x) |
| pulp(显式) | 504.667µs (4.169x) | 158.542µs (0.963x) | 485.458µs (1.094x) | 1233.500µs (3.633x) |

**结论**：简单指标区分度低、易受写法影响（nalgebra 显式索引循环、pulp 朴素分窗写法反而慢），不作主评判；faer EMA 0.457x 再证"递归 scalar 不变慢"。复杂指标 + 真实数据才暴露分工与开发难度，故简单四指标仅作 warm-up。fp-09 的 VectorTA 标量基线（RSI 92µs/ATR 154µs/BBands 25.5µs）是不同 harness/指标口径，仅作量级参照，不与本表逐格比。

### 4.5 开发难易度（维护者标准 5；pulp harness 实测 LOC + 摩擦）

- **pulp**：SuperTrend ~185 行、Ichimoku ~189 行、Squeeze ~322 行；主要摩擦是**类型/借用/布局**——每个输入/输出列以不同 lifetime 的 `&[f64]`/`&mut [f64]` 进 `WithSimd`（多输出 struct 迅速变宽），`S::f64s` 不透明不能按 lane 索引、每切片手写 head/tail，位移列须先在输出上按目标下标写、validity 手动维护。**作者被迫写 SIMD**。
- **ndarray/faer/nalgebra/mdarray**：递归/状态机/位移全部**被迫退回手写标量循环**（无 scan/rolling/ewm/shift 原语）；窗口靠 `Zip`/fold + autovec，作者不写 SIMD 但也拿不到"库保证的向量化"。
- **polars**：窗口部分可调 `rolling_*`/`ewm_mean`（pandas 词汇顺手），但 SuperTrend RMA/状态机、Ichimoku shift、Squeeze linreg/组合仍手写；且输出是新 Series。难度：Arrow 5/5、Polars 4/5、scalar 3/5（placement/arrowpolars agent 记）。

---

## 5. 放置方案先例（(a) 库层 / (b) 算子层；不裁决）

六个跨生态先例均**源码/文档级**证据（`[OBSERVED source]`），未跑 Rust viability/bench（标准 7：非 Rust 不作候选实现证据）。用三个复杂指标测"词汇能否表达"。

| 先例 | 向量化如何隐藏（用户不写 SIMD） | rolling | scan/递归 | SuperTrend | Ichimoku | Squeeze(linreg) | ARM/NEON 状态 |
|---|---|---|---|---|---|---|---|
| **NumPy** | ufunc universal-SIMD intrinsics + dtype dispatch（`loops_arithm_fp.dispatch.c.src:45-123`）；但源码 TODO "Vectorize reductions" | `sliding_window_view`(view，文档警告常非最优) | `ufunc.accumulate`(简单二元 scan)，**无 ewm** | 强制 scalar/custom | 可组合(view+max/min+切片位移)但可能昂贵 | 部分：BB/KC 可组合，`linreg` **未找到**须手写 | 源码有 `NPY_SIMD_F64`；M4 NEON 命中未验证 |
| **pandas** | rolling/ewm 绑 Cython kernel(增量窗口/Kahan/Welford)，可选 Numba；无 SIMD intrinsic | 一等 `.rolling().sum/mean/std/min/max` | **一等 `.ewm(adjust=False)`**（`y_t=(1-α)y_{t-1}+αx_t`），`expanding` 累计 | 部分：ATR 可 ewm，`up/dn/trend` 需 apply/Numba | **可组合**：`rolling().max/min`+`shift(26)`+算术 | 部分：`linreg` 须 rolling apply/Numba | 无 SIMD；纯 Cython/标量 |
| **Arrow C++ Compute** | Kernel `SimdLevel{NONE..AVX512,NEON}` + registry；runtime 只 dispatch AVX512/AVX2 后回退 NONE | **未找到** rolling | `cumulative_sum/prod/min/max/mean`，无 ewm/泛化 scan | 强制 custom VectorKernel | 强制 custom(无 rolling/shift) | 强制 custom(无 rolling/linreg) | 枚举有 NEON 但**未找到 runtime NEON 分支** |
| **Polars expr engine** | lazy Expr 编译到 kernel；`simd` feature 是 nightly portable_simd；某 kernel 明注 ARM autovec 更慢退 scalar | 一等 `rolling_*` + `over` | **一等 `ewm`**、`cum_sum`、`cumulative_eval`(闭包回退) | 部分：ewm+when+shift，状态机需 cumulative_eval/plugin | **可组合**(rolling+shift+算术) | 部分：`linreg` 未在 rolling 枚举，须 map/plugin | 有一等 `shift`；stable 无 SIMD，ARM 有 scalar fallback 注记 |
| **Bottleneck** | 生成的 C `move_*` loop(GIL-free 增量窗口)，无 SIMD | `move_mean/sum/std/var/min/max/median/rank` | **无 ewm/通用 scan**(fallback 是 Python loop) | 强制 Python/scalar | 部分：`move_max/min` 可，shift/算术回 NumPy | 部分：`linreg` 无，须 NumPy 自定义 | stride-aware C，但**无显式 SIMD** |
| **numexpr** | 表达式→bytecode，VM 分块(默认 4096)融合 elementwise/where + 少量整轴 reduce | **未找到** | **无 scan/ewm**(只 sum/prod/min/max reduce) | 强制外部 loop | 不能单独表达(无 rolling/shift) | 部分：预算好后做 arithmetic/where | Intel VML 仅 Intel；无 NEON kernel |

### 5.1 六先例逐个（源码/文档级 [OBSERVED source]，S 编号见 `local://fp10-placement.md`）

- **NumPy**（BSD-3，`c8899977`）：ufunc 隐藏 SIMD（`loops_arithm_fp.dispatch.c.src:45-123` 用 `npyv_*` + `NPY_SIMD_F64`；但同文件 TODO "Vectorize reductions"，reduce 分支逐元素累加 `S8:L12-19`）。`sliding_window_view` 返回 view（`_stride_tricks_impl.py:180-220,407-444`），文档警告通用 rolling "often not optimal"；`np.where`/broadcast/`ufunc.reduce`/`accumulate` 一等，`out=` 是已有 ndarray；**无 `ewm`、无独立 validity bitmap（用 NaN/bool mask）**。
- **pandas**（BSD-3，`baac16ef`）：`.rolling` 绑 Cython kernel（`aggregations.pyx:139-191,269-323,420-485` 增量窗口 + Kahan/Welford），`.ewm(adjust=False)` 明写 `y_t=(1-α)y_{t-1}+αx_t`（`ewm.py:168-187`，注释"iterative with each point"）。词汇最贴 AI 认知；但 `_prep_values` 会 dtype coerce/`np.where` 改 inf（`rolling.py:342-364`），kernel `np.empty` 新建输出（`aggregations.pyx`），**无调用方 `out=`、无 bitmap、windowing 只支持 numeric 且总返回 float64**（`window.rst:64-67`）。
- **Arrow C++ Compute**（Apache-2.0，`0bd8def0`）：`Function`/`Kernel` registry 把 SIMD 作 kernel variant（`kernel.h:435-550` 有 `SimdLevel::NEON` 枚举），但 `function.cc:122-181` runtime 只 dispatch AVX512/AVX2 后回退 NONE，**未找到 NEON runtime 分支**。scalar executor 可预分配但都是 Arrow buffer（`exec.cc:711-748`），公共 `Execute` 返回 `Datum`；validity/NullHandling 一等（最贴 UTA 位图）；有 `cumulative_sum` 顺序 kernel（`vector_cumulative_ops.cc:67-175`）；**无 rolling/ewm、无写调用方 slice**。
- **Polars expr engine**（MIT，docs `473ca8ef`）：lazy `Expr` 编译到 kernel，`rolling_*`/`over`/`shift`/`cum_sum`/`cumulative_eval`/`when-then-otherwise` 一等（`dsl/mod.rs:637-685`、`function_expr/rolling.rs:6-57`），`ewm` feature-gated（`function_expr/mod.rs:370-394`、`ewm/options.rs`）。内部 rolling kernel 借 `&[T]` + `Bitmap`（`rolling/no_nulls/mod.rs:26-80`、`window.rs:88-197`）——**方案(b) rolling/ewm 的直接设计参考**；但 `simd` 是 nightly portable_simd（`polars-compute/src/lib.rs:1-3`），if-then-else kernel 明注"Auto-generated SIMD was slower on ARM"退 scalar（`if_then_else/simd.rs:69-123`）。
- **Bottleneck**（BSD，`15124a00`）：C `move_*` 增量窗口（`move_template.c:74-121,156-214`），iterator 读 NumPy stride（`iterators.h:8-68`）；**"accelerated" 指 C fast path 非 SIMD**（`README.rst:86-90` 只四种 dtype 加速），`PyArray_EMPTY` 自建输出，通用 fallback 是 Python loop（`slow/move.py:116-151`）；**无 ewm/通用 scan/bitmap**。
- **numexpr**（MIT，`7031844c`）：字符串表达式→bytecode，VM 分块（默认 4096）融合 elementwise/where + 少量整轴 reduce（`interp_body.cpp:19-180`、`necompiler.py:418-428`），`out=` 已有 ndarray、优化 strided/unaligned；**无 rolling/scan/ewm**（`isReduction` 只认 sum/prod/min/max，`opcodes.hpp:176-206`），Intel VML 仅 Intel（`user_guide.rst:53-71`），worker 有 output buffering 分配。适合"预算好列后做 elementwise/where 融合"。

**共同结论 [OBSERVED source]**：窗口/归约类词汇在 pandas/Polars/Bottleneck 是一等；递归平滑仅 pandas/Polars 有一等 `ewm`；**SuperTrend 类 loop-carried 状态机在所有先例都强制 scalar/custom（apply/Numba/VectorKernel/Python loop）**；Ichimoku 的 shift 只有 pandas/Polars 一等，linreg 无一处一等。SIMD 隐藏方式各异且 ARM 状态普遍弱（NumPy 有 intrinsics 但 M4 未验证、Arrow 无 NEON runtime、Polars ARM 退 scalar、Bottleneck/numexpr/pandas 无 SIMD）。

### 5.2 方案 (b)：词汇取自 numpy/pandas 的闭合原语集

闭合最小集：`broadcast`+`ufunc`/`where`/`rolling(sum/mean/std/var/min/max)`/`cumsum`/`reduce`/`ewm`/`mask`。逐指标覆盖（分档：可组合 / 部分-回退 / 强制 scalar-custom）：

| 指标 | 仅闭合原语的表达 | 覆盖档 | 缺口 |
|---|---|---|---|
| SMA(20) | `rolling(20).mean` | 可组合 | first_valid=19、warm-up/validity 须显式契约 |
| EMA(14) | `ewm(α=2/15,adjust=False,min_periods=14)` | 可组合 | seed 须固定为 oracle idx13 mean，不用默认黑盒 |
| RSI(14) | `diff`+`where(>0)/(<0)`+两条 `ewm(α=1/14)`+除法 | 部分 | 需 `diff/shift` 或对齐视图；零分母分支 where；Wilder seed 须显式 |
| Bollinger(20) | `rolling.mean`+`rolling.std(ddof=0)`±2σ | 可组合 | rolling std 须支持 ddof=0、三列输出 |
| SuperTrend(10,3) | ATR=`ewm`；TR/bands/where=elementwise | **强制 scalar/custom** | `up/dn/trend` 三 loop-carried 状态无法由 cumsum/固定 rolling/ewm 闭合表达，须 scalar scan 逃逸口 |
| Ichimoku(9,26,52) | `rolling.min/max`+算术 | 部分 | `shift(±26)` 不在集合，须 ABI offset 视图或额外 shift 原语；五列输出/未位移 senkouB first_valid=51/77 |
| Squeeze Momentum | BB/KC `rolling.mean/std/max/min`+where | 部分 | `linreg` 不在集合，须窗口相对索引 + `reduce(sum)` 代数展开或 custom；`val` first_valid=38 |

递归/窗口分工（原语 → kernel 分工，[INFERENCE]）：`ufunc/where` = 连续 f64 的 SIMD map/select（NEON f64=2 lane、AVX2 f64=4 lane 由中间层选）；`rolling/reduce` = 窗口增量或分块归约，适合 SMA/BBands/Ichimoku/Squeeze 窗口分量；`cumsum` 仅单一可结合累计，不能替 SuperTrend 状态；`ewm` = 单 weighted state 顺序更新（EMA/RSI 平滑顺手，不承诺沿时间 SIMD）；`mask` = validity bitmap 与数值选择同步。**这组内部可把 elementwise/窗口送稳定 Rust SIMD backend（pulp）、把 ewm/递归留顺序 scalar——即"窗口向量化、递归标量"的分工，且作者只写原语不碰 SIMD。**

**方案 (b) 词汇距离（软标准）**：`broadcast/ufunc/where/rolling/cumsum/reduce/mask` 对 NumPy 零到低距离，`ewm` 对 pandas 零距离；内部 Rust trait/enum 名可 idiomatic，真正影响 AI 作者的是公开表达式是否保持 numpy/pandas 认知。按标准 6 非 numpy 名称不淘汰，但每个 `cumulative_eval`/custom registry 概念增加学习/fallback 面。

**方案 (b) 实现/维护成本**（非 LOC 估算，从六先例维护面归纳）：
- 表达节点 shape/stride/aliasing；rolling 的 warm-up/min_periods/ddof/NaN/bitmap 与增量数值稳定性；ewm 的 α/seed/adjust/ignore_na 顺序 kernel；validity 位图在每个原语的传播（Arrow/Polars 证明 bitmap 可作 kernel contract，但 NumPy/pandas/Bottleneck/numexpr 主用 NaN，迁移不能混用）；多表达式临时列 vs caller-owned 输出与 50ms 预算的融合边界；SuperTrend/shift/linreg 三个逃逸口。

### 5.3 方案 (a)：直接采用/嵌入一个成熟数组库（对称评估，不裁决）

方案 (a) 让作者直接用某个库的 API 写指标，库隐藏向量化。本调查把它落到**过 UTA ABI 闸门**的具体候选（§3），不是抽象"用某库"：

- **(a1) bless `ndarray`（唯一"numpy 式 + 过 ABI"路径）**：typed SDK 把借出列包成 `ArrayView`/`ArrayViewMut`，作者写 `Zip`/`mapv`/`windows` + 手写递归/位移 loop。**优点**：词汇最贴 NumPy、零拷贝借入 + 写调用方输出实测 `allocs=(0,0)`、依赖中等、复杂三指标实测不慢（Ichimoku 0.984x、Squeeze 0.987x，靠 LLVM autovec 拿 NEON）。**代价**：无库级 rolling/scan/ewm/where/validity——SMA/EMA/RSI/Bollinger/Ichimoku/Squeeze 的窗口与递归**全由作者手写**（fp-09 §8.1"ndarray 无指标，全部自写"在本篇被实测确认）；向量化只得 autovec，朴素 rolling max/min 拿不到（须作者选好算法）。作者仍需理解 first_valid/warm-up/validity 语义。
- **(a2) bless `faer`（分工最自然但线代词汇 + 作者写 SIMD）**：过 ABI、`pulp` runtime NEON、窗口/归约-SIMD 与递归-scalar 分工最清晰（EMA 0.457x）；**代价**：线代词汇（`ColRef`/`zip!`）离行情认知远，且要作者写 `WithSimd`——与"作者不碰 SIMD"冲突，更接近方案(b) 的后端而非(a) 的作者面。
- **(a3) 采用 pandas 式引擎（polars）——本调查判违 ABI**：polars 词汇最贴 pandas（`rolling_*`/`ewm`）、作者不碰 SIMD，但 kernel 返回新 `Series`（`try_as_mut_slice` 仅 Vec backing），**不能写调用方输出列**、复杂路径慢 1.5–2.3x + 分配、stable 无 SIMD。作 UTA 数据面(a) 不成立；其内部 `rolling`/`ewm` kernel（借 `&[T]`+`Bitmap`）只能作方案(b) 的设计参考。
- **(a4) arrow-rs / candle / burn——均违 ABI**：返回新 Array/Tensor/TensorData，无写调用方列；arrow validity 模型最贴 UTA、burn/candle 有 f64 NEON runtime，但输出 sink 缺失是硬阻断，不作(a) 候选。

**(a) vs (b) 的核心张力**：(a1) 是唯一现成、过 ABI、numpy 词汇的路径，但把窗口/递归/validity 的**全部实现与语义**留给作者；(b) 把这些收进子系统原语、作者只组合，但要子系统自建 rolling/ewm/validity/dispatch 并为 SuperTrend 类留 scalar 逃逸口。两者都能让作者"不碰 SIMD"（(a1) 靠 autovec + 手写标量、(b) 靠原语层 pulp 后端）；差别在**"谁承担窗口/递归/validity 的实现与数值语义"**——作者(a1) 还是子系统(b)。本篇不裁决。

---

## 6. 横向表（按 aarch64 NEON 实际收益 + ABI 契合读；不裁决）

分级：**底座可用（过 ABI 闸门）** / **SIMD 后端（过闸门但要作者写 SIMD）** / **违 ABI（新数组，闸门失败）** / **闸门失败（其它）**。

| 库 | ①维护/许可 | ②借外部列 | ②写调用方输出 | ③aarch64 NEON（主评判） | ④rolling | ⑤scan/ewm | ⑦validity | ⑧窗口SIMD/递归分工 | ⑨numpy/pandas 词汇(软) | 分级 |
|---|---|---|---|---|---|---|---|---|---|---|
| **ndarray** | 活跃/MIT-Apache | ✅含 stride | ✅`Zip`→`&mut` (0,0) | autovec 命中 elementwise/密集窗口；无库级 SIMD | ✗(仅 windows 视图) | ✗ | ✗ | 窗口 autovec、递归手写 scalar 不变慢 | 中等偏近(broadcast/map/windows) | **底座可用** |
| **faer** | 活跃/MIT | ✅含 stride | ✅`zip!`→`&mut` (0,0) | pulp runtime NEON `f64x2` + autovec | ✗ | ✗ | ✗ | **分工最自然**(WithSimd/dispatch) | 远(线代) | **底座可用/SIMD 后端** |
| **pulp** | 活跃/MIT | ✅连续(无 stride API) | ✅`&mut` (0,0) | **显式 NEON**(唯一 Ichimoku 3.3x) | ✗ | ✗ | ✗ | 后端级最自然；**作者须写 SIMD** | 中等偏远(SIMD DSL) | **SIMD 后端(违"不碰 SIMD")** |
| **mdarray** | 新/MIT-Apache | ✅含 stride(unsafe 最大) | ✅`assign` (0,0) | 仅 autovec | ✗ | ✗ | ✗ | 窗口靠 LLVM、递归手写 | 中等 | **底座可用(实验性)** |
| **nalgebra** | 活跃/Apache | ✅含 stride | ✅索引写 (0,0) | simba 泛型/autovec，无 dispatch | ✗ | ✗ | ✗ | 不自然(索引循环慢) | 远(矩阵) | **底座可用(不适合)** |
| **polars** | 活跃/MIT-Apache | ✅连续 | ❌新 Series (allocs) | stable 无 SIMD；ARM autovec 有注记退 scalar | ✅`rolling_*` | ✅`ewm`/顺序 scan | ✅Bitmap | 窗口自然但 materialize 违 ABI、慢 1.5-2.3x | **最近(rolling/ewm)** | **违 ABI** |
| **arrow-rs** | 活跃/Apache-MIT | ✅连续 | ❌新 Array | "no runtime detection"、build 期 target-feature | ✗ | ✗(有 cumulative) | ✅BooleanBuffer(最贴 UTA) | elementwise 自然、窗口不在 kernel 层 | 中等偏远 | **违 ABI** |
| **candle** | 活跃/MIT-Apache | ❌输入即复制 | ❌新 Tensor | f64 无 NEON kernel(仅 f32) | ✗ | ✗ | ✗ | dense 自然、指标不自然 | tensor 近/pandas 远 | **闸门失败** |
| **burn(ndarray)** | 活跃/MIT-Apache | ⚠仅连续 COW | ❌新 TensorData | macerator f64 NEON runtime，但 COW 复制 | ✗ | ⚠固定 cumsum | ✗ | elementwise 自然、无 window/scan | broadcast/mask 近/pandas 远 | **闸门失败** |
| **numrs2** | 新/Apache(262k 行) | ❌owned Arc | ❌`&mut Array` | 宣传 NEON 但 gate 前 materialize | ✗ | ✗ | ✗ | where_cond/map 有 owned 复制 | 词汇近/契约远 | **闸门失败** |
| numrs | 停滞/MIT | ❌owned Vec | ❌新 Vec | 无 | ✗ | ✗ | ✗ | 无 | 极远 | **闸门失败** |
| ndarray-simd | — | — | — | — | — | — | — | — | — | **不存在(crates.io 空)** |

---

## 7. 对子系统的可迁移命题

形式："库 X 在条件 Y 下用机制 Z 解决 W；子系统满足 Y 才可迁移。"

1. **ndarray 的 stride view + `Zip` 写外部输出**：在"作者接受手写窗口/递归 loop、且以严格 unsafe/lifetime 契约接入借出列"条件下，用 `ArrayView::from_shape(strides)` + `Zip::for_each(&mut out)` 机制，在**不复制**下借入外部列、写调用方输出（实测 `allocs=(0,0)`）。子系统满足"typed SDK 把借出列包成 `ArrayView`/`ArrayViewMut`、作者写数组表达式"即可迁移——**代价是放弃现成指标/rolling/scan，向量化只得 LLVM autovec**。[OBSERVED]
2. **LLVM autovec 在 aarch64 覆盖 elementwise/密集窗口**：在"作者写惯用连续 f64 循环、release opt-level=3"条件下，编译器自动生成 NEON `.2d`（实测标量基线即含 NEON，Squeeze autovec ~14%）。子系统满足"列连续 + 惯用写法"即可迁移这份收益，**作者完全不碰 SIMD**——但**朴素 rolling max/min 不被 autovec**（Ichimoku 0.999x），须显式 SIMD 或更好标量算法。[OBSERVED]
3. **pulp 的 runtime NEON 派发**：在"stable Rust + 单一 aarch64/x86 二进制 + 作者愿写 `WithSimd` kernel"条件下，用 `Arch::new().dispatch` + `f64x2` 机制拿到显式 SIMD（Ichimoku 3.3x）。子系统满足"作者写 SIMD kernel 或把 pulp 藏在原语集背后"才可迁移——**它不做 AoS→SoA、不定义指标语义、无 rolling/scan/output 协议/validity，也不满足"作者不碰 SIMD"**。[OBSERVED]
4. **faer 的 `WithSimd`/`dispatch!` 窗口-向量/递归-标量分工**：在"作者接受线代词汇 + 写 SIMD"条件下，机制让"归约/elementwise 走 SIMD、时间递归留 scalar 且不变慢"（EMA 0.457x、复杂三项不慢）。子系统满足即可迁移这份**分工范式**（不是 API）——证明维护者标准 1 的分工在 stable Rust 可实现。[OBSERVED]
5. **polars 的 `EwmMeanState` 顺序 scan + `rolling_*`**：在"接受新 Series/所有权 materialization"条件下，用一等 `ewm`/`rolling` 机制提供 pandas 式递归/窗口词汇。子系统**当前不可迁移到 ABI**——它不写调用方列（`try_as_mut_slice` 仅 Vec backing）、复杂路径慢 1.5-2.3x + 分配；但其 **kernel 借用 `&[T]` + `Bitmap`** 的内部形状（`rolling/no_nulls`）证明"借连续列 + validity 位图"可作 kernel contract，是方案(b) rolling/ewm 原语的直接设计参考。[OBSERVED]
6. **Arrow 的 `SimdLevel` + validity/NullHandling + cumulative 注册**：在"kernel owner 写 dispatch、作者只调函数名"条件下，registry 机制隐藏 SIMD 并把 bitmap 作一等契约（最贴 UTA 段的 validity）。子系统满足即可迁移**"作者写函数名、原语层管 dispatch/bitmap"的分层**（方案 b 的组织范式）——但 Arrow 本身无 rolling/ewm、无写外部列、runtime 只 dispatch x86，不能直接作 aarch64 数据面。[OBSERVED source]
7. **pandas rolling/ewm 的词汇 + Cython 增量窗口**：在"用户不写 SIMD、库用增量窗口 + Kahan/Welford"条件下提供最贴 AI 认知的 `rolling`/`ewm` 词汇与数值稳定实现。子系统若采方案(b)可迁移**词汇与数值语义规范**（min_periods/adjust/ddof/warm-up）——但 pandas 无 SIMD、seed 语义须显式对齐 oracle，不能沿用默认黑盒。[OBSERVED source]
8. **闭合原语集对 SuperTrend 的不可表达性**：所有纯向量/窗口词汇（numpy/pandas/arrow/polars/numexpr/(b) 最小集）对 SuperTrend 的三状态机都**强制退回 scalar/custom**。这是一条**否定命题**：任何"作者只写数组表达式"的中间层都必须为 loop-carried 状态机保留一个 scalar/custom kernel 逃逸口；子系统的 ABI 与作者面必须显式容纳它，否则该类指标无法表达。[OBSERVED]

---

## 8. 对 `hpc-derivation-subsystem.md` 的修正建议（逐段，支持/需改/冲突）

### §7（外部编译与迭代 / 作者面）

1. **§7 "作者写 `fn compute(window,out)->Written`、typed SDK 导出列切片视图结构"** —— **支持并补强**。[OBSERVED] 往返实测证明"借入外部连续列 + 写调用方 `&mut [f64]`、零额外分配"在 stable Rust 可行，但**只有把列包成数组视图的库（ndarray/faer/nalgebra/mdarray）能做到；列式/张量引擎（arrow-rs/polars/candle/burn）全部返回新数组，过不了这个 ABI**。建议 §7 明确："SDK 导出的可写输出列必须是连续 `&mut [f64]`；不得选用只能返回新 `Array`/`Series`/`Tensor` 的库作数据面（arrow-rs/polars/candle/burn 已实测不满足）。"
2. **§7 作者面的"作者写什么"未定** —— **需补裁决点（本篇核心）**。当前 §7 只说作者写 `compute` + 列视图，未回答"作者写 SIMD 还是 numpy 式表达式"。建议 §7 补一节，把 fp-10 的 (a)/(b) 作为显式待裁决点：(a) SDK 只 bless 一个数组库（ndarray 最贴 numpy + 过 ABI），作者写数组表达式、向量化靠 LLVM autovec + 手写 loop；(b) SDK 提供词汇取自 numpy/pandas 的闭合原语集（rolling/ewm/where/reduce/cumsum/mask + pulp 派发），作者只组合、原语层隐藏 SIMD。**两案都要为 SuperTrend 类状态机保留 scalar/custom 逃逸口（命题 8）。**
3. **§7 未提"作者不碰 SIMD"的可达性** —— **需补一句 [OBSERVED] 结论**。建议补："aarch64（首要平台）上 LLVM autovec 已对 elementwise/密集窗口生成 NEON，作者写惯用连续 f64 表达式即可拿到向量化收益，无需写 SIMD；显式 SIMD（pulp/faer 的 WithSimd）要求作者写 SIMD kernel，与'作者不碰 SIMD'冲突，只应作为原语层(b)的内部后端，不下放给 op 作者。"

### §8.1（算法层：作者的依赖，不是子系统的组件）

4. **§8.1 表格（VectorTA/ta/yata/polars/ndarray 等）** —— **需扩充为"数组中间层"维度**。当前表只按"列式 f64 段下是否可用"评 TA 指标库。建议新增一张"数组/向量化中间层候选"表（本篇 §6），列：过 ABI 闸门(借外部列/写外部输出)、aarch64 NEON 实测、rolling/scan/ewm/validity、窗口SIMD/递归分工、numpy/pandas 词汇。要点入表：**ndarray 是唯一"numpy 式 + 过 ABI"的底座但无向量化/rolling/scan**；**faer/pulp 分工最自然但要作者写 SIMD**；**polars 词汇最贴 pandas 但违 ABI + 慢**；**arrow validity 最贴 UTA 但无 rolling/写外部列**。
5. **§8.1 "SDK 层用 `pulp` 做运行期 ISA 派发；`multiversion` 多版本化"** —— **支持，但需加"作者不碰 SIMD"约束**。[OBSERVED] pulp 在 aarch64 NEON runtime dispatch + 写 caller slice 实测可行、且是稳定 Rust 首选后端。但 pulp 要求作者写 `WithSimd`/head-tail——建议改为："pulp 作为**原语层内部**的 SIMD 后端（方案 b），不直接暴露给 op 作者；若采方案(a) 让作者用 ndarray，则向量化主要来自 LLVM autovec，pulp 仅用于原语层需超越 autovec 的少数窗口 kernel（如 rolling max/min）。"
6. **§8.1 未记"向量化收益依指标/算法"** —— **需补 [OBSERVED] 定量**。建议补："本机 M4 实测显式 SIMD 增量收益按指标：递推状态机(SuperTrend)≈1.0x、混合窗口(Squeeze)≈1.36x、朴素 rolling max/min(Ichimoku) 3.3x；且 3.3x 相对 naive O(n·w) 标量，改单调队列 O(n) 可能反超 SIMD。故'全向量化'不是目标（印证维护者标准 1），原语层应对不同指标类型选不同实现（elementwise/密集窗口靠 autovec、朴素 rolling 靠显式 SIMD 或更好标量算法、递归保持 scalar）。"
7. **§8.1 "指标语义不凭同名假定一致，作者逐指标对照后注册"** —— **支持并补 EWM seed 冲突**。[OBSERVED] polars `EwmMeanState`/pandas `.ewm` 的 seed/adjust/ignore_na 是通用 EWM，**不等于**行情 EMA(14) 的 idx13-mean seed 与 Wilder RSI 平滑；复杂实验中 polars_supertrend 直接退回 scalar。建议补："若原语层提供 `ewm`，其 seed/adjust 语义必须显式定义并与作者指标 oracle 对照，不得沿用库默认 EWM 黑盒；RSI/ATR/SuperTrend 的 Wilder/状态语义不能由通用 ewm 表达。"
8. **§8.1 未记"复杂用户指标的表达边界"** —— **需补 [OBSERVED] 命题 8**。建议补："以 PineScript 级复杂指标实测：多窗口 rolling max/min（Ichimoku）与线性回归（Squeeze linreg）可由 rolling/reduce 组合或代数展开表达；但 loop-carried 状态机（SuperTrend）在任何纯向量/窗口词汇下都强制 scalar/custom——作者面与 ABI 必须显式提供 scalar/custom kernel 逃逸口，位移(shift)与 linreg 也须明确机制。"

### 8.2 维护者七条标准 → 本篇证据映射（裁决速查，不裁决）

| # | 维护者标准 | 本篇一手证据 | 对 (a)/(b) 的含义 |
|---|---|---|---|
| 1 | 不追求全向量化，递归标量可 | SuperTrend 所有候选≈1.0x；Ichimoku pulp 3.3x/ndarray-faer≈1.0x；autovec 对照(§4.3) | 两案都应"窗口向量化 + 递归标量"；(b) 原语层显式编码此分工更可控 |
| 2 | 词汇距离软指标不淘汰 | §6/§9 词汇列：ndarray 近 NumPy、polars 近 pandas、pulp/faer 远 | (b) 词汇取 numpy/pandas 可零学习成本；(a1) ndarray 词汇近但缺 rolling/ewm |
| 3 | 只 f64、目标 AVX2/NEON、AVX-512 备注 | 全实验 f64；本机 NEON 实测、AVX2 源码推断、AVX-512 仅注记 | 无差别影响；两案都在 NEON 首要平台成立 |
| 4 | aarch64 NEON 首要主评判 | §4 全为 M4 NEON 实测；仅 x86 SIMD/aarch64 标量者降级（arrow/candle 等） | (a1) autovec 在 NEON 拿收益；(b) pulp NEON runtime 实测可行 |
| 5 | 真实数据 + 复杂指标 + 开发难度 | SPY 8465 行；SuperTrend/Ichimoku/Squeeze；LOC/摩擦(§4.5) | (a1) 作者手写窗口/递归/位移；(b) 作者只组合原语，实现落子系统 |
| 6 | 高级 AI 用户，词汇非 numpy 不淘汰(b) | (b) 可用非 numpy 内部名；软成本记学习面(§5.2) | (b) 不因词汇被否；但每个 custom 概念增学习/fallback 面 |
| 7 | 全 Rust 实现，Python 仅 golden | 所有候选实现/计时/汇编 Rust release；Python 仅 §4.1 golden | 证据链满足；(a)/(b) 后续实现同样须 Rust release 验证 |

**贯穿三点（供裁决）**：(i) 没有单一 Rust 库同时满足"numpy 式 + 作者不碰 SIMD + 过 UTA ABI + 有 rolling/ewm/validity"——ndarray 缺后半，polars 缺 ABI；(ii) aarch64 上 autovec 已让"惯用数组表达式"拿到 elementwise/密集窗口的 NEON，显式 SIMD 只在朴素 rolling max/min 有大收益且可能被更好标量算法抵消；(iii) 任何"作者写表达式"的中间层都必须为 SuperTrend 类 loop-carried 状态机保留 scalar/custom 逃逸口。

---

## 9. 未覆盖 / 待实测

- [需实测] **50 ms 端到端**：本篇只测 CPU 侧算法 + 往返；iceoryx2 IPC/借用/触发/写回未纳入（承 fp-09 §9 闸门 7）。
- [需实测] **x86 AVX2 实机**：本机仅 aarch64；arrow/polars/burn/pulp 的 x86 SIMD 均据源码推断 [INFERENCE]，AVX2 f64=4 lane 的实际收益未跑。AVX-512 仅备注。
- [需实测] **单调队列 O(n) rolling max/min vs SIMD**：Ichimoku 3.3x 相对 naive O(n·w)；deque 标量可能反超，未实现对比。
- [需实测] **方案(b) 闭合原语集的 Rust 实现**：本篇未写(b)的 Rust 代码，其 LOC/编译/分配/NEON 命中/50ms 为未测；(b) 的 SuperTrend 逃逸口、shift、linreg 具体 ABI 未设计。
- [需实测] **validity 位图端到端**：各候选 validity 为"unsupported"或 Arrow/polars 内部 bitmap；UTA 借出位图 + 原语传播 + 输出 first_valid 的完整链路未跑。
- [需实测] **ndarray autovec 逐指标汇编归因**：ndarray/faer 复杂候选符号命中 `.2d`，但与被 autovec 的标量参考未做纯净隔离对照（承 fp-09 §11）。
- [未覆盖] **更大窗口的 SIMD 收益放大**：主实验 N=8465（真实数据）；100k+ tile 下 SIMD/autovec 收益比例未系统扫描。
- [未覆盖] **候选的 nightly `portable_simd` 路径**：polars/其它的 nightly SIMD feature 未开（stable 优先）；若允许 nightly 结论可能变。
- [未覆盖] **numrs2 脱离 gate 的 SIMD 实测**：其 owned/COW 契约已判 ABI 失败，未对其 NEON kernel 单独 bench。

---

## 10. 来源清单（编号 · URL · 打开状态 · 本地路径/commit）

> 重产物均在独立盘 `/Users/mouriya/Ext/tmp/fp10/`。数据 SHA-256 `eeee85933f3b26b783ee7dfae2aaa0d59db6d0205b6ec54d18fbbcc828da96f4`。完整逐行来源见各子段落 `local://fp10-{ndarrayfaer,arrowpolars,pulp,placement}.md`。

### 原生 Rust 候选（clone pin commit，本机 release 实测）
1. `ndarray` v0.17.2 · github.com/rust-ndarray/ndarray · 已打开(本地 clone) · `/Users/mouriya/Ext/tmp/fp10/ndarrayfaer/ndarray` · `2cf23d6abf5f7a8a5e638fa1c69779dc4d7219a0`
2. `ndarray-stats` v0.7.0 · 已打开 · `.../ndarrayfaer/ndarray-stats` · `67d0292f483c90b8afa0c975832d979d24204e95`
3. `ndarray-npy` v0.10.0 · 已打开 · `.../ndarrayfaer/ndarray-npy` · `e403bc75915ac4dfd82dd7a7a086fcac2a68e740`
4. `faer` v0.24.4 · codeberg.org/sarah-quinones/faer · 已打开 · `.../ndarrayfaer/faer` · `0539947ffb757a739d7e703a7d2fa0c792a909c1`
5. `nalgebra` v0.35.0 · github.com/dimforge/nalgebra · 已打开 · `.../ndarrayfaer/nalgebra` · `5f927f6c821d0ced07a0e086e5f37251cab2a8c3`
6. `mdarray` v0.8.0 · github.com/fre-hu/mdarray · 已打开 · `.../ndarrayfaer/mdarray` · `8eddbfc3b15d91ceb9c7659412b4cbffb11f7952`
7. `numrs` v0.2.0 · github.com/sankha93/numrs · 已打开 · `.../ndarrayfaer/numrs` · `bdb812f353d1ed5e48d30b07442866cfdbe50b66`
8. `numrs2` v0.4.1 · github.com/cool-japan/numrs · 已打开 · `.../ndarrayfaer/numrs2` · `ee569fa620b91857760c7c3bdf71fac249b163c9`
9. `ndarray-simd` · crates.io `q=ndarray-simd` · 已打开，结果 `[]`(不存在) · `.../fp10/crates-search-ndarray-simd.json` · 无
10. `pulp` v0.22.3 · github.com/sarah-quinones/pulp · 已打开 · `.../fp10/pulp/src` · `5eb07fd7b68edf0a5e19f71737d315f72a510295`
11. `arrow-rs` v60.0.0 · github.com/apache/arrow-rs · 已打开 · `.../arrowpolars-candleburn/arrow-rs` · `ef1fa157977633f0ba21aa921f9ef3d5c669235b`
12. `polars` v0.55.2(tag rs-0.55.2，代码) · github.com/pola-rs/polars · 已打开 · `.../arrowpolars-candleburn/polars` · `d7488c71ecfbc77790292ff5b365b991c08380ce`
13. `candle-core` v0.11.0 · github.com/huggingface/candle · 已打开 · `.../arrowpolars-candleburn/candle` · `31f35b147389700ed2a178ee66a91c3cc25cc80d`
14. `burn` v0.21.0 (+`macerator` v0.3.4 registry) · github.com/tracel-ai/burn · 已打开 · `.../arrowpolars-candleburn/burn` · `546cacb55fe00168854d19bdf0a5d79bd8060e03`

### 放置先例（curl 源码/文档，pin commit）
15. NumPy · github.com/numpy/numpy · 已打开(HTTP 200) · `.../PlacementPrecedent/curl-source/numpy` · `c8899977104fbb1336417fb6150d8c9f41a0d582`
16. pandas · github.com/pandas-dev/pandas · 已打开(HTTP 200) · `.../PlacementPrecedent/curl-source/pandas` · `baac16ef9eb46b14abdebca405b8f2cf2a2c8e8d`
17. Apache Arrow C++ Compute · github.com/apache/arrow · 已打开(HTTP 200) · `.../PlacementPrecedent/curl-source/arrow` · `0bd8def07f87443df4f96ade5917b268d2ff01d0`
18. Polars(docs/DSL) · github.com/pola-rs/polars · 已打开(HTTP 200) · `.../PlacementPrecedent/curl-source/polars` · `473ca8efde86aca28fb689bb27881fdeb934b6d5`
19. Bottleneck · github.com/pydata/bottleneck · 已打开(HTTP 200) · `.../PlacementPrecedent/curl-source/bottleneck` · `15124a00c6edf2693a02b9db02e9127b37510152`
20. numexpr · github.com/pydata/numexpr · 已打开(HTTP 200) · `.../PlacementPrecedent/curl-source/numexpr` · `7031844cb0c3a7f84082e0a2cb89bebd88cae70d`

### 数据、参考、本机运行证据
21. SPY 日线 · `query1.finance.yahoo.com/v8/finance/chart/SPY?period1=728000000&period2=<now>&interval=1d` · 已打开 · `/Users/mouriya/Ext/tmp/fp10/data/spy_daily.csv` · SHA-256 `eeee8593…`（8465 行）
22. Pine 参考 · tradingview.com/pine-script-reference/v5/#fun_ta.supertrend / #fun_ta.hma；/support/solutions/43000589152（Ichimoku）；/script/nqQ1DT5a（LazyBear Squeeze） · 语义对照(经 golden 核对)；TradingView 页可能 JS-gated · `local://fp10-experiment-spec.md`
23. 实验契约 · `local://fp10-experiment-spec.md` · 已打开 · 同左
24. Golden 独立参考 · Python 一次性核对(仅 golden，非候选实现) · 已运行 · 值见 §4.1
25. 本机运行/汇编产物 · 已运行(exit 0) · `/Users/mouriya/Ext/tmp/fp10/{pulp/{run.log,repeats.log,repeats-novec.log,disassembly.txt},ndarrayfaer/gates-*,{ndarray,faer,faer-complex,nalgebra,mdarray}-otool.txt,arrowpolars-candleburn/{rt-*/run.log,complex-bench/{run.log,complex-bench.otool.txt}}}` · 依赖各库 pin
26. 子段落(完整 ①–⑨/逐条来源) · `local://fp10-ndarrayfaer.md`(S1–S25)、`local://fp10-arrowpolars.md`(1–43)、`local://fp10-pulp.md`(1–18)、`local://fp10-placement.md`(S1–S55) · 已打开

**只读设计来源**：`hpc-derivation-subsystem.md`、`research/fp-09-hpc-compute-layer-and-vectorta.md`、`uta-core-design.md` §0.2/§5.3。
