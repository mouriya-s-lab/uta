# Gate 9：完全不考虑优化的标准 Rust 普通写法

## 结论

[OBSERVED] 三个指标都通过固定 SPY 数据的 golden、全数组数值与 NaN 有效性比较；进程最后打印 `ALL GOLDEN + VALIDITY ASSERTIONS PASSED`。

[OBSERVED] 朴素实现的 release 结果并不代表“必然比所有基线慢”：SuperTrend 为 scalar 的 0.804 倍；这是本机、该编译器和该写法的实测，不改变其刻意的分配与逐窗算法。

[OBSERVED] Ichimoku 和 Squeeze 分别为 scalar 的 1.947 倍、2.389 倍，且明显慢于 pulp/nd；这两项更直接暴露了每一步重扫窗口和中间 `Vec` 的代价。

[推断] 下面的 debug/基线比值只是把 debug 单次结果除以 release 基线，不能当作同一优化级别的公平速度结论。

## 实验口径

[OBSERVED] 数据文件是 `/Users/mouriya/Ext/tmp/fp10/data/spy_daily.csv`，`shasum -a 256` 得到 `eeee85933f3b26b783ee7dfae2aaa0d59db6d0205b6ec54d18fbbcc828da96f4`，共 8465 行。

[OBSERVED] `load()` 跳过表头，按 `split(',')` 取字段 2/3/4 作为 High/Low/Close，与 `demo/src/main.rs::load()` 相同。

[OBSERVED] naive kernel 输入为借用的 `&[f64]` 列，输出和所有中间量均为拥有的 `Vec<f64>`；趋势和 squeeze 状态是 `Vec<i8>`。

[OBSERVED] 窗口函数每个输出位置重新求和或扫描 `[start..=i]`；mean、max、min 没有增量状态或单调队列。

[OBSERVED] `rolling_std` 每个位置第一遍求和得到均值，第二遍求平方和，再按总体方差 `ddof=0` 求平方根。

[OBSERVED] 所有滚动输出先用 NaN 填充；SuperTrend 的递推状态只在 `i >= 9` 写入，Ichimoku tenkan/kijun/raw senkouB 起点分别是 8/25/51，Squeeze val 起点是 38。

[OBSERVED] 实现使用直接 `a[i]` 索引，没有消除边界检查；没有 SIMD 类型、对齐分配、增量 rolling、迭代器窗口技巧或复用输出列。

[OBSERVED] release 使用 `bench`：7 次独立 pass，每次 20 次 warm-up 加 300 次取中位数，并报告 7 个 pass 的中位数、最小值和最大值。

[OBSERVED] debug 使用 `opt-level=0`，在正确性检查后对一次完整调用直接计时；这符合本任务“debug 单次即可”。

[OBSERVED] `probe` 只包住一次 naive 计算调用和 `black_box`，输入数据已在计时区域外准备；计数器同时记录 alloc 与 realloc。

[OBSERVED] scalar/pulp/nd 来自 `gate9/demo/src/baselines.rs` 的同一源文件（main 通过 `#[path = "../../demo/src/baselines.rs"]` 引入），基线按 `bench_once` 的 20+300 口径取一次中位数。

## 结果表

| 指标 | naive-release µs（中位数 [min..max]） | naive-debug µs（单次） | scalar / pulp / nd µs（release） | release/基线（s/p/n） | debug/基线（s/p/n） | allocs (alloc,realloc) | LOC |
|---|---:|---:|---:|---:|---:|---:|---:|
| SuperTrend(10,3) | 58.250 [57.916..58.333] | 1210.875 | 72.459 / 72.417 / 49.416 | 0.804 / 0.804 / 1.179 | 16.711 / 16.721 / 24.504 | (9,0) | 79 |
| Ichimoku(9,26,52) | 642.916 [635.958..648.541] | 11912.084 | 330.292 / 92.500 / 227.208 | 1.947 / 6.950 / 2.830 | 36.065 / 128.779 / 52.428 | (12,0) | 53 |
| Squeeze Momentum | 412.333 [411.334..412.875] | 10602.375 | 172.583 / 113.750 / 114.167 | 2.389 / 3.625 / 3.612 | 61.433 / 93.208 / 92.867 | (14,0) | 50 |

[OBSERVED] SuperTrend golden `[100,1000,5000,8464]` 为 `45.315821/73.348225/137.373727/775.277032`，trend 为 `-1/1/1/-1`，first_valid=9。

[OBSERVED] Ichimoku tenkan 为 `44.703125/75.070312/140.959999/761.815002`，kijun 为 `44.828125/74.632812/139.209999/764.484985`，raw senkouB 为 `44.546875/73.742188/140.930001/754.234985`。

[OBSERVED] Ichimoku 主 tenkan first_valid=8；shifted senkouA、shifted senkouB、chikou 的尾部/前部 NaN 也逐项与 scalar oracle 一致。

[OBSERVED] Squeeze val 为 `-0.632299/1.777589/3.481440/-9.281345`，first_valid=38；sqz 的 `-1/0/1` 数组与 oracle 一致。

[OBSERVED] `naive.rs` 为 305 行物理 LOC：SuperTrend 79 行（122–200），Ichimoku 53 行（202–254），Squeeze 50 行（256–305），共用 rolling/回归/NaN helper 120 行；`main.rs` harness 为 668 行。

[推断] 9/12/14 次分配来自每个指标调用内部的新列和返回列；三项 realloc 都为 0，因为每个 `Vec` 以最终长度直接建立。分配次数是本实现的结构性成本，不是优化器可见的语义要求。

[推断] naive kernel 没有显式 SIMD/对齐路径；本次没有对 naive 二进制做汇编归因，因此不能把任何潜在 LLVM 自动向量化误报为显式 SIMD。

## SuperTrend 源码（`src/naive.rs` 原文）

```rust
#[derive(Debug)]
pub struct SuperTrendOutput {
    pub supertrend: Vec<f64>,
    pub trend: Vec<i8>,
}

fn nan_vec(n: usize) -> Vec<f64> {
    vec![f64::NAN; n]
}

pub fn supertrend(high: &[f64], low: &[f64], close: &[f64]) -> SuperTrendOutput {
    const PERIOD: usize = 10;
    const FACTOR: f64 = 3.0;
    let n = close.len();

    let mut tr = nan_vec(n);
    if n > 0 {
        tr[0] = high[0] - low[0];
        for i in 1..n {
            let high_low = high[i] - low[i];
            let high_previous_close = (high[i] - close[i - 1]).abs();
            let low_previous_close = (low[i] - close[i - 1]).abs();
            tr[i] = high_low.max(high_previous_close).max(low_previous_close);
        }
    }

    let mut atr = nan_vec(n);
    if n >= PERIOD {
        let mut seed = 0.0;
        for i in 0..PERIOD {
            seed += tr[i];
        }
        atr[PERIOD - 1] = seed / PERIOD as f64;
        for i in PERIOD..n {
            atr[i] = (atr[i - 1] * (PERIOD - 1) as f64 + tr[i]) / PERIOD as f64;
        }
    }

    let mut hl2 = nan_vec(n);
    let mut up = nan_vec(n);
    let mut down = nan_vec(n);
    for i in PERIOD - 1..n {
        hl2[i] = (high[i] + low[i]) * 0.5;
        up[i] = hl2[i] - FACTOR * atr[i];
        down[i] = hl2[i] + FACTOR * atr[i];
    }

    let mut final_up = nan_vec(n);
    let mut final_down = nan_vec(n);
    let mut trend = vec![0i8; n];
    let mut output = nan_vec(n);
    for i in PERIOD - 1..n {
        if i == PERIOD - 1 {
            final_up[i] = up[i];
            final_down[i] = down[i];
            trend[i] = 1;
        } else {
            let previous_up = final_up[i - 1];
            let previous_down = final_down[i - 1];
            final_up[i] = if close[i - 1] > previous_up {
                up[i].max(previous_up)
            } else {
                up[i]
            };
            final_down[i] = if close[i - 1] < previous_down {
                down[i].min(previous_down)
            } else {
                down[i]
            };
            trend[i] = if trend[i - 1] == -1 && close[i] > previous_down {
                1
            } else if trend[i - 1] == 1 && close[i] < previous_up {
                -1
            } else {
                trend[i - 1]
            };
        }
        output[i] = if trend[i] == 1 {
            final_up[i]
        } else {
            final_down[i]
        };
    }

    SuperTrendOutput {
        supertrend: output,
        trend,
    }
}
```

## 路径与复现

[OBSERVED] 代码：`/Users/mouriya/Ext/tmp/fp10/gate9/naive/src/naive.rs`、`/Users/mouriya/Ext/tmp/fp10/gate9/naive/src/main.rs`、`/Users/mouriya/Ext/tmp/fp10/gate9/naive/Cargo.toml`。

[OBSERVED] 基线源：`/Users/mouriya/Ext/tmp/fp10/gate9/demo/src/baselines.rs`；参考计时/加载源：`/Users/mouriya/Ext/tmp/fp10/gate9/demo/src/main.rs`。

[OBSERVED] 设计依据：`/Users/mouriya/Ext/orca/workspaces/OpenAlice/new-uta/plans/uta-refactor/design/hpc-derivation-subsystem.md` §3.2、§8.1、§9；指标契约：`local://fp10-experiment-spec.md`。

[OBSERVED] 实际命令（`CARGO_TARGET_DIR` 按契约使用共享 target）：`CARGO_TARGET_DIR=/Users/mouriya/Ext/tmp/fp10/gate9/target cargo run --manifest-path /Users/mouriya/Ext/tmp/fp10/gate9/naive/Cargo.toml`；`CARGO_TARGET_DIR=/Users/mouriya/Ext/tmp/fp10/gate9/target cargo run --release --manifest-path /Users/mouriya/Ext/tmp/fp10/gate9/naive/Cargo.toml`。

[OBSERVED] debug 与 release 两次实际运行都通过 correctness、golden、NaN/有效性和状态数组检查；release 输出已完成 7-pass bench。
