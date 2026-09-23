# 4 观察宇宙

观察宇宙是不可写的一半：记录派生内容，用可撤回代数修正，供订阅与程序消费。它的类型不引用效应侧（§3.2）。

## 4.1 `Journal`、撤回代数、压缩

> 图：D2.5、D3.1（`design/diagrams/02-record-model.md`、`03-observation-ingest.md`）。

观察侧的载体是 `Journal`。存储原语可与效应侧共享，抽象不共享。

### 规则

```rust
trait Delta: Monoid { fn is_zero(&self) -> bool }                  // fp-05 案例 12 difference.rs
trait RetractableDelta: Delta { fn neg(&self) -> Self }            // 只有派生侧要求

struct Journal<Record, D: Delta> { stream: StreamId, ops: Vec<(LogPosition, D)> }
fn fold_state<Record, D: Delta>(journal: &Journal<Record, D>, at: LogPosition) -> State<Record>   // state = 前缀和 = fold
fn compact_below_retention<Record, D: RetractableDelta>(journal: &mut Journal<Record, D>, frontier: Frontier)
```

- **`Record`**：该流的记录词表，由集成侧在边界处解析给出（§2.1）。`Record` 必须为具体类型；退化为 `dyn Any` 即违反解析边界。[证据：fp-04 命题 15/16]
- **`StreamId = (source, stream, epoch)`**：每个范围独立维持 `LogPosition` 单调递增；系统内不存在全局入口序（§2.3）。[证据：fp-04 命题 11；域 P2]
- **状态计算**：状态是 `fold_state` 的计算结果（前缀和折叠），可随时重建与缓存，而非由规则原地修改的对象。[证据：fp-03 命题 1；fp-01 M8]

**不变量：**

- 派生侧使用 `Delta: RetractableDelta`（可撤回多重集、正负 diff）。
- 执行事实侧的 append-only 记录链在存储上可复用同一原语（`Delta: Monoid`，无逆元，第一边界的第一层，§3.1），但它属于效应抽象，类型不与观察侧共享。由单向边 + crate 依赖方向保证（§3.2）。
- `compact_below_retention` 仅对 `RetractableDelta` 表存在；压缩后仍被引用的位置 ≥ 保留边界（§2.4）。

### 为什么

派生内容会被迟到 tick 修正，需要在代数上表达撤回（负 diff）。`RetractableDelta` 的 `neg` 使旧贡献可撤、新贡献可加、受影响子图可重算。

执行事实不可如此修正（第一边界，§3.1），所以逆元只在派生侧要求。[证据：fp-05 命题 12；fp-03 命题 1；域 P15]

### 不选

- **把执行历史也建成可撤回 delta**：负 diff 不能当外部 Write 的补偿，已发出的写不能被撤回代数抹成“从未发生”（§3.1）。
- **让状态由规则原地修改而非 fold**：状态就无法随时重建与缓存，重放 / 审计失去依据。

### 实现事实与设计事实

两侧共用存储原语（单 SQLite 文件、同一 `(stream_id, log_position)` 主键形状，§7.4）是实现事实；不共享类型宇宙是设计事实。表结构是否物理共表由实现按写放大与查询成本定，不改本节。

## 4.2 消费方式、gap、cursor 与确认

> 图：D3.4 订阅/cursor/ack/gap、D3.5 三种消费方式、D3.6 gap 来源判定（`design/diagrams/03-observation-ingest.md`）。

### 三种消费方式

在 `LogPosition` 的积序 `Set<LogPosition>`（≅ `Map<StreamId, Seq>`）上定义比较子，支持三种消费策略：

| 方式 | 语义 | 损失 |
|---|---|---|
| await-all | 待所有指定输入均达到要求位置/完备进度后再行计算 | 无（引入等待） |
| ordered | 按单条流的顺序依次处理，不跳过中间记录 | 无（引入背压） |
| latest / conflated | 允许合并中间更新，仅保留最新值 | **有**：`Gap{origin: Delivery}` 原因记录为 `conflated`，或以消费者声明的窗口界作为其可接受丢失界 |

- `await-all` 按**完备进度**触发，而非按消费位置（§2.3）。
- `latest` 属于传输层 conflation，而非 frontier 消费。UI 报价显示可接受 conflation，但依赖完整状态路径的阈值策略不能默认接受。
- 损失语义必须由消费者显式声明。[证据：fp-05 命题 2/15；域 C6]

### gap 的三种来源

三种 gap 同一形状，按 `origin` 区分。

**`Gap{origin: Source, reason}`：来源流有缺口。** 新 epoch 首条记录后的补齐边界由 frontier 声明（域 P3/P4）。`reason` 取 P3 的集合，各自的触发者如下：

| `reason` | 触发者 |
|---|---|
| `start`、`disconnect`、`quota`、`ingress_overflow` | 集成上报 |
| `credential_rotated` | 会话重建（§7.2） |
| `schema_change` | 载荷版本变化（§8.1） |
| `backfill_incomplete` | 回填穷尽（§8.4） |
| `program_upgrade` | 程序产出的派生流也是来源；程序升级或 `Reset` 开新 epoch 时记（§8.6） |

**`Gap{origin: Delivery, reason}`：投递有缺口。** `reason ∈ {slow_consumer, compacted, conflated}`（P3），分别是慢消费者被停投、订阅位置已被压缩到保留边界之下、`latest` 消费合并。程序是订阅消费者（输入 = 位置推进，§4.3），程序滞后被跳过的区间是它的 `Delivery` gap。

**`Gap{origin: Channel, channel}`：读渠道不可用。** `channel` 为返回 `Unavailable` 的那个渠道：对账取证渠道（§6.6）、回填操作（§8.4）或一次性读 `read`（§8.2）。落点随发起者：

- 取证渠道的 gap 记在执行事实侧，属该 Attempt（§6.5）。
- 回填与一次性读的 gap 记在观察侧该流上。

**集成崩溃的观察流面。** 集成在**观察流侧**崩溃（订阅 / 推送进程掉线）时，仅波及其负责的流，记录为 `Gap{origin: Source}`，核心不受影响。这是集成崩溃两个故障面之一；另一面与两面的判别边界见 §6.7。

### cursor 与确认

每个订阅者持有一个 `Set<LogPosition>` cursor，由核心作为唯一物理写者持久化（§7.5）。

- **确认 = 消费方已处理**：消费者向核心提交“已处理到 `LogPosition` p”，核心把 cursor 推进到 p。已投递未确认的记录是消费者内存里的事。
- **重投**：确认前崩溃或断连后，核心从已确认 cursor 之后重投，所以同一记录可能被同一订阅者重复看到。重复只发生在未确认区间；消费者按 `LogPosition` 去重，每条记录的 `(StreamId, Seq)` 唯一（§2.3）。
- **已确认区间不重投**。退回 cursor 是显式控制动作（§8.5），不是恢复路径。
- **程序订阅同样受此约束**：程序状态 `Checkpoint` 与其 cursor 同事务持久化（§7.5、§8.6）。因此程序重启后从其 checkpoint 对应的 cursor 续读，不会看到已折入状态的记录。

## 4.3 派生 DAG 与程序解释①

> 图：D4.1 一轮 `Advance`（`design/diagrams/04-program-host.md`）。

程序是 **deep embedding 的小闭合值**，节点即值树（§2.5）：

```rust
struct Program { nodes: Vec<DerivationNode>, rules: Vec<DecisionStep>, state: Vec<NamedProj> }
// DerivationNode 见 §2.5；DecisionStep 见 §6.1
```

### 规则

同一个程序值有两种解释。观察半边是**解释①（派生）**；解释②见 §6.1。

- `nodes` → 增量 DAG，仅重算受影响节点，并通过 cutoff 截断。
- 输出写回派生侧 `Journal`。alert 本质上是派生观察，与外部观察同形。
- **增量在节点粒度**（哪些节点因输入变化重跑），不在算法内部。一个节点被触发时可以看它声明的完整窗口，输出相等时 cutoff 仍成立。记录渐进不要求算法渐进。[证据：fp-01 M3 Mu `Work_`；fp-05 案例 7 Incremental；域 B3/P2]
- **输入 = 位置推进**：frontier + cursor 之后的记录。程序可见 gap 与两种时间。
- **输出 = (effect 请求集, 派生记录集)**：锚点经处理器成为 Intent 或读结果。

**`Window` 与 `Pooled{window}`。** 两者都以窗口为入口，但语义与物化策略不同：

- `Window(Id, W)` 是**核心内**按位置产出值的滑动窗口节点，输出是逐条值流，走普通增量 DAG。
- `Pooled{input, window}` 把完整窗口**物化为可借用的段视图**交给原生 op（§4.5），输出是段视图，不是逐条值。

**状态正交划分：**

- **可安全重算的数据状态**：`Scan`/`Window` 累加器，归属解释①。
- **绑定不可逆外部行为的执行状态**：冷却、lane 等待、审批等待、过期，归属解释②。

### 不变量

- 执行状态不进入派生 DAG（无自反馈环）。由增量引擎的 height/cycle 约束保证。
- 核心越小且越封闭，穷尽验证、预算控制与静态分析能力越强；构造子膨胀至 TradingView 当量即退化为 Pine-with-limits。由封闭构造子集保证（§2.5）。[证据：fp-01 M9/M10；fp-05 案例 14⑥]
- **无写能力**：程序无写能力，Intent 是值而非外部调用，capability token 仅用于授权而不执行动作。预算依赖宿主进程隔离（受监督子进程，§8.6），而非类型系统（§6.1）。[证据：fp-03 命题 7；域 H2/H3]

### 为什么

值代数替代黑盒函数，是为了可预算、可静态检查、状态由结构本身保证纯性（§2.5）。

执行状态不进入派生 DAG：Incremental 的 height/cycle 约束与 Salsa 的 cycle panic 均禁止自反馈环。[证据：fp-05 案例 7⑤/9⑤；fp-01 M7 Mercury Workflow]

### 不选

- **黑盒函数 `(State, Input) -> (State, Output)`**：无法预算、无法静态检查，且状态可序列化仅依赖作者承诺。
- **增量在算法内部而非节点粒度**：记录渐进不要求算法渐进，节点粒度的 cutoff 足够且更简单。

程序表达力不足时加构造子（扩展轴，§2.5）。程序状态的序列化与重放边界，由 `Checkpoint` 与 cursor 同事务持久化给出（§4.2、§7.5、§8.6）。

## 4.4 读模型

> 图：D2.4 边的承载、D4.3 读处理器、D9.2 消费方 `read`（`design/diagrams/02-record-model.md`、`04-program-host.md`、`09-alice-session.md`）。

**读模型**（read model）是读副作用（§3.4）的一种消费产物：消费侧对执行事实 `Journal` 的**可重建、非权威** fold（订单、持仓等），经同一对外接口暴露。

- 消费方也可直接订阅原始记录自行 fold。
- 读模型不作权威、不被规则引用（§6.3），只服务下游可实现性，避免每个消费方重复 fold。
- 读模型的集合、`as_of` 与 gap 一致性见 §8.5。

### 不变量

- 读模型非权威、不被规则引用。由关系表（规则禁止引用读模型，§3.3）保证。
- 一次性读写入派生侧 `Journal`，与推送观察同形。由载体（§4.1）保证。

### 不选

- **让规则直接引用读模型（把它当权威）**：读模型是可重建 fold，一旦被规则依赖就要为它承担权威与一致性，违反“核心状态最小化”（§6.3）。
- **为每个消费方各自 fold 而不提供读模型**：利益相关者 S10 要求 Alice 消费面可实现，重复 fold 是浪费且易分叉。

## 4.5 `Pooled`：观察侧的可选扩展点

`Pooled` 是值树（§2.5）里的**读侧组合子**，也是核心暴露给可选行情派生计算子系统的唯一接口。

- 形式：`DerivationNode::Pooled { input, window }`，输出是可借用的完整窗口**段视图**，不是逐条值。
- 原生 op 是注册表（§2.1）里由子系统提供的黑盒 op。它要求输入是 `Pooled` 的，输出是一条派生观察流，下游像读任何派生流一样读它。
- “程序不是黑盒函数”对决策（解释②）继续成立。
- 子系统未安装时，含 `Pooled` 的程序在**装载期被拒绝**，其余程序不受影响。

`Pooled` 的存在理由、四条前置条件及其装载期判定、未安装 / 不满足时的失败语义、对核心的零影响与核心层最小验收，见 §8.7。子系统的实现细节在 `design/hpc-derivation/design.md`。
