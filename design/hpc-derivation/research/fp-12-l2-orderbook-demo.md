# fp-12：L2 订单簿指标 demo——原语层 vs 手写 vs 零优化，兼答多档字段布局

状态：一手实测，闭合。**定位**：`hpc-derivation-subsystem.md` §8.1（原语中间层，decision-log F7）与 §3（列式段布局）的第二个实证，配 fp-11。fp-11 只测了时间轴指标（SuperTrend/Ichimoku/Squeeze，全是 rolling/ewm）；本文换成 **L2 订单簿指标**——引入一条 fp-11 没有的 **档轴（level-axis）归约**维度，并回答"定长多档字段在列式段里怎么摆"。全部 Rust、Apple M4 aarch64、stable rustc 1.95.0、release `opt-level=3, lto=true, codegen-units=1, panic=abort`；NEON 首要，不用 `std::simd`；产物落 `/Users/mouriya/Ext/tmp/fp12/`，不触本仓库构建。复用 fp-11 原语 crate（拷贝到 fp12 下，未改一行）、计时/allocator/golden 断言口径。标注 **[OBSERVED]**（本机亲测/汇编逐行看到）/ **[INFERENCE]**（据源码/结果推断）。

---

## 1. 摘要

**L2 指标上，原语层相对手写不再省 LOC，多档布局 A/B 没有单一赢家。** 具体 [OBSERVED]：

- **原语层的 LOC 优势在 L2 消失**：原语集 404 行 ≈ ndarray 手写 389 行（原语反而略多），而 fp-11 是降 57–73%。根因单一——原语词汇**只覆盖时间轴**（rolling/ewm/shift/linreg），本 demo 的**档轴归约**（OBI 跨档、累计深度剖面）两种写法都得手写 `ndarray` Zip/逐行 fold，于是原语版在这些指标上**塌缩成 ndarray 版**（OBI-A 两版源码几乎逐字相同）。原语层真正省 LOC 的只剩 z-score 这类纯时间轴 rolling 指标（3 行原语 vs ndarray 手写增量窗口）。
- **多档布局 A/B 取决于输出形状，不是一个通用答案**：归约成"每快照一个标量"的指标（OBI(k)）——布局 A（4N 条列式流、跨快照垂直 SIMD）在向量化写法下更自然更快（prims/ndarray obi_a ≈1.2 ms vs obi_b ≈1.9 ms）；输出本身是"每档一条剖面"的指标（累计深度 n×N）——布局 B（行主序 `n×N`）对所有写法快 **2–5×**（depth-A 11–15 ms vs depth-B 2.8–5.5 ms）。**朴素逐元素索引下结论反转**：naive 的布局 A 因 `Vec<Vec<f64>>` 双重间接比布局 B 慢 5×。
- **时间轴 rolling/递推指标**：原语 / ndarray / pulp 三者 O(n) 增量后互相 0.4–2×，零优化 O(n·w) 慢 **20–60×**（OFI 35 ms、z-score 30 ms、slope 92 ms）。递推（OFI e_t）与所有 rolling 全是设计使然的标量，只有 elementwise 与布局 A 的档轴列归约被 LLVM autovec 成 NEON `.2d`。
- **正确性**：四写法 × 全部指标逐点过 golden（独立标量 oracle，1e-9），validity/NaN 位对齐；计时区分配除 naive slope (2,0) 外全 (0,0)。oracle 另经 numpy 全数组一次性核对（12 条数组，1e-9）。[OBSERVED]

---

## 2. 数据与 golden

**数据**：LOBSTER 官方免费样本 AMZN 2012-06-21 level-10（维护者裁定用 LOBSTER，不录 Binance；官网 DataSamples 页现已改为账号/审批+付费流程，原直链样本已下线，本样本经 `kpetridis24/lobsim` 仓库 Git LFS 取得，与 `lobsterdata/mat4lobster` README 列出的同名文件一致）。[OBSERVED]

| 文件 | 字节 | SHA-256 | 行数 |
|---|---|---|---|
| `AMZN_..._orderbook_10.csv` | 63,216,242 | `decfe952b3fa06c9922dc8fba763a37a0fb032d4231767997e4abf2aa0d29b50` | 269,748 |
| `AMZN_..._message_10.csv` | 11,248,750 | `c85a75b51f3616a683f825c2e03f984535e9329a8be5ab12325130d32223a2c5` | 269,748 |

- 时间范围 34200.017–57599.959 s（09:30:00.017–16:00:00.959）；10 档；orderbook 每行 40 列，列序 `ask_px_1, ask_sz_1, bid_px_1, bid_sz_1, …`；价格是 1e-4 美元整数。**价格 /10000 转 f64 就是"洗入到 f64"（§3.5）**——一次拷贝里完成，不落每次触发。[OBSERVED]
- orderbook 第 i 行 = 第 i 条 message 之后的簿状态，两文件 1:1 对齐（各 269,748 行）。message 类型分布：新增限价 131,954、全撤 123,458、部分撤 2,917、可见成交 8,974、隐藏成交 2,445。**message 文件用于确认事件对齐与时间轴；OFI 指标本身由连续 best bid/ask 的 price/size 快照差分算出，不使用 message 的 direction/size**（见 §3 指标 3）。[OBSERVED]

**golden**：独立"直白标量 oracle"（`oracle.rs`，逐点/逐窗朴素重算，不复用任何原语或代数递推）算全部 12 条输出数组，四写法逐点断言 `|o−g| ≤ 1e-9 + 1e-9·|o|` 且 NaN 位逐位相等。oracle 再对 numpy 全数组一次性核对，全过 1e-9 [OBSERVED]：

| 数组 | max\|abs\| oracle vs numpy | 数组 | max\|abs\| |
|---|---|---|---|
| obi1/obi5/obi10 | 0 | dws | 4.4e-16 |
| microprice | 0 | cumbid/cumask (n×10) | 0 |
| e | 0 | z-score | 2.8e-11 |
| ofi50/ofi200 | 0 | slope | 3.3e-19 |

> **z-score 的数值敏感性**（写进契约、四写法都遵守）[OBSERVED]：OBI5 有大段近常值窗口（最坏窗口仅 3 个不同值，std≈0.0098，\|z\|≈13），一次方差 `Σx²/w − mean²` 在此处灾难性抵消。**用全局 270k prefix cumsum 差分会丢 ~6e-8、过不了 1e-9**；改为每窗 fresh 求和或补偿（Kahan/Neumaier）滑动即回到 ~1e-11。oracle 用每窗 fresh，四写法各用增量补偿滑动，全过 1e-9。

**golden 采样值**（idx 100/1000/50000/269747）[OBSERVED]：`obi1` 0.5745/0/0/0.4269；`microprice` 224.1357/224.265/224.145/220.6028；`e` 100/0/0/100；`ofi50` 1551/727/−233/−151；`dws` 5.8650/1.6785/0.3417/0.3251；`zscore` NaN/−0.1957/0.7287/−0.0790；`slope` NaN/3.559e-4/7.726e-5/3.023e-5。

---

## 3. 指标定义（每个给公式 + 来源；n=269748 快照，N=10 档，0-based）

`ap/asz/bp/bsz` = 各档 ask 价/量、bid 价/量（f64，价已 /10000）。

1. **OBI(k)**（多档不平衡），k∈{1,5,10}——**跨档归约**。请求的是 k=1,5,20；20>N=10，第三个截到 10（记录）。
   `OBI_k[t] = (Σ_{i<k} bsz_i − Σ_{i<k} asz_i)/(Σ_{i<k} bsz_i + Σ_{i<k} asz_i)`，全 t 有效。
2. **microprice**（level-0）——**elementwise**。`mp[t] = (ap_0·bsz_0 + bp_0·asz_0)/(bsz_0+asz_0)`，全 t 有效。来源：Gatheral–Oomen microprice。
3. **OFI**（Cont, Kukanov, Stoikov 2014，`https://arxiv.org/abs/1011.6402`）——**跨时间递推 + rolling**。best 档逐快照
   `e[t] = bsz_t·[bp_t≥bp_{t-1}] − bsz_{t-1}·[bp_t≤bp_{t-1}] − asz_t·[ap_t≤ap_{t-1}] + asz_{t-1}·[ap_t≥ap_{t-1}]`（t≥1，`[·]`为0/1指示；e[0]=NaN），再滑动和 `ofi_w[t]=Σ_{j=t-w+1..t} e[j]`，w=50/200（首个有效在 t=w）。
4. **深度加权 spread + 累计深度剖面**——**跨档 cumsum**。`dws[t] = Σ_i (ap_i−bp_i)(asz_i+bsz_i)/Σ_i (asz_i+bsz_i)`（全档，size 加权平均档内价差）；累计剖面 `cumbid[t,k]=Σ_{i≤k} bsz_i`、`cumask` 同（输出 n×N，行主序）。
5. **队列不平衡 z-score**——**rolling 统计**。`z[t]=(OBI5[t]−rollmean(OBI5,200))/rollstd(OBI5,200,ddof=0)`，首个有效在 t=199。
6. **Kyle/Hasbrouck rolling 回归斜率**（窗口 500）——**rolling 回归**。`mid=(ap_0+bp_0)/2`、`dp[t]=mid_t−mid_{t-1}`；`slope[t]=(Σe·dp − ΣeΣdp/w)/(Σe² − (Σe)²/w)`，j∈[t-499,t]，分母 0 → NaN，首个有效 t=500。Kyle λ 型价格冲击。

**布局 A / B**：只有 OBI 与 depth 有 A/B 两版（其余读 level-0，布局无关）。A = 每 (side,level,field) 一条连续 `Vec<f64>`，共 4N=40 列（Arrow fixed-size-list 展平）；B = 每字段一条 `(n×N)` 行主序 `Array2`，沿 `Axis(1)` 归约。**契约禁止在计时区 `sum_axis`**（它分配），布局 B 用逐行 lane 迭代 fold 进预分配输出。

---

## 4. 主表：四写法 × 指标（µs/call，中位数，同机同数据同次运行，7 pass × (20 warmup+300 iter)）

分配全 (0,0)，除 naive slope (2,0)。OBI/depth 取布局 A。[OBSERVED]

| 指标（写法归类） | 零优化 naive | 原语集 prims | ndarray 手写 | pulp 手写 NEON |
|---|---:|---:|---:|---:|
| **OBI(1,5,10)** 跨档归约 | 4100.1 | 1241.9 | **1202.2** | 2385.7 |
| **microprice** elementwise | 338.1 | 93.6 | **91.4** | 92.2 |
| **OFI(e,50,200)** 递推+rolling | 34984.4 | 1428.6 | **788.0** | 1043.3 |
| **depth+累计剖面** 跨档 cumsum | 12536.0 | 15033.3 | 14953.3 | **10988.0** |
| **z-score(200)** rolling 统计 | 29940.8 | 1588.6 | **1259.0** | 1261.2 |
| **slope(500)** rolling 回归 | 92149.2 | 3110.4 | 1494.1 | **642.9** |
| **作者 LOC（fp-11 口径）** | **209** | 404 | 389 | 466 |

**每格写法**（Main 要求标注）[OBSERVED]：
- OBI-A：naive 三段标量重求和；prims/ndarray `ndarray` Zip 逐列累加 + 比值 helper（**两者几乎同码**）；pulp 显式 `WithSimd`（`add/sub/div_f64s` + head/tail）。
- microprice：naive 标量；prims 原语 `ewise4`；ndarray `Zip`；pulp `WithSimd`。
- OFI：四写法的 e_t 都是标量前一快照递推（逃逸口）；rolling 和——naive O(n·w) 每窗重算，prims 原语 `rolling.sum`，ndarray/pulp 手写 O(n) 滑动。
- z-score：naive O(n·w) 每窗；prims `rolling.mean`+`rolling.std(0)`+`ewise2`（3 行）；ndarray/pulp 手写补偿滑动 sum/sum_sq。
- slope：naive O(n·w)；prims 4 次 `rolling.sum` 物化 4 条 temp 列 + `ewise4`；ndarray/pulp 融合滑动回归（pulp 用 SIMD 累加 → 最快 642.9）。

**读法** [OBSERVED]：
- **时间轴 rolling/递推指标（OFI、z-score、slope）**：三种优化写法都是 O(n)，互相 0.4–2×；零优化 O(n·w) 慢 20–60×——**价值在 O(n) 算法，不在 SIMD**（印证 fp-11/E11）。prims slope 3110 慢于 ndarray/pulp，是"逐原语物化中间列"的代价（4 条 rolling temp 列，fp-11 未实测的一项，这里量到 ~2× pulp）。
- **elementwise（microprice）**：三优化写法 ~92 µs 齐平（全 autovec 成 NEON），naive 338。
- **跨档指标（OBI、depth）**：写法差异被**布局差异**盖过（见 §5），depth 甚至 pulp 显式 SIMD 也没赢过它自己的布局 B。

---

## 5. 布局 A vs B 对比

µs/call 中位数，同次运行 [OBSERVED]：

| 写法 | OBI-A(40列) | OBI-B(n×N) | depth-A(40列) | depth-B(n×N) |
|---|---:|---:|---:|---:|
| naive | 4100.1 | **823.2** | 12536.0 | **2772.8** |
| prims | **1241.9** | 1929.3 | 15033.3 | **2787.3** |
| ndarray | **1202.2** | 1872.9 | 14953.3 | **5538.2** |
| pulp | 2385.7 | **1967.1** | 10988.0 | **2912.2** |

**结论：没有一个通用赢家，看两件事——归约输出形状 + 访问写法** [OBSERVED]：

1. **OBI（档轴归约成每快照一个标量）**：
   - **向量化写法（prims/ndarray）下 A 赢**（1.2 ms vs 1.9 ms）：布局 A 每档一条连续列，`Zip` 逐列累加是**跨快照垂直 SIMD**（`fadd.2d`，§7），把 40 个列指针各只加载一次、内层完全向量化；布局 B 每行做**档内横向 fold**（长度仅 N=10，标量、无法向量化）。
   - **朴素逐元素索引（naive）下 B 反赢 5×**（823 vs 4100）：布局 A 是 `Vec<Vec<f64>>`，`a_bsz[lev][t]` 每档一次指针追逐（双重间接）；布局 B 是 `Array2` 行主序，一行连续。**同一布局，向量化能不能吃到 A 的连续性，决定 A 是最快还是最慢**。
2. **depth 累计剖面（输出本身是每档一条 n×N 剖面）**：**布局 B 对所有写法快 2–5×**（2.8–5.5 ms vs 11–15 ms）。因为累计写 `cumbid[t,0..N]` 是沿档递推、逐行连续，布局 B 的输入行与输出行都行主序对齐；布局 A 要把每档结果散写回 40 条列，读写都跨 cache line。pulp 显式 SIMD 也救不了布局 A 的散写（11 ms）。

即：**§3.0 "多字段同点访问退化为多次加载"在 L2 上被量化**——凡"一次快照要同时看多档/输出多档"，行主序（B）显著更好；凡"跨档归约成单标量、且能垂直向量化"，列式（A）更好。

---

## 6. 开发体验

### 6.1 作者 LOC（fp-11 口径：非空/非注释/非属性行；原语 crate 不计入）[OBSERVED]

naive **209** · prims **404** · ndarray **389** · pulp **466**。原语 crate `uta-prims/lib.rs` 428 行（未改，可复用底座，摊销不计作者）。

**关键发现——原语层在 L2 不省 LOC**：prims 404 ≈ ndarray 389（原语反而多 15 行）。对照 fp-11 的降 57–73%，差别只在**指标结构**：fp-11 三指标全是时间轴 rolling，原语词汇完整；L2 的 OBI/depth 是档轴归约，**原语词汇里没有档轴算子**，prims 只能用 `ndarray` Zip/逐行 fold 手写这部分——与 ndarray 版**同码**。原语真正省行的只有纯时间轴的 z-score（prims 3 行 `rolling.mean/std+ewise2` vs ndarray 手写补偿滑动约 20 行）。**原语层的 LOC 收益是"时间轴指标占比"的函数。**

### 6.2 逃逸口清单 [OBSERVED]

| 写法 | 逃逸口 | 性质 |
|---|---|---|
| naive | 全部指标（设计如此，Scratch 空 struct）；唯一分配是 slope 的 mid/dp 两条中间 `Vec` | 基线 |
| prims | ① OFI e_t 前一快照递推（无数组原语）；② OBI 跨档归约（原语无档轴算子→ndarray Zip）；③ depth 加权/累计剖面（原语 cumsum 只沿时间轴→逐档 fold）；**z-score 无标量逃逸**（`rolling.mean/std` 补偿滑动直接过 1e-9） | 档轴与递推 |
| ndarray | OFI e_t 递推标量；各 rolling 手写增量滑动；跨档在 `rows()` 内标量 fold | 递推+rolling 手写 |
| pulp | OFI e_t / 所有 rolling 和 / z-score / slope 回归全部 loop-carried 标量；SIMD 仅用于 microprice、OBI、depth 的 dws | 递推标量强制 |

**共性**：**OFI e_t 的跨时间递推在四种写法都是标量逃逸口**（印证 fp-11 命题 8 / §8.1"递归标量"）。原语层新增的、fp-11 没有的逃逸口是**档轴归约**——不是递推限制，是**原语词汇缺一个维度**。

### 6.3 并排源码一份：OBI-A（档轴归约，四写法逐字）[OBSERVED]

零优化 naive（每 k 从头重求和，`Vec<Vec>` 索引）：
```rust
fn obi_a(d: &Data, o1: &mut [f64], o5: &mut [f64], o10: &mut [f64], _s: &mut Self::Scratch) {
    let n = d.n;
    for t in 0..n {
        let b1 = d.a_bsz[0][t]; let a1 = d.a_asz[0][t];
        o1[t] = (b1 - a1) / (b1 + a1);
        let (mut bid5, mut ask5) = (0.0, 0.0);
        for lev in 0..5 { bid5 += d.a_bsz[lev][t]; ask5 += d.a_asz[lev][t]; }
        o5[t] = (bid5 - ask5) / (bid5 + ask5);
        let (mut bid10, mut ask10) = (0.0, 0.0);
        for lev in 0..10 { bid10 += d.a_bsz[lev][t]; ask10 += d.a_asz[lev][t]; }
        o10[t] = (bid10 - ask10) / (bid10 + ask10);
    }
}
```
原语集 prims（`ndarray` Zip 逐列累加 + 原语比值 helper——**注意这不是原语，是 ndarray**）：
```rust
fn obi_a_impl(d, o1, o5, o10, s: &mut PrimsScratch) {
    let (k1,k5,k10) = (1.min(d.nlev), 5.min(d.nlev), 10.min(d.nlev));
    s.sum_bid.fill(0.0); s.sum_ask.fill(0.0);
    for lev in 0..k10 {
        Zip::from(&mut s.sum_bid[..]).and(&d.a_bsz[lev][..]).for_each(|sum,&v| *sum += v);
        Zip::from(&mut s.sum_ask[..]).and(&d.a_asz[lev][..]).for_each(|sum,&v| *sum += v);
        if lev+1==k1  { obi_ratio(&s.sum_bid,&s.sum_ask,&d.valid,o1,&mut s.obi_valid); }
        if lev+1==k5  { obi_ratio(&s.sum_bid,&s.sum_ask,&d.valid,o5,&mut s.obi_valid); }
        if lev+1==k10 { obi_ratio(&s.sum_bid,&s.sum_ask,&d.valid,o10,&mut s.obi_valid); }
    }
}
```
ndarray 手写（**与 prims 几乎逐字相同**）：
```rust
fn obi_a(d, o1, o5, o10, s: &mut Self::Scratch) {
    s.bid.fill(0.0); s.ask.fill(0.0);
    let (k1,k5,k10) = (1.min(d.nlev), 5.min(d.nlev), 10.min(d.nlev));
    for lev in 0..d.nlev {
        let bsz = ArrayView1::from(d.a_bsz[lev].as_slice());
        let asz = ArrayView1::from(d.a_asz[lev].as_slice());
        Zip::from(&mut ArrayViewMut1::from(s.bid.as_mut_slice())).and(&bsz).for_each(|s,&v| *s+=v);
        Zip::from(&mut ArrayViewMut1::from(s.ask.as_mut_slice())).and(&asz).for_each(|s,&v| *s+=v);
        if lev+1==k1  { obi_from_sums(o1, &s.bid, &s.ask); }
        if lev+1==k5  { obi_from_sums(o5, &s.bid, &s.ask); }
        if lev+1==k10 { obi_from_sums(o10, &s.bid, &s.ask); }
    }
}
```
pulp 手写 NEON（显式 `WithSimd`，垂直跨快照）：
```rust
fn obi_a_k<S: Simd>(simd: S, bsz: &[Vec<f64>], asz: &[Vec<f64>], k: usize, output: &mut [f64]) {
    let (head, tail) = S::as_mut_simd_f64s(output);
    for (chunk, dst) in head.iter_mut().enumerate() {
        let (mut bid, mut ask) = (simd.splat_f64s(0.0), simd.splat_f64s(0.0));
        for lev in 0..k {
            let (b,_) = S::as_simd_f64s(&bsz[lev]);  let (a,_) = S::as_simd_f64s(&asz[lev]);
            bid = simd.add_f64s(bid, b[chunk]);       ask = simd.add_f64s(ask, a[chunk]);
        }
        *dst = simd.div_f64s(simd.sub_f64s(bid,ask), simd.add_f64s(bid,ask));
    }
    let hl = head.len()*S::F64_LANES;                 // 手写 tail
    for (off, dst) in tail.iter_mut().enumerate() {
        let t = hl+off; let (mut bid,mut ask)=(0.0,0.0);
        for lev in 0..k { bid+=bsz[lev][t]; ask+=asz[lev][t]; }
        *dst = (bid-ask)/(bid+ask);
    }
}  // + ObiA struct + WithSimd + Arch::new().dispatch 转发
```
**并排要点**：prims 与 ndarray 的 OBI-A **实现相同**（Zip 逐列累加 + 比值 helper），因为原语集不提供档轴归约，两版都退回 ndarray 容器——这是"原语层在 L2 不省 LOC"的直接证据。pulp 用垂直 SIMD 换来 §7 的 `.2d` 但多 20+ 行 head/tail 样板，且在 40 列的档轴上并没有跑赢 autovec 的 prims/ndarray（2385 vs 1200）。

---

## 7. NEON 逐 kernel（[OBSERVED]，`otool -tvV` 逐符号，单态化符号定位）

| kernel | naive | prims | ndarray | pulp | 未命中原因 |
|---|---|---|---|---|---|
| OBI-A（档轴列归约，跨快照垂直） | 标量 | **`fadd.2d` autovec** | **`fadd.2d` autovec** | 显式 `.2d` | — |
| OBI-B（档内横向 fold，长度 N=10） | 标量 | 标量 | 标量 | 标量/部分 | 横向短归约无法垂直向量化 |
| microprice（elementwise） | 标量 | **`fmul/fadd/fdiv.2d`** | `.2d` | 显式 `.2d` | — |
| depth dws（加权 reduce） | 标量 | 部分 `.2d` | 部分 `.2d` | 部分 `.2d` | 加权和被 Zip 向量化，累计剖面散写标量 |
| OFI e_t（跨时间递推） | 标量 | 标量 | 标量 | 标量 | loop-carried，`y_t=f(y_{t-1})` |
| rolling sum/mean/std、slope 回归 | 标量 | 标量 | 标量 | 标量 | 归约重结合被禁 + 循环携带（增量滑动） |

样本反汇编 [OBSERVED]：
- ndarray obi_a：`fadd.2d v0,v0,v4` / `v1,v1,v5` /…（4×2-lane 展开）——**列归约按快照垂直 autovec 成 NEON**。
- pulp microprice：`fmul.2d`+`fadd.2d`+`fdiv.2d`——显式 `WithSimd`。
- naive obi_a：`fadd d3,d2,d1` / `fdiv d3,d4,d3`——纯标量。
- pulp OFI：向量算术 0 条——递推纯标量。

**结论**：与 fp-11 一致——只有 elementwise 与"跨快照垂直"的归约（布局 A 的档轴列累加）被 autovec；所有 rolling/递推是**设计使然的标量**。**新增一条 L2 特有的观察**：档轴归约能否 autovec 完全取决于布局——布局 A（每档一列、跨快照垂直）成 `.2d`，布局 B（每行档内横向、长度 10）标量。数据只有 16 B 分配器对齐，`ewise`/列归约仍 autovec 成 `.2d`——对齐不是 autovec 前提（复证 fp-11）。

---

## 8. 对子系统 §3 / §8.1 的修正建议（逐条 支持 / 需改 / 冲突）

### §3 布局：列式段、推导、对齐、导出、身份

- **§3.0"一段=每字段一条连续对齐列"——【需改】**：对 L2 这类**定长多档字段**，"一个字段"歧义——`bid_sz` 是"一条长度 N 的列"还是"N 条标量列"？demo 两种物化都成立且**性能相反**：布局 A（每 (side,level,field) 一条标量列，4N 条，Arrow fixed-size-list 展平）对"归约成每快照标量"的指标好；布局 B（每字段一条 `(n×N)` 二维列）对"输出每档剖面"的指标好 2–5×。§3.0 应明确：**定长多档字段折成 A 或 B 是布局 fold 的一个显式选择，必须记录进布局身份**，因为它决定 kernel 形态与性能，且二者对不同指标最优相反。
- **§3.1 fold 产出 `(name, format, element_size, column_offset)`——【需改】**：这个一维列描述符**无法表达多档字段的内层 N 维**。需给每字段加 `list_size`/`shape`（=N），让**档数进 `layout_hash`**（否则 4 档段与 10 档段布局身份撞车）。Arrow 侧：布局 A 用 `+w:N`（fixed-size-list）已覆盖；布局 B 的二维列需 shape 元数据。
- **§3.3"每列就是一条 Arrow buffer"——【支持】**（带 shape 注脚）：布局 A 每 (side,level,field) 一条 buffer，展平 fixed-size-list；布局 B 的 `(n×N)` 是一条 FixedSizeList child buffer + shape。两者都仍是"一列一 buffer"。
- **§3.2 对齐 64 B——【支持】**：本 demo 列是普通 `Vec`（16 B 对齐），elementwise 与档轴列归约照样 autovec 成 `.2d`；64 B 的理由仍只是 cache line/false sharing/身份规范化，不是性能。复证 fp-11。
- **§3.0"多字段同点访问退化为多次加载"——【支持】并量化**：L2 里"一次快照同时看/输出多档"正是这一退化——布局 A 散在 4N 条列、逐档一次加载/散写（depth-A 11–15 ms），布局 B 行连续（depth-B 2.8–5.5 ms）。建议 §3.0 把这句从定性升级为"**输出/访问是每档剖面的算子应选行主序（B）**"。

### §8.1 中间层裁决：原语集 + `ndarray` 视图

- **原语词汇是纯时间轴的——【需改（补一个维度）】**：§8.1 原语表（rolling/ewm/shift/diff/where/cumsum/reduce/linreg）**全是时间轴**。L2 引入**档轴归约**（OBI 跨档、累计深度剖面）。demo 裁定：**不加专门的档轴归约原语**——用 `ndarray` 容器直接表达（布局 A 用 Zip 逐列累加、布局 B 用 `rows()` 逐行 fold）就够，且一个档轴原语并不能消掉布局相关的行主序输出处理。建议 §8.1 明确写：**跨档（档轴）归约由 `ndarray` 容器承担，不进原语词汇**；原语集是"时间轴词汇 + `ndarray` 作容器"，容器要能同时表达时间轴列与档轴（二维/多列）。
- **"作者代码降 57–73%"——【需改（限定范围）/ 部分冲突】**：该结论对 L2 **不成立**——prims 404 ≈ ndarray 389（原语略多）。原因：档轴归约两版都手写 ndarray（同码），原语只在纯时间轴 z-score 上省行。建议把"降 57–73%"**限定为"时间轴 rolling 指标"**；对含档轴归约的混合负载，LOC 优势基本消失，应显式承认。
- **"绝大多数指标性能 ≤ 手写 pulp"——【需改（补物化成本）】**：prims slope 3110 µs = pulp 642 的 **4.8×**、OFI 1429 vs pulp 1043。根因是 fp-11 列为"未实测"的**中间列物化**——prims slope 用 4 次 `rolling.sum` 物化 4 条 temp 列，pulp 融合单遍。**这里量到了：多 rolling 项的回归类指标，原语版逐原语物化中间列可达手写的 ~2–5×**。z-score（单 rolling.mean+std）则 1589 ≈ pulp 1261，物化成本小。建议 §8.1 记入："原语版性能随 rolling 项数/中间列数下降，融合循环（pulp/ndarray）在多项 rolling 指标上有 2–5× 优势"。
- **"窗口向量化、递归标量"——【支持】**：复证——所有 rolling/递推标量，只有 elementwise + 布局 A 档轴列归约 autovec。
- **`rolling.min/max` 内部 SIMD 阀门——【本 demo 未触发】**：L2 指标集无 rolling 极值，§8.1 该条 N/A（fp-11 已测）。
- **`rolling.std` 一次方差的数值稳健性——【支持并补一条告警】**：prims `rolling.std`（Neumaier 滑动）在 z-score 近常值窗口过了 1e-9；但一次方差 `Σx²/w−mean²` 的抵消是真实风险，§8.1 应注明"标准化类指标的 rolling 方差需补偿求和，且忌全局 prefix 差分"。

---

## 9. 未覆盖

- [需实测] 布局 A 的 `Vec<Vec<f64>>` 双重间接是本 demo 的实现选择；真实段池布局 A 是**单块连续 4N·n 内存**（列偏移寻址），naive obi_a 的 5× 惩罚会缩小——但 prims/ndarray 的 Zip 已按连续列处理，A>B 的向量化结论不变 [INFERENCE]。
- [需实测] iceoryx2 真实链路的 `T_ipc`（§9 闸门 7）；本 demo 只测段内 kernel + 洗入。
- [需实测] 人造 gap × `gap_policy`：LOBSTER 快照全有效，validity 全 1，OFI 递推遇 invalid 的分支未触发（同 fp-11）。
- [未覆盖] x86 AVX2 实机（f64×4）；本机仅 aarch64 NEON（f64×2）。
- [未覆盖] level-50 样本（LOBSTER 免费 AAPL/MSFT/SPY 有 level-50，但档数 20→50 只放大档轴维度、不改结论方向 [INFERENCE]）；请求的 OBI k=20 因 N=10 截到 10。
- Binance 15 分钟实时录制方案未采用（维护者裁定用 LOBSTER；官网免费样本已改审批流程，记此一句）。

---

## 10. 路径（[OBSERVED]，可复现）

- 原语 crate（未改，复用 fp-11）：`/Users/mouriya/Ext/tmp/fp12/uta-prims/`（`src/lib.rs` 428 行）。
- demo harness：`/Users/mouriya/Ext/tmp/fp12/demo/`——`src/load.rs`（LOBSTER 解析 + 布局 A/B + 洗入 /10000）、`src/oracle.rs`（独立标量 golden）、`src/style.rs`（`Style` trait）、`src/main.rs`（计时/allocator/断言/两张表 + RAW dump）、`src/{naive,prims_impl,ndarray_impl,pulp_impl}.rs`（四写法）。
- 四写法隔离开发副本：`/Users/mouriya/Ext/tmp/fp12/dev-{naive,prims,ndarray,pulp}/`（各自 target，合并前独立验证）。
- 构建/运行：`CARGO_TARGET_DIR=/Users/mouriya/Ext/tmp/fp12/target cargo build --release`（在 demo/），`./target/release/fp12-demo all`（末行 `ALL GOLDEN + VALIDITY ASSERTIONS PASSED`）。
- 数据：`/Users/mouriya/Ext/tmp/fp12/data/AMZN_2012-06-21_..._{orderbook,message}_10.csv`。
- 契约（委派前冻结的公式/接口/口径）：`/Users/mouriya/Ext/tmp/fp12/CONTRACT.md`。
- 汇编：`otool -tvV target/release/fp12-demo`。golden 一次性 numpy 核对：oracle `--dump` → `oracle_dump.bin`，与 numpy 全数组 1e-9 比对。
