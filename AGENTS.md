# UTA

UTA（Unified Trading Agent）是 OpenAlice 的独立进程。从上游到下游分五层：**提供商**（券商、交易所，账户、订单、持仓、行情的原值在这里）→ **集成**（每个提供商一个外挂进程，只向核心交出固定头加原文的信封）→ **核心**（本仓库的主体：观察日志、效应请求、每一次外部写尝试的记录、订阅投递；映射在核心内装载执行）→ **解释层**（CLI 与双向长连接，不持有状态）→ **Alice / CLI 使用者 / 外部客户程序**（下游，与 UTA 生命周期互相独立）。本仓库是 UTA 的设计与实现仓库。

设计文档位于 `design/`，采用 `ssot`（文学式 VDM-SL 规格 + 最小补充文字），从 `design/README.md` 的阅读次序读起：

- **架构模块（`design/architecture/`）**：与具体业务无关的核心连接、状态与不变量。根模块 `Source`（来源会话与握手声明）、`Mapping`（每来源一份当前映射：解码、编码、通道键、取证分类、能力合成；不记版本，失配报错）、`Observation`（观察日志：分代、核心分配的位置、来源序号覆盖、保留边界、引用保护、依据有效性）；子模块 `delivery/Delivery`（持久订阅、游标、投递缺口）、`effect/Request`（效应请求是值，种类由处理器开放注册，答复作为流事件回投）、`effect/Write`（核心内唯一的外部写发出点：准入、持久屏障、等待轴与结果轴分离、取证收敛、放弃即死信）。
- **业务单元（`design/business/`）**：只经架构导出的连接接入。`Ticket`（单据：锁、线性版本链、绑定版本的裁决、放行门，以及接手写请求的唯一入口）、`Trading`（交易操作封闭集、成交归并与订单状态偏序判定、改单按能力分流、撤阻塞头）、`Program`（确定性程序按输入订阅推进检查点，产出派生记录与效应请求）。
- **附录（`design/appendix/`）**：`evidence/` 存放依据的外部事实证据（`venue-facts.md`、`integration-facts.md`、`lane-facts.md`、`approval-facts.md`），不设立规范规则。

代码在 Cargo 工作区（`crates/`、`xtask/`）。crate 划分与依赖方向由 `xtask/src/main.rs` 的允许表定义并经 `cargo xtask check-deps` 检查；每个 crate 的职责、所有权与线缆约定写在该 crate 的 `lib.rs` 模块文档里，不另设总览文档。

## 阶段与范围

- UTA 正在**从零重写**为独立 Rust 二进制，已进入**预开发阶段**：按切片实现，每个切片只创建在该切片里有真实调用方与运行期路径的 crate；新增 crate 同时在 `xtask` 的允许表里登记它的依赖。切片顺序按设计依赖：核心↔集成契约 → `Source` → `Observation`/`Delivery` → `Mapping` → `Request`/`Write` → `Ticket`/`Trading`/`Program` → 解释层。
- 跨仓库契约是解释层的对外面：CLI 命令与双向长连接的消息协议。它们的 schema 与 CLI 由本仓库拥有并随 release 发布，OpenAlice 从 release artifact 消费。核心↔解释层的 JSON-RPC IDL 只在本仓库内部；核心↔集成的 IDL 随 release 发布给集成作者。
- 映射 DSL 是实现：它的语法、装载方式与表达力只写在代码里，设计只规定 `Mapping` 导出的连接与失配结局。

## 设计与实现的分界

设计文档编写与修改严格遵守 `.claude/rules/design-ssot.rule.md` 与 `ssot` skill：

1. **三层分离**：
   - `design/architecture/` 只放领域无关的权威、生命周期、连接与不变量，绝不引用业务单元。
   - `design/business/` 只写业务单元如何通过架构连接运行与组合，不把常变的业务阈值、允许集合、冷却参数或上游协议字段搬进设计。
   - 在连接签名与不变量不变时可替换的一切做法（SQLite 表结构与事务批次、进程内任务/通道分工、线缆帧格式、映射 DSL 的语法与 AST、启动/停止分步脚本、错误原因细分）均属实现，只写在代码里，不进 `design/`。
2. **先立 VDM 框架，再填内容**：改设计时先定模块划分、`imports`/`exports` 连接签名与 `token` 占位并跑通 `ssot check design` 与 `ssot graph design`，再填隐式 `post`、状态 `inv`、显式场景与最小补充文字，最终必须同时通过 `ssot check design`（0 错误、0 警告）与 `jev` 模型反模式审查（0 检出）。

## 设计纪律

- **权威不在 UTA**：订单、持仓、余额、行情的原值都在上游，UTA 只有意图及其推进记录（`design/architecture/README.md`）。不写按上游记录列全字段的 `Order`/`Account`/`Position` 权威对象；上游协议原文经信封原样进出，核心只认映射抽出的字段。
- **权威源头与生命周期**：写任何状态、记录或连接之前，先确定它的唯一权威源头、记录性质（自有事实 / 取回的副本 / 索引或意图）与生命周期开始/结束锚点及确认者；非自有状态只向源头请求并等待结论（如 ``Mapping`CurrentCapability`` 在离线时返回 `<Unavailable>`，``Write`Resolve`` 只以来源证据终结 `<Unknown>`，``Write`Abandon`` 不断言来源是否执行）。
- **写通道按品种**：``Mapping`LaneKey`` 由映射从意图按品种导出，阻塞只在同一品种内；资金面能否成交只由上游裁决，本地只在放行门上读一次，不做阻塞依据。
- **冲突处理**：发现文档之间或文档内部的冲突，核实后直接替换冲突内容，不留遗留层；历史交给 git。

## 仓库规则

- 改文件前先 `git fetch origin` 与 `git status -sb`；保留他人未提交的改动，不 reset、不 stash、不覆盖。
- 主分支 `main`。本地 clone 位于 `~/Ext/code/uta`。
- Secrets 不进任何跟踪文件、日志或 PR 正文。
