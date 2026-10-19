# UTA 代码架构

本文规定代码层面的事：crate 怎样划分与依赖、进程里有哪些线程和任务、每份状态由谁持有、类型允许派生什么、线缆约定、库的选择，以及这些约定怎样被检查。架构与业务单元的形式化规格在 `design/`（按 `design/README.md` 阅读次序组织的 VDM-SL 模块：`Source`、`Observation`、`Effect`、`Ticket`、`Trading`、`Program`）；设计不规定 crate 布局与进程内实现机制，因此代码布局与实现机制写在这里。

## 1 进程、线程与任务

```mermaid
flowchart TB
  subgraph CORE["uta-core 进程"]
    MAIN["主线程<br/>持有 InstanceLock（OS 文件锁）<br/>运行 tokio runtime"]
    CT["核心线程<br/>独占 Store（唯一读写连接）<br/>持有 CurrentInstance 与全部核心自有状态"]
    TASKS["tokio 任务<br/>信号 · 孤儿回收<br/>（后续：集成会话、宿主会话、消费方会话、投递）"]
  end
  CHILD["子进程：集成 × N、程序宿主 × M"]
  MAIN -->|"启动握手：开库 + fence 事务成功才返回 CoreHandle"| CT
  TASKS -->|"Command（有界 mpsc，值移入）"| CT
  CT -->|"oneshot 回复（事务已提交）"| TASKS
  TASKS <-->|"核心创建的通道（socketpair / 匿名管道对）<br/>长度前缀 JSON-RPC"| CHILD
```

- **主线程**依次执行：先建好 tokio runtime，再取 OS 锁（`fence::acquire`），然后启动核心线程并等待它报告就绪（fence 事务已提交），最后在 runtime 里进入运行阶段。停止信号的监听在任何等待之前装好，所以孤儿回收期间和"core ready"之后的停止请求都走受控停止，不走系统默认动作；Unix 上是 SIGINT、SIGTERM，Windows 上是 Ctrl-C、Ctrl-Break 与控制台的 close、shutdown 事件。运行阶段结束后先丢掉 runtime，再在主线程上阻塞调用 `CoreHandle::stop`：核心线程写结束锚点、在自己线程上关闭 SQLite、被 join，之后主线程才释放锁。`stop` 是阻塞调用而不是 future，不存在停到一半被取消的情况；`CoreHandle` 被丢弃而没有调用 `stop` 时，`Drop` 关闭命令通道并 join 线程，线程丢掉 store、不写结束锚点（即崩溃路径），锁在 `CoreHandle` 之后才被释放。
- **核心线程**是 `rusqlite::Connection` 唯一的所有者，也是核心自有状态唯一的写入者。它按顺序执行 `Command`：写命令是一个事务，回复在提交之后发出；读命令是一条语句（SQLite 的语句级快照）。连接、事务和闭包都不会跨出这个线程。
- **tokio 任务**持有 I/O 对象（通道、子进程句柄、信号），以及嵌套在某个会话之内、只存在于内存的生命周期状态，例如一条通道上的在途调用表。它们要持久化什么，一律发 `Command` 给核心线程。
- **子进程**只经由核心创建的通道和凭据句柄与核心交换数据，不接触 SQLite（[design/architecture/README.md](design/architecture/README.md)、[design/business/program/README.md](design/business/program/README.md)）。

## 2 crate 地图

### 已有 crate

| crate | 层 | 拥有 | 依赖（工作区内） |
|---|---|---|---|
| `uta-base` | 值 | 源头在核心之外的标识：`IntegrationId`、`ProgramId`（Alice 写的配置文件），`StreamName`、`WriteLaneKey`（集成声明与程序值），以及带命名空间标签的 `Source` 与 `ProcessRole`；serde 直接读写字符串，只经解析器构造 | — |
| `uta-proc` | OS | 进程身份 `OsProcessId{pid, start_time}`（`start_time` 是 OS 给出的精确启动标记，只做相等比较）与退出证明 `ExitConfirmed`：只有 OS 明确回答"该进程已不存在"时才铸造，查询失败不算退出 | — |
| `uta-rpc` | 传输 | 对称 JSON-RPC 2.0 peer：长度前缀分帧，每个调用恰好完成一次 | — |
| `uta-channel` | 传输 | 核心创建的会话通道与凭据句柄，只交给被拉起的子进程；子进程一侧用 `take_inherited` 取回 | — |
| `uta-store` | 存储 | 单写者 SQLite：格式版本与迁移、实例表、进程表、两侧 Journal 的存储原语（观察流 `StreamId`/epoch/`Seq` 的分配、按边界与按位置删除；执行事实流只 append）、`Store::transact` / `Committed` | `uta-base`、`uta-proc` |
| `uta-core` | 进程 | 守护进程：fence、核心线程、孤儿回收、受控停止、退出码 | `uta-base`、`uta-proc`、`uta-store` |
| `uta-testkit` | 测试 | fixture 子进程和跨 crate 的真实进程测试，不发布 | `uta-base`、`uta-proc`、`uta-rpc`、`uta-channel`、`uta-store` |
| `xtask` | 工具 | `cargo xtask check-deps` | — |

### 后续切片的落点

下表是 `design/` 各模块（`Source`、`Observation`、`Effect`、`Ticket`、`Trading`、`Program`）与边界适配将来落在哪个 crate。某个元素的代码随它的实现切片一起创建，不预先建空 crate。创建时要同时在 `xtask` 的依赖表里登记它所在的层。

| crate（待建） | 对应设计模块与职责 | 位置 |
|---|---|---|
| `uta-contract` | 核心↔集成 IDL 的 DTO，用 schemars 导出 JSON Schema，随 release 发布 | 值层，依赖 `uta-base` |
| `uta-session` | `Source`（[design/architecture/source.md](design/architecture/source.md)）：集成会话生命周期、调用通道与当前有效能力判定 | 架构根层，依赖 `uta-contract`、`uta-rpc`、`uta-channel`、`uta-store` |
| `uta-observe` | `Observation`（[design/architecture/observation.md](design/architecture/observation.md)）：观察流追加、依据有效性判定、活跃引用保护、保留边界推进与消费投递 | 观察层，依赖 `uta-session` |
| `uta-program` | `Program`（[design/business/program/program.md](design/business/program/program.md)）：活动程序集合、检查点保存与 `Advance` 编排 | 业务接入层，依赖 `uta-observe` |
| `uta-effect` | `Effect`（[design/architecture/effect/effect.md](design/architecture/effect/effect.md)）、`Ticket`（[design/business/ticket/ticket.md](design/business/ticket/ticket.md)）、`Trading`（[design/business/trading/trading.md](design/business/trading/trading.md)）：单据与审批、受控写边界、通道阻塞与取证决议、交易操作组合与观察归并 | 效应与交易业务层，依赖 `uta-observe`、`uta-program`、`uta-session` |
| `uta-api` | 核心↔解释层 IDL，只在本仓库内部使用 | 值层 |
| `uta-cli` | 解释层 CLI 与长连接端点 | 依赖 `uta-api`，不依赖核心内部 crate |
| `uta-host` | 程序宿主子进程：程序求值器与 `Checkpoint` 序列化（[design/business/program/README.md](design/business/program/README.md)） | 外部进程 |
| `uta-mapping`、`uta-conformance` | 集成侧记录映射解释器、一致性测试与 fixture 上游 | 随 release 发布 |

crate 之间不能循环依赖；需要反向通知时，由下层 crate 定义回调 trait，上层 crate 实现它。

## 3 依赖方向

- 观察模块不依赖效应模块（[design/architecture/README.md](design/architecture/README.md)），这一点落实为 crate 依赖方向：`uta-observe` 不依赖 `uta-effect`（对应 VDM-SL 中 `Effect` 单向 `imports from Observation`）。
- 允许的工作区内依赖列在 `xtask/src/main.rs` 的表里，由 `cargo xtask check-deps` 对照 `cargo metadata` 检查。出现表外的边，或有 crate 不在表里，都会失败。dev-dependencies 不受这张表约束。
- 传输 crate（`uta-rpc`、`uta-channel`）和 OS crate（`uta-proc`）不依赖任何领域 crate，领域语义也不进入这些 crate。

## 4 所有权规则

1. **标识定义在铸造它的 crate，构造器私有。**
   - `InstanceId` 只由 `uta-store` 的 fence 事务（`Store::begin_instance`）铸造。
   - `ExitConfirmed` 只由 `uta-proc` 在 OS 确认之后铸造。
   - 将来的 `SessionEpoch` 只由 `uta-session` 在创建通道时铸造。`StreamId`（其中的 epoch）与 `Seq` 只由 `uta-store` 在事务里分配（`Observations::open_epoch`、`append`）；`Seq` 按流单调，用每流计数器分配，删除记录后也不会复用。执行事实流的身份 `ExecStream`（控制流、声明流、lane 流、请求流）是已有的流名字，其上的位置同样由存储分配。
   - 集成交来的线缆输入里不存在这些字段，所以集成无从伪造。
2. **证明是可移动、不可复制的值，由它授权的操作消费。**
   - `CurrentInstance` 只由 `Store::begin_instance` 在 fence 事务提交之后交出，只被 `Store::end_instance` 消费；结束锚点的 UPDATE 必须恰好改动本实例那一行，否则报 `EndError::NotOpen`。所以一个实例至多结束一次，且只能结束自己。fence 与结束锚点不经 `transact` 的闭包：闭包出错时 `TxError::Aborted(E)` 会把 `E` 带出事务，在已回滚的事务里铸造的证明因此可能流出去。同理，事务里分配的位置不能经 `Aborted` 带出，它们指向的行已被回滚。
   - `ExitConfirmed` 只被 `Processes::clear` 消费，所以进程表的行只能凭 OS 确认清除。
   - `Responder` 在 `respond` 时消费；被丢弃时自动回错误，因此每个请求恰好得到一个回复。
   - 将来的发送前屏障证明（对应 `Effect`Barrier`，[design/architecture/effect/effect.md](design/architecture/effect/effect.md)）同理：私有构造器，在 durable append 提交之后才产生，由外部写调用消费。
3. **内存状态只从已提交的值推进。** `Store::transact` 只在 `COMMIT` 返回之后交出 `Committed<T>`；闭包失败时整个事务回滚，调用方拿不到 `Committed`。核心线程上的 fold、索引等内存副本只根据 `Committed` 更新，所以不存在"内存领先于磁盘"的状态。
4. **事务不外泄。** `Tx<'c>` 借用连接，只存在于 `transact` 的闭包里。表视图（`Processes`、`Observations`、`Executions`）只暴露允许的操作：`Executions` 只有 `append`，这一点由视图类型保证，不靠 SQL 权限；`Observations` 另有压缩原语，受 `Observation`AdvanceRetention` 的活跃引用保护约束（[design/architecture/observation.md](design/architecture/observation.md)）。epoch 的上限是 u32，`Seq` 计数器用尽时分配失败，不会溢出成别的类型。记录体是带 `RecordKind` 标签的不透明字节，存储不解析。存储不提供执行任意 SQL 的入口，所有语句都是固定文本。
5. **I/O 对象由单个任务独占，共享只靠消息。**
   - 一条通道由一个 peer 任务持有，同一个任务还持有该通道的在途调用表；其他任务只拿到 `PeerHandle`（mpsc 发送端）。
   - 不用 `Arc<Mutex<_>>` 包裹 I/O 或生命周期状态。
   - 跨线程只移动拥有所有权的值：`Command` 各变体只携带自有数据和 `oneshot` 回复端。
6. **载荷字节不复制。** 收到的帧转成 `Bytes` 后，`params` / `result` 用 `Bytes::slice_ref` 从帧上切片交出；核心不解析的载荷（`Observation`ObsPayload`、`Effect`IntentPayload`）一直以 `Bytes` 流转到存储和下游。
7. **生命周期内层先结束。** 拥有者先结束自己的内层，再结束自己。例如 `CoreHandle::stop` 消费句柄，join 核心线程之后才返回，主线程随后才释放锁；peer 结束时先让全部在途调用以 `Closed` 完成，再丢掉传输。

## 5 状态所有权表

| 状态 | 所有者（线程/任务） | 开始 | 结束 | 持久化 | 边界上的表示 |
|---|---|---|---|---|---|
| 实例 OS 锁 | 主线程，`InstanceLock` | `fence::acquire` 成功 | 核心线程被 join 之后 `release`；崩溃时由 OS 随进程释放 | 锁文件 `<root>/data/uta/uta.lock`（内容为空）；`<root>` 与 Alice 同法求出：设了 `OPENALICE_HOME`（空串也算）就用它，否则 `~/.openalice`，再按 Node `path.resolve` 对工作目录做词法归一 | 不可继承的文件句柄 |
| SQLite 连接 | 核心线程，`Store` | 核心线程里 `Store::open` | 同一线程里 `Store::close` | `<root>/data/uta/uta.db`（WAL） | 不跨线程 |
| 当前实例 | 核心线程，`CurrentInstance` | `Store::begin_instance` 提交 | `Store::end_instance` 提交（结束锚点） | 实例表的一行；时间取核心时钟，时钟不可表示时报错，不代入任何值 | 对外只给 `InstanceId`（`Copy`） |
| 格式版本 | 存储 | 首次打开或迁移事务 | 只前进 | `schema_meta` | 版本更新或不可读（缺行、非整数、超出 u32）的文件拒绝打开（退出码 11） |
| 进程表的行 | 核心线程 | 拉起之后 `Processes::register` | `Processes::clear(ExitConfirmed)`；启动时读不出或清不掉，本实例不进入就绪，走受控停止并以 1 退出 | `processes` 表 | `OsProcessId` 是索引，本身不授予任何权利 |
| 子进程句柄 | 拉起它的任务，`ChildProcess` | `uta_channel::spawn` | `wait` 消费句柄，之后 `uta_proc::confirm_exit` 得到 `ExitConfirmed` | 不持久化 | 不可 Clone |
| 会话通道（核心端） | 持有它的 peer 任务 | `spawn` 创建 | peer 结束或句柄被丢弃时关闭；关闭后读不到该化身的任何消息（[design/architecture/README.md](design/architecture/README.md)） | 不持久化 | `SessionChannel`：`AsyncRead + AsyncWrite` |
| 凭据副本 | 核心：`spawn` 执行期间（写入后清零）；子进程：直到进程退出 | 写入继承句柄 | 核心关闭写端；子进程读到 EOF 后关闭读端 | 从不持久化 | `Credential`（zeroize）；不出现在 env 或参数里 |
| 在途调用表、请求 id | peer 任务 | `call` 发出 | 收到响应，或 peer 结束时统一以 `Closed` 完成 | 不持久化 | 调用方持有的 future |
| 观察流的 epoch 与 `Seq` | 核心线程（经 `Observations`） | `open_epoch` / `append` 在事务里分配 | 流的 epoch 由下一个 `open_epoch` 结束；`Seq` 不回收 | `obs_streams`（含每流计数器）、`obs_records` | `StreamId`、`LogPosition`：`Clone`，私有构造 |
| 执行事实记录 | 核心线程（经 `Executions`） | `append` 提交 | 不结束：只追加，没有删除入口 | `exec_streams`、`exec_records` | `ExecPosition`：私有构造 |
| 核心自有的内存副本（将来：各尝试的 fold、订阅表缓存、活动集合） | 核心线程 | 启动时从记录 fold 出 | 实例结束 | 由记录派生，不是源头 | 只从 `Committed` 推进 |

## 6 类型派生规则

| 类型类别 | 允许派生 | 禁止派生 | 理由 |
|---|---|---|---|
| 外部标识（`IntegrationId` 等） | `Clone`（内部是 `Arc<str>`）、`Eq`、`Ord`、`Hash`、`Serialize`；`Deserialize` 经 `try_from = "String"` 走解析器 | 公开的无校验构造器 | 在边界解析一次，内部直接信任（parse-don't-validate） |
| 核心铸造的标识（`InstanceId`、将来的 `SessionEpoch`、`LogPosition`） | `Copy`、`Eq`、`Ord`、`Hash`、`Debug` | `Deserialize`、公开构造器 | 源头是核心；从自己的库里读回时，由铸造它的 crate 解码 |
| 证明（`CurrentInstance`、`ExitConfirmed`、`Responder`、将来的 `SendBarrier`） | `Debug`，加 `#[must_use]` | `Clone`、`Copy`、`Default`、`Serialize`、`Deserialize` | 用一次即失效，不能伪造 |
| 线缆 DTO（将来的 `uta-contract`、`uta-api`） | `Serialize`、`Deserialize`、`JsonSchema` | 在 DTO 上附加业务方法 | DTO 由 `TryFrom` 消费、转成领域类型；schema 从 DTO 生成，不另外手写 |
| 程序与表达式描述（将来的 `uta-program`、`uta-host`） | `Serialize`、`Deserialize`、`JsonSchema` | — | 装载期校验之后包成私有构造的已校验类型 |
| 结果与生命周期状态 | 闭合 enum，`match` 必须穷尽 | 用布尔组合或只在部分状态下有效的 `Option` 字段表示状态 | 例如 `InstanceEnd::{Stopped, NoEndAnchor}`、`CallError`、`PeerEnd` |

## 7 线缆约定

- **信道**：核心拉起集成进程时创建通道（[design/architecture/README.md](design/architecture/README.md)）。
  - Unix 上是 `socketpair`，子进程端放在 fd 3，凭据管道的读端放在 fd 4，由 `command-fds` 在同一次映射里放置；其余描述符一律 `CLOEXEC`。
  - Windows 上是两条匿名管道加一条凭据管道，经 `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` 只交给该子进程。
  - 子进程从环境变量 `UTA_SESSION_CHANNEL`、`UTA_CREDENTIAL` 读句柄号。句柄号不是秘密；凭据字节只经过句柄传递。
- **分帧**：4 字节大端长度前缀，后接 UTF-8 JSON；单帧上限默认 16 MiB，超限视为协议违规，关闭通道。
- **消息**：JSON-RPC 2.0，两端都可以发起调用。本端发出的请求 id 是从 1 开始的 u64；收到的 id 原样回显。
- **调用语义**：
  - 每个调用恰好完成一次。
  - 不设超时，不重试。
  - 通道结束时，在途调用一律得到 `Closed`；由会话层按操作映射为 `<NoResponse>`（交入 `Effect`CompleteSend`）或 `<Unavailable>`。
  - 本地序列化失败是单独的 `Encode` 结果，不冒充远端错误。
- **值编码**（随 `uta-contract` 落地）：
  - u64 标识和位置编码为十进制字符串，因为下游有 JS，精度只到 2^53。
  - 精确数值编码为十进制字符串。
  - 时刻编码为 RFC 3339 UTC。
  - 二进制原文编码为 base64。
  - enum 使用带标签的外部表示。
  - 这些约定只写在 `uta-contract` 的 serde 属性里一处，由 schema diff 门禁检查。

## 8 存储

- 单个 SQLite 文件，WAL 模式，`synchronous=FULL`：`COMMIT` 返回时 WAL 已经同步到磁盘。[design/architecture/effect/README.md](design/architecture/effect/README.md) 要求 `Effect`Barrier` 先持久化落盘再发送，这一点由此成立。
- 写命令对应一个 `BEGIN IMMEDIATE` 事务，读命令是一条语句。
- 格式版本记在 `schema_meta`。`MIGRATIONS[n]` 把 n 迁到 n+1，所有待执行的迁移在同一个事务里完成，所以文件只可能是旧版本或新版本。遇到版本更高的文件，拒绝打开。
- SQLite 的访问只由构造保证：只有核心线程持有读写连接。没有使用 `locking_mode=EXCLUSIVE`：进程间的互斥已经由 OS 锁保证，而 EXCLUSIVE 会挡住后续投递要用的进程内只读连接。只读连接只能看到已提交的数据。

## 9 退出码

| 码 | 含义 |
|---|---|
| 0 | 受控停止完成：结束锚点已写，锁已释放 |
| 1 | 没有专用码的失败，原因写在 stderr。包括：状态目录 I/O、SQLite 错误；启动时孤儿回收失败（进程表读不出或清不掉，此时已走受控停止、写了结束锚点）；结束锚点已写但关闭 SQLite 失败 |
| 10 | 该状态根已有实例持有 fence |
| 11 | 文件格式比本构建新、格式版本不可读，或迁移失败 |
| 14 | 受控停止没能完成：没有写结束锚点 |

## 10 库选择

| 用途 | 选择 | 理由与约束 |
|---|---|---|
| 异步运行时 | tokio（多线程） | 信号、进程、通道 I/O；CPU 重的计算不放在 worker 线程上 |
| 存储 | rusqlite（`bundled`） | 同步事务，只在核心线程上使用 |
| 单实例锁 | std `File::try_lock`（Rust 1.89 起） | Unix 用 `flock`，Windows 用 `LockFileEx`；锁随句柄释放 |
| 进程身份 | 各平台 OS 接口（Linux `/proc/<pid>/stat` 的 starttime，macOS `proc_pidinfo`，Windows `GetProcessTimes` 与 `WaitForSingleObject`） | 启动标记精确到 OS 能给出的粒度，pid 复用不会被当成同一进程；存活判断分"存活 / 已不存在 / 查不到"三种，只有"已不存在"才铸造退出证明 |
| 系统调用 | Unix 用 `rustix`（发信号），`command-fds`（子进程 fd 3/4 放置）；Windows 建管道用 std `io::pipe`；其余直接调 `libc`（仅 macOS `proc_pidinfo`、子进程收养 fd 前的 `F_GETFD`）与 `windows-sys` | `unsafe` 只留在没有安全替代的地方：把句柄号认作己有（子进程收养 fd 3/4 与继承句柄）；Windows 的可继承句柄复制、`PROC_THREAD_ATTRIBUTE_HANDLE_LIST` 与 `CreateProcessW`（std 对应能力 `spawn_with_attributes` 未稳定，rust-lang/rust#114854）；同一进程句柄上先核对身份再终止。`sysinfo` 按 PID 终止、`libproc` 接受短读并把错误变成字符串，都会削弱三值存活判断，不用 |
| 分帧 | tokio-util `LengthDelimitedCodec` | 直接产出 `BytesMut`，可以零拷贝切片 |
| JSON-RPC | 自写对称 peer（`uta-rpc`） | jsonrpsee 的服务端只支持 HTTP/WS，自定义传输只有客户端可用，且是非对称的；会话语义要求同一连接双向调用、恰好完成一次 |
| 集成出站 HTTP / WS（写集成时） | reqwest、tokio-tungstenite | reqwest 默认会重试协议层 NACK，写路径必须设 `retry::never()` 并关闭重定向（写尝试至多一次上游写，[design/architecture/effect/README.md](design/architecture/effect/README.md)） |
| 线缆 schema（随 `uta-contract`） | schemars，从 Rust DTO 生成 | 生成的 schema 提交进仓库，CI 做 diff 门禁；不另外手写 |

具体版本以 `Cargo.lock` 为准，工作区的 `rust-version` 是 1.89。

**不选 Pingora。**

- 它的模型是入站请求 → `upstream_peer` → 双向转发。UTA 没有把下游请求原样转给 venue 的职责；集成是主动发起连接的客户端，把上游清洗成契约值。
- 它的默认重试对 HTTP 语义里的幂等方法（如 `DELETE`）开放，而写路径要求至多一次。
- `Server` 拥有进程与 runtime，平滑升级靠接管旧实例，与单实例 fence 相反。
- Windows 支持只是社区初步支持。

## 11 验证

- `ssot check design`：检查 `design/` 的目录、VDM-SL 语法、结构方向、反模式、形式、文字引用、类型检查与 QuickCheck 证明义务。
- `cargo test --workspace`：存储事务（回滚无痕、重开可见、格式拒绝）、peer 语义（恰好完成一次、协议违规、`Responder` 被丢弃时的回复）、真实子进程上的通道与凭据交付，以及孤儿回收。
- `cargo xtask check-deps`：第 3 节的依赖方向。
- CI（`.github/workflows/ci.yml`）在 Linux 和 Windows 上跑 rustfmt、build、test、clippy、check-deps，另有一个 job 用 1.89 检查 MSRV。Windows 上的通道、句柄继承和进程终止只在 CI 的 Windows job 上实际运行。
- 守护进程的运行期测试（`crates/uta-core/tests/daemon.rs`）直接起真实二进制，覆盖：两个实例竞争同一状态根、SIGTERM 受控停止后再启动、SIGKILL 之后留下没有结束锚点的实例、启动时回收孤儿、格式版本拒绝。真实子进程上的通道测试在 `crates/uta-testkit/tests/session_channel.rs`。
