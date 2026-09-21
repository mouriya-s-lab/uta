# Gate 9 对齐/cache-line 对照

## 1. 范围与口径

1. [OBSERVED] 本报告只覆盖 `gate9/align/` 独立 crate，没有改动 `gate9/demo/` 或 `gate9/uta-prims/`。
2. [OBSERVED] 数据入口是 `/Users/mouriya/Ext/tmp/fp10/data/spy_daily.csv`，SHA-256 为 `eeee85933f3b26b783ee7dfae2aaa0d59db6d0205b6ec54d18fbbcc828da96f4`。
3. [OBSERVED] loader 原文如下，等价于 Gate9 demo 的 `load()`：

```rust
for line in text.lines().skip(1) {
    let mut fields = line.split(',');
    fields.next();
    fields.next();
    high.push(fields.next().unwrap().parse().unwrap());
    low.push(fields.next().unwrap().parse().unwrap());
    close.push(fields.next().unwrap().parse().unwrap());
}
```

4. [OBSERVED] 共 8465 行，输入列顺序和字段为 High/Low/Close；没有复制、平铺或转置数据。
5. [OBSERVED] `src/baselines.rs` 是 demo 文件的逐字复制：两文件均为 29880 bytes，运行前直接比较文本结果为 `same: true`。
6. [OBSERVED] 被测实现只有复制的 `scalar_supertrend`、`scalar_ichimoku`、`scalar_squeeze` 和 `pulp_*`；算法正文没有重写。
7. [OBSERVED] 计时严格使用 20 次 warm-up、300 次样本、每 pass 取样本中位数，再做 7 pass 中位数/min/max。
8. [OBSERVED] 计时闭包只调用算法并 `black_box` 消费输出；输入、输出和地址字符串均在计时外建立。
9. [OBSERVED] `probe` 在同一调用形状下记录分配；30 个 release 行的 `allocs` 都是 `(0,0)`。

## 2. 起点实现

10. [OBSERVED] `aligned64`、`offset+8B`、`offset+24B`、`offset+40B` 都由 over-allocate 后取 64 B 边界子切片实现。
11. [OBSERVED] 起点计算的代码原文是：

```rust
let base = storage.as_ptr() as usize;
let round_up = (64 - base % 64) % 64;
let byte_offset = layout.offset_bytes();
let start = (round_up + byte_offset) / size;
```

12. [OBSERVED] 每一个 f64 输入列和每一个 f64 输出列都使用同一组起点策略；SuperTrend 的 i8 trend、Squeeze 的 i8 sqz 也从独立的 64 B-rounded owner 加相同 byte offset。
13. [OBSERVED] `ordinaryVec` 使用普通 `Vec<T>` 的自然分配，不强制 mod64；本次所有实际 slice 地址恰好记录为 mod64=0，不能把这个偶然结果推广为分配器保证。
14. [OBSERVED] 每个受控列都断言实际 slice 指针的 mod64 是 0、8、24 或 40；所有断言通过。
15. [OBSERVED] 64 B-rounding 保留了 f64 的 8 B 自然对齐；+8/+24/+40 都是字节偏移而不是元素值偏移。

## 3. 正确性证据

16. [OBSERVED] SuperTrend oracle 的固定下标值为 `45.315821/73.348225/137.373727/775.277032`，first_valid=9，trend 为 `-1/1/1/-1`。
17. [OBSERVED] Ichimoku tenkan 为 `44.703125/75.070312/140.959999/761.815002`，kijun 为 `44.828125/74.632812/139.209999/764.484985`。
18. [OBSERVED] Ichimoku 未位移 senkouB 为 `44.546875/73.742188/140.930001/754.234985`，scalar/pulp 全数组均与 oracle 在 1e-9 相对容差内一致。
19. [OBSERVED] Squeeze val 为 `-0.632299/1.777589/3.481440/-9.281345`，first_valid=38，sqz i8 全数组逐点一致。
20. [OBSERVED] release 程序最后输出 `ALL RELEASE GOLDEN + FULL-ARRAY CHECKS PASSED`。

## 4. Release 结果

21. [OBSERVED] 下表单位全部为 µs/call；`ratio64` 是同一指标、同一实现的 `median / aligned64 median`；每行的分配列为 `(alloc,realloc)`。

| 指标 | 起点 | 实现 | median | min | max | ratio64 | allocs |
|---|---|---:|---:|---:|---:|---:|---:|
| SuperTrend | aligned64 | scalar | 72.917 | 72.791 | 73.042 | 1.000 | (0,0) |
| SuperTrend | aligned64 | pulp | 72.625 | 70.084 | 72.708 | 1.000 | (0,0) |
| Ichimoku | aligned64 | scalar | 390.375 | 331.791 | 392.375 | 1.000 | (0,0) |
| Ichimoku | aligned64 | pulp | 92.208 | 91.834 | 93.042 | 1.000 | (0,0) |
| Squeeze | aligned64 | scalar | 172.791 | 172.417 | 173.500 | 1.000 | (0,0) |
| Squeeze | aligned64 | pulp | 113.709 | 113.541 | 113.959 | 1.000 | (0,0) |
| SuperTrend | offset+8B | scalar | 72.000 | 71.709 | 72.375 | 0.987 | (0,0) |
| SuperTrend | offset+8B | pulp | 70.375 | 66.583 | 71.084 | 0.969 | (0,0) |
| Ichimoku | offset+8B | scalar | 390.208 | 331.875 | 391.541 | 1.000 | (0,0) |
| Ichimoku | offset+8B | pulp | 93.208 | 93.083 | 93.458 | 1.011 | (0,0) |
| Squeeze | offset+8B | scalar | 173.167 | 172.416 | 173.541 | 1.002 | (0,0) |
| Squeeze | offset+8B | pulp | 113.375 | 113.250 | 113.542 | 0.997 | (0,0) |
| SuperTrend | offset+24B | scalar | 71.875 | 71.791 | 72.333 | 0.986 | (0,0) |
| SuperTrend | offset+24B | pulp | 71.375 | 70.167 | 71.500 | 0.983 | (0,0) |
| Ichimoku | offset+24B | scalar | 391.708 | 390.709 | 393.667 | 1.003 | (0,0) |
| Ichimoku | offset+24B | pulp | 92.458 | 92.084 | 92.834 | 1.003 | (0,0) |
| Squeeze | offset+24B | scalar | 172.958 | 172.083 | 173.416 | 1.001 | (0,0) |
| Squeeze | offset+24B | pulp | 113.583 | 113.417 | 113.917 | 0.999 | (0,0) |
| SuperTrend | offset+40B | scalar | 72.333 | 72.125 | 72.458 | 0.992 | (0,0) |
| SuperTrend | offset+40B | pulp | 71.458 | 69.083 | 71.750 | 0.984 | (0,0) |
| Ichimoku | offset+40B | scalar | 332.250 | 331.250 | 390.917 | 0.851 | (0,0) |
| Ichimoku | offset+40B | pulp | 92.291 | 92.042 | 92.583 | 1.001 | (0,0) |
| Squeeze | offset+40B | scalar | 172.708 | 171.875 | 173.375 | 1.000 | (0,0) |
| Squeeze | offset+40B | pulp | 113.584 | 113.458 | 113.791 | 0.999 | (0,0) |
| SuperTrend | ordinaryVec | scalar | 72.417 | 72.291 | 72.541 | 0.993 | (0,0) |
| SuperTrend | ordinaryVec | pulp | 72.125 | 67.667 | 72.333 | 0.993 | (0,0) |
| Ichimoku | ordinaryVec | scalar | 391.459 | 391.292 | 391.792 | 1.003 | (0,0) |
| Ichimoku | ordinaryVec | pulp | 92.333 | 91.875 | 92.917 | 1.001 | (0,0) |
| Squeeze | ordinaryVec | scalar | 172.667 | 172.041 | 173.125 | 0.999 | (0,0) |
| Squeeze | ordinaryVec | pulp | 113.417 | 113.250 | 113.708 | 0.997 | (0,0) |

## 5. 实际地址 mod64

22. [OBSERVED] 下列是实际参与算法调用的每个输入/输出 slice 起点；同一布局的 scalar/pulp 行共用这些已分配且未移动的列。
23. [OBSERVED] SuperTrend：

```text
aligned64:   h=0 l=0 c=0 | atr=0 st=0 trend=0
offset+8B:   h=8 l=8 c=8 | atr=8 st=8 trend=8
offset+24B:  h=24 l=24 c=24 | atr=24 st=24 trend=24
offset+40B:  h=40 l=40 c=40 | atr=40 st=40 trend=40
ordinaryVec: h=0 l=0 c=0 | atr=0 st=0 trend=0
```

24. [OBSERVED] Ichimoku：

```text
aligned64:   h=0 l=0 c=0 | tenkan=0 kijun=0 senkouA=0 senkouB=0 chikou=0 senkouBraw=0
offset+8B:   h=8 l=8 c=8 | tenkan=8 kijun=8 senkouA=8 senkouB=8 chikou=8 senkouBraw=8
offset+24B:  h=24 l=24 c=24 | tenkan=24 kijun=24 senkouA=24 senkouB=24 chikou=24 senkouBraw=24
offset+40B:  h=40 l=40 c=40 | tenkan=40 kijun=40 senkouA=40 senkouB=40 chikou=40 senkouBraw=40
ordinaryVec: h=0 l=0 c=0 | tenkan=0 kijun=0 senkouA=0 senkouB=0 chikou=0 senkouBraw=0
```

25. [OBSERVED] Squeeze：

```text
aligned64:   h=0 l=0 c=0 | basis=0 dev=0 rangeMA=0 highest=0 lowest=0 source=0 val=0 sqz=0 range=0
offset+8B:   h=8 l=8 c=8 | basis=8 dev=8 rangeMA=8 highest=8 lowest=8 source=8 val=8 sqz=8 range=8
offset+24B:  h=24 l=24 c=24 | basis=24 dev=24 rangeMA=24 highest=24 lowest=24 source=24 val=24 sqz=24 range=24
offset+40B:  h=40 l=40 c=40 | basis=40 dev=40 rangeMA=40 highest=40 lowest=40 source=40 val=40 sqz=40 range=40
ordinaryVec: h=0 l=0 c=0 | basis=0 dev=0 rangeMA=0 highest=0 lowest=0 source=0 val=0 sqz=0 range=0
```

## 6. Debug 量级

26. [OBSERVED] 命令 `GATE9_DEBUG_ONLY=1 CARGO_TARGET_DIR=/Users/mouriya/Ext/tmp/fp10/gate9/target cargo run` 使用默认 debug profile（opt-level=0），只在 aligned64 上各做一次 scalar/pulp 调用，仍通过 golden 检查。

| 指标 | scalar 单次 µs | pulp 单次 µs |
|---|---:|---:|
| SuperTrend | 351.500 | 1125.750 |
| Ichimoku | 7001.167 | 8981.750 |
| Squeeze | 3479.750 | 8692.375 |

27. [OBSERVED] debug 程序最后输出 `ALL DEBUG GOLDEN CHECKS PASSED`。
28. [推断] debug 单次值仅作未优化量级参照，不与 release 的 7-pass 中位数混作性能结论。

## 7. min/max 区间判读与结论

29. [OBSERVED] 本次有 24 个“非 aligned64 起点 vs 同指标同实现 aligned64”的区间比较；若两个 7-pass `[min,max]` 有交集，记为“在噪声内”。
30. [OBSERVED] SuperTrend/scalar 的 +8、+24、+40、ordinaryVec 四个区间都不与 aligned64 `[72.791,73.042]` 相交；对应 ratio 为 0.987、0.986、0.992、0.993，方向反而是非 aligned 起点略快。
31. [OBSERVED] SuperTrend/pulp 的四个区间全部相交 aligned64 `[70.084,72.708]`；因此这些差异在本次噪声内。
32. [OBSERVED] Ichimoku/scalar 的四个区间全部相交 aligned64 `[331.791,392.375]`；+40 的 0.851 不能仅按中位数宣称加速，因为区间仍相交。
33. [OBSERVED] Ichimoku/pulp 的 +24、+40、ordinaryVec 区间相交 aligned64 `[91.834,93.042]`；+8 的 `[93.083,93.458]` 与该区间有约 0.041 µs 的微小空隙，ratio=1.011。
34. [OBSERVED] Squeeze/scalar 的四个区间全部相交 aligned64 `[172.417,173.500]`。
35. [OBSERVED] Squeeze/pulp 的四个区间全部相交 aligned64 `[113.541,113.959]`；+8 只在 113.541–113.542 的边界处相交。
36. [OBSERVED] 严格按 min/max，19/24 个比较在噪声内，5/24 个不相交；不相交的 4 个是 SuperTrend/scalar，另 1 个是 Ichimoku/pulp +8B。
37. [推断] 因为不相交结果没有一致的“aligned 更快”方向，且普通 Vec 本次实际也落在 mod64=0，不能据此证明 64 B cache-line 对齐带来稳定收益或稳定惩罚。
38. [推断] 对这台 M4、这组 8465 行 SPY 数据和本协议，最稳妥的结论是：对齐/cache-line 差异总体不可稳定测出；严格区间规则只捕捉到少数微小、方向不一致的分离，不能支持“未对齐慢多少”的固定百分比。
39. [推断] 这组对照回答的是本机本数据的物理起点效应，不证明其他 allocator、数组长度、CPU 或跨 cache-line 工作集的普遍规律。

## 8. 路径与复现

40. [OBSERVED] crate manifest：`/Users/mouriya/Ext/tmp/fp10/gate9/align/Cargo.toml`。
41. [OBSERVED] benchmark harness：`/Users/mouriya/Ext/tmp/fp10/gate9/align/src/main.rs`。
42. [OBSERVED] verbatim baseline：`/Users/mouriya/Ext/tmp/fp10/gate9/align/src/baselines.rs`。
43. [OBSERVED] 数据：`/Users/mouriya/Ext/tmp/fp10/data/spy_daily.csv`。
44. [OBSERVED] 共享 target：`/Users/mouriya/Ext/tmp/fp10/gate9/target`。
45. [OBSERVED] release 复现：`CARGO_TARGET_DIR=/Users/mouriya/Ext/tmp/fp10/gate9/target cargo run --release`。
46. [OBSERVED] debug 复现：`GATE9_DEBUG_ONLY=1 CARGO_TARGET_DIR=/Users/mouriya/Ext/tmp/fp10/gate9/target cargo run`。
47. [OBSERVED] scoped `cargo check` 通过；release 与 debug 两条实际入口都通过 golden/full-array（debug 为 aligned 参考）检查。
