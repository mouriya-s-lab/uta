# 行情派生高性能计算子系统：`Pooled` → 段池 → 原生 op

状态：子系统设计 v1，闭合。上位：`uta-core-design.md` §5.3（`Pooled` 组合子）、§0.2（值树与五个 fold）、§7.0（注册表）、D11/D12。证据：`research/fp-07`（类型导出与外部编译）、`research/fp-08`（段池选库）。本文把 §5.3 的原则落到可实施的接口、契约与实测闸门；不改核心设计中心。标注：**[证据]** / **[设计]** / **[spike]** 同核心设计。

---

## 0. 一句话

**`Pooled` 把一条线性行情派生洗入类型化只读共享内存段；原生 op 在独立进程里按导出的对齐布局对整段做向量化计算；输出是一条新的派生流。** 核心只维护契约表与位置映射，库（iceoryx2）承担映射、生命周期与三 OS。

```mermaid
flowchart LR
  IN["输入流 Journal&lt;Record&gt;<br/>(集成清洗后的观察记录)"]
  POOLED["Pooled { input, window }<br/>值树读侧组合子"]
  WASH["洗入：一次拷贝<br/>Record → 对齐定长布局"]
  SEG["段 = Sample&lt;Slice&lt;Layout&gt;&gt;<br/>iceoryx2 只读共享内存"]
  TBL["契约表（核心）<br/>type_id · layout_hash · segment · stream · position range · version · writer"]
  OP["原生 op（独立进程）<br/>subscriber，只读映射，SIMD"]
  OUT["输出段 = 新派生流"]
  PROG["程序其他节点<br/>DerivationNode::Input"]
  IN --> POOLED --> WASH --> SEG
  WASH -.登记.-> TBL
  TBL -.段句柄 + 位置范围.-> OP
  SEG --> OP --> OUT --> PROG
```

---

## 1. 边界与不做

| 在子系统内 | 在子系统外 |
|---|---|
| `Pooled` 节点、洗入、布局推导与导出、契约表、段生命周期、原生 op 装载与握手、触发与预算 | 输入流的 `Journal` 与持久化（§2/§6.1）、程序其他节点、效应侧一切、集成进程 |
| 观察侧（§0.3） | 效应侧永不进段池 |
| 行情类流（前置条件四条） | 余额/持仓/订单状态/新闻——不满足前置条件，不能应用 `Pooled` |

明确不做：段池不持久（§5.3 E6/E7）；不提供 hostile-code sandbox（D11）；不自研映射层（E7）；不把段池概念泛化到 `Journal`（E3/E6）。

---

## 2. 前置条件与判定（装载期）

`Pooled { input, window }` 合法当且仅当 `input` 的记录类型满足：**定长**（洗入后每条记录字节数固定）、**位置线性**（`LogPosition` 单调、相邻记录相邻）、**无指针**（`repr(C)` 无引用/`Vec`/字符串）、**可容忍 ring 回收**（旧位置被回收只产生 `BeyondRetention`，不产生错误结果）。四条由输出类型 fold 在装载期判定，不满足即拒绝该程序。**[设计：§5.3]**

---

## 3. 布局：推导、对齐、导出、身份

### 3.1 推导（fold）
`Pooled` 之前的组合子树（`field::<T>` 访问器 + 转换算子）经 §0.2"输出类型" fold 得到记录布局 `Layout`：字段名、基类型、偏移、宽度、记录 stride、对齐。fold 必须**确定性**且输出**规范化**描述（字段顺序稳定、不含默认值、不含名字以外的语言细节）——这是 hash 的输入。**[spike S13①：无外部先例，需自证确定性]** **[证据：fp-07 S13 关闭建议 ①]**

### 3.2 对齐
洗入时按 SIMD 通道宽度对齐（记录 stride 与段基址均对齐到目标平台向量宽度；默认 64 字节覆盖 AVX-512/NEON）。对齐是 `Layout` 的一部分，进入 hash。**[设计：E8]**

### 3.3 导出格式
语言无关的布局描述，形状取 Arrow `ArrowSchema` format 码 + NumPy dtype offsets：每字段 `(name, format, offset)`，format 码覆盖洗入映射（`'l'` i64、`'tsn:'` ns 时间戳、`'d:19,10'` 定点、`'C'` u8 枚举）；附 `stride`、`alignment`、`layout_hash`。Rust 源（`#[repr(C, align(64))] struct`）与 C 头是它的两种渲染，由工具生成。**[证据：fp-07 命题 2；S13 关闭建议 ②]**

### 3.4 身份
`layout_hash = SHA256(规范化布局描述)`（RIHS01 式）。**不依赖 iceoryx2 的 `is_compatible_to`**——它只比 `type_name + size + alignment`，同尺寸同对齐、字段语义不同会误配。**[证据：fp-07 命题 1；fp-08 §3.3]**

---

## 4. 契约表（核心维护，唯一的跨段真相）

| 字段 | 含义 |
|---|---|
| `type_id` | 输出类型 fold 给出的类型身份 |
| `layout_hash` | §3.4 |
| `service` / `segment` | iceoryx2 service 名 + sample 句柄（段） |
| `stream` | 输入 `StreamId` |
| `position_range` | 该段覆盖的 `LogPosition` 区间 `[from, to)` |
| `version` | 契约版本（布局变 = 新版本 = 新 service） |
| `writer` | 写者（核心洗入器或某原生 op） |

**为什么必须有它**：iceoryx2 的段槽由 `PoolAllocator` 自由列表分配，**没有流内全局位置算术**；"地址即位置"只在一个段（一个 `Sample<Slice<Layout>>`）内成立（`base + (pos - from) * stride`），跨段靠此表把无序 allocator offset 归一到有序位置区间。窗口 = 按 `position_range` 排序的段句柄列表。**[证据：fp-08 §3.1/§3.9、推荐 1]** **[设计]**

**ABA 纪律**：iceoryx2 无 per-slot generation。核心与原生 op 只经 `Sample` 借用访问段，**禁止缓存裸 offset 跨释放复用**；段回收后其 `position_range` 从表中删除，旧位置查表即 `BeyondRetention`。**[证据：fp-08 推荐 2]**

---

## 5. 段生命周期

- **写者**：核心洗入器（或原生 op 对其输出段）是 publisher；`loan_slice_uninit(n) → 写 → send`。每段一次性写满即发，不原地追加（段是快照）。**[证据：fp-08 §3.2]**
- **读者**：原生 op 进程是 subscriber port：payload 段 OS 只读映射（`AccessMode::Read` → `PROT_READ` / `PAGE_READONLY`），控制面（连接队列、refcount）可写但是 iceoryx2 内部结构，不是核心内存。**接受"计算进程是可写控制面 subscriber，不是零协议只读附着"**。**[证据：fp-08 §3.5、推荐 3]** **[设计：显式接受]**
- **借用**：`Sample` 持有即 refcount>0，chunk 不回收；`subscriber_max_borrowed_samples` 上限防慢读者拖垮池。窗口 = 核心持有的一组 `Sample` 的有序列表（history 是回放队列不是随机访问，随机访问由核心保留引用集实现）。**[证据：fp-08 §3.4、推荐 4]**
- **回收**：ring 深度与 overflow 由 iceoryx2 配置（`history_size`、`subscriber_max_buffer_size`、`enable_safe_overflow`）；safe overflow 只回收无人借用的 chunk。**[证据：fp-08 §3.4]**
- **有效期**：段是输入窗口的快照，有效期是段池内部的事，不外泄；计算只见"这一版窗口"。**[设计：E7]**
- **崩溃**：原生 op 进程死 → 其借用经 `retrieve_returned_chunks` + H10 fence 回收；核心死 → op 成孤儿，fence 回收。**[证据：fp-08 §7.1 第 3 项]**

---

## 6. 原生 op：装载、握手、执行

### 6.1 注册项（§7.0 注册表）
`{ op_id, inputs: [(stream, layout_hash)], output: (stream, layout_hash), code_version, artifact, principal }`。`required_inputs` fold 对 `Pooled` 输入解析到契约表得段句柄。

### 6.2 装载 = 启动进程 + 握手
1. 核心启动 op 进程（或 op 自行启动并连接），传入 service 名与期望的 `layout_hash` 集。
2. op 进程 open service；iceoryx2 校验 `type_name/size/alignment`；**核心再比对契约表 `layout_hash` 与 op 编译时携带的 hash**，不等即 fail-closed，op 不进入运行。**[证据：fp-07 命题 3、S13 ⑤]**
3. 握手通过后 op 是 subscriber；触发经 iceoryx2 事件或核心的触发通道（S14）。

### 6.3 执行
op 收到触发 → 借用当前窗口的段列表 → 按导出布局做向量化计算（SIMD 对齐已由布局保证）→ `loan_slice_uninit` 输出段 → 写 → `send` → 释放借用。默认 **50 ms** 内完成；超时即该 op 的失败观察记录，不等待。**[设计：E7]**

### 6.4 替换
布局变 = 新 `layout_hash` = 新 service；旧 op 继续读旧 service 直到被停止；新 op 连新 service。不存在原地改布局。未决：调用中持有的段何时可回收（借用释放后自然回收，S12 实测）。

---

## 7. 外部编译与迭代

- **作者面**：typed SDK 从契约表导出 Rust 源（`#[repr(C, align(N))]` 结构 + `const LAYOUT_HASH`）；作者写一个小 crate，依赖 SDK，实现 `fn compute(windows: &[&[Input]]) -> impl Iterator<Item = Output>` 形状的入口；`zerocopy`/`bytemuck` 派生宏作为"导出布局确实可零拷贝访问"的编译期校验。**[证据：fp-07 组四；fp-08 §3.3]**
- **迭代**：改一个 op 只重编译一个小 crate，warm rebuild 亚秒级（实测 0.94 s → 0.12 s，单 OS 最小 crate）；替换走 §6.4。**[证据：fp-07 命题 5]**
- **交付**：source 或预编译均可；三 OS 产物按 target triple 矩阵构建（cargo-dist 式），Windows 需 MSVC、macOS 需 SDK——工具链缺口是独立条件，不是子系统能消掉的。**[证据：fp-07 组四]**
- **不做**：宿主内嵌编译器（Cranelift 实验性、rustc 作库不稳定）。**[证据：fp-07 案例 4.5]**

---

## 8. 选库裁决

**首选 `iceoryx2`**（维持 D12）。理由：唯一同时满足 Rust 一等、三 OS（CI + 本机 macOS 实测跨进程）、`repr(C)` 类型校验、`Slice<T>` 定长数组样本、单写多读、payload OS 只读映射、ring 深度可配、refcount 借用生命周期，且由库承担映射与簿记的候选。**[证据：fp-08 §7]**

**核心侧显式接受的五条补齐**（fp-08 推荐）：① 自建契约表做跨段位置映射；② 只经 `Sample` 访问、禁缓存裸 offset；③ 计算进程是可写控制面 subscriber；④ 随机访问历史由核心持引用集；⑤ 跨语言时 `type_name` 需覆写且以 `layout_hash` 补强。

**备选 `memmap2` + 自研**：只在 S12 实测 iceoryx2 在 macOS/Windows 跨独立进程不可靠、或"可写控制面 subscriber"被判不可接受、或必须把裸 offset 导出给 op 做零协议随机寻址时启用；启用即违反"不自研"，须记入 S12 并列出自写清单（命名段生命周期、契约表、位置算术、seqlock、ring + generation、借用簿记、控制/数据面分离）。**[证据：fp-08 §7]**

不选：Aeron（可写 `MmapMut`、需 media driver、CI 仅 Linux）、disruptor-rs（进程内）、Chronicle（持久、查表、付费 Rust 绑定）、ipc-channel/shm_ringbuf/shared_memory（三门 FAIL）。Arrow `FFI_ArrowSchema` 只作布局导出面，不作数据面。

---

## 9. 实测闸门（S12，落地前必过；macOS + Windows 各一遍）

1. 跨独立进程只读消费：publisher 发 `Sample<Slice<Layout>>`，另一进程 subscriber 按 `&[Layout]` 算术遍历；写 payload 触发段错误。
2. 借用期回收安全：长持最老 `Sample` 的 reader 与正常 reader 并存，publisher 超发 `buffer + history` 条；长持仍有效，overflow 行为符合配置。
3. reader 崩溃回收：持借用的 op 进程异常退出，chunk 经 fence 回收，不泄漏不死锁。
4. Slice 动态重分配：非 Static 策略下超 `max_slice_len` 扩容，快照不丢样本。
5. 契约表归一：无序 `PointerOffset` 归一到有序 `LogPosition` 区间并跨段拼接；不同 subscriber 对同一位置得同一内容（跨独立进程复验）。
6. 布局 fold 确定性：同一组合子树多次 fold 得同一规范化描述与 hash；字段顺序扰动不改变 hash（S13①）。

任一失败 → §8 备选条件。

---

## 10. 与核心 spike 的对应

| 核心 spike | 本文处理 |
|---|---|
| S12 | §5 生命周期 + §9 闸门 1–5；选库已裁决，剩实测 |
| S13 | ②导出 ③交付 ⑤校验已由 §3/§6/§7 关闭；剩 ① fold 确定性（§9 闸门 6）与三 OS 工具链缺口（§7，外部条件） |
| S14 | 触发语义（edge/level、合并、冷却）、背压、失败观察的形状——§6.3 给了 50 ms 预算与失败观察，触发通道的具体形式仍开放 |
| S15 | 不在本文：输入流的持久化归 `Journal` |

---

## 11. 来源

- `research/fp-07-type-export-and-external-compilation.md`：24 案例，pinned iceoryx2 `aec1ed8` / arrow `b274238` / rosidl `00d13c5`。
- `research/fp-08-segment-pool-libraries.md`：14 库，三淘汰门 + 12 维；本机 macOS 实测 iceoryx2 跨进程 pub/sub 与 `Slice` 连续性。
- `native-computation-design-handoff.md`：场景与约束来源；其"同地址空间"方向已被 D12 取代。
- `decision-log.md` E1–E9。
