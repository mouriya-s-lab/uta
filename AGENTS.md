# UTA

UTA（Unified Trading Agent）是 OpenAlice 的独立进程：拥有 broker 连接、账户、观察流与全部交易写入。Alice 启动 Workspace 并注入交易上下文，通过 JSON-RPC 与 UTA 通信；两者生命周期互相独立。本仓库是 UTA 的设计与实现仓库；旧 UTA 实现留在 OpenAlice 仓库（`services/uta/`、`packages/uta-protocol/`、`src/services/uta-client/`），不具备正确性，只作"既有机器事实"（`design/uta-design.md` §1.1 O 表）被引用。

## 现状：整体重写，当前处于设计阶段

- UTA 正在**从零重写**为独立 Rust 二进制。当前阶段是**设计**，不是实现。不要开始写实现代码，除非维护者明确要求。
- 跨仓库契约只有一个：UTA 的 JSON-RPC IDL（二进制编码同一 IDL）。它的操作集与语义由 `design/uta-design.md` §6.3 定义；IDL 文件是实现阶段产物，将由本仓库拥有并随 release 发布；OpenAlice 从 release artifact 消费。
- **hpc 不在当前阶段范围内**：`design/hpc-derivation/`（`design.md` 与 `research/fp-07..12-*.md`）是前期补充设计，当前阶段完全不考虑、不验证其正确性、不据它调整核心。核心设计的完备性审查以 `design/uta-design.md` 为对象，只需保证 §6.6 的 `Pooled` 接口在核心侧自包含（核心无子系统时完整可运行）；主设计中对 `hpc-derivation/design.md` 章节号的引用（§0.3、§1.4 Q24/Q30、§7.2 #17–#19、§8.3、§8.5 #7、附录 A）只作指针，不作为核心结论的依据。

## 设计文档

|文件|作用|
|---|---|
|`design/uta-design.md`|主设计：问题域与驱动、设计中心、边界契约、走查、评估与未决；各阶段内容都在这一份里。域事实取自 §1|
|`design/hpc-derivation/design.md`|可选行情派生计算子系统，独立模块，前期补充设计；当前阶段不在考虑范围内（见上）。与核心的接口在主设计 §6.6，其证据在 `design/hpc-derivation/research/fp-07..12-*.md`|
|`design/research/fp-00..06-*.md`|核心设计的一手案例调查，主设计 [证据] 的出处；索引在主设计附录 A|
|`design/investigation/*.md`|旧系统与 venue 能力调查，主设计 §1 域事实的证据|

## 设计阶段纪律

- **设计中心优先**：任何新文档、类型、协议先回答"它是核心设计哪个代数的组合/解释结果"。答不上来就不写。
- **禁止业务对齐大对象**：不写把字段列全的 `Order`/`Account`/`Position` 权威对象；订单、持仓、审批是日志 fold 或解释结果。
- **证据纪律**：论断标 `[证据]`/`[设计]`/`[推断]`（含义见 `uta-design.md` §0.5）；条件式命题不得写成无条件结论；运行期未知不得冒充静态保证。域事实只从 `design/uta-design.md` §1 取。设计未决项不再以 `[暂定]`/`[未决]` 标签登记：§8.3 说明为何没有未决设计项，需要实测的量进 §8.5 验收项，会推翻决定的观测进 §8.4 证伪条件。
- **新设计内容并入 `uta-design.md` 对应章节**；只有能形成干净决定边界、核心无它仍完整可运行的子系统才单独成文，并在 §6 登记接口。
- **冲突处理**：发现文档间或文档内冲突时直接核实并修改冲突内容，不以声明权威/以某文为准解决；历史交给 git。
- 研究类工作只读一手来源（论文、官方文档、公开仓库 pin commit），不凭回忆写。实验产物放 `/Users/mouriya/Ext/tmp/`（独立硬盘），不进仓库。

## 仓库基本规则

- 改文件前 `git fetch origin`、`git status -sb`；保留他人未提交改动，不 reset/stash/覆盖。
- 主分支 `main`。设计阶段的文档改动直接提交 `main`（维护者裁定）；进入实现阶段后再启用分支/PR 流程。本地 clone 位于 `~/Ext/code/uta`。
- Secrets 不进任何跟踪文件、日志、PR 正文。
