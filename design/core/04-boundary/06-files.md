# 统一路径文件

- **统一路径**：核心与 Alice 共享的配置，以文件形式持久化在 `OPENALICE_HOME` 下的统一路径。两个进程均从文件读取，不经进程间注入、环境变量或启动参数传递（集成进程得到凭据的方式另见 [01-integration.md §4.5 进程、通道与凭据](01-integration.md#45-进程通道与凭据)）。
- **每个文件的写者是 Alice**：核心只读，不写统一路径下的任何文件。核心只在这些时刻读一个文件：启动第 3 步；读取该文件的控制动作生效时（下表“读取时刻”）；拉起集成进程时读该集成的凭据；装载程序时按清单读程序值文件并核对 `Applied` 所钉的内容 hash（启动第 5 步、等待中的成员重新判定时）；拉起引用原生 op 的宿主之前按安装 `Applied` 所记的引用读原生计算制品并核对内容 hash。文件变了而没有这些时刻，核心不重读。控制动作读到的文件以控制记录带它的内容 hash 记下，启动时读到的集成登记由采纳记录记下。
- **格式版本只前进**：文件附带格式版本，升级只前进（C14）；迁移失败拒绝启动而非部分迁移。schema 的文本形式（JSON schema）随 IDL 一起作为 release 产物发布，是实现阶段工件。
- **原子替换与重载失败**：配置文件以整文件原子替换（临时文件 + rename，与既有快照 index 的写法一致，O9/`existing-capabilities.md:189`），不存在半写可见态。运行期重载失败保留上一有效版本，控制结果为 `Rejected(reason)` 记录；启动期读取失败按 C14 拒绝启动。
- **Alice 侧的适配**（密钥位置调整、移除 flag 重启）属于接入阶段，不在本设计范围内；本设计只定义核心读取的文件契约。

| 文件 | 写者 | 内容 | 读取时刻 | 内容规格与读取效果 |
|---|---|---|---|---|
| 账户封存信封 | Alice | 每账户的封存凭据密文与账户身份（`WriteScope` 对齐用的上游账户标识） | 每次拉起该集成进程；`rotate_credential` | [01-integration.md §4.5 进程、通道与凭据](01-integration.md#45-进程通道与凭据) |
| 封存密钥引用 | Alice | 解封密钥的位置引用（OS keychain / 文件路径），不含密钥本体 | 同上 | 同上 |
| 集成登记 | Alice | 每个集成的 id、二进制路径、启动参数、负责的账户集、契约版本；id 的稳定是写者的义务 | 启动第 3 步整份采纳；`restart_integration(id)` 重读该 id 的条目 | [01-integration.md §4.7 集成登记的采纳](01-integration.md#47-集成登记的采纳) |
| 策略 / 审批规则 | Alice | principal → scope 表、审批条件、允许集合、检查目录的取值、依据有效性窗口、冷却间隔、`deadline` 缺省、控制动作授权；版本 = 内容 hash | 启动；`reload_config(rules)` | [04-decision-chain.md §4.9 规则文件的内容与合法性](../03-read-write/04-decision-chain.md#49-规则文件的内容与合法性) |
| 程序装载清单 | Alice | 程序值文件引用、预算、接受的 `state_version` | `load_program`；启动第 5 步与等待中的成员重新判定时读程序值文件 | [01-program-host.md §4.2.4 按钉住的内容装载](../05-runtime/01-program-host.md#424-按钉住的内容装载) |
| 原生计算制品目录 | Alice | 原生 op 的制品文件，每个文件声明 op 名与签名 | `install_native_op`；每次拉起引用该 op 的宿主之前 | [01-program-host.md §4.7 核心↔可选行情派生计算子系统](../05-runtime/01-program-host.md#47-核心可选行情派生计算子系统) |
| 运行期参数 | Alice | 快照频率、派生侧留存窗口、`deadline` 全局缺省（仅在规则未按 scope 给出时生效）、投递缓冲上限、回填深度（全局缺省与按逻辑流的取值） | 启动；`reload_config(runtime)` | 各项的含义在使用处：快照频率 [02-storage.md §2.4 快照是近处副本](../01-base/02-storage.md#24-快照是近处副本-设计)；留存窗口 [01-journal.md §2.6 推进判定](../02-read/01-journal.md#26-推进判定-设计)；`deadline` 缺省 [04-decision-chain.md §4.9 规则文件的内容与合法性](../03-read-write/04-decision-chain.md#49-规则文件的内容与合法性)；投递缓冲上限 [03-consumption.md §4.8 背压、conflation 与停投](../02-read/03-consumption.md#48-背压conflation-与停投)；回填深度 [05-backfill.md §4.2 实时边界、回填任务与回填进度](../02-read/05-backfill.md#42-实时边界回填任务与回填进度) |

`reload_config` 的共同语义（授权、所读文件 hash、`Rejected` 保留上一有效版本）在 [04-control.md §4.4 reload_config(kind)](04-control.md#44-reload_configkind)。

