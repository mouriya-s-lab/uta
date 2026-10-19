# UTA

UTA（Unified Trading Agent）是 OpenAlice 的独立进程，位于上游（券商、交易所）与下游（Alice、CLI 使用者、外部客户程序）之间：经集成接触上游，持有观察流、意图与全部交易写入的记录；账户、订单、持仓的原值在上游，不在 UTA。下游只经解释层接触 UTA；Alice 是下游之一，与 UTA 生命周期互相独立。本仓库是 UTA 的设计与实现仓库。

设计文档位于 `design/`，采用 `ssot`（文学式 VDM-SL 规格 + 最小补充文字），从 `design/README.md` 的阅读次序读起：

- **架构模块（`design/architecture/`）**：定义与具体业务无关的核心连接、状态与不变量。根模块含 `Source`（`design/architecture/source.md`，外部来源会话存续与当前有效能力判定）与 `Observation`（`design/architecture/observation.md`，观察流位置、消费投递、保留边界与活跃引用保护）；子模块为 `Effect`（`design/architecture/effect/effect.md`，受控单次外部写尝试阶段、发送前屏障、同通道未决阻塞、来源证据决议与放弃本地等待）。
- **业务单元（`design/business/`）**：只经架构导出的连接接入，定义业务在架构连接上的运行与组合规律，包括 `Ticket`（`design/business/ticket/ticket.md`，单据责任人、版本绑定审批与放行门）、`Trading`（`design/business/trading/trading.md`，下单/改单/撤阻塞头组合、成交按执行身份与修订号归并、订单状态按来源定序证据求偏序极大元）与 `Program`（`design/business/program/program.md`，确定性程序按输入游标推进检查点、产出派生观察与提交单据）。
- **附录（`design/appendix/`）**：存放支撑依据的外部事实证据（如 `design/appendix/evidence/venue-facts.md`），不设立规范规则。

代码在 Cargo 工作区（`crates/`、`xtask/`）；crate 划分、依赖方向、线程与任务模型、所有权与派生规则、线缆约定在根目录 `ARCHITECTURE.md`。

## 阶段与范围

- UTA 正在**从零重写**为独立 Rust 二进制，已进入**预开发阶段**：按切片实现，每个切片只创建在该切片里有真实调用方与运行期路径的 crate，其余落点登记在 `ARCHITECTURE.md` 第 2 节。改代码前先读 `ARCHITECTURE.md`；新增 crate 同时在 `xtask` 的依赖表里登记它的层。
- 跨仓库契约是解释层的对外面：CLI 命令与双向长连接的消息协议。它们的 schema 与 CLI 由本仓库拥有并随 release 发布，OpenAlice 从 release artifact 消费。核心↔解释层的 JSON-RPC IDL 只在本仓库内部；核心↔集成的 IDL 随 release 发布给集成作者。

## 设计与实现的分界

设计文档编写与修改严格遵守 `.claude/rules/design-ssot.rule.md` 与 `ssot` skill：

1. **三层分离**：
   - `design/architecture/` 只放领域无关的权威、生命周期、连接与不变量，绝不引用业务单元。
   - `design/business/` 只写业务单元如何通过架构连接运行与组合，不把常变的业务阈值、允许集合、冷却参数或上游协议字段搬进设计。
   - 在连接签名与不变量不变时可替换的一切做法（SQLite 表结构与事务批次、进程内任务/通道分工、线缆帧格式、AST 节点枚举、启动/停止分步脚本、错误原因细分）均属实现，只写在代码与 `ARCHITECTURE.md` 中，不进 `design/`。
2. **先立 VDM 框架，再填内容**：改设计时先定模块划分、`imports`/`exports` 连接签名与 `token` 占位并跑通 `ssot check design` 与 `ssot graph design`，再填隐式 `post`、状态 `inv`、显式场景与最小补充文字，最终必须同时通过 `ssot check design`（0 错误、0 警告）与 `jev` 模型反模式审查（0 检出）。

## 设计纪律

- **权威不在 UTA**：订单、持仓、余额、行情的原值都在上游，UTA 只有意图及其推进记录（`design/architecture/README.md`）。不写按上游记录列全字段的 `Order`/`Account`/`Position` 权威对象；上游协议只在集成进程内消费。
- **权威源头与生命周期**：写任何状态、记录或连接之前，先确定它的唯一权威源头、记录性质（自有事实 / 取回的副本 / 索引或意图）与生命周期开始/结束锚点及确认者；非自有状态只向源头请求并等待结论（如 `Source`CurrentCapability` 在离线时返回 `<Unavailable>`，`Effect`Resolve` 只以来源证据 `<Found> | <Absent>` 终结 `Effect`InDoubt`）。
- **冲突处理**：发现文档之间或文档内部的冲突，核实后直接替换冲突内容，不留遗留层；历史交给 git。

## 仓库规则

- 改文件前先 `git fetch origin` 与 `git status -sb`；保留他人未提交的改动，不 reset、不 stash、不覆盖。
- 主分支 `main`。本地 clone 位于 `~/Ext/code/uta`。
- Secrets 不进任何跟踪文件、日志或 PR 正文。
