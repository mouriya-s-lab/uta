# UTA

UTA（Unified Trading Agent）是 OpenAlice 的独立进程：拥有 broker 连接、账户、观察流与全部交易写入。Alice 启动 Workspace 并注入交易上下文，通过 JSON-RPC 与 UTA 通信；两者生命周期互相独立。本仓库是 UTA 的设计与实现仓库；旧 UTA 实现留在 OpenAlice 仓库（`services/uta/`、`packages/uta-protocol/`、`src/services/uta-client/`），不具备正确性，只作"既有机器事实"（`design/problem-domain.md` O 表）被引用。

## 现状：整体重写，当前处于设计阶段

- UTA 正在**从零重写**为独立 Rust 二进制。当前阶段是**设计**，不是实现。不要开始写实现代码，除非维护者明确要求。
- 跨仓库契约只有一个：UTA 的 JSON-RPC IDL（二进制编码同一 IDL）。由本仓库拥有并随 release 发布；OpenAlice 从 release artifact 消费。

## 设计文档（唯一权威）

|文件|作用|
|---|---|
|`design/uta-core-design.md`|核心设计：设计中心、边界、验收标准、未决 spike。所有类型、模块、协议必须能从这里的代数组合出来|
|`design/problem-domain.md`|问题域事实（F/O/S/H/P/C 编号）与维护者原话（B/C）。域事实只从这里取|
|`design/research/fp-00..12-*.md`|一手案例调查，核心设计每条 [证据] 的出处（00–05 FP 案例；06 对账/in-doubt；07 类型导出与外部编译；08 段池选库；09 原生 op 算法层/VectorTA、AoS/SoA、Rust SIMD；10 数组中间层与真实指标实测；11 闸门 9 原语层 demo；12 L2 订单簿 demo）|
|`design/investigation/*.md`|问题域 §1 引用的旧系统与 venue 能力调查|
|`design/hpc-derivation-subsystem.md`|行情派生高性能计算子系统（`Pooled` → 段池 → 原生 op）：**可选、独立、不属于核心**；核心 §5.3 只定接口|
|`design/decision-log.md`|维护者原话与裁决归档；正文与之冲突时以更晚条目为准；F 表列出已知未对齐处|
|`design/native-computation-design-handoff.md`|自定义原生计算的场景与约束交接稿；核心 §5.3 为其落点|
|`TRIAGE.md`|遗留文档裁决记录|

## 设计阶段纪律

- **设计中心优先**：任何新文档、类型、协议先回答"它是核心设计哪个代数的组合/解释结果"。答不上来就不写。
- **禁止业务对齐大对象**：不写把字段列全的 `Order`/`Account`/`Position` 权威对象；订单、持仓、审批是日志 fold 或解释结果。
- **证据纪律**：论断标 [证据]（研究报告编号）/ [设计] / [spike]；条件式命题不得写成无条件结论；运行期未知不得冒充静态保证。
- **不偏离位置**：新设计文档放在 `design/` 下并在 `uta-core-design.md` §15 登记。
- 研究类工作只读一手来源（论文、官方文档、公开仓库 pin commit），不凭回忆写。实验产物放 `/Users/mouriya/Ext/tmp/`（独立硬盘），不进仓库。

## 仓库基本规则

- 改文件前 `git fetch origin`、`git status -sb`；保留他人未提交改动，不 reset/stash/覆盖。
- 主分支 `main`。设计阶段的文档改动直接提交 `main`（维护者裁定）；进入实现阶段后再启用分支/PR 流程。本地 clone 位于 `~/Ext/code/uta`。
- Secrets 不进任何跟踪文件、日志、PR 正文。
