# fp-09：原生 op 算法层与 VectorTA 一手深度调查

状态：一手调查，闭合。**定位**：为 `hpc-derivation-subsystem.md`（可选、独立于核心的行情派生高性能计算子系统）补充"原生 op 的算法层用什么"这一层的证据；不做 UTA 设计。**服务对象**：子系统 §6.3 原生 op 执行、§3 布局、§1.3 预算、§5 段生命周期。**证据标注**：**[OBSERVED]**（亲眼在源码/文档/本机运行输出看到）/ **[INFERENCE]**（据观察推导到子系统契约）/ **[需实测]**（本调查未证）。

---

## 1. 摘要（≤10 行）

- **VectorTA 0.3.1 在子系统当前已定契约（`repr(C)` 定长 AoS 记录、价格可能定点 i64、只读共享段完整窗口、调用方输出段）下，不能直接作原生 op 算法层；分类是"需全窗物化 + 数值/语义适配"。最大契合点：源码审计显示 340 个指标文件全部有 `xxx_into(&mut [f64])`（所测三项 RSI/ATR/BBands 运行时零额外分配）。最大冲突：核心输入 API 只收连续 `&[f64]`（SoA 分列），而 AoS 字段的相邻地址间隔是 `sizeof(Layout)` 非 `sizeof(f64)`，无法零拷贝借成 `&[f64]`——真实不匹配是"strided i64 AoS ↔ contiguous f64 columns"整体输入 ABI，被迫每次触发做全窗融合 gather/scale/convert，直接击穿子系统"洗入之后不再搬窗口"的零拷贝前提。** [OBSERVED]+[INFERENCE]
- 数值冲突独立于且更根本于布局冲突：VectorTA 核心价格类型是 `f64`（`Candles` 价格列 `Vec<f64>`），无定点 i64 入口；即便改 SoA，只要洗入存 i64 定点，仍需整列 i64→f64 转换。[OBSERVED]
- 平台：Apple M4 `aarch64-apple-darwin` release（默认与 `nightly-avx` 两种）实测 kernel 恒为 `Scalar/ScalarBatch`——VectorTA 的 SIMD 全部 `#[cfg(all(feature="nightly-avx", target_arch="x86_64"))]`，**aarch64 无任何显式 SIMD 内核**；递归指标（RSI）即便在 x86+AVX 也走标量。[OBSERVED]
- gap：递归指标遇中间 NaN 不把它当边界或 validity——本机观测 RSI 在 NaN 处不产 NaN、递归态重复/继续某状态（平台化），因此**无法独立表达显式 gap**，须外部分段/reset/validity 策略；gap 后 reset/保持/缺失仍须**系统契约定义**。这与子系统"gap 显式、不伪造连续"不契合。[OBSERVED]+[需实测]
- 布局裁决保持中立：方案 A（保持 AoS）是当前契约下唯一无需转换的直接借用基线；方案 B（段内 SoA/AoSoA）只有生产端预建才可能契合；方案 C（列即段）改变段 ABI，须另立架构决策。不预写"应改 SoA"。[INFERENCE]
- 50 ms 端到端整体预算 [需实测]；但本机测得的"物化 + 计算"分项可给参数化容量上界（§7）。

---

## 2. 范围与方法

- **只读来源**：`hpc-derivation-subsystem.md` 全文、`uta-core-design.md` §0.2/§5/§5.3。不读本仓库其他文件。
- **一手取证**：VectorTA 以 crates.io 发布 tarball `vector-ta-0.3.1` 为源代码锚（SHA-256 `b530eeccbf2577e6c5e583f85a9e5f7b75042379d77936bdf5eed2116210af4f`，`.cargo_vcs_info.json` 指向 git commit `802518e2392c5d011744b75e56e108e97a0682b4`），双路独立解包核验（`/tmp/hpc-verify/vector-ta-0.3.1` 与 `/tmp/hpc-research-vectorta/vector-ta-0.3.1`）。对照库与 SIMD 库各 clone 并 pin commit（§12）。README/docs.rs/benchmark/issue 用 `curl`（crates.io 需 User-Agent，首次无 UA 返回 403）取原文。
- **本机运行**：`rustc 1.95.0 (59807616e 2026-04-14)`，host/target `aarch64-apple-darwin`，Apple M4，LLVM 22.1.2，`cargo build --release`（`opt-level=3`, `lto=true`）。所有计时不代表 x86 AVX。
- **裁决门槛**（每库对照，来自子系统 §1.3/§3/§6.3 与 mentor 校准）：① 直接借共享完整窗口，不做 AoS→SoA 或 i64→f64 全窗物化；② 写调用方输出区或所有权模型兼容 op ABI；③ warm-up/无效值/gap 可表达且不错误串接 gap 两侧递归态；④ `aarch64-apple-darwin` release 有可查证执行路径（"无 AVX"≠"必然标量"）；⑤ 指标语义（周期边界、EMA seed、RSI 零分母、NaN 传播）不凭同名假定一致；⑥ 完整适配+计算 ≤ 50 ms（与"直接契合"分开裁决）。
- **分级口径**：直接可用 / 需薄封装 / 需全窗物化 / 语义不兼容。

---

## 3. VectorTA 深挖

crate `vector-ta` 0.3.1（`github.com/VectorAlpha-dev/VectorTA`）。license Apache-2.0（`Cargo.toml`）。crate 创建 2026-01-21、更新 2026-07-18、downloads 820、`max_stable_version=0.3.1`（crates.io API）。README 自称 340 indicators；tarball `src/indicators/**` 实为 356 个 `.rs`（含 `moving_averages/`、`dispatch/` 子目录），指标数量级属实。[OBSERVED]

> 路径约定：下文 `src/...` 均相对发布 tarball 目录；行号基于该固定 tarball。tarball 与 git checkout 全树 diff 非空（539 行），但关键 API blob（`sma.rs`/`ema.rs`/`rsi.rs`/`lib.rs`）两者 SHA-256 相等。

### 3.1 五问（逐门槛）

**问一：能否直接借用只读共享段完整窗口？——不通过。** [OBSERVED]+[INFERENCE]
- 单列 API 收单个连续 `&[f64]`：`RsiInput::from_slice(sl: &'a [f64], ...)`（`src/indicators/rsi.rs:91`）、`SmaInput` 只接受 `SmaData::Slice(&[f64])` 或 `Candles` 某一列（`src/indicators/moving_averages/sma.rs:40-68`）。多列 API 收多个连续 `&[f64]`：`AtrInput::from_slices(high: &[f64], low: &[f64], close: &[f64], ...)`（`src/indicators/atr.rs:80-90`）、`AdxInput::from_slices(h,l,c: &[f64],...)`（`src/indicators/adx.rs:61-85`）。
- 库自有的 `Candles` 完全 SoA：`timestamp: Vec<i64>`，`open/high/low/close/volume: Vec<f64>`，外加预算列 `hl2/hlc3/ohlc4/hlcc4: Vec<f64>`（`src/utilities/data_loader.rs:17-30`），构造时 `precompute_fields()` 再分配（`data_loader.rs:32-63`）。
- 决定性源码（`sma.rs:40-68`，我方独立核验）——`SmaInput` 的 `AsRef<[f64]>` 只能从 SoA 列返回连续 slice，AoS 记录无对应路径：
  ```rust
  impl<'a> AsRef<[f64]> for SmaInput<'a> {
      fn as_ref(&self) -> &[f64] {
          match &self.data {
              SmaData::Slice(slice) => slice,
              SmaData::Candles { candles, source } => match *source {
                  "close" => &candles.close, "high" => &candles.high, /* ... 每列独立 Vec<f64> ... */
                  _ => source_type(candles, source),
              },
          }
      }
  }
  ```
- 对 `src` 搜索 stride/gather/`ArrayView`/公开 iterator 输入路径：**未找到**可供 native 指标使用的形状（唯一相关是 CUDA 内部代码，不改 native API）。
- [INFERENCE] 子系统 `Layout` 中某价格字段的相邻记录地址间隔 `sizeof(Layout)`，`zerocopy`/`bytemuck` 只能借出整条记录 `&[Layout]`，不能把跨记录同字段拼成连续 `&[f64]`；必须逐列 gather/转置。此项已使门槛①失败。

**问二：能否写入调用方输出区？——条件通过。** [OBSERVED]
- **源码审计**：353 个 `src/indicators/**/*.rs` 中，340 个含 `pub fn *_into`；无 `_into` 的 13 个全是基础设施文件（`dispatch/`、`registry`、`ma_batch`、`ma_stream`、`param_schema`、`utility_functions`），非指标——即**每个指标文件都有 `_into` 入口**。示例：`rsi_into(input, out: &mut [f64])`（`rsi.rs:230`）、`rsi_into_slice`（`rsi.rs:274`）、`sma_into`（`sma.rs`）、`ema_into`/`ema_into_slice`、`atr_into`（`atr.rs:279`）、`bollinger_bands_into`（`bollinger_bands.rs:413`）；底层 `xxx_compute_into(..., out: &mut [f64])`。要求 output 与 input 等长、连续。
- 默认返回路径分配 `Vec<f64>`：`RsiOutput{values: Vec<f64>}`（`rsi.rs:54-55`）、`BollingerBandsOutput{upper_band/middle_band/lower_band: Vec<f64>}`（`bollinger_bands.rs:52-56`）；经 `alloc_with_nan_prefix`（`src/utilities/helpers.rs:103-134`）建 `Vec`。
- 本机实测（**范围限于 RSI/ATR/BBands 三项 single 路径**，非全部指标）：`rsi_into` 写入调用方 buffer 成功，首个有效值 @ idx14（=period）。VectorTAProbe 的 allocator 计数：`_into` 路径 `allocs=(0,0)`（计数窗口排除输入/输出预建，含指标内部工作区），返回 Vec 路径 `allocs=(1,0)`（`/tmp/hpc-research-vectorta/harness-run-default.log:3-6`）。batch/param-sweep/多输出/派生输入的内部 scratch 未逐一审计（§11）。
- [INFERENCE] `_into` 使输出所有权兼容 op ABI——**前提是输入已是连续 f64**。它不能写回只读输入段，也不支持带 stride 的输出 view，且不是 `impl Iterator<Item=Output>` 形状。

**问三：warm-up/无效值/gap 能否表达且不串接两侧？——部分通过；契约待系统定义。** [OBSERVED]+[需实测]
- warm-up = NaN 前缀：`alloc_with_nan_prefix(len, warm)` 前 warm 个填 quiet NaN `0x7ff8_0000_0000_0000`（`helpers.rs:103-118`）；前缀长度各指标不同——SMA `first+period-1`、RSI `first+period`、ATR `first+length-1`、ADX `first+(2*period-1)`。
- 递归指标遇 gap 行为**各异且非系统语义**：EMA batch 对非 finite 输入跳过更新、保持前值（`ema.rs:446-499`）；RSI batch 在 seed delta 遇非 finite 时把 avg gain/loss 设 NaN 并递归传播（`rsi.rs:326-414`），RSI stream 有 `poisoned` 态（`rsi.rs:1015-1120`）；ATR streaming 用 debug assertion 要求 finite 并注明"prefilter NaNs/Infs upstream if needed"（`atr.rs:758-827`）。
- **本机决定性观测**（`period=3`）：RSI 在 idx10 插入 NaN → idx8..16 = `[100, 87.00, 87.00, 87.00, 25.05, 11.90, 6.98, 4.78]`——**NaN 不被当作边界或 validity**：不产 NaN 输出，递归态重复/继续某状态（平台化，87 重复），随后照常递推。**准确表述是：实现无法独立表达显式 gap，须外部分段/reset/validity 策略**；本调查不判定 gap 后应 reset 还是传播缺失（属系统契约）。batch 与 stream 对同一 `[1,2,NaN,4,5]` 输出还不同（VectorTAProbe：EMA batch `[1,1.5,1.5,2.75,3.875]` vs stream `[None,None,None,2.333,3.667]`）。这已足以判"不直接契合"子系统"gap 显式、不伪造连续"。
- [需实测]/[系统契约] gap 后每个递归指标该 reset/保持/缺失，须由 UTA 定义；不能替系统选择。

**问四：`aarch64-apple-darwin` release 有可查证执行路径？——scalar 通过；加速路径不通过。** [OBSERVED]
- `Kernel` enum 只有 `Auto/Scalar/Avx2/Avx512/+Batch`，无 NEON 变体（`src/utilities/enums.rs:2-9`）。`detect_best_kernel()`（`helpers.rs:10-25`）的 AVX 检测整块 `#[cfg(all(feature="nightly-avx", target_arch="x86_64"))]`，其余 target 返回 `Kernel::Scalar`；aarch64 恒 Scalar。
- 所有 SIMD 内核门 `#[cfg(all(feature="nightly-avx", target_arch="x86_64"))]`：202/282 顶层指标文件含该门。`portable_simd`（`core::simd`）只在 `lib.rs:9` `#![cfg_attr(all(feature="nightly-avx", rustc_is_nightly), feature(portable_simd))]` 启用——**仅作 x86 AVX 载体，aarch64 从不用**。递归指标即便 x86+AVX 也走标量：`rsi.rs:258-259` `Kernel::Avx2 => rsi_compute_into_scalar`。NEON 字面仅 `pivot.rs`（疑似命名，非 SIMD）。
- 本机实测：默认与 `--features nightly-avx` 两种 release build 均成功，运行时均打印 `single=Scalar batch=ScalarBatch`（`/tmp/hpc-research-vectorta/harness-run-{default,nightly-avx}.log:1-2`）；`cfg!` 报告 `target_arch="aarch64"`, `target_feature="neon"`。
- [INFERENCE] "无 AVX"不能推出编译器不对 scalar loop 自动向量化；但本库无可查证 aarch64/NEON/portable-SIMD 算法路径。x86 AVX 文本扫描不是 M4 加速证明。

**问五：指标语义与 50 ms 能否直接假定？——语义需适配；50 ms [需实测]。** [OBSERVED]
- EMA `alpha=2/(period+1)`，首个 finite sample 直接作均值（`ema.rs:323-326,446-499`）——非"同名即等于 UTA EMA"。
- RSI seed=period 个 delta，输出前缀 `first+period`，**零分母返回 50**（`rsi.rs:326-414`；本机常数序列观测 `[NaN,NaN,NaN,50,50,50,50,50]`）。
- 核心价格类型均 `f64`（`SmaData::Slice(&[f64])`、`Candles` 价格列 `Vec<f64>`）；**未找到**支持定点 i64 价格并保留定点语义的 native 指标 API。`Candles.timestamp` 虽 i64，价格列仍 f64。
- 官方 benchmark（100k candles、每指标单参数、warm-up 后单调用）环境为 AMD Ryzen 9 9950X / RTX 4090 / Windows 11 / Rust 1.95-nightly（README/docs.rs），不含 UTA 的 IPC/gather/定点转换/写回，不是 M4。

### 3.2 五个深挖子项

**(a) 输入形状 zero-copy 冲突量化。** [OBSERVED]+[本机实测]
签名确证为 `from_slice(&[f64])` / 多个 `&[f64]`，无 `&[Layout]+stride` 形状（`sma.rs:96-113`, `atr.rs:71-90`, `adx.rs:67-85`）。真实不匹配是"strided i64 AoS ↔ contiguous f64 columns"整体输入 ABI。本机测得**融合 gather+scale+convert（AoS-i64→SoA-f64）3 列 100k = 88.9 µs/call**（≈ 转置-only 86.4µs，i64→f64 缩放融合进同一内存受限 pass、几乎免费；分项相加会高估约 40µs，见 §7）——与单指标计算同量级（RSI 100k=91.7µs）。这不是"加一个 unsafe zerocopy cast"能消除的 mismatch。

**(b) 输出写入调用方段。** [OBSERVED] `_into(&mut [f64])` 是最接近子系统 `fn compute(windows) -> impl Iterator<Item=Output>` 的路径，但不是 iterator、不支持输出 stride、不能写只读输入段。零分配观察只覆盖简单 single indicator + 5 点样本，不证明全部 340 指标/batch/大窗口无 scratch 分配。

**(c) SIMD 分派实际后端。** [OBSERVED] 见问四。dispatch 表在 `rsi.rs:249-270`（Scalar/AVX2/AVX512 都调 `rsi_compute_into_scalar`）、`sma.rs:292-313`（AVX2 arm 调 scalar，仅 x86 cfg AVX512 arm 调实际 AVX512）。EMA `detect_ema_kernel` 同由 `nightly-avx+x86_64` cfg 包住（`ema.rs:329-355`）。

**(d) streaming vs 全量重算。** [OBSERVED] `RsiStream`/`into_stream()`（`rsi.rs:154`）、`SmaStream`（`sma.rs:551-584` 需 `Vec<f64>` ring）、`EmaStream`（period 大小 boxed table，`ema.rs:549-588`）均存在。子系统"每次看完整窗口"→ stream 非必需；且 stream 的有限样本计数、gap hold/poison 与 batch full recompute 可观测地不同（见问三），跨触发保留 stream 状态会悄悄固定系统未定义的 gap 语义，[INFERENCE] 不可采。

**(e) 数值与 CUDA。** [OBSERVED] 见问五（f64 only、零分母=50、warm-up=NaN 前缀）。CUDA：非默认 feature（`cuda=[cust,cust_derive]`），`kernels/ptx/compute_89/*.ptx` 预编译 PTX（SM 8.9=Ada），`src/cuda/*_wrapper.rs` host 包装，`build.rs` 非 `cuda-build-ptx` 时默认 stage PTX。CUDA SMA host API 收 `&[f32]`，host→`LockedBuffer`/`DeviceBuffer`→算→copy 回 `&mut [f32]`（`src/cuda/moving_averages/sma_wrapper.rs`）。README 原话：CUDA "predominantly only worth using if used in a VRAM-resident workflow"（例：ALMA 250M 点 RTX4090 3.129ms vs CPU AVX-512 140.61ms/AVX2 188.64ms/scalar 386.20ms）。issue #57/#58/#59 记录 CUDA 13 缺 `<cfloat>`、`CUDA_ARCH=120` 被误规整为 `compute_12`（环境标 0.2.9）。[INFERENCE] host↔device copy + f32 输入不符子系统只读共享 AoS + 可能 i64 + 直接借用要求；CUDA 不是补救路径，与子系统 §1.3"GPU 现在不做"一致。

**工程。** [OBSERVED] features：`default=[]`（纯标量 stable 即可用）、`nightly-avx`（空 feature 标记，仅 cfg 门，需 nightly toolchain）、`cuda`/`python`(pyo3+numpy)/`wasm` 均 optional 干净。依赖：`aligned-vec 0.6`, `ndarray 0.16`, `num-traits`, `itertools`, `smallvec`, `lru`, `once_cell`, `anyhow`, `thiserror`, `chrono`, `csv`, `serde/json`, `libc`, `float-cmp`, `pastey`。`unsafe` 用量重（scalar 内核多为 `unsafe fn`；tarball 全树 `unsafe` 文本约 1.2 万处，含测试与宏展开，量级证据非精确 API 计数）。`lib.rs` 顶部 `#![allow(warnings)]`。

### 3.3 本机 harness（可复现）

- 两套 harness：我方独立 harness `/tmp/hpc-verify/harness`（依赖 `vector-ta = "=0.3.1"`，`opt-level=3 lto=true`），VectorTAProbe 另有 `/tmp/hpc-research-vectorta/harness`（`default-features=false`，另测 `--features nightly-avx`）。两套结论一致：backend 恒 `single=Scalar batch=ScalarBatch`。
- 我方计时（M4, scalar, 100k 样本, 200/500 iter 均值，消费输出防 DCE）：**RSI(14)=91.7 µs、ATR(14)=154 µs、BBands(20)=25.5 µs；融合 AoS-i64→SoA-f64 3 列=88.9 µs**（转置-only 86.4µs、i64→f64 单列 13.9µs，仅作分项解释，不加总）。VectorTAProbe 官方 x86 对照（AMD 9950X, nightly）：rsi 100k=0.208ms、sma=0.037ms、ema=0.103ms（README benchmark 表）——RSI 两处都标量，M4 单核反快于 9950X。
- 我方 harness 运行输出（节选，`/tmp/hpc-verify/harness` 运行结果）：
  ```text
  RSI(period=3) on monotone 1..8 -> [NaN, NaN, NaN, 100.0, 100.0, 100.0, 100.0, 100.0]
  RSI on constant 5.0 (zero-denominator) -> [NaN, NaN, NaN, 50.0, 50.0, 50.0, 50.0, 50.0]
  RSI with NaN at idx10 -> [100.0, 87.003, 87.003, 87.003, 25.052, 11.904, 6.984, 4.780]
  rsi_into wrote caller buffer, first valid @ Some(14)
  RSI(14) recur     91.7 us/call  (100000 samples)
  ATR(14) recur    154.0 us/call  (100000 samples)
  BBands(20) win    25.5 us/call  (100000 samples)
  AoS->SoA transpose 3col   98.4 us/call
  i64 fixed->f64 convert    14.3 us/call
  ```
- VectorTAProbe harness 运行输出（default 与 nightly-avx 逐行相同，`harness-run-{default,nightly-avx}.log:1-14`）：
  ```text
  HARNESS backend single=Scalar batch=ScalarBatch
  HARNESS target_arch=aarch64 os=macos
  SMA batch  values=[NaN,NaN,2,3,4] allocs=(1, 0)
  SMA _into  values=[NaN,NaN,2,3,4] allocs=(0, 0)
  EMA batch  values=[1,1.5,2,3,4]   allocs=(1, 0)
  EMA _into  values=[1,1.5,2,3,4]   allocs=(0, 0)
  RSI constant   values=[NaN,NaN,NaN,50,50]
  RSI increasing values=[NaN,NaN,NaN,100,100]
  EMA batch gap  values=[1,1.5,1.5,2.75,3.875]
  RSI batch gap  values=[NaN,NaN,NaN,NaN,NaN]
  EMA stream gap values=[None,None,None,2.333333,3.666667]
  RSI stream gap values=[None,None,None,NaN,NaN]
  ```
- 关键交叉证据：`_into` 路径 `allocs=(0,0)`（零额外分配写调用方 buffer），返回 Vec 路径 `allocs=(1,0)`；同一 `[1,2,NaN,4,5]` 上 EMA batch（idx3=2.75）与 stream（第三返回值 2.333）**不同**，证明 batch full-recompute 与 stream 增量语义不可互换。

### 3.4 VectorTA 六门槛裁决表

| 门槛 | 观察与裁决 | 状态 |
|---|---|---|
| ① 直接借共享完整窗口，不做 AoS→SoA / i64→f64 全窗物化 | 输入 API 只接连续 `&[f64]` / SoA `Candles`；AoS stride=`sizeof(Layout)` 与定点 i64 无法直接借成该 slice | **不通过** |
| ② 写调用方输出区或兼容 op ABI | `xxx_into(&mut [f64])` 存在，等长连续 output 零额外分配（`allocs=(0,0)` 实测）；不能写只读输入、不能输出 stride、非 iterator | **条件通过** |
| ③ warm-up/invalid/gap 可表达且不错误串接 | warm-up=NaN 前缀（各指标长度不同）；EMA hold、RSI poison、ATR 要求 finite；NaN 不作边界/validity，实现继续/重复状态、无法独立表达显式 gap（本机 RSI 平台化）；batch/stream 不同 | **部分通过；契约待系统定义** |
| ④ aarch64 release 有可查证执行路径 | M4 release（default + nightly-avx）均可运行 scalar；无显式 NEON 路径，x86 AVX 文本≠M4 加速 | **scalar 通过；加速不通过** |
| ⑤ 指标语义一致 | EMA seed、RSI period/零分母=50、NaN 传播须逐指标对照，不能凭同名假定 | **未通过，需语义适配测试** |
| ⑥ 完整适配+计算 ≤ 50 ms | 只测 CPU 侧物化+计算（§7），未测 IPC/调度/大窗口/并发 | **[需实测]** |

**最终分类（按当前固定 AoS+i64+调用方输出契约）**：**否——需全窗物化 + 数值/语义适配，不是可直接挂接的"原生 op 算法层"**。[INFERENCE] 若上游改为 SoA f64 段，单列 `from_slice(&[f64]) + xxx_into(&mut [f64])` 可降为"需薄封装"，仍须定义 warm-up/gap、验证数值语义、M4 测 50 ms。

---

## 4. 对照库（每库五问 + 分级）

一手来源见 §12；均本机 Apple M4 aarch64 release，rustc 1.95.0。

### 4.1 `ta`（ta-rs, crate 0.5.0）——需薄封装
- **① 做什么/维护** [OBSERVED]：EMA/SMA/RSI/MACD 等约二十余指标；crates.io 最新 0.5.0（2021-06-26），874 stars，最近 push 2024-07-12，未归档——release cadence 很低。
- **② 输入/输出** [OBSERVED]：核心 `Next<T>::next(&mut self, input: T) -> Output` 逐值消费（`src/traits.rs:14-50`）；`DataItem` 五个 f64 字段（`src/data_item.rs:33-75`）。可让 `Layout` 实现 `Close/High/...` trait 按 AoS 逐记录借读，但**无 `&[Input]`/`_into`/caller-buffer**；结果标量须调用方 loop 写出。无 i64/validity。
- **③ SIMD/aarch64** [OBSERVED]：manifest 仅可选 `serde`，源码无 `std::simd`/AVX/NEON/intrinsics——非显式 SIMD，仅可能 LLVM autovec。
- **④ 递归/warm-up/gap** [OBSERVED]：EMA 首值即输入、无 warm-up 缺失（`exponential_moving_average.rs:58-95`）；RSI 首点 up/down 置 0.1 "to avoid division by zero"（`relative_strength_index.rs:74-123`）；只有 `Reset`，无 gap token。
- **⑤ 局限** [OBSERVED]：README 明说序列化 "backward compatibility is not guaranteed ... subject to change"。本机 smoke：`ta_ema=[2.0,3.5,2.25,4.25]`、`ta_rsi=[50.0,85.714,35.294,16.216]`。
> **分级：需薄封装**——AoS 可逐记录借用无全窗复制，但须自适配 i64→f64、gap reset、标量→caller buffer；EMA/RSI 首值语义非系统契约。

### 4.2 `yata`（crate 0.7.0）——需薄封装
- **① 做什么/维护** [OBSERVED]：EMA/DEMA/TEMA/SMA/RSI/MACD 等大量 method/indicator；0.7.0（2024-03-07），399 stars，push 2024-09-19。
- **② 输入/输出** [OBSERVED]：`Method::next(&mut self, &Self::Input)` 单值状态机；`over/new_over` 产生等长新 `Vec`；`Sequence::apply` 仅 `Output=T` 时对 `AsMut<[T]>` 原地覆盖（`src/core/method.rs:66-183`, `src/core/sequence.rs:6-27`）。`OHLCV` trait 从记录取 `ValueType`（默认 f64，feature 可 f32）。自定义 `Layout: OHLCV` 可零拷贝逐记录读，但无 `_into`/validity，i64 仍需逐点转换。
- **③ SIMD/aarch64** [OBSERVED]：features 仅 `value_type_f32`/period-width/`unsafe_performance`；README 原话 `unsafe_performance` 多为 vector element access，增益约 5–10%——非 SIMD。
- **④ 递归/warm-up/gap** [OBSERVED]：EMA `new(length,&initial)` 以初值为态、`next` 做 `(value-old).mul_add(alpha,old)`，无 warm-up 缺失（`methods/ema.rs:49-85`）；`Method` "There is no reset method ... create a new one"；RSI pos/neg 全零输出 0.5（`indicators/relative_strength_index.rs:59-79`）。
- **⑤ 局限** [OBSERVED]：README "stable Rust 支持但 you can't run benchmarks with it"。本机 smoke：`Sequence::apply=[3.0,4.5,6.75,9.375]`。
> **分级：需薄封装**——逐条借用 AoS 可行且 `apply` 可覆盖同型 caller slice；但完整 op ABI、i64 转换、gap reset、mask 仍需 wrapper。

### 4.3 `tulipindicators` C 0.9.2 + Rust binding `Uzaaft/tulip-rs`——需全窗物化
- **① 做什么/维护** [OBSERVED]：ANSI C，header 写 104 indicators（`indicators.h:30-46`）；GitHub 最新 v0.9.1（2022-01-18），943 stars，push 2024-02-02。Rust binding 是非官方 `Uzaaft/tulip-rs`（2 stars，push 2023-10-20），采用风险高（crates.io 同名 `tulip_rs` 指向另一项目）。
- **② 输入/输出** [OBSERVED]：统一函数指针 `int(size, TI_REAL const *const *inputs, TI_REAL const *options, TI_REAL *const *outputs)`，`TI_REAL=double`，每 indicator 从 `inputs[k]` 取连续列、向 `outputs[k]` 顺序写（`indicators.h:73-97`, `indicators/ema.c:34-55`）——SoA `double*`，无 stride/record；**输出由调用方预分配（ABI 半兼容）**。Rust binding 是 bindgen 裸 `*const *const f64`（`indicators.rs:7742-7750`），`build.rs` 先 `make libindicators.a` 静态链接。
- **③ SIMD/aarch64** [OBSERVED]：Makefile 仅 `-std=c99 -O2 -g`，无 AVX/NEON/SIMD——标准 C 标量。
- **④ 递归/warm-up/gap** [OBSERVED]：EMA `ti_ema_start` 返回 0、首值即 `input[0]`（`ema.c:28-50`）；RSI `ti_rsi_start` 返回 period，平盘 `smooth_up/(smooth_up+smooth_down)` 无零分母保护；本机 arm64 smoke：`rsi_start=3`、flat 输入首值 `nan`。无 gap/null mask。
- **⑤ 局限** [OBSERVED]：Rust binding README "It should work on OS X and Linux. Haven't been tested on Windows."
> **分级：需全窗物化**——预分配输出满足半 ABI，但输入连续 `double` 分列，AoS+i64 须完整列物化；Rust binding 增加 C 静态库/unsafe FFI 边界。

### 4.4 `TA-Lib` C v0.8.1 + `ta-lib-rs`(`ta-lib-wrapper` 0.2.0)——需全窗物化
- **① 做什么/维护** [OBSERVED]：TA-Lib C 活跃（v0.8.1 于 2026-09-12，1674 stars，push 2026-09-17）；Rust wrapper `CLevasseur/ta-lib-rust` 陈旧（crate `ta-lib-wrapper` 0.2.0，2019-10-08，73 stars）。
- **② 输入/输出** [OBSERVED]：C batch `TA_RSI/TA_EMA(startIdx,endIdx,const double inReal[], ..., outBegIdx, outNBElement, double outReal[])`——`outBegIdx/outNBElement` 报 warm-up、写调用方 `double outReal[]`（`include/ta_func.h:8609-8625,14218-14234`），并有 float 变体与 streaming `Update/Peek/Advance`。Rust wrapper 只是 bindgen + `cargo:rustc-link-lib=ta_lib`，示例用裸指针 + `unsafe set_len`。`const double*` 单列 SoA + i64 仍须拆列转换；**输出所有权兼容**。
- **③ SIMD/aarch64** [OBSERVED]：vector-backends 文档状态 "PROPOSED — investigation complete, implementation NOT STARTED"，标准 C scalar 是 fallback（`docs/vector-backends.md`）；EMA 是 SMA seed 后 `while`+`fma` 递归（`ta_EMA.c:143-165`），无 NEON/AVX/std::simd。
- **④ 递归/warm-up/gap** [OBSERVED]：EMA lookback `period-1+unstable`（先 period 个 SMA seed 再递推）、RSI lookback `period+unstable`（`ta_EMA.c:62-69`, `ta_RSI.c:67-75`）；RSI gain+loss 为零输出 `0.0`（`ta_RSI.c:190-207`）；header 文档非 finite `Update` 返回 `TA_BAD_PARAM`、`Advance` 对未喂 bar 保持上个 `Value`（carry-forward）。batch 无 validity mask。
- **⑤ 局限** [OBSERVED]：wrapper README "requires that you have installed the TA-lib C library."；vector 文档明确旧 Accelerate PR "不应原样合并"、Apple 优化未实现。
> **分级：需全窗物化**——caller output + lookback API 很强，但 `double*` 分列 + i64 违反直接借用门槛；`ta-lib-rs` 只薄暴露 unsafe FFI。

### 4.5 `polars` rolling/ewm（列式引擎替代路线）——需全窗物化
- **① 做什么/维护** [OBSERVED]：活跃 DataFrame/query engine（0.55.2，2026-08-06，39746 stars，push 2026-09-17），非行情递归指标专库。
- **② 输入/输出** [OBSERVED]：fixed rolling 收 `&Series`/`ChunkedArray`，先 `to_float()`/`rechunk()`，从 `values().as_slice()` 调 kernel，构造**新 `Series`**（`polars-time/.../rolling_window/dispatch.rs:17-69`）；no-null kernel `values:&[T]`，输出 `PrimitiveArray::from_trusted_len_iter` 新建（`polars-compute/.../no_nulls/mean.rs:6-41`）；`ewm_mean(s:&Series, options)->PolarsResult<Series>` 新建数组（`polars-ops/.../ewm.rs:12-54`）。须拆列 + 接受 rechunk/新 Series 所有权，**不能写既有 `&mut [T]`**。
- **③ SIMD/aarch64** [OBSERVED]：README 宣称 "multi-threaded, vectorized (SIMD)"；但 arrow `simd` feature 用 `std::simd` 且"requires the nightly channel"，本调查读到的 rolling/ewm kernel 无 `std::simd`/NEON 调用——不能把全局 SIMD 宣传当成 rolling/ewm 已有 NEON。
- **④ 递归/warm-up/gap** [OBSERVED]：rolling 用 `window_size/min_periods` + validity bitmap（不满足即 validity=false）；EWM `EWMOptions{adjust,bias,min_periods,ignore_nulls}`，null 输出 `None`。null validity ≠ IEEE NaN；无现成"gap 后 reset"系统语义。
- **⑤ 局限** [OBSERVED]：README 老 CPU 无 AVX2 需 `LTS_CPU=1`；显式 SIMD 需 nightly——CPU/feature/build 是部署变量。
> **分级：需全窗物化**——列式输入 + rechunk + 新 `Series` + null bitmap 与固定 AoS+caller buffer 不同；gap/NaN 语义需另定。作为"用 DataFrame 引擎当算法层"路线：可行但代价是完整列物化 + 所有权适配。

### 4.6 `ndarray` 0.17.2 + `ndarray-stats` 0.7.0——需薄封装（且指标需自写）
- **① 做什么/维护** [OBSERVED]：`ndarray` 通用 n 维容器（0.17.2，2026-01-10，4321 stars）；`ndarray-stats` 提供 quantile/summary/correlation/deviation（0.7.0，2025-12-29，237 stars）——**无 EMA/RSI/rolling 指标**。
- **② 输入/输出** [OBSERVED]：`ArrayView::from_shape` 支持 custom strides 的 `&[A]`，`from_shape_ptr` 可从裸指针造 stride view（列出完整 unsafe 生命周期/对齐/越界责任，`impl_views/constructors.rs:23-118`）；`as_slice/as_slice_mut` 仅 contiguous standard layout 才返回 slice。`SummaryStatisticsExt::mean` 收 `ArrayRef` 返回标量 `Result<A, EmptyInput>`；无 rolling/ewm/`_into` 指标输出。**这是最有机会保留 stride/零拷贝的数值底座**，但结果 buffer 与算法由调用方自写。
- **③ SIMD/aarch64** [OBSERVED]：加速开关是可选 BLAS（限矩阵乘）；ndarray/stats 无 `std::simd`/AVX/NEON/指标 intrinsic。
- **④ 递归/warm-up/gap** [OBSERVED]：两库无 EMA/RSI 递归态/warm-up/gap；`MaybeNan` 提供 `is_nan`/skip-NaN（非 Arrow validity mask）。
- **⑤ 局限** [OBSERVED]：stats 0.5.1 release note 记录过 "non-standard layouts" 导致越界的实现 bug——自定义 stride view 须保守验证 + 回归测试。
> **分级：需薄封装（且指标需自写）**——最可能保留 stride/零拷贝，但不是行情算法库：递归 op、i64 策略、validity/gap、caller output loop 全须自写。

---

## 5. AoS vs SoA（一手依据 + 三方案 + 对子系统三处影响）

**中立立场**：给整合者证据，不预设"必须改 SoA"。当前 AoS 是子系统已定契约。

### 5.1 一手机制依据 [OBSERVED]
- Intel Optimization Manual：传统 AoS 布局"不能充分利用 SIMD"（`intel-optimization-manual.txt:12589-12593`），SoA 是把每坐标放独立数组（`12611-12625`），两路：AoS 上直接算或运行时 swizzle 到 SoA（`12629-12631`）。AoS 每成员是 scalar 常对应 horizontal computation（`16502-16510`），SoA 同成员是数组易形成 homogeneous vertical computation（`16522-16526`），SSE 示例中 SoA vertical 效率更高（`16543-16546`）。
- **转置非零成本**：Intel 说 swizzle/deswizzle 是独立操作，示例用多条 `MOVHLPS/MOVLHPS/SHUFPS`+store 形成 SoA（`16618-16677`）；non-unit-stride 需 `PINSRD/PEXTRD`（`13758-13785`）。Arm Neon guide：交错数据逐次 `LD1`+mask/shift/recombine"不太可能高效"，`LD2/LD3/LD4` structure load 可加载时分离寄存器（`arm-coding-for-neon.txt:235-249,348-361`）；并**明确警告**预先重排整个数据集"通常代价高，流式输入甚至不可行"（`833-840`），permute 只有总性能确实提升才值得（`850-861`）。
- **cache 折中**：Intel——字段总一起访问且随机访问时 AoS 反而避免无谓 prefetch（`9751-9787`）；连续 sweep 且字段频率不均时 SoA 避免带入低频字段（`9788-9804`），代价是更多 memory streams / DRAM page miss（`9805-9808`）；给出 `hybrid_struct_of_array`（AoSoA，常同用字段打包成小 struct 数组）作折中（`9808-9819`）。
- **列式引擎理由**：Arrow 入门——列式对过滤/分组/聚合因 locality 更高效，连续内存利于 vectorization，每列由一或多个连续 Buffer 表示（`arrow-intro.html:606-637`）。DataFusion——`RecordBatch` 每列内部 contiguous Arrow array，projection 只读所需列、Parquet 只解码所需列（`datafusion-arrow.html:483-493`, `datafusion-explain.html:795-825`）。**但这证明"只扫少数列"是列式引擎核心收益，不证明 AoS 原地 stride kernel 一定慢多少。**
- **本机 toy 佐证** [OBSERVED]+[需实测]：`repr(C,align(64))` 64B 记录、N=1,048,576，SoA sum-of-two-fields（829/674/677µs）快于 AoS stride（990/1146/943µs），但 transpose 本身 2.6–3.7 ms（AoSSoASimd `runtime/aos_soa_probe.out`）；release 汇编 AoS 用 `ldp d..` 固定偏移取值，SoA 出现 `ldp q..`/`fmla.2d`，transpose 出现 `zip1/zip2`。toy 非生产窗口，不外推 50ms。

### 5.2 三方案对固定段契约（保持中立，不下最终结论）[INFERENCE]

| 维度 | A. 保持段内 AoS | B. 段内 SoA/AoSoA | C. 列即段 |
|---|---|---|---|
| `Sample<Slice<Layout>>` | 完全保持 | 若仍是 `Layout` 则只能 sidecar/派生缓存；改主段则改契约 | 改变契约最强 |
| 共享借用 | `&[Layout]` 经 zerocopy/bytemuck 直接借用，无全窗转换 | 仅生产者已写成同 AoSoA/sidecar 才零拷贝；即时转置违反门槛① | 每列独立借用，须新增多段长度/时间轴/一致性协议 |
| SIMD | 跨样本同字段需 stride/gather 或 NEON structure load；多字段同点访问自然 | 跨样本字段连续 vector load；AoSoA 块内折中 | 单列最适连续 SIMD；跨字段指标须并行取列 + row align |
| Arrow/NumPy 导出 | format 码/offset 准确描述一条 AoS record | 须导出每 field 真实 buffer/块宽/padding；`layout_hash` 须覆盖物理布局 | 各列近似 Arrow Array，但组合 schema/时间轴/缺失 bitmap 须定义 |
| gap/warm-up | 与布局无关，但 `Layout` 内缺失/validity 表示须被 op 读取 | 每列须共享同 gap 边界，否则错位 | 须跨列共同样本 ID/时间/gap bitmap |
| 直接契合 | **最契合固定契约**；性能靠真实访问模式验证 | 内部候选/sidecar，非从 AoS 每次转换的直接 op | 架构候选，须先获准改段抽象 |

### 5.3 对子系统三处的影响（洗入 / 契约表 / `Pooled` 输出 fold）[INFERENCE]
- **洗入（§0/§1.2）**：A 单记录写入最简单；B 须多流或 AoS+sidecar，增加容量/写入复杂度；C 须多列同步发布、任一列缺失破坏记录语义。**关键**：无论哪案，若价格存定点 i64 而算法层要 f64，i64→f64 是数值轴的独立根因；当布局已强制物化时它可融合进 gather pass（本机 fused 3 列=88.9µs，§7），无独立增量成本。
- **契约表（§4）与身份（§3.3/§3.4）**：A 的 Arrow format 码（`'l'`/`'tsn:'`/`'d:19,10'`/`'C'`）+ NumPy offsets 准确描述一条 record，但**它是布局描述不是"Arrow 列 buffer"证明，也不是一次转换许可证**——定点 i64 不能静默按 f64 读。B/C 要求 `layout_hash` 从"字段名/偏移"升级到覆盖真实物理布局（块宽、padding、lane 排布、validity 表示、定点尺度），否则同 hash 物理不同。
- **`Pooled` 输出类型 fold（§0.2/§5.3）**：A 每 op 读同一 `Layout` 窗口、输出区由调用方提供，字段带宽可能浪费；B fold 按列/块聚合、需输出物理布局与转换责任、已有 AoS op 需适配；C 从"记录流"变多 buffer 对齐/合并，`fn compute(windows: &[&[Input]])` 的 `Input` 须从单条 `Layout` 重建为列集合——属 ABI/模型变化，非库适配细节。

### 5.4 小结：布局冲突与数值冲突是两条独立成本轴
两者须分别计价、分别排名，不可混为一谈：
- **布局轴（AoS stride vs 连续列）**：现成库全要连续 `&[f64]`/`double*`/`Series`；AoS 段每次触发付适配成本。
- **数值轴（定点 i64 vs f64）**：VectorTA 及全部对照库价格类型是 f64；定点 i64 须整列 i64→f64 + 精度语义变化。**即便改成 SoA，只要仍存 i64 定点，此轴依旧冲突**。
- **成本可融合但根因不可合并**：当布局已强制物化时，i64→f64 缩放可融合进同一 gather pass（本机 fused 3 列 100k=88.9µs ≈ 转置-only 86.4µs，缩放几乎免费，§7）——因此两轴的**边界成本融合为一个 pass**；但它们是两个**可分别修复**的根因：段布局 与 价格数值表示，整改任一方不自动解决另一方（若改成 SoA-f64 段则两轴同时消除；若只改 SoA 仍存 i64 则数值轴仍在，只是它已无独立增量成本）。分项基准（转置 0.328/列、i64→f64 0.143/列）仅用于**解释**两条独立契约，不用于加总。

---

## 6. Rust SIMD 现状（2026-09 快照）

- **`std::simd`/`core::simd` 仍 nightly-only** [OBSERVED]：tracking issue `rust-lang/rust#86656` `state=open`（RFC 2948 Portable SIMD，页面更新 2026-08-12）；stable 文档标 "nightly-only experimental API"，明说 portable 抽象每 target 编译、缺硬件时可生成 scalar、不保证一操作一指令。本机 `rustc 1.95.0` 编译 `std::simd::f64x2` 失败 E0658 指向 #86656。因此原生 op 若要 stable toolchain，不能直接依赖 `std::simd`（这正是 VectorTA 把 `portable_simd` 锁在 `nightly-avx+nightly` 的原因）。
- **运行期分派库** [OBSERVED]（本机 aarch64 release 均编译通过；dispatch smoke `arch_dispatch=[6,12,18,24] wide_f64x2=[3,6]`）：
  - `pulp`（crates.io 0.22.3 / pinned git 0.23.0）：safe SIMD 抽象 + 运行时 feature 派发；AArch64 `f64s=f64x2`（`aarch64.rs:274-290`），`Arch::new()` NEON 可用选 NEON 否则 Scalar（`aarch64.rs:3409-3433`），x86 V4→V3→Scalar。`WithSimd` 直接收 `&mut [f64]`（`lib.rs:23-45`）。**最接近"稳定 Rust + runtime dispatch + caller slices"**。
  - `wide`（1.7.1）：portable 向量类型，AArch64 `f64x2` 内部 `float64x2_t`；**只 build-time 检测，README 明说 runtime detection/multiversion 不工作**——适合已知部署 CPU 的统一 aarch64 binary。
  - `simdeez`（3.0.1，Beta）：scalar/SSE/AVX/AVX-512/NEON/WASM 多宽度 + runtime/compile-time/manual 选择；AArch64 检测 NEON（`invoking.rs:189-223`），NEON `f64=float64x2_t`。
  - `multiversion`（0.9.0）：属性宏做函数多版本化 + 安全派发（`AtomicPtr` resolver 首次选定缓存），可 `targets("aarch64+neon")`；**是派发工具，不提供向量类型**——适合给手写 kernel 做 ISA variant，摆在完整窗口调用边界。
- **NEON vs AVX 统一成本** [OBSERVED]+[INFERENCE]：Arm NEON 寄存器 64/128-bit；上述库 AArch64 `f64` 均 `f64x2`。统一 API 非免费：须处理 lane 数、tail、水平归约、mask、gather/deinterleave、不同指令可用性。对递推 EMA/RSI 的 loop-carried dependency，SIMD 只能用于同窗独立通道/多 lane 部分运算，不能假定时间轴递推平行化——**这与 VectorTA 让 RSI 恒走标量一致**。
- **Apple M4 f64 宽度** [OBSERVED]：本机 sysctl `AdvSIMD=1`/`neon=1`/`FEAT_SME=1`/`FEAT_SME2=1`/`sme_max_svl_b=64`，`FEAT_SVE/SVE2` 返回 unknown oid；默认 target cfg 只有 `target_feature="neon"`，无默认 `sve/sme`。**普通 128-bit NEON 对 f64 = 2 lanes**（128/64）；lane 数不是每周期吞吐。SME 的 streaming SVE/矩阵语义另有上下文，`sme_max_svl_b=64` 不等于普通 NEON 的 8 个 f64 lane。若用 SVE/SME 须另测。
- **候选库横向对比**（版本以 crates.io 为准，另 pin git commit 见 §12）：

| 库 | crates.io / git | 派发模型 | aarch64 f64 | 是否提供向量类型 | 对本子系统适配 |
|---|---|---|---|---|---|
| `pulp` | 0.22.3 / git 0.23.0 | 运行时 feature 派发（NEON/Scalar；x86 V4→V3→Scalar） | `f64x2`（NEON） | 是（`WithSimd` 收 `&mut [f64]`） | **首选**：stable + runtime dispatch + caller slices；不做 AoS→SoA/指标语义/输出协议 |
| `wide` | 1.7.1 | **仅 build-time**（README 明说 runtime/multiversion 不工作） | `f64x2`=`float64x2_t` | 是 | 已知部署 CPU 的统一 aarch64 binary；不能异构 x86 运行时选择 |
| `simdeez` | 3.0.1（Beta） | runtime/compile-time/manual | `f64`=`float64x2_t` | 是（宏生成多 ISA 版本） | 探索用；Beta + 宽度差异需严格验收 |
| `multiversion` | 0.9.0 | 函数级 `AtomicPtr` resolver，可 `targets("aarch64+neon")` | 不适用 | 否（只派发，不提供向量类型） | 给手写 kernel 做 ISA variant，摆在完整窗口调用边界 |

---

## 7. 50 ms 预算下的容量估算

**公式**（[INFERENCE]，M4 scalar，release）：
```
T_total(N, C, K) = T_ipc + T_adapt(N, C) + Σ_k T_indicator(N, k) + T_output(N)
```
- `N`=窗口样本数，`C`=需物化的输入列数，`K`=指标集合，`T_ipc`=IPC/调度/借用 [需实测]。
- **`T_adapt` 是一个融合 pass**：从 strided i64 AoS 一次性 gather+scale+convert 到连续 f64 列——gather 与 i64→f64 缩放**融合在同一内存受限 pass 内**，不叠加。本机实测 **fused AoS-i64→SoA-f64 3 列 100k = 88.9 µs**（≈ 转置-only 86.4µs，缩放几乎免费）；naive 分项相加（转置 86.4 + 3×i64转换 13.9 = 128µs）**高估约 40µs**，本文不采分项相加。
- 单位成本（µs/1000 样本）：**融合物化 0.296/列**（=88.9/3/100）、RSI(14) **0.917**、ATR(14) **1.54**、BBands(20) **0.255**。分项基准（转置 0.328/列、i64→f64 0.143/列）**仅用于解释布局与数值是两条独立契约冲突**（§5.4），不用于加总成本。

**单条 (stream × indicator) 从 AoS+i64 融合物化的合计成本与 50 ms（=50000µs，未扣 IPC）内可容份数**：

| 场景 | N=1k | N=10k | N=100k | N=1M | 物化占比 |
|---|---|---|---|---|---|
| close→RSI (C=1) 合计 | 1.2µs | 12.1µs | 121.3µs | 1213µs | 24% |
| 　50ms 可容 | ~41000 | ~4100 | ~412 | ~41 | |
| HLC→ATR (C=3) 合计 | 2.4µs | 24.3µs | 242.9µs | 2429µs | 37% |
| 　50ms 可容 | ~20500 | ~2050 | ~205 | ~20 | |
| close→BBands (C=1) 合计 | 0.6µs | 5.5µs | 55.1µs | 551µs | 54% |
| 　50ms 可容 | ~90000 | ~9000 | ~906 | ~90 | |

**读法与告诫**：
- 融合物化占合计：RSI ~24%、ATR ~37%、BBands ~54%——**AoS/i64 融合适配不可忽略，与计算同量级**。这是"直接契合失败"的定量代价：改成能零拷贝借用 f64 SoA 段可省掉这部分（RSI 约 1/4、BBands 约一半）。
- 上表是 **kernel/adapter calibration，不是含 IPC 的端到端保证**；**未含 `T_ipc`**（iceoryx2 借用/触发/写回/调度）[需实测]。子系统 §1.3 称 50ms 下 IPC 层"不需关心"，但物化+计算已占可观预算，须在真实链路复核 IPC 是否仍可忽略。
- 计时方法：release、有 warm-up、200/500 iter 均值、消费输出防 DCE（`acc` 累加/guard 打印）；报告均值，未采分位数——正式验收须取分位数/最坏窗口。
- 数字是 **M4 标量**；不代表 x86 AVX（VectorTA 官方 rsi 100k=0.208ms 是 9950X，反而慢于我方 M4 标量 0.092ms，因 RSI 两处都标量、M4 单核更快）。
- 递归指标沿时间不可平行化，K 个指标近似线性叠加；窗口 N 与指标组合未定时，以"每 1000 样本 µs"和上表参数化上界为准，不擅自选生产规模。

**多指标同窗摊薄的工作示例**（[INFERENCE]，融合物化）：一条 OHLC 流窗口 N=100k，同时算 RSI+ATR+BBands，融合物化 OHLC 4 列一次（`4×0.296×100 ≈ 118µs`），计算 `91.7+154+25.5=271µs`；合计 ≈ 389µs（未含 IPC）。若每指标独立物化则物化按需列重复多次——同窗复用把物化从"每指标一次"降到"每窗一次"，是唯一在不改布局下压缩物化占比的杠杆。据此 50ms 内约可容 128 组"OHLC 窗口 × 3 指标"（未含 IPC，[需实测]）。

**参数化上界**（去掉 IPC，仅 CPU 侧）：单指标可容份数 ≈ `50000 / ( C·0.296·N_k + t_indic·N_k )`，`N_k` 为千样本数、`t_indic` 为该指标 µs/千样本。此式给出的是**乐观上界**：真实系统须叠加 `T_ipc`、并发调度抖动、cache 竞争，且以分位数/最坏窗口而非均值验收。

---

## 8. 横向表

分级：**直接可用** / **需薄封装** / **需全窗物化** / **语义不兼容**。所有"直接借用/输出/平台"列针对子系统固定 AoS+i64+完整窗口+调用方输出契约。

| 库 | ①做什么/维护 | ②输入形状 | ②输出所有权 | ③SIMD/aarch64 | ④递归/warm-up/gap | ⑤局限 | 分级 |
|---|---|---|---|---|---|---|---|
| **VectorTA 0.3.1** | 340 指标, 活跃(2026) | SoA 连续 `&[f64]`/SoA `Candles` | `Vec<f64>` + `_into(&mut[f64])`(340/340 指标文件) | x86 AVX only, **aarch64 全标量** | NaN 前缀 warm-up; NaN 非边界/validity, 递归态继续(无法表达 gap); 零分母=50 | f64 only, 无定点; nightly 才有 SIMD | **需全窗物化 + 数值/语义适配** |
| ta-rs 0.5.0 | ~20 指标, cadence 低 | `Next<T>` 逐值/`DataItem` | 调用方 loop 写出 | 无显式 SIMD | RSI 首点 0.1 防零除; 只有 Reset | 序列化无向后兼容承诺 | **需薄封装** |
| yata 0.7.0 | 大量指标, 中等活跃 | `Method` 逐值/`OHLCV` trait | `apply` 原地覆盖同型 slice | `unsafe_performance` 非 SIMD | EMA 初值即态; RSI 全零=0.5; 无 reset | benchmark 需 nightly | **需薄封装** |
| tulipindicators 0.9.2 + Uzaaft binding | 104 指标 C; binding 非官方 | `double*` 分列 SoA | 调用方预分配 `double*` | `-O2` 标量 | `_start` 报 warm-up; 平盘 RSI 无零除保护 | binding 未测 Windows | **需全窗物化** |
| TA-Lib 0.8.1 + ta-lib-rs 0.2.0 | C 活跃; wrapper 陈旧 | `const double[]` 分列 SoA | `double outReal[]` 调用方 | vector backend "NOT STARTED", 标量 | `outBegIdx` warm-up; RSI 零除=0; Advance carry-forward | wrapper 只裸 FFI, 需装 C 库 | **需全窗物化** |
| polars rolling/ewm 0.55 | 活跃引擎, 非指标专库 | `&Series`/列式 + rechunk | 新 `Series`(不可写既有) | 宣称 SIMD, rolling/ewm kernel 未见 NEON | validity bitmap; EWM null=None | null≠NaN; 显式 SIMD 需 nightly | **需全窗物化** |
| ndarray 0.17 + ndarray-stats 0.7 | 数值底座, 无指标 | `ArrayView` 支持 stride(unsafe) | 调用方自写 | 可选 BLAS(限矩阵乘), 无 SIMD | 无递归/gap; `MaybeNan` skip | 指标全须自写; stride view 曾有越界 bug | **需薄封装(指标自写)** |

---

## 9. 对子系统的可迁移命题

形式："库 X 在条件 Y 下用机制 Z 解决 W；子系统满足 Y 才可迁移。"

1. **VectorTA / TA-Lib / Tulip 的 `_into`/预分配输出**：库 X 在"调用方持有连续等长输出缓冲"条件下，用 `xxx_into(&mut [f64])` / `double outReal[]` 机制实现零额外分配写回，解决"输出所有权与 op ABI 对齐"。**子系统满足**（§6.3 `loan_slice_uninit` 得到可写输出段）**才可迁移**——但要求输出段是连续 f64 且不带 stride。[OBSERVED]

2. **VectorTA 全窗 batch API**：库在"每次给完整连续窗口"条件下，用一次性 batch 调用（非 stream 增量）机制契合子系统 §5"每段一次性写满即发"，解决"每次触发看完整窗口、不必渐进"。子系统满足（触发即传完整窗口段）才可迁移；**但前提仍是输入为连续 f64 列**。[OBSERVED]+[INFERENCE]

3. **pulp 的运行时 NEON 派发**：`pulp` 在"stable Rust + 单一 aarch64/x86 二进制需运行时选后端"条件下，用 `Arch::new().dispatch` + `WithSimd(&mut [f64])` 机制解决"跨平台 SIMD 且写调用方 slice"。子系统满足"原生 op 自持算法、按访问模式选 kernel"才可迁移——它**不做** AoS→SoA、不定义指标语义、不提供 output 协议，这些仍属 op wrapper。[OBSERVED]

4. **multiversion 的函数级 ISA variant**：在"已有手写 scalar/NEON kernel、想在调用边界安全选版本"条件下，用属性宏 + `AtomicPtr` resolver 机制解决"ISA 多版本化"。子系统满足"kernel 由 typed SDK 作者手写、dispatch 摆在完整窗口边界"才可迁移。[OBSERVED]

5. **ndarray 的 stride view**：`ndarray` 在"愿写严格验证的 unsafe pointer+stride 构造 + 自实现递归指标"条件下，用 `ArrayView::from_shape_ptr(custom strides)` 机制在**不转置**下读 AoS 字段，解决"保持 AoS 段的直接借用"。子系统满足（方案 A + 自写指标 + 回归测试覆盖越界）才可迁移——代价是放弃现成指标库，得到零物化。[OBSERVED]+[INFERENCE]

6. **Arrow/DataFusion 的列式 + projection pushdown**：列式引擎在"负载以跨样本单列扫描/过滤/聚合为主"条件下，用连续列 buffer + 只解码所需列机制获得 locality/SIMD/带宽收益。子系统满足（方案 C 列即段 + 指标确以单列扫描为主）才可迁移——但这是**改段 ABI 的架构决策**，非算法层库适配。[OBSERVED]+[INFERENCE]

7. **TA-Lib 的 carry-forward gap 语义**：TA-Lib 在"stream `Advance` 未喂 bar 时保持上个 `Value`"条件下提供一种确定的 gap 处理。子系统**当前不可迁移**——因为这会替系统选定"gap=保持"，而子系统要求"gap 显式、不伪造连续"且语义未定；须先由 UTA 定义 gap 后 reset/保持/缺失，再验证等价。[OBSERVED]+[需实测]

---

## 10. 对子系统文档的修正建议（逐段对应）

每条对应 `hpc-derivation-subsystem.md` 现有段落，标 **支持 / 需改 / 冲突** 与依据。

1. **§1.3 / §3.2 "洗入按 SIMD 通道宽度对齐（默认 64 字节覆盖 AVX-512/NEON）"** —— **需改**。依据 [OBSERVED]：64B 对齐对 x86 AVX-512（64B 向量）成立，但 **aarch64 NEON 向量是 16B（128-bit，f64=2 lane）**；Apple M4 无 AVX、默认无 SVE/SME。64B 对齐在 M4 上是 cache-line 对齐（合理）而非"向量宽度对齐"。建议措辞改为"按 cache line 与目标平台向量宽度的较大者对齐（x86 AVX-512=64B；aarch64 NEON=16B，64B 亦满足）",避免暗示 M4 有 64B 向量。

2. **§1.3 / §6.3 "向量化优先……作者面对对齐定长数组"** —— **需改（收窄预期）**。依据 [OBSERVED]：现成算法库在 aarch64 上普遍**无显式 SIMD**（VectorTA aarch64 全标量；ta/yata/tulip/TA-Lib 标量；polars rolling/ewm kernel 未见 NEON）；`std::simd` 仍 nightly-only。M4 上的向量化只能靠 (a) `pulp`/`wide`/`simdeez`/`multiversion` 手写 NEON kernel，或 (b) LLVM 对 scalar loop 自动向量化。且**递归指标（EMA/RSI/ATR）loop-carried 依赖使 SIMD 收益有限**（VectorTA 让 RSI 即便在 AVX 也走标量）。建议补一句"向量化收益强依赖指标类型（窗口/归约类可向量化，递归类受限）与平台（aarch64 需自写 NEON 或依赖 autovec）"。

3. **§1.2 / §2 "定长逐秒记录（AoS）是自然形状" + `Layout` 为 AoS 记录** —— **冲突（与现成算法库），布局裁决需显式记录**。依据 [OBSERVED]：所有成熟算法库输入都是 **SoA 连续列**（`&[f64]`/`double*`/`Series`），无一接受 AoS stride 视图。AoS 段用现成库必须每次触发融合 gather/scale/convert（本机 AoS-i64→SoA-f64 3 列 100k=88.9µs，与计算同量级），违反 §1.4"zero-copy……不再搬窗口"。建议：明确记录"若原生 op 采用现成 SoA 算法库，AoS 段须付每触发融合适配成本；保持 AoS 则须自写 stride/NEON kernel（方案 A）"；把方案 A/B/C（§5.2）作为一个显式待裁决点，**不预设改 SoA**。

4. **§1.2 / §3.3 "值与 schema 分离……十进制字符串 vs 定点" + Arrow `'d:19,10'` 定点导出** —— **冲突（与现成库）**。依据 [OBSERVED]：VectorTA 及全部对照库价格类型是 **f64**，无定点 i64 入口。若洗入按 §3.3 存定点 i64，用任何现成库都须整列 i64→f64 转换（本机 14µs/列/100k），且**丢失定点精度语义**。这与布局冲突**独立**：即便改 SoA，i64 定点仍不兼容 f64 算法层。建议明确"若价格存定点 i64，算法层选型须么支持定点、么接受 i64→f64 转换成本 + 精度语义变化",并把它列为独立于 AoS/SoA 的裁决轴。

5. **§5 / §6.3 输出段 = `loan_slice_uninit` 写满即发** —— **支持**。依据 [OBSERVED]：VectorTA/TA-Lib/Tulip 的 `_into`/预分配 `double[]` 输出与"调用方提供输出段、一次写满"完全对齐（本机 `_into` 零额外分配）。唯一约束：输出段须**连续、等长、f64、无 stride**。建议补注"输出段布局须满足算法库的连续等长 f64 要求;多输出指标（如 Bollinger 三带）产生多列，需相应多段或多列布局"。

6. **§1.3 "默认内部所有计算在 50 ms 内产生……这一层不需关心 IPC"** —— **需改（补测量前提）**。依据 [OBSERVED]+[需实测]：本机测得"融合物化+计算"对 100k 窗口/单指标为 55–243µs（融合物化占 24–54%）；50ms 内单指标 100k 约可容 205–906 份（kernel/adapter calibration，未含 IPC）。IPC 是否仍可忽略[需实测]。建议改为"50ms 预算须按 `T_ipc + 融合适配 + 计算 + 写回` 分项核算（§fp-09 §7 给 M4 参数化上界）；IPC 可忽略性须在真实 iceoryx2 链路复核，不默认成立"。

7. **§0.2 / §5.3 "`Pooled` 节点的输出类型即段布局，导出给原生计算作者" + 五个 fold** —— **支持（fold 语义），但需补物理布局条件**。依据 [OBSERVED]+[INFERENCE]："输出类型 fold → 段布局 → 导出"的机制成立且与库无关。但若未来采方案 B/C，`layout_hash`/Arrow format 码须从"字段名+偏移"升级到覆盖**真实物理布局**（块宽、padding、lane 排布、validity 表示、定点尺度）——否则同 hash 物理不同（§5.3）。建议在 §3.4 补注"layout_hash 须覆盖物理布局全量;若采 SoA/AoSoA/列段,字段名+偏移不足以唯一标定"。

8. **§7 "作者写 `fn compute(windows: &[&[Input]]) -> impl Iterator<Item=Output>`"** —— **需改（与现成库形状不符）**。依据 [OBSERVED]：现成库入口是 `xxx_into(&mut [f64])`（不是 iterator）或返回 `Vec`/`Series`。`impl Iterator<Item=Output>` 若逐条产出会阻碍库的批量列式路径。建议把 ABI 描述为"批量：输入若干只读列窗口 + 调用方输出段;返回写入的段长/warm-up 边界",与 `_into` 家族对齐，而非逐条 iterator。

---

## 11. 未覆盖 / 待实测

- [需实测] **端到端 50 ms**：iceoryx2 IPC/借用/触发/写回/调度未纳入计时；只测了 CPU 侧物化+计算。
- [需实测] **多指标共享窗口的物化摊薄**：同窗 K 指标只需一次转置，实际收益未测。
- [需实测] **LLVM autovec 对 M4 scalar loop**：未逐指标看生成汇编/性能；VectorTA scalar 是否被 LLVM 部分向量化未查。
- [需实测] **指标数值逐向量对照**：EMA seed / RSI 零分母 / NaN 传播须用小型可手算序列（含常数、零分母、中间 gap）逐指标对照 UTA 参考定义——本调查只覆盖 SMA/EMA/RSI/ATR/BBands 少数。
- [需实测] **gap 后递归语义**：属系统契约缺口，非库可回答；须 UTA 先定义 reset/保持/缺失。
- [需实测] **SVE/SME**：M4 有 SME/SME2 但无普通 SVE；未评估其对 f64 密集计算的可用性。
- [未覆盖] **VectorTA CUDA 实机**：本机无 NVIDIA 设备；PTX/host-device 只读源码，未运行。
- [未覆盖] **对照库完整性能赛**：TA-Lib/polars/ndarray 未做 release 端到端 bench（只 ta/yata/Tulip 本机 smoke）。
- [未覆盖] **VectorTA batch/param-sweep 与全部 340 指标的分配审计**：只对 SMA/EMA/RSI/ATR/BBands single 路径做了 allocator 计数；batch sweep、多输出、派生输入的 scratch 分配未逐一审计。
- [未覆盖] **`ta`/`yata` 的 `Layout: trait` 逐记录借读实测**：仅读源码确认 trait 形状，未在 M4 实际用 AoS `repr(C)` 记录驱动其 `Next`/`OHLCV` 跑一遍。

---

## 12. 来源清单

格式：编号 · URL · 打开状态 · 本地路径/commit 或 version。

**VectorTA（我方 + VectorTAProbe 双路核验）**
1. `https://crates.io/api/v1/crates/vector-ta/0.3.1/download` · 成功（需 UA，无 UA 首次 403） · tarball SHA-256 `b530eeccbf2577e6c5e583f85a9e5f7b75042379d77936bdf5eed2116210af4f`；`/tmp/hpc-verify/vector-ta-0.3.1`、`/tmp/hpc-research-vectorta/vector-ta-0.3.1`；git provenance `802518e2392c5d011744b75e56e108e97a0682b4`
2. `https://crates.io/api/v1/crates/vector-ta` · 成功 · metadata（default_version 0.3.1, downloads 820, updated 2026-07-18）
3. `https://raw.githubusercontent.com/VectorAlpha-dev/VectorTA/802518e2.../README.md` · 成功 · 340 indicators, Tulip benchmark 表, CUDA VRAM-resident 原话
4. `https://docs.rs/crate/vector-ta/0.3.1` · 成功 · `/tmp/hpc-research-vectorta/sources/docsrs-0.3.1.{html,txt}`
5. `https://vectoralpha.dev/projects/ta/benchmarks/` · 成功 · benchmark 页
6–8. `https://api.github.com/repos/VectorAlpha-dev/VectorTA/issues/{57,58,59}` · 成功 · CUDA 13 `<cfloat>` / `CUDA_ARCH=120` 误规整（环境标 0.2.9）

**对照库（CompareLibs，均成功 clone/curl）**
9. ta-rs · `github.com/greyblake/ta-rs` commit `ecd65d85def675e578ddd3c7187fff2b6fad28fc` · crate 0.5.0（2021-06-26）·（`/releases/latest` 返回 404，日期以 crates.io 为准）
10. yata · `github.com/amv-dev/yata` commit `e331b48243f32b3817c6f97240c6be0188b90c6a` · crate 0.7.0（2024-03-07）
11. tulipindicators · `github.com/TulipCharts/tulipindicators` commit `be18abb13e075ba866898dcc7cb52399603302a6` · v0.9.1
12. Uzaaft/tulip-rs · commit `917919c50184536da61dfbf4ab87d7b073bb9d54` · 非官方 binding, 2 stars
13. TA-Lib · `github.com/TA-Lib/ta-lib` commit `8b94ee7ff9dbc80b94d62e57e6b2683a8420adb6` · v0.8.1
14. ta-lib-rust · `github.com/CLevasseur/ta-lib-rust` commit `133c10b20416662c947bd8cbe410b8c2eaed526b` · crate `ta-lib-wrapper` 0.2.0
15. polars · `github.com/pola-rs/polars` commit `3c771bd067f5852eebd731cd6c4624f6d41fa267` · crate 0.55.2
16. ndarray · `github.com/rust-ndarray/ndarray` commit `bd3ade99c1f6d1fbd0f31866153e2d155e7b75ab` · 0.17.2
17. ndarray-stats · `github.com/rust-ndarray/ndarray-stats` commit `67d0292f483c90b8afa0c975832d979d24204e95` · 0.7.0

**AoS/SoA + SIMD（AoSSoASimd，均成功）**
18. Intel 64/IA-32 Optimization Manual · `intel.com/.../64-ia-32-architectures-optimization-manual.pdf` · 成功 · `intel-optimization-manual.txt`
19. Arm Coding for Neon Guide (Issue 04/2020) · `developer.arm.com/.../102159_0104_01_CodingForNeon.pdf` · 成功 · `arm-coding-for-neon.txt`
20. Apache Arrow Intro/Columnar · `arrow.apache.org/docs/format/{Intro,Columnar}.html` · 成功
21. DataFusion Arrow/Explain · `datafusion.apache.org/user-guide/{arrow-introduction,explain-usage}.html` · 成功
22. Rust portable SIMD tracking #86656 · `github.com/rust-lang/rust/issues/86656` · 成功 · state=open（2026-08-12）
23. Rust `std::simd` docs (stable/nightly) · `doc.rust-lang.org/{stable,nightly}/std/simd/index.html` · 成功
24. portable-simd repo · `github.com/rust-lang/portable-simd` commit `3d6357800395769aed21838cb1c27d71bf5b78a5`
25. pulp · `github.com/sarah-quinones/pulp` commit `601de0c071ba32df3fd7ff33fc4218c9bd712aba`（git 0.23.0 / crates.io 0.22.3）
26. wide · `github.com/Lokathor/wide` commit `a0519073776eb89579b8e12c6c7689df13f93c53` · 1.7.1
27. simdeez · `github.com/arduano/simdeez` commit `ac5ac28fa461ff57df5d816075f96f6cf82e28c6` · 3.0.1
28. multiversion · `github.com/calebzulawski/multiversion` commit `357638046dd3834815b34df80403d1050c30e1c7` · 0.9.0

**本机运行证据（无外部 URL）**
29. 我方 harness `/tmp/hpc-verify/harness`（RSI/ATR/BBands 计时、AoS→SoA 转置、i64→f64 转换、gap/warm-up 观测）· rustc 1.95.0 aarch64-apple-darwin release
30. VectorTAProbe harness `/tmp/hpc-research-vectorta/harness`（default + nightly-avx，backend=Scalar/ScalarBatch，allocator 计数）
31. CompareLibs smoke `/tmp/hpc-research-CompareLibs/smoke`（ta/yata Rust）+ Tulip C smoke（Apple clang 17 arm64 -O2）
32. AoSSoASimd `/tmp/hpc-research-AoSSoASimd/runtime`（4 库 release build、dispatch smoke、AoS/SoA toy 汇编与计时、Apple sysctl/target-feature probe、stable std::simd 失败）

**只读设计来源**：`hpc-derivation-subsystem.md`、`uta-core-design.md` §0.2/§5/§5.3。
