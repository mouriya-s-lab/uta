# 待处理问题

本文件登记设计内容里已发现、尚未处理的问题。本次目录重构只搬运内容、调整结构，不改任何论断；下列问题留待以后逐条处理。

## 1 待设计部分被写成定论

- [README.md](README.md) §0.1、§0.2、§6.3 称四个部分在本阶段范围内都已定；集成、解释层、hpc 的文档已移到 [drafts/](drafts/)，其中各文档仍标“已定”，走查写“走通 / 卡点：无”。
- README Q24、Q30 的响应度量与 §2.5、§6.3 引用 hpc 文档 §10 的验收。
- core 以待设计部分为依据：[core/00-design.md](core/00-design.md) §2 用解释层 §4.1 论证 core 的边界；崩溃矩阵 #20（[05-runtime/00-design.md](core/05-runtime/00-design.md) §5.1）的验收是解释层的 #23；W14、W19、W20 的细化链接指向解释层文档；验收 #54、#55、#87（[03-consumption.md](core/02-read/03-consumption.md) §6.1、[05-backfill.md](core/02-read/05-backfill.md) §6.2、[02-read/00-design.md](core/02-read/00-design.md) §6.1）含集成一致性条款；集成会话 §0（[01-integration.md](core/04-boundary/01-integration.md)）把一致性测试指向集成进程文档 §6.1。

## 2 主线表述与新目录不一致

- README §1.5 与 [core/00-design.md](core/00-design.md) §3.1 说 core 的主线是两个类型宇宙与它们之间唯一的边；目录现在按效应链（单读链、读写链）组织，两种说法的关系待定。
- [core/00-design.md](core/00-design.md) §4.5 组件指南与 §4.7.1 uses 图仍按观察侧 / 效应侧分组，与新目录的划分不一致。
- 各文档“层级与元素”沿用 C4 L2 / L3 的说法，新目录不再按容器切分。

## 3 两条效应链在内容里混在一起

- 出站请求处理器用一张注册表接三条路：读请求走一次性读，写请求走单据 → STS → IO 壳，定时器由核心时钟作答。[02-requests.md](core/05-runtime/02-requests.md) §3.2 与 [03-timer.md](core/05-runtime/03-timer.md) §4.1 把定时器归为读处理器、“不另设处理器类别”。写处理器只能构造交易协议的意图（[03-intent.md](core/03-read-write/03-intent.md) §4.1 第 1 步），02-requests.md §3.1 举的 `notify.telegram` 没有路可走。
- 入站字段注册表（[02-envelope.md](core/04-boundary/02-envelope.md) §3.2）每一行的消费都定义在别处，它只是索引，却被列为一个组件；表里还装着交易协议的注册字段。
- 一次性读一个组件服务三类发起方：消费方读与程序读请求属单读链，单据的“先查后判”属读写链。
- `Gap{origin: Channel}` 同名而是两种记录（[02-read/00-design.md](core/02-read/00-design.md) §3.1）：一次性读与回填的落在观察流；取证的落在执行事实、属于尝试，“不算取证、不换渠道”（[07-resolution.md](core/03-read-write/07-resolution.md) §4.1）。
- `CapabilityObserved` 由 IO 壳 append（[06-write.md](core/03-read-write/06-write.md) §4.3），内容包括流的读 / 回填能力；[01-integration.md](core/04-boundary/01-integration.md) §3.6 曾“不选”改由集成会话 append。
- 订阅与投递用一套机制同时承载观察流、执行事实与控制流。
- “动作轴”标签与 [core/00-design.md](core/00-design.md) §3.7 矛盾：§3.7 定义写副作用是改变世界、只由 IO 壳发起；控制动作（[04-control.md](core/04-boundary/04-control.md) §4.1、§4.4）、`bypass_lane`（[05-lane.md](core/03-read-write/05-lane.md) §4.3）、单据组（[01-ticket.md](core/03-read-write/01-ticket.md) §4.3）、结果未知组（[07-resolution.md](core/03-read-write/07-resolution.md) §4.4）、`advance_retention`（[01-journal.md](core/02-read/01-journal.md) §3.2）、`restart_integration` 与 `rotate_credential`（[01-integration.md](core/04-boundary/01-integration.md) §4.8）都标“写”；`read_model` 不产生记录却标“读”（[05-views.md](core/04-boundary/05-views.md) §4.1）；`subscribe`、`route`、`handshake` 标“非动作”。
- [core/00-design.md](core/00-design.md) §3.4、§4.5 的“效应侧”含读模型、控制面、会话入口，三者都不在读写链上。
- 交易协议的两半分散：效应半边在 [02-trade-protocol.md](core/03-read-write/02-trade-protocol.md) §4.1，观察半边在 [07-trade-observation.md](core/02-read/07-trade-observation.md) §3.1–§3.3，注册字段在 [02-envelope.md](core/04-boundary/02-envelope.md) §3.2。

## 4 缺少一类效应

结果由核心自己定的效应（定时、`decide`、单据动作、`abandon`、控制动作、订阅变更、`advance_retention`）没有单独的类别，现被并入读或写。

## 5 契约放置

README §4.2 规定每个操作写在实现它的组件文档；集成作者要跨多份文档才能读全核心↔集成契约。

## 6 design/ 之外的引用

design/ 之外引用旧设计路径的文件（至少 `AGENTS.md` 描述的设计树）未随本次重构修改，需要决定是否更新。

## 7 重构过程中发现的问题

### 7.1 标题与内容不符

- [06-write.md](core/03-read-write/06-write.md) 的 H1 是“IO 壳（含对账、放弃跟踪、恢复）”，对账、放弃跟踪与恢复在 [07-resolution.md](core/03-read-write/07-resolution.md)；它的 §4.5 标题“写与取证操作的规格（核心→集成）”下只有 `submit`、`cancel` 与 `NotSent`，取证操作在 07-resolution.md §4.2。
- [01-ticket.md](core/03-read-write/01-ticket.md) 的 H1 是“单据（含交易协议）”，交易协议在 [02-trade-protocol.md](core/03-read-write/02-trade-protocol.md)。
- [03-consumption.md](core/02-read/03-consumption.md) 的 H1 是“持久订阅”，本文另含投递调度：§0 下的“投递调度”定位块、§1.3–§1.4、§2 第二张表、§3.6–§3.10、§4.6–§4.9、§5.7–§5.10。

### 7.2 合并与拆分留下的接缝

- 03-consumption.md 有两份定位（持久订阅与投递调度），各自的“本组件 / 本文”指不同元素，两份非目标互相排除对方的内容；§2 有两张驱动表，§5 有两处卡点（§5.6、§5.10）。
- 03-consumption.md 的 §1.1–§1.2、§2、§3.4 不变量 4–8 与 §3.5 术语（回填任务、到达 / 闭合等）仍写供给、回填与序号覆盖；[04-supply.md](core/02-read/04-supply.md)、[05-backfill.md](core/02-read/05-backfill.md) 没有自己的 §0–§2。
- 06-write.md 的 §1.1 需求与 §3.5 不变量含属于 07-resolution.md 的条目（三渠道收敛、等待至多结束一次、`Undetermined` 的收敛、`Inconclusive` 停等）；07-resolution.md 没有 §0–§2。
- [05-views.md](core/04-boundary/05-views.md) 的 §1.2 域性质（上游的执行身份、订单身份、来源顺序）与 §2 的 Q1/Q5 一行写的是 [07-trade-observation.md](core/02-read/07-trade-observation.md) 的内容；07-trade-observation.md 没有 §0–§2。
- [02-one-shot-read.md](core/02-read/02-one-shot-read.md) §4.5 读处理器（来自出站请求处理器）里的“本组件”原指出站请求处理器，现在指代不明；这一节带来的 W17 段与原有的 W17 走查重叠；验收 #50 含写门、`read_model`、持久订阅的供给与配额，超出一次性读。
- [02-read/00-design.md](core/02-read/00-design.md) 的“gap 的三种来源”与验收 #87 含写侧、取证与 `submit` 的内容，却在单读链目录；验收 #87 两次链接同一处 01-integration.md §4.3.2。
- 07-resolution.md §4.6 说效应侧归因处理器“不单独成文”、是入站处理器注册表“效应侧分区”的处理器，它现在是 07-resolution.md 的一节；§5“W5 扩展路径的取证细化”里“读到目标（任何状态）即 `Found`”与 §4.1 撤单尝试“目标订单自己的记录不归因到撤单尝试”之间可能有语义接缝。
- [06-derivation.md](core/02-read/06-derivation.md)、[03-timer.md](core/05-runtime/03-timer.md)、[03-intent.md](core/03-read-write/03-intent.md)、[01-value-tree.md](core/01-base/01-value-tree.md) 里来自程序宿主进程或核心进程文档的段落仍写“本进程”，独立成文后指代不明；06-derivation.md 里“本进程”与“本组件”并列，所指不同。
- [01-program-host.md](core/05-runtime/01-program-host.md) §6.2 引言说“另有两项关于程序输入与输出的整体取舍”，其下有五条；输出相关的另几条在 06-derivation.md §6.2。
- 04-supply.md、05-backfill.md 里来自核心进程的验收条目（#55、#57、#84）写作 `**#N …**`、子项缩进 4 空格，来自持久订阅的写作 `**验收 #N …**`、子项缩进 2 空格；[01-integration.md](core/04-boundary/01-integration.md) 里来自核心进程的验收 #43、#73 子项缩进 4 空格，原有的 2 空格。验收 #57 含持仓跨流冲突的检查，验收 #28 含配额与数据等级的检查，混合条目整条留在一处。
- [05-runtime/00-design.md](core/05-runtime/00-design.md) 验收 #24 写“启动第 5 步之前，`handshake` 成功，其余操作都返回 `Starting`”，§4.1.3 写“消费方在第 5 步之前连接，得到 `Starting`”，两处措辞可能不一致。

### 7.3 结构缺口

- 新拆出的 01-value-tree.md、04-supply.md、05-backfill.md、06-derivation.md、07-trade-observation.md、03-intent.md、02-trade-protocol.md、07-resolution.md、03-timer.md 与 [06-files.md](core/04-boundary/06-files.md) 没有 §0 定位；它们的范围只写在所在目录 `00-design.md` 的“本目录”一行与 [core/00-design.md §4.4 模块地图](core/00-design.md#44-模块地图)。
- 多数组件文档的“上级文档”仍指核心进程（05-runtime/00-design.md）或 core/00-design.md §4.5 组件指南，不指所在目录的 `00-design.md`（§2 第三条的一部分）。
- core/00-design.md 同时写 C4 L1（core 作为黑盒）与 L2（核心进程的模型、组件与走查）；§6 引言“本层的决定只有一个”说的是 L1。

### 7.4 指向与所说内容不符的引用

- [02-envelope.md](core/04-boundary/02-envelope.md) §3.1“意图的构造前提见 ticket §3.4 参数合规”：意图的锚点构造在 01-ticket.md §3.3。
- 05-views.md §4.3“gap 三种来源的定义见 01-base/00-design.md §3.1 流、位置与三种进度”：gap 的三种来源在 02-read/00-design.md §3.1。
