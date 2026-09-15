# UTA 重构文档 triage 清单

权威：`design/uta-core-design.md`（其 §15 界定保留边界）。保留 = 核心设计 / fp 证据链 / 问题域事实（§1 与附录 B 及 §1 引用的 investigation 报告）。删除 = 旧模型/结构/状态机/协议/字段/ADR/走查，或与 UTA 无关的方法论研究。

| 路径 | 判定 | 理由 |
|---|---|---|
| `plans/uta-refactor/design/uta-core-design.md` | 保留 | 唯一权威核心设计文档 |
| `plans/uta-refactor/design/research/fp-00-synthesis.md` | 保留 | 证据链综合索引（核心设计 §15 指定） |
| `plans/uta-refactor/design/research/fp-01-haskell-finance-cases.md` | 保留 | 一手证据链（Haskell 金融案例） |
| `plans/uta-refactor/design/research/fp-02-scala-jvm-cases.md` | 保留 | 一手证据链（Scala/JVM 案例） |
| `plans/uta-refactor/design/research/fp-03-effect-composition-and-open-providers.md` | 保留 | 一手证据链（效应组合/开放 provider） |
| `plans/uta-refactor/design/research/fp-04-base-types-and-domain-primitives.md` | 保留 | 一手证据链（基础类型/域原语） |
| `plans/uta-refactor/design/research/fp-05-streams-incremental-frp.md` | 保留 | 一手证据链（流/增量/FRP） |
| `plans/uta-refactor/design/investigation/venue-capabilities.md` | 保留 | §1 F6/F7/1.4.1 venue 原生能力事实证据 |
| `plans/uta-refactor/design/investigation/rust-feasibility.md` | 保留 | §1 F5/1.4.2 Rust 生态可行性事实证据 |
| `plans/uta-refactor/design/investigation/existing-capabilities.md` | 保留 | §1 O11/1.4.3 既有缺陷事实证据 |
| `plans/uta-refactor/design/investigation/alice-consumers.md` | 保留 | §1 O10/S10/S11/1.4.4 Alice 消费面事实证据 |
| `plans/uta-refactor/design/problem-domain.md` | 新建（保留） | 原 uta-design.md §1 + 附录 B 逐字迁移；核心设计 §15 继续有效 |
| `plans/uta-refactor/report/00-overview.md` | 删除 | 旧 UTA 代码调查总览；未被 §1 事实表或 fp-* 引用 |
| `plans/uta-refactor/report/01-process-lifecycle.md` | 删除 | 旧进程生命周期代码调查；未被 §1 事实表或 fp-* 引用 |
| `plans/uta-refactor/report/02-http-api-and-protocol.md` | 删除 | 旧 HTTP API/协议代码调查；未被 §1 事实表或 fp-* 引用 |
| `plans/uta-refactor/report/03-account-orders-positions.md` | 删除 | 旧账户/订单/持仓代码调查；未被 §1 事实表或 fp-* 引用 |
| `plans/uta-refactor/report/04-market-data-contracts-fx.md` | 删除 | 旧行情/合约/FX 代码调查；未被 §1 事实表或 fp-* 引用 |
| `plans/uta-refactor/report/05-staging-approval-ledger.md` | 删除 | 旧 staging/审批/帐票代码调查；未被 §1 事实表或 fp-* 引用 |
| `plans/uta-refactor/report/06-snapshots-and-guards.md` | 删除 | 旧快照/guard 代码调查；未被 §1 事实表或 fp-* 引用 |
| `plans/uta-refactor/report/07-brokers-and-packs.md` | 删除 | 旧 broker/pack 代码调查；未被 §1 事实表或 fp-* 引用 |
| `plans/uta-refactor/report/08-alice-consumers-and-ui.md` | 删除 | 旧 Alice 消费面/UI 代码调查；未被 §1 事实表或 fp-* 引用 |
| `plans/uta-refactor/report/09-persisted-state-tests-docs-issues.md` | 删除 | 旧持久化/测试/文档缺陷调查；未被 §1 事实表或 fp-* 引用 |
| `plans/uta-refactor/report/10-alice-config-persistence.md` | 删除 | 旧 Alice 配置持久化调查；未被 §1 事实表或 fp-* 引用 |
| `plans/uta-refactor/report/11-alice-event-flow.md` | 删除 | 旧 Alice 事件流调查；未被 §1 事实表或 fp-* 引用 |
| `plans/uta-refactor/report/12-haskell-effect-systems.md` | 删除 | 旧 Haskell 效应系统研究，已被 fp-01..05 取代；未被 §1 事实表或 fp-* 引用 |
| `plans/uta-refactor/report/13-haskell-provider-adapters.md` | 删除 | 旧 Haskell provider 适配研究，已被 fp-01..05 取代；未被 §1 事实表或 fp-* 引用 |
| `plans/uta-refactor/report/14-haskell-ledgers-and-consistency.md` | 删除 | 旧 Haskell 账本/一致性研究，已被 fp-01..05 取代；未被 §1 事实表或 fp-* 引用 |
| `plans/uta-refactor/design/research/industry.md` | 删除 | 通用设计文档方法论研究，非 fp-00..05 证据链，未被 §1/附录 B 引用 |
| `plans/uta-refactor/design/research/literature.md` | 删除 | 通用设计文档方法论研究，非 fp-00..05 证据链，未被 §1/附录 B 引用 |
| `plans/uta-refactor/design/research/methods.md` | 删除 | 通用设计文档方法论研究，非 fp-00..05 证据链，未被 §1/附录 B 引用 |
| `plans/uta-refactor/design/investigation/rust-feasibility-plan.md` | 删除 | 旧调查计划稿；§1 只引用报告本体 rust-feasibility.md |
| `plans/uta-refactor/design/adr/ADR-001-rewrite-rust-process.md` | 删除 | 基于旧设计的 ADR 决定（重写为 Rust 进程） |
| `plans/uta-refactor/design/adr/ADR-002-five-axes.md` | 删除 | 基于旧设计的 ADR 决定（五轴投影（被核心设计取代）） |
| `plans/uta-refactor/design/adr/ADR-003-write-ahead-recovery.md` | 删除 | 基于旧设计的 ADR 决定（写前恢复） |
| `plans/uta-refactor/design/adr/ADR-004-lane-unknown-bypass.md` | 删除 | 基于旧设计的 ADR 决定（lane/unknown 旁路） |
| `plans/uta-refactor/design/adr/ADR-005-ranges-gaps.md` | 删除 | 基于旧设计的 ADR 决定（范围/gap） |
| `plans/uta-refactor/design/adr/ADR-006-subscriptions-ack.md` | 删除 | 基于旧设计的 ADR 决定（订阅 ack） |
| `plans/uta-refactor/design/adr/ADR-007-program-executor.md` | 删除 | 基于旧设计的 ADR 决定（程序执行器） |
| `plans/uta-refactor/design/adr/ADR-008-single-writer-fence.md` | 删除 | 基于旧设计的 ADR 决定（单写者栅栏） |
| `plans/uta-refactor/design/adr/ADR-009-clock-deadline.md` | 删除 | 基于旧设计的 ADR 决定（时钟/期限） |
| `plans/uta-refactor/design/adr/ADR-010-session-principal.md` | 删除 | 基于旧设计的 ADR 决定（会话/principal） |
| `plans/uta-refactor/design/adr/ADR-011-control-plane.md` | 删除 | 基于旧设计的 ADR 决定（控制面） |
| `plans/uta-refactor/design/adr/ADR-012-integration-processes.md` | 删除 | 基于旧设计的 ADR 决定（集成进程） |
| `plans/uta-refactor/design/adr/ADR-013-storage-log.md` | 删除 | 基于旧设计的 ADR 决定（存储日志） |
| `plans/uta-refactor/design/adr/ADR-014-transport-idl.md` | 删除 | 基于旧设计的 ADR 决定（传输 IDL） |
| `plans/uta-refactor/design/adr/ADR-015-derived-streams.md` | 删除 | 基于旧设计的 ADR 决定（派生流） |
| `plans/uta-refactor/design/adr/README.md` | 删除 | 基于旧设计的 ADR 决定（ADR 索引） |
| `plans/uta-refactor/design/contracts/01-operations.md` | 删除 | 定义旧 UTA 操作契约（旧设计结构） |
| `plans/uta-refactor/design/contracts/02-idl-and-types.md` | 删除 | 定义旧 UTA IDL/类型/字段（旧设计结构） |
| `plans/uta-refactor/design/walkthrough/observation.md` | 删除 | 基于旧设计（模型/结构/协议）的场景走查 |
| `plans/uta-refactor/design/walkthrough/effects.md` | 删除 | 基于旧设计（模型/结构/协议）的场景走查 |
| `plans/uta-refactor/design/walkthrough/recovery.md` | 删除 | 基于旧设计（模型/结构/协议）的场景走查 |
| `plans/uta-refactor/design/walkthrough/programs.md` | 删除 | 基于旧设计（模型/结构/协议）的场景走查 |
| `plans/uta-refactor/design/walkthrough/round2/programs.md` | 删除 | 基于旧设计（模型/结构/协议）的场景走查 |
| `plans/uta-refactor/design/walkthrough/round2/recovery-control.md` | 删除 | 基于旧设计（模型/结构/协议）的场景走查 |
| `plans/uta-refactor/design/walkthrough/round2/observation.md` | 删除 | 基于旧设计（模型/结构/协议）的场景走查 |
| `plans/uta-refactor/design/walkthrough/round2/effects.md` | 删除 | 基于旧设计（模型/结构/协议）的场景走查 |
| `plans/uta-refactor/design/walkthrough/round3/control-recovery.md` | 删除 | 基于旧设计（模型/结构/协议）的场景走查 |
| `plans/uta-refactor/design/walkthrough/round3/delivery-programs.md` | 删除 | 基于旧设计（模型/结构/协议）的场景走查 |
| `plans/uta-refactor/design/walkthrough/round4/confirm.md` | 删除 | 基于旧设计（模型/结构/协议）的场景走查 |
| `plans/uta-refactor/design/uta-design.md` | 删除 | §1+附录 B 已迁移至 problem-domain.md；§3–§6 旧设计中心被 uta-core-design.md 取代 |

> 说明：维护者在本次 triage 前已在工作区将若干旧设计文件标记删除（旧命名 ADR、`contracts/03-field-audit.md`、`investigation/broker-capabilities.md`、`research/maintainer-quotes.md`、`walkthrough/{confirm,fills,split-brain,streams-reads-recovery,write-and-unknown}.md`），本清单不重复处理这些已 `D` 的文件。
