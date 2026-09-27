# 控制面

## 0 定位

- **层级与元素**：L3 component。核心进程内的“控制面”组件：认证 principal 传入的运维动作通道（P14）。
- **上级文档**：[core-process/design.md §4.1 组件指南](design.md#41-组件指南)（核心进程 L2）。
- **本文决定什么**：控制组全部动作共用的部分——谁能发、怎样授权、结论记成什么、读到的文件怎样留痕、受控停止期间怎样结束；以及 `reload_config` 的完整规格。
- **读者**：核心实现者；解释层作者（控制组的结果语义）。
- **状态**：已定。
- **非目标**：除 `reload_config` 之外每个控制动作的生效语义与错误，写在实现它的组件里（4.1 表），本文不重新定义。规则文件与运行期参数文件的内容规格在使用它们的组件里（4.4）。控制流作为记录模型的定义在 [core-process/design.md §4.5 持久化视图](design.md#45-持久化视图)。

标签与编号约定见 [README.md §0.4 阅读约定](../../README.md#04-阅读约定)。

## 1 问题域

### 1.1 上级分配的需求

| 编号 | 本组件承担的部分 |
|---|---|
| P14 | 控制现象的入口：principal、配置变更、凭据轮换、重启集成、装卸程序、请求快照；结果（生效 / 拒绝 + 原因） |
| C11、H7 | 控制动作按认证 principal 授权，不按连接 |
| Q18 | 改策略：下一笔意图使用新策略；控制结果可读 |
| Q19 | 已认证但无 scope 的控制请求被拒、记安全事件、无其他副作用 |
| C14 | 配置文件带格式版本；重载失败保留上一有效版本 |

### 1.2 本组件直接面对的域

- **运维者与下游**：经解释层、以会话绑定的 principal 发出控制动作（[session-entry.md §3 模型](session-entry.md#3-模型)）。同一 OS 用户的进程本就能终止核心或改文件（H7），所以控制面防的是越权的误操作与审计缺口，不是同用户攻击者。
- **Alice 写的统一路径文件**：每个文件唯一写者是 Alice；以整文件原子替换（临时文件 + rename）写入，不存在半写可见态 [证据：O9，`existing-capabilities.md:189`]。文件带格式版本，只前进（C14）。核心只读，只在规定时刻读（[core-process/design.md §4.6 统一路径文件](design.md#46-统一路径文件)）。

## 2 驱动

| 场景 | 本组件的响应度量 | 验收 |
|---|---|---|
| Q19 | 未授权控制请求：无 venue 调用、无配置变更、除控制记录与安全事件外没有记录 | #13 |
| Q18 | 规则重载之后的每条 `Outcome` / `Rejection` 带新规则版本；重载失败时生效版本 hash 不变 | [decision-chain.md §6.3 验收](decision-chain.md#63-验收) |

## 3 模型

- **控制动作是一次带 principal 的请求，它的结论是一条控制记录** [设计]：每个动作以 `Applied(position) | Rejected(reason)` 结束，结论作为控制记录 append 为执行事实，带 principal、动作、所读配置文件的内容 hash；生效时的规则版本按 [decision-chain.md §4.6 规则版本变更](decision-chain.md#46-规则版本变更-设计) 写入。控制记录都在控制流上，只有 `bypass_lane` 的随它所绕过的 lane 的流（[lane.md §4.3 显式绕过 bypass_lane(ticket)](lane.md#43-显式绕过-bypass_laneticket)）。
- **结论是持久事实，不是返回值**：发起它的消费方会话可能先于结论结束（会话不拥有它，[session-entry.md §4.4 会话的生命周期](session-entry.md#44-会话的生命周期-设计)）；发起方重连之后从记录得知结论。
- **读到的文件以 hash 记下**：控制动作读到的文件，其内容 hash 进入这条控制记录。文件内容的源头仍是 Alice；控制记录说的是“核心在这一刻按这一版生效”这件核心自己的事实。
- **只经认证 principal，不经进程信号或 flag 文件**：信号与 flag 文件没有 principal，无从授权与审计。
- **不变量**：
  1. 每个被接受执行的控制动作至多一条结论记录；没有结论记录的动作没有生效。
  2. 未授权的动作不产生除 `Rejected(Unauthorized)` 与安全事件之外的任何记录，不读它要读的配置文件，不进入实现组件。
  3. 核心不写统一路径下的任何文件。
  4. 重载失败时生效的配置不变。

## 4 结构与接口

### 4.1 控制组的操作集与各自的规格所在

控制组（P14）是核心↔解释层同一 RPC 的一组操作。**动作轴**：写（append 控制记录）。

| 操作 | 生效语义与错误的规格 |
|---|---|
| `reload_config(kind)` | 本文 4.4 |
| `load_program(manifest_ref, cold_start?)`、`unload_program(id)`、`install_native_op(artifact)`、`remove_native_op(name)` | [program-host-element.md §4.1 控制动作](program-host-element.md#41-控制动作) |
| `rotate_credential(integration)`、`restart_integration(id)` | [integration-session.md §4.8 控制动作 restart_integration 与 rotate_credential](integration-session.md#48-控制动作-restart_integration-与-rotate_credential) |
| `request_snapshot` | [storage.md §3.1 控制动作 request_snapshot](storage.md#31-控制动作-request_snapshot) |
| `advance_retention(to)` | [observation-journal.md §3.2 控制动作 advance_retention(to: Set<LogPosition>)](observation-journal.md#32-控制动作-advance_retentionto-setlogposition) |
| `rewind_cursor(subscription, to)` | [subscription.md §4.1 核心↔解释层：订阅组与 rewind_cursor](subscription.md#41-核心解释层订阅组与-rewind_cursor) |
| `bypass_lane(ticket)` | [lane.md §4.3 显式绕过 bypass_lane(ticket)](lane.md#43-显式绕过-bypass_laneticket) |

结果未知组（`abandon`、`retry_reconciliation`）不是控制记录，但与控制动作共用 4.2 的授权与安全事件规则；其规格在 [io-shell.md §4.8 结果未知组（核心↔解释层）：放弃跟踪与重开](io-shell.md#48-结果未知组核心解释层放弃跟踪与重开)。

### 4.2 共同流程

1. **接收**：请求经会话入口到达，带会话绑定的 principal；请求体里的身份字段不参与授权（[session-entry.md §4.2 请求的身份与授权的入口](session-entry.md#42-请求的身份与授权的入口)）。启动第 5 步之前会话入口已对它返回 `Starting`，不到达本组件；受控停止第 1 步之后不再接受新的控制动作（4.5）。
2. **授权**：按 `(principal, 动作种类)` 授权，与写授权同一规则族（授权步，[decision-chain.md §4.2 顺序固定链：授权 → 输入约束 → 审批 → lane → 过期](decision-chain.md#42-顺序固定链授权--输入约束--审批--lane--过期)），授权表写在策略 / 审批规则文件里。越权 → append 控制记录 `Rejected(Unauthorized)`，并 append 一条安全事件（执行事实，P7），返回 `Unauthorized`；不读任何配置文件，不进入实现组件，没有其他副作用。
3. **读文件**（动作要读文件的）：在生效时读，记下内容 hash。
4. **交给实现组件判定与生效**：实现组件按 4.1 表所引规格判定成立与否、决定这条结论与哪些记录同事务、何时可以提交（例如 `restart_integration` 的 `Applied` 要等旧进程 OS 确认退出、与 `Connecting` 健康观察同一事务；`load_program` 的 `Applied` 与程序订阅的建立同一事务；`unload_program` 的 `Applied` 要等宿主执行结束）。本组件不改写这些条件。
5. **写结论**：本组件经存储的 append 接口写下控制记录（它是控制记录的写者，[core-process/design.md §4.3.4 执行事实的唯一写入口](design.md#434-执行事实的唯一写入口)），在实现组件编排的事务里提交。结论返回给发起的会话（若它仍在）。

生效动作再触发的记录（新 epoch、`Gap`、保留边界推进等）由实现组件写，规格在各自文档。

- 理由（结论由本组件写、事务由实现组件编排）：“是谁、依据哪版文件、结论是什么”是控制面自己的事实；“什么时候算生效、与什么同时成立”是实现组件隐藏的决定。把二者分开，控制记录只有一个写者，各动作的生效条件也只在一处定义。
- 不选：**每个实现组件自己写控制记录**：控制记录有十一个写者，principal、hash 与授权的规则要在每处重写一遍；**控制面编排全部事务**：控制面就得知道每个动作的生效条件，成了没有秘密的转发者之外又复制了各组件的决定。

### 4.3 错误与 undesired events（共同部分）

- 越权 → `Rejected(Unauthorized)` + 安全事件（4.2）。
- 配置文件不合法 → `Rejected(reason)`，保留上一有效版本（适用于读文件的动作；各动作另有的错误见 4.1 表所引规格）。
- 受控停止中的动作 → 4.5。
- 动作结束之前它的实例先结束（受控停止失败或崩溃）→ 动作随实例结束，没有结论记录、不生效。结束锚点即继任实例取得 fence 的事务。发起方重连之后在控制流上看不到它的结论记录，即它没有生效。

### 4.4 `reload_config(kind)`

- **语义**：按 `kind` 重读统一路径上的一个配置文件并使之生效。`kind ∈ {rules, runtime}`：
  - `rules`：策略 / 审批规则文件。内容规格、合法性校验与判据在 [decision-chain.md §4.9 规则文件的内容与合法性](decision-chain.md#49-规则文件的内容与合法性)；规则版本 = 内容 hash。
  - `runtime`：运行期参数文件：快照频率、派生侧留存窗口、`deadline` 全局缺省（仅在规则文件未按 scope 给出时生效）、投递缓冲上限、回填深度（全局缺省与按逻辑流的取值）。各参数的含义在使用处：[storage.md §2.4 快照是近处副本](storage.md#24-快照是近处副本-设计)、[observation-journal.md §2.6 推进判定](observation-journal.md#26-推进判定-设计)、[ticket.md §3.3 意图构造与锚点：构造不出的不是意图](ticket.md#33-意图构造与锚点构造不出的不是意图)、[delivery.md §4.3 背压、conflation 与停投](delivery.md#43-背压conflation-与停投)、[subscription.md §4.5 实时边界、回填任务与回填进度](subscription.md#45-实时边界回填任务与回填进度)。
- **动作轴**：写。
- **核心内部结果**：
  - 文件合法 → 控制记录 `Applied`，带文件内容 hash；新配置自提交起生效。
    - `rules`：此后每条 `Outcome` / `Rejection` / 控制记录带新规则版本；规则不冻结进单据，待决单据在下一次求值时按新规则从授权步起重过（[decision-chain.md §4.6 规则版本变更](decision-chain.md#46-规则版本变更-设计)）；必要项集或 `Lag` 的变化触发单据 `alignment` 的重算（[ticket.md §3.5 第一层：依据有效性 basis_validity](ticket.md#35-第一层依据有效性-basis_validity)）。
    - `runtime`：不改规则版本；各参数按使用处的规则生效，例如回填深度只影响此后流 epoch 的回填任务判定，不改已建立的任务。
  - 文件不合法（格式版本不支持、内容不合法、读不到）→ `Rejected(reason)`，保留上一有效版本；生效配置版本 hash 不变。
- **重试**：幂等；同一文件重复重载得到同一生效版本。
- **与启动的关系**：启动时读取这两个文件失败是核心级失败，拒绝启动而非部分生效（[core-process/design.md §4.7.3 启动五步](design.md#473-启动五步)）；运行期的重载失败只得 `Rejected`，不影响运行。
- 理由（文件为什么只在控制动作时重读）：文件在两次读之间的改动不生效，运行态只有一个源头，就是控制流上的 `Applied`；监视文件变更自动重载会让文件成为运行态的第二个源头，半写与连续改动都要另行仲裁。

### 4.5 受控停止期间

- 受控停止第 1 步之后不再接受新的控制动作（[core-process/design.md §4.7.4 受控停止](design.md#474-受控停止-设计)）。
- 停止开始时已在执行的控制动作照常完成，结论在实例结束锚点之前 append。各动作怎样得出结论由实现组件规定：`unload_program`、替换与 `abandon` 等到它们的内层结论；`Applied` 还没有提交的 `restart_integration` 与 `rotate_credential` 在第 1 步以 `Rejected(Stopping)` 结束（[integration-session.md §4.8 控制动作 restart_integration 与 rotate_credential](integration-session.md#48-控制动作-restart_integration-与-rotate_credential)）。
- 未完成的控制动作的结束锚点：停止失败时仍没有结论的动作，随实例结束，同 4.3 最后一条。
- 理由：停止不能丢弃一个已开始、内层已部分结束的动作而不留结论；动作的结论都有确认者，所以能在外层结束之前写下。
- 不选：**以控制动作触发停止**：停止要在控制面也关闭之后完成，且同一 OS 用户的进程本就能终止核心（H7）。

## 5 组件内走查

### 5.1 W8 改规则的本组件细化（Q18）

1. 会话入口交来 principal P 的 `reload_config(rules)`。
2. 授权：规则文件（当前生效版本）给 P `reload_config` 的 scope → 通过。
3. 读规则文件，得内容 hash h2；交给 STS 规则链校验合法性 → 合法。
4. append `Applied{P, reload_config(rules), h2}`。
5. 输出：此后 `Outcome` / `Rejection` 带 h2；待决单据在下一次求值时从授权步重过（主 trace 见 [core-process/design.md §5.1 W8（Q18）配置热变更 / 换凭据](design.md#w8q18配置热变更--换凭据)）。
- 失败分支：第 3 步不合法 → `Rejected(reason)`，生效版本仍是 h1。
- 崩溃矩阵 #12（配置文件重载中途）的本组件部分：rename 未落，读到旧文件完整内容；rename 已落，读到新文件完整内容——文件层没有第三态。核心在第 4 步提交之前崩溃：这次重载没有结论记录、没有生效；重启时按启动规则读此刻磁盘上的完整文件（新版或旧版，没有半写）并校验，不合法即拒绝启动。

### 5.2 W19 第 3 步的本组件细化（Q19）

主 trace：[core-process/design.md §5.1 W19](design.md#w19q19会话身份与未授权控制)。

1. 已认证会话的 principal Q 发 `restart_integration(X)`。
2. 授权：Q 在规则文件里没有该动作的 scope。
3. append `Rejected(Unauthorized)` 与安全事件；返回 `Unauthorized`。
4. 输出：登记文件未被读；集成会话未收到动作，X 的会话未重建；除这两条记录外没有记录（验收 #13）。

### 5.3 卡点

逐步走过 4.2–4.5 与上面两条 trace，没有登记卡点：每一步的写者、事务编排者与确认者都已定义。走查确认的一处归属是控制记录的写者与事务编排者分离（4.2 第 4、5 步），它与上级文档的同事务集合一致。

## 6 评估

### 6.1 风险 / 敏感点 / 权衡点

本组件没有独立的敏感点：授权规则族的风险在 [decision-chain.md §6 评估](decision-chain.md#6-评估)，各动作的风险在实现组件。

### 6.2 验收标准

- **#13 未授权控制无副作用**（4.2、[session-entry.md §4.2 请求的身份与授权的入口](session-entry.md#42-请求的身份与授权的入口)）：未认证连接、伪造请求体身份、已认证但无 scope 的控制请求均被拒绝并记安全事件；期间无 venue 调用、无配置变更、无单据状态变化。（对应 Q19/Q20）
