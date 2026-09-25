# 11 术语与索引

本章给出规范词表、五种关系与其承载、同名异义对照，以及研究与调查报告索引。

## 11.1 规范词表

按名字给出一句话定义与定义所在节；同名异义（一个词两种意思）见 §11.3。

| 名字 | 一句话定义 | 定义所在节 |
|---|---|---|
| 域事实 | 外部世界中不能由 UTA 设计改变的事实，附一手证据。 | §1.1 |
| 既有机器事实 | 旧 UTA 实现的可观察行为，用于说明新设计不能预设旧实现能力。 | §1.1 |
| 共享现象 | 机器与问题域之间交换的带名记录类别及其语义字段。 | §1.2 |
| 既有行为 | 当前 Alice/旧 UTA 真实入口可观察到的调用和结果形状。 | §1.3.1 |
| 质量场景 | 由刺激源、刺激、构件、环境、响应、响应度量六项组成的可证伪场景。 | §1.4 |
| `Record` | 某条流的记录词表，由集成在边界处解析给出的具体类型 | §4.1 |
| `Delta` / `RetractableDelta` | 幺半群增量；后者额外要求逆元（`neg`），只派生侧要求 | §4.1 |
| `StreamId` | `(source, stream, epoch)`：独立维持单调位置的有序范围 | §2.3 |
| `Seq` | 某 `StreamId` 内每 epoch 独立单调递增的持久序号 | §2.3 |
| cursor（消费位置） | 某消费者已确认（已处理）的位置，每条选中流一个；只说日志，不说任何完备 | §2.3、§4.2 |
| 完备进度（序号覆盖） | 来源证据证明的覆盖：今天只有声明 `joinable_venue_seq` 的流上的**序号覆盖**，命题是“流 epoch e 内 venue 序号落在 `[from, through)` 的记录都已 append”，坐标是 e 的 venue 序号；只计入 append 在 e 上一条同会话、确认整条流供给（`All` 且 `refused` 为空）的路由结论记录之后的推送，需求不是输入；没有证据的流没有完备进度（“完备未确立”），契约里没有事件时间上的完备 | §2.3、§8.4 |
| 覆盖检查点 | 保留边界推进要删掉当前 epoch 的记录之前，持久订阅把序号覆盖到新边界为止的 fold 状态 append 成的健康观察（按逻辑流为键），使覆盖在压缩后仍可重建 | §8.4、§2.4 |
| 等待窗口 | 消费者声明的按时间截止的意图（例如收到时间 t + w 时用已到的记录计算）；写在消费声明里，不是流的元数据，不冒充完备 | §4.2 |
| retention（保留边界） | 每条观察流上存储仍能精确重建的最早位置；执行事实侧没有 | §2.3、§2.4 |
| 信封 | UTA 坚持解析的少数字段（锚点 + 已注册处理器读的字段），入口验证 | §2.1 |
| 载荷 | 集成对上游的消费结论，按契约 schema（公共或扩展）写成，打 `payload_schema`；核心不解释，程序与钩子解释 | §2.1、§8.1 |
| 原始负载 | 集成所消费的上游原文，作证据保留；写路径与可带 `attribution` 的流必须完整保留（C13），其余观察流由记录映射声明；不进任何解释器 | §2.1、§6.5、§8.1 |
| `Evidence` | 执行事实侧永存的证据值 `{payload, payload_schema, raw}`：契约载荷与原始负载一起保存 | §6.5 |
| `execution_id` / `execution_revision` | 成交记录的执行身份：上游证明的、在来源 × 作用域内唯一且跨渠道不变的不透明值 / 上游明确给出修正次序时的修订号；按执行计数的 fold 每个身份至多贡献一次，同身份只按修订取值，冲突与无身份照实标出、不计入 | §8.1 |
| 逐笔增量 / 累计快照 | 成交记录的数量与价格是这一笔执行的 / 订单状态的 `cumulative_filled_quantity` 与平均价是到该次观察为止的累计；二者不互相推算 | §8.1 |
| 权威（authority） | 值的原件所在处；订单、持仓、余额、行情的权威都在上游，UTA 只对自己的意图及其推进记录有权威 | §0.1 |
| `Projection` | 集成握手声明的值：作用域、流、写能力、订阅配额、扩展 schema、记录映射（`Projection{scopes, streams, capabilities, quotas, extension_schemas, mappings}`） | §2.2 |
| 声明版本 / 最近声明 / 会话有效声明 | 每次握手成功由集成会话 append 的执行事实：该投影除记录映射外的全部（§8.1 的“声明”）/ 该来源最近一版加引用它（同一 `SessionEpoch`）的 `CapabilityObserved` 的 fold，是历史：`sources`、账户解析、供给项接纳与配额、程序装载期的 `required_inputs` 比对读它 / `Established{epoch, 声明} \| NoSession{…}`：只在会话 `Established` 时存在，写门、发出前门、取证渠道、回填与一次性读的“此刻能不能”只读它；只投递项与执行事实 selector 另读历代声明出现过的流与作用域键 | §7.3、§7.5、§8.2、§8.6 |
| `StreamDecl` | 一条观察流的声明：流名、种类、`payload_schema`、`request_schema`、一次性读与回填的三值能力、名义数据等级、有无游标 / 事件时间 / 可衔接 venue 序号（`joinable_venue_seq`）、推送顺序（`push_ordered`） | §2.2 |
| `request_schema` | 一次性读请求参数的 schema 身份 `(schema_id, schema_version)`；参数含查询主体与领域过滤条件，核心只校验不解释；不与 epoch 绑定 | §2.2、§8.2 |
| 名义数据等级 | 流声明里的数据等级（时效 × 覆盖）：来源对该流开通情况的声明，不担保逐条记录；记录上报告的实际等级以记录为准 | §2.2 |
| 配额池（`Quota`） | 属于来源的订阅上限：一组流上当前需求（`route`）里的不同订阅主体数 | §2.2、§8.5 |
| `account_ref` | 作用域的对外账户引用：集成给出，集成内唯一、跨握手稳定、不复用；与声明历史矛盾时不可解析；与 `label`、`WriteLaneKey` 都不同 | §2.2 |
| 读结论记录 | 一次性读或回填被集成作答（含上游拒绝）时，与结果项同一事务 append 在该流上的控制记录：请求身份、发起方、本次项的位置或覆盖边界；不是载荷记录 | §8.2 |
| 一次性读（元素） | 观察侧核心元素：除回填外全部 `read` 的判定、发起与完成；同一集成会话 epoch 内同一 identity 的在途调用必然并入，`origins` 在完成提交时冻结，一次调用计一次；发起方离开后调用照常完成 | §7.3、§2.2 |
| `route` / 路由结论记录 | 核心→集成的订阅需求：每条流要推送的主体全集（或 `All`、空集），带核心在本会话为该流最后接受的 `generation`；每个会话建立后与每个会话内新开的流 epoch 上各起一项 `route` 义务（每条流至多一项，按 pacing 的重试与 gap 触发的重发是同一项，第一次 `Routed` 了结）、需求一变再发，全集替换；跨过流 epoch 更替只由集成按 `generation` 判定（旧值得 `Unavailable`、供给不变），核心不另判 / `Routed` 时同一事务 append 在该流上的控制记录：这次生效的主体全集（`All` 或主体集）、`refused` 与原因，不记增减；相对同一流 epoch 内上一条的增减由相邻两条算出；主体在该流 epoch 内的记录从加入它的这条记录之后开始；序号覆盖的计入只读单独一条记录（同会话、`All` 且 `refused` 为空） | §8.2、§8.3、§8.4 |
| `generation` | 集成在会话内为某流上报的每条 `Gap{origin: Source}` 所带的值：由集成创建，按流、按会话计，会话开始为 0，每上报一条加一并带上新值，所以本会话第一条带 1；`route` 带本会话里集成为该流上报、核心已接受的最近一条这样的 gap 所带的值（没有则 0），以它指名所问的流 epoch；集成只以 `Routed` 回答当前值，gap 之后只在收到带新值的 `route` 之后推送 | §8.2、§8.3 |
| `joinable_venue_seq` | 流声明：推送与回填的记录带流 epoch 内连续、可衔接的 venue 序号；决定回填坐标，也是边界能否闭合（`Closed`）的前提 | §2.2、§8.4 |
| `backfill_from_origin` / `Origin` | 流声明：集成断言该流可从上游历史的真实起点回填 / `backfill` 窗口起点的一个取值，指上游该流历史的真实起点，只对声明了 `backfill_from_origin` 的流合法 | §2.2、§8.2 |
| 完整界 `(e, n)` | `orders` 每作用域给出的成交完整性：当前流 epoch e 上的一个 venue 序号 n，表示上游在 e 上序号小于 n 的全部执行都在 fold 里；n 是 e 的序号覆盖 `[Origin, n)` 的上端，不说任何事件时间 | §8.1、§8.5 |
| 完整性令牌 | 解释层对外给出完整界的不透明形式：内含周期（该账户成交流的当前流 epoch）与界 n，编码不可读、不可排序；下游只比较两个令牌的周期部分是否相等，不同周期的不比较，旧周期的完整确认不沿用 | §8.5；`design/downstream/design.md` 第 5 节 |
| `order_revision` / `query_not_lagging` / `push_ordered` | 流声明的来源顺序证据：同一订单跨推送与查询渠道单调的修订值（值大者新）/ 该流上不带序号的回答（一次性读、回执、取证）不早于在它的 `dispatch_end` 及之前 append 到该流的任何记录 / 同一会话 epoch、同一流 epoch 内该流两条推送按送达次序（上游经一条有序通道按状态次序交付，不是多路 feed 合并或轮询合成；上游重连是供给中断，以 `Gap{origin: Source}` 开新流 epoch） | §2.2、§8.1 |
| `dispatch_end` / 发出 epoch | 每条回答记录（一次性读的结果项与读结论记录、回执与取证的观察记录）上核心记下的调用出处：这次调用发出时这条记录所在的流已提交的流末位置，发出方在发出时为调用可能写入的每条流各记一个，同一回应的各条出处相同而 `dispatch_end` 各是自己那条流的；供 `query_not_lagging` 比较，本身不给上游状态定序 / 一次调用发出时该流所在的流 epoch：一次性读与回执、取证为记录所带 `dispatch_end` 所在的 epoch，回填为任务所在的 epoch，`route` 为它所带 `generation` 指名的 epoch；它是调用的属性，指名调用问的是哪个 epoch、决定结果能否准入，不是调用的外层（调用只嵌在集成会话里）；`read`、`backfill` 的结果到达时它已不是当前流 epoch 的，由核心完成为 `Unavailable`、不作新 epoch 的记录；`route` 由集成按 `generation` 判定；回执与取证照常 append 而所带序号不作证据 | §6.5、§8.1、§8.2、§8.4 |
| 回填深度 | 运行期参数，按逻辑流给出每次回填任务从 `live_from` 往前补多少：该流坐标上的长度、`0` 或 `Origin` | §8.4、§7.6 |
| `subject` | 观察记录信封上的注册字段，给出该记录的订阅主体，与 `route` 用同一主体身份；订阅按它投递 | §8.1、§8.5 |
| 观察流订阅 | `subscribe` 的一种选择器：一组 `(来源, 流, 主体集?, 用途)` 项，可跨来源，逐项接纳、逐项状态；每条流各自有序，流间不定序 | §8.5 |
| 供给项 / 只投递项 | 观察流订阅项的两种用途：供给项的主体进入 `route` 需求、计入配额，按采纳集合与最近声明接纳 / 只投递项只搬运已有或将有的记录，不增加需求、不占配额，只按声明历史接纳（登记已移除的来源的历史也可订；一次性读 `Pending` 之后等结果即用它） | §8.5 |
| `WholeStreamInPool` | 订阅项挂起原因：整条流的供给项所在的流进入配额池；该流不在任何配额池时恢复 | §8.5 |
| 执行事实订阅 | `subscribe` 的一种选择器：某来源（或其一个作用域）的执行事实与声明版本，含此后才出现的 lane 流；按声明历史里的作用域键接纳；只能 `ordered`、可从起点、无投递损失；程序的请求流不经它订阅；另有 `Control` selector 选核心的控制流（§7.3） | §8.5 |
| `venue_order_id` / 最近观察 | 订单状态记录的订单身份：来源 × 作用域内唯一、生命周期内不变，一笔订单只在一条流上 / 同一身份在它所在流上按来源顺序的极大记录；`orders`、`positions` 与检查项共用 | §8.1 |
| 来源顺序 / 顺序未确立 | 同一身份两条记录之间的先后只凭四种声明的来源证据：同一流 epoch 的 venue 序号（发出 epoch 不是所在 epoch 的回执与取证记录，其序号不算）、`order_revision`、`query_not_lagging`（只对 `dispatch_end` 及之前的记录）、`push_ordered`（只在同一会话 epoch、同一流 epoch 内的两条推送之间）；其余（到达顺序、不声明 `push_ordered` 的流上推送的送达次序、`received_at`、累计量、终态格）都不定序 / 几条极大记录彼此不可比而内容不同时，最近观察并列给出，检查项得 `Undecidable` | §8.1 |
| 记录映射（`RecordMapping`） | 一条上游记录怎样落到对齐点的值：处置表（上游字段名 → 对齐并附换算 / 进扩展 / 丢弃）、枚举映射表、原始负载保留声明；换算是 `Comb<上游值, 契约值>`；随 `Projection.mappings` 交出，集成侧求值，核心握手时静态校验 | §8.1、§2.5 |
| 对齐点 | 契约里上游数据可落的位置：锚点、已注册字段、公共或扩展载荷 schema 的字段 | §8.1 |
| 组合子值树 / `DerivationNode` | 一个 `enum`（deep embedding），判断的共同底层表示 | §2.5 |
| fold | 对值树的一次解释；共五个（`required_inputs`、输出类型、求值、失败、说明） | §2.5 |
| `Pred<X>` / `Comb<X,Y>` / `Scan` | 输出 `bool` 的树 / 输入 X 输出 Y 的树 / 状态累加节点 | §2.5 |
| `DecisionStep` | 程序决策侧节点（`On`/`Emit`/`Require`/`Expire`） | §6.1 |
| `EffectRequest` | 程序的唯一出口：一个副作用请求值，由处理器决定响应；记下发出成员 `member`（开始该成员的 `load_program` / 替换 `Applied` 在控制流上的位置），写处理器与重派按该 `Applied` 所记事实处理 | §6.1 |
| 发出成员 / 装载 principal | `EffectRequest` 的 `member` 所指、开始该成员的 `load_program` 或替换 `Applied` / 该 `Applied` 的 principal；写处理器以它为单据负责人、按该 `Applied` 所记的执行事实输入判定 `ScopeNotObserved`，成员已被替换或卸载、程序值文件已改或删去时亦然 | §6.1、§8.5、§8.6 |
| `EffectResponse` | 每条被处理的 `EffectRequest` 的完成事实（执行事实侧）：读：`Concluded`/`Unavailable`/`NotCalled`；写：`Drafted`/`NotDrafted(Malformed \| ScopeNotObserved)` | §6.1 |
| 请求流 / 程序的输入 | 每个程序一条、按程序 id 的执行事实流，承载它的 `EffectRequest` 与全部 `EffectResponse` / 程序只经三种输入看到记录：`required_inputs` 的观察流、值树声明的执行事实输入 `(来源, WriteScope?)` 与隐含的请求流；各有 cursor，在开始成员的 `load_program` `Applied` 同一事务按声明的起点建立（`Tail` 取该 `Applied` 时的流末；沿用旧状态的替换原样沿用共有输入的），`Reset` 时重建；写请求的作用域不在声明之内即 `NotDrafted(ScopeNotObserved)` | §6.1、§8.6 |
| 观察宇宙 / 效应宇宙 | 不可写 / 可写两套独立类型宇宙，不共享类型 | §3.2 |
| `Ticket` / `TicketAction` | 意图形成期的锁 = 责任持有；`responsible` 字段的存在即锁 | §6.2 |
| `IntentAlignment` / `Revision` | 意图专属逐项对账 / 意图版本间的结构差 | §6.2 |
| 交易协议的操作种类 | 预置写侧基本类型“订单”的 `OperationKind` 封闭集合：`Place`（下单）/ `Cancel`（撤单）/ `Replace`（改单）/ `Close`（平仓）；各自的 `target` 与守卫字段 | §6.2 |
| 意图参数 schema / `parameter_validity` | 写能力 `(scope, OperationKind)` 在 `CapabilityProof` 里声明它接受的意图参数 schema（公共意图 schema 或其只加字段与约束的扩展，JSON Schema）/ 当前版本参数是否合它、目标种类是否被接受的单据 fold 状态（`Valid` / `Invalid` / `CapabilityNotEstablished` / `NotSupported` / `SchemaMismatch` / `TargetNotAccepted`），输入约束步无条件读取 | §6.2、§6.3 |
| `CapabilityNotEstablished` | 会话有效声明对该 `(scope, OperationKind)` 是 `Unknown` 或来源没有已建立的会话时的参数合规状态：非终结，单据在输入约束步等待能力确立，到 `deadline` 关闭 | §6.2、§6.3 |
| 检查目录 | 交易协议的第二层检查项闭合集合：能力、可交易性、敞口、持仓在、原单仍在；每项的输入、判据与参数由协议写定，规则文件只给取值与必要 / advisory | §6.2、§7.6 |
| `PositionRef` | 平仓意图的 `target`：从同一条持仓观察记录一次构造的不透明值，含作用域内稳定的持仓身份与 instrument | §6.2 |
| 冷却 | `(WriteLaneKey, instrument)` 上最近一次 `Place` 或 `Replace` 尝试的 `SendBarrier` 时间；间隔按 `(WriteLaneKey, OperationKind)` 给出、跨 principal 共享；单据经 lane 步放行时判定（不是等待条件；从任一等待重入都重过 lane 步，冷却随之重判），冷却期内否决 | §6.3 |
| `RuleState` 是记录的 fold | STS 判断所用的状态（lane 阻塞头、冷却、待决集合）每次求值时由执行事实 fold 出，不另存、没有规则状态表；重启后与平时同样按记录求值 | §6.3、§7.5 |
| `checked_as_of` | 一次 `alignment` 评估实际消费的位置集，随 Decision/`Outcome`/`Rejection` 记录；与 `basis`（拟单时看到的）互补 | §6.2、§6.3 |
| `basis` / `basis_validity` | 意图引用观察侧位置的承载：位置集 `Set<LogPosition>`；其有效性 `Fresh`/`Stale`/`Retracted`/`BeyondRetention`。唯一边是效应侧读观察侧这个方向，`basis` 是其上意图的那一份位置集 | §5.1、§5.2 |
| `Journal` | 记录载体；观察侧 `Journal<RetractableDelta>` 可撤回可压缩，执行事实侧只 append | §4.1、§3.1 |
| STS / lane | 决策代数：顺序固定规则链（授权 → 输入约束 → 审批 → lane → 过期）/ 每 `WriteLaneKey` 的写通道全序 | §6.3、§6.4 |
| `WriteScope` / `WriteLaneKey` | 可写作用域（多账户在核心里的存在形式，带 `account_ref`、`label` 与挂在其上的流名）/ 其不透明键，不外露 | §2.2、§6.4 |
| `Capability` / `Verdict` | 写侧 (scope, operation) → `Supported`/`Unsupported`/`Unknown` 的能力证据；读侧的三值能力按流声明在 `StreamDecl` 上；两侧握手后的变化都经 `CapabilityObserved`（§8.3、§7.5） | §2.2 |
| `CapabilityProof`（`WriteProof`） | 写能力 `Supported` 所带的声明：写操作（`Submit` \| `Cancel`，一次上游写）、键角色、取证渠道、意图参数 schema 身份、接受的订单目标种类 | §2.2、§6.2 |
| 调用方键 / 键角色 / `KeyGuarantee` | `SendBarrier` 所记的键 `K(AttemptRef)`：核心以 `AttemptRef` 的固定单射编码铸造，`EffectRequest` 与意图里没有键 / 这个键标识订单（订单键）、只标识这次请求（请求键）或不带键（`None`） / 集成声明的键作用域与上游保证唯一的期限，集成按 `barrier_at` 判断键是否仍在期内；三者随 `SendBarrier` 落盘，之后不按声明重读 | §2.2、§6.5、§6.6 |
| 可执行性 | 一版意图对当前能力证据可执行：`Supported`、声明接受其参数 schema、`target` 的种类在接受之列；在参数合规（`TargetNotAccepted` 等）、能力项与发出前门三处读同一个谓词 | §6.2 |
| IO 壳 | 效应侧的解释器：核心中唯一向集成发出写调用、把集成的返回值变成记录的地方 | §6.5 |
| `Attempt` / `AttemptRef` | 一条 `Prepared` 记录及其后继阶段链 = 对上游的一次写，身份 `AttemptRef = attempt_position`（该 `Prepared` 的位置）；所有尝试级记录与归因以它关联；正常路径下同 lane 至多一次写在等待 | §6.5 |
| `Prepared` / `SendBarrier` / `Undetermined` / `Expired` | 阶段链的记录：已放行待执行 / 发送屏障（已 fsync，之后才可调用这次尝试的写操作；记下写操作、键 `K(p)`、键角色与 `barrier_at`）/ 已发出但结果未知 / 交出之前 `deadline` 已过 | §6.5 |
| `NotSent` | 集成能证明这次写未交出上游时的写调用结果（`SchemaMismatch` / `LocalRefusal`；已交给 SDK 缓冲的不算）：结果确立为“未发生”，结束等待、解除 lane | §6.5、§8.3 |
| `barrier_at` | `SendBarrier` 的核心时刻；by-key 与 replay-by-key 取证带它，由集成判断键是否仍在上游的唯一期与保留期内 | §6.6、§8.2 |
| 发出前门 | IO 壳在 append `SendBarrier` 之前对一次尝试求值的三个条件：`deadline` 未过、该集成会话已建立、意图对会话有效声明可执行；结果为发送、等待（尝试保持 `Prepared`，不 append 记录）或 `Expired` | §6.5 |
| `ResolutionEvidence` / `ReconciliationReopened` | 一次取证的执行事实记录（`channel × outcome`，`Found` 含 `Evidence` 并以位置引用命中的那条观察记录）/ 重开一轮取证的标记（`SessionRestored` / `Manual(principal)`） | §6.5、§6.6 |
| 等待 / 结果两根轴 | 一次尝试的两个独立状态：等待（UTA 还要不要等，源头是 UTA）`Active` / `Finished` / `Expired` / `Abandoned`；结果（这次写发生没有，源头是上游）未知 / 已确立；principal 只能动等待轴 | §6.5 |
| `Abandoned`（放弃跟踪） | principal 经 `abandon` 结束 UTA 对一次结果未知的写的等待：在途取证调用先各自完成，结果仍未知才 append；不断言写发生或未发生，之后结果仍可由来源证据补上，`retry_reconciliation` 可再问一轮；写可能已交出之后，结果未知而结束等待的唯一出口（`Expired` 只在交出之前） | §6.5、§6.6 |
| `Gap{origin}` | 显式标记的缺口，`origin ∈ {Source, Delivery, Channel}`：`Source` 与 `Channel` 是流上或执行事实侧的记录；`Delivery`（投递缺口）是订阅表里按（订阅，流）记的状态，不在任何流上 | §4.2、§7.5 |
| `Pooled` | 值树里的读侧组合子，核心暴露给可选子系统的唯一接口 | §4.5、§8.7 |
| 段视图 | `Pooled` 物化完整窗口后交给原生 op 的可借用视图（非逐条值） | §4.5、§8.7 |
| money / quantity / 身份 / 时间 | 写侧基本类型的守卫字段与核心计算处用的基础值类型；观察侧信封不含价格 | §2.6 |
| 集成进程（integration） | 一个独立 OS 进程，上游的唯一消费点：把上游协议消费为信封 + 契约载荷，按保留规则附原始负载，握手声明投影，是凭据终点与独立故障域 | §7.1、§8.1 |
| 契约的声明 / 记录映射 / 行为 | 核心在任何操作前读取并 fold 的值（投影、锚点对齐、字段注册、扩展 schema）/ 上游记录落到对齐点的值，集成侧求值 / 适配器代码：对每个契约操作的上游调用编排与需上下文的判定，只以封闭返回值可见 | §8.1 |
| 契约操作的粒度 | 每个操作是 UTA 的一个意图（要什么），不照搬上游接口；一个操作一个封闭结果，写意图至多一次上游写 | §8.1 |
| 业务 | 策略与信号、风控与审批规则的具体内容、下单组合方式、指标、告警、组合视图；在二次开发时组装，不在 UTA 里 | §0.1 |
| 写侧基本类型 | 写意图的类型，信封与 STS 的概念由它而来；预置“订单”（交易协议），可再加；只含核心代数实际读取的字段 | §0.1、§2.2 |
| 解释层 | 核心之外的下游侧：把核心的操作、记录与状态逐项清洗成对外概念，经一次性命令与双向长连接交给下游；不持有状态，落点由实现定 | §0.1、§7.1、§8.5；`design/downstream/design.md` |
| 对外面 | 解释层交给下游的命令、参数、结构化输出与长连接消息；跨仓库契约，不出现核心概念 | §0.1、§7.1；`design/downstream/design.md` |
| 续传令牌 | 对外的不透明续收凭据，编码核心侧的确认进度；下游保存，解释层不保存 | `design/downstream/design.md` |
| 程序宿主（program host） | 解释值树程序的受监督子进程，只提供隔离与预算，不进设计中心；协议 = `Load`/`Advance`/`Reset`/`Unload` | §7.1、§8.6 |
| `Checkpoint` / `state_version` | 程序状态的显式序列化字节及其版本号；与程序 cursor 同事务持久化；`Load` 只交回本成员的（开始成员的 `Applied` 所沿用的，或其后持久化的）；本成员可交回的 `Checkpoint` 的 `state_version` 总在本成员接受的集合内（替换的 `Applied` 比对沿用的，`Output` 检查其后持久化的，违反视同 trap），`Load` 不比对版本 | §8.6、§7.5 |
| 程序的活动集合 / `ProgramHalted` | 控制流上 `load_program` / `unload_program` 的 `Applied` 的 fold，`Applied` 以值记下成员事实：装载 principal、内容 hash、预算、接受的 `state_version` 集合、声明的执行事实输入集合；改清单或程序值文件不改变它；所引用来源尚无声明版本的成员留在集合里等待（首次装载、替换或卸载后再装载开始的新成员皆然），不拉起宿主 / 超预算、trap、内容不符与装载期校验失败时与观察 `ProgramFailed` 同事务 append 在控制流上的执行事实：失败抑制，跨重启保持，只由以位置引用它的 `load_program` `Applied` 解除 | §8.6 |
| 替换（程序） / 沿用 | 对已在活动集合里的程序 id 再 `load_program`：先结束旧宿主执行（停调度 `Advance`、等在途输出事务、`Unload` 且 OS 确认退出），再以一条 `Applied` 结束旧成员、开始新成员 / 替换时 `cold_start` 为假、且旧成员没有 `Checkpoint` 或新程序接受它的 `state_version`：共有输入的 cursor 与保留引用在该事务原样转给新成员，`Applied` 记下沿用的 `Checkpoint`；不沿用时该事务照 `Reset` 处理（`ProgramReset{Replace \| Operator}`） | §8.5、§8.6 |
| `SessionEpoch` / `instance_id` | 会话 epoch `(instance_id, session_seq)`：`instance_id` 随 fence 单调递增，`session_seq` 在核心每拉起一个集成进程时分配（一个进程恰一个会话，`Unavailable` 的再握手不换 epoch）；核心给经该会话通道读入并 append 的记录盖上它，集成不在消息里回填 | §7.1、§7.2 |
| 会话即通道化身 | 核心拉起集成进程时创建、只有该子进程继承的通道就是会话；接受条件是“经当前会话的通道读入”，通道关闭之后那个会话送来的任何东西都不再读入；在途调用恰好完成一次、核心关闭通道时会话结束 | §7.1、§7.2 |
| 凭据副本 | 核心拉起集成进程时经继承句柄交付的凭据，只在该进程内存在，随进程的 OS 确认退出结束；换凭据 = 换进程（`rotate_credential`） | §7.1 |
| 会话状态 | 集成会话为每个登记的集成运行的 `Connecting`（无会话，自动重连）/ `Established(SessionEpoch)` / `Halted{cause}`（无会话，需运维动作；`cause ∈ {ProjectionInvalid, ContractIncompatible, Refused}`；跨核心重启由执行事实 `IntegrationHalted` 保持） | §7.2 |
| 集成会话 | 不属任一侧的核心元素：会话状态机、集成进程的拉起与终止、握手与边界接受、核心→集成的调用通道（每个调用恰好完成一次）与调用计数、每个来源的最近声明与会话有效声明、登记的采纳记录与 `IntegrationHalted` | §7.3 |
| 采纳记录 / 采纳集合 | 启动第 3 步由集成会话 append 在控制流上的执行事实：集成登记文件的内容 hash、其中列出的 id 与本实例 `instance_id`（`restart_integration` 的 `Applied` 另采纳单个 id，带同一 `instance_id`）/ 控制流上一个位置的切面：该位置及以前最近一条采纳记录的 id，并上其后到该位置、带同一 `instance_id` 的 `restart_integration` `Applied` 的 id；本实例运行哪些集成、健康面列哪些；一次性读与供给项的“来源未登记”按它判定，只投递项与执行事实 selector 不看它 | §7.2 |
| 控制流 | 每个用户状态根一条、不属任何来源的执行事实流：登记的采纳记录、除 `bypass_lane`（随其 lane 流）之外的全部控制记录、`IntegrationHalted` 与 `ProgramHalted`；采纳集合、持久 `Halted`、程序的活动集合与失败抑制都只按它的位置 fold；以自己的执行事实 selector `Control` 订阅，只能 `ordered`、可从起点 | §7.3、§8.5 |
| 集成运行 | 一个采纳的集成在一个核心实例里的运行：始于让它进入初始状态的 `Connecting` 健康观察（启动第 3 步，或 `restart_integration` 采纳本实例尚未运行的 id），或解除 `Halted` 的 `Applied`（只在上一次运行结束后提交）；`Halted` 路径上止于 `IntegrationHalted` 之后最后一个进程 OS 确认退出、清除进程表的行，否则随实例结束；`Connecting` 中换进程不结束它 | §7.2 |
| 实例表 / 进程表 | 核心实例的 fence 行（`instance_id`）与受控停止写的结束锚点 / 指向核心拉起的 OS 进程的索引 `(instance_id, pid, start_time, role)`，进程存亡只问 OS，OS 确认退出后清除 | §7.2、§7.5 |
| 受控停止 | OS 的停止请求按生命周期从内到外结束本实例：关闭新工作入口与消费方会话 → 程序 `Unload` → 集成会话以在途调用恰好完成一次结束 → OS 确认全部子进程退出、清空进程表 → 写实例结束锚点 → 释放 fence；有进程得不到确认时不写结束锚点 | §7.2 |
| `IntegrationHalted` | 集成进入 `Halted` 时由集成会话 append 在控制流上的执行事实（原因、`SessionEpoch`），与健康观察同事务；它开始 `Halted` 抑制，不是集成运行的结束锚点；解除它的控制记录 `Applied` 以位置引用它 | §7.2 |
| 健康面（`IntegrationHealth`） | 每个集成的会话状态、逐流 readiness、逐流当前 epoch 的回填进度、按调用目标（作用域或逻辑流）的连续失败数与最近成功时间；成员是 `as_of` 时的采纳集合（控制流的 fold），各字段是健康观察的 fold，不进写路径 | §8.4 |
| 健康流按键保留 | 健康观察是按键的状态值，同键后一条取代前一条；压缩时每个键在保留边界之下的最新一条留作基线，fold 对任一不低于边界的 `as_of` 不变 | §2.4、§8.4 |
| 回填进度 | 核心按流 epoch 判定的回填任务状态 `Backfilling{through}` / `Closed`（可衔接序号，边界闭合）/ `Reached`（只有事件时间，补到 `live_from` 而衔接未证明）/ `Incomplete{through}`：`through` 由读结论记录证明；任务是否建立（能力、需求、深度）在 `live_from` 声明时判定一次，本 epoch 之后不补建；不属 readiness | §8.4 |
| principal | 会话绑定的身份 `(os_user, actor)`：OS 对端凭据给出 `os_user`，下游自报 `actor`、经解释层带入握手；授权与审计的键 | §8.5 |
| `Snapshot` / `as_of` | 读模型的一次读取结果，及其吃到的位置集 `Set<LogPosition>` 与该 fold 所依赖的观察输入上未被来源证据闭合的 `Gap{origin: Source}`（`gaps` 只列来源缺口；只 fold 执行事实的 `lanes`、`sources` 为空）；`tickets`、`subscriptions` 只给当前态，没有历史切面 | §8.5 |
| `sources` | 读模型的一种：各来源最近声明版本加引用该版本的能力变化的 fold，解释层据此得到账户、流与读写能力；可按历史 `as_of` 读 | §8.5 |
| 信任边界 = OS 用户 | 写权限来自认证得到的 principal × 策略 scope，不来自连接；同用户进程视为用户本人（H7） | §7.1 |
| 凭据链 | `统一路径封存文件 → UTA 核心 → 该集成进程`，拉起时经继承句柄交付凭据副本；程序与消费方只见账户身份 | §7.1、§7.6 |
| 单实例（single instance） | 同一用户状态根（`OPENALICE_HOME`）只允许一个核心实例，由 OS 文件锁 + fence 保证 | §7.1、§7.4 |
| 传输（transport） | IDL 之下的编码/信道，按 OS 选择（本地回环 + 令牌或命名管道），不改变 IDL | §7.1、§8.1 |
| 模块指南（module guide） | 每个元素的 拥有 / 隐藏 / 假设 三列，及其对应的 §2–§6 抽象 | §7.3 |
| uses 图 | 模块间的依赖方向；唯一跨宇宙依赖是效应侧 → 观察侧（以位置引用承载：`basis`、`checked_as_of`、读模型 `as_of` 等），反向不成立 | §7.3 |
| 操作集（operation set） | 核心经集成会话对集成的调用与集成对核心的推送，构成跨协议 IDL 契约 | §8.2、§8.3 |
| 持久化归属表 | 每份状态谁写、谁读、怎么传播 | §7.5 |
| 配置/凭据归属 | 统一路径下的文件契约：每文件唯一写者、格式版本只前进 | §7.6 |

## 11.2 五种关系，五种承载

| 关系 | 承载 | 验证阶段 |
|---|---|---|
| operation ↔ capability | 能力证据值（§2.2） | 运行期握手 |
| request/resource ↔ provider 身份 | `StreamId`、外部订单 id、幂等键 | 构造期 / 运行期 |
| 多 effect ↔ 同一作用域 | 单条 STS 事务 | 构造期 |
| program ↔ 解释器 | 解释选择（①派生 / ②决策） | 构造期 |
| intent ↔ 结果 / 审计 / 重放 | 执行事实日志的位置 + causation id | 运行期，持久 |

把这五种关系塞进一个对象的字段互指是反模式。[证据：fp-03 命题 2]

## 11.3 同名异义表

| 词 / 概念对 | 甲 | 乙 | 区分依据 | 节 |
|---|---|---|---|---|
| 意图的三个阶段 | `EffectRequest`：程序发出的请求，未定型、无负责人 | `Ticket` 的 `Version<Intent>`：定型的意图，有负责人与 `basis` | 第三阶段是 STS 的 `Input`：规则输入，含意图、回执、超时、证据 | §6.1、§6.2、§6.3 |
| `Prepared` | `Ticket.Close(Prepared(position))` 的结果：单据关闭 | IO 壳链的起点：`Prepared → SendBarrier → …` | 同一条记录，单据 → IO 壳的唯一交出点，两者同属效应宇宙；单据认“我已交出”，IO 壳认“我该做的” | §5.3、§6.2、§6.5 |
| 对账 / 决议 | 对账（alignment）：我的意图还对不对世界（`IntentAlignment`） | 决议（resolution）：我的动作发生了没有（`ResolutionEvidence`） | 前者在 `Prepared` 之前、只看观察侧；后者在之后、由 IO 壳驱动 | §6.2、§6.6 |
| 三种“对齐” | 锚点对齐：集成把上游账户、市场、身份结构对到锚点契约（判断性设计动作） | 记录映射的字段对齐：上游字段对到契约字段 | 第三种是对账（`IntentAlignment`，见上一行），与前两者无关 | §2.1、§8.1、§6.2 |
| 修订 / 偏离 | 修订（`Revision<Intent>`）：意图改了，世界没变 | 偏离（`Diverged`）：世界变了，意图没变 | 来源不同（`Revise` vs 观察推进） | §6.2 |
| 修订 / 撤回 | `Revision<Intent>`：意图版本间结构差，无逆元需求 | `RetractableDelta`：观察侧撤回代数，有逆元 | 前者属效应宇宙，后者属观察宇宙 | §6.2、§4.1 |
| 三种“无法判断” | `InputMissing`：检查项需要的观察流根本没有（集成不提供） | `Undecidable`：流存在但有 gap，没有本项主体（该 instrument / 该持仓）的观察，或最近观察顺序未确立 | 第三种 `inconclusive`：读渠道穷尽而写结果仍未知，属决议；等待继续，或由 principal 放弃跟踪 | §6.2、§6.6、§8.1 |
| 能力未知 / 结果未知 | `Verdict::Unknown`：venue 是否支持该操作不知道 | `Undetermined`：发出的写是否生效不知道 | 前者约束启动、是握手结果；后者约束恢复、是链状态 | §2.2、§6.5 |
| `SendBarrier` / `AwaitingDecision` | IO 壳发送屏障：`Prepared` 之后，外部动作即将发生 | 单据送审：`Prepared` 之前，无外部动作 | 二者相隔整条 STS 链 | §6.5、§6.2 |
| `basis` / `required_inputs` | 值：这张单据实际引用了哪些 `LogPosition` | 类型：这个检查/处理器要读哪些 `StreamKind` | `required_inputs` 从组合树派生；`basis` 从实际评估记录 | §6.2、§2.5 |
| `LogPosition` / `Hash` | 日志位置：顺序身份 | 内容寻址：版本身份 | `current_version` 是 `Hash`，`Prepared(position)` 是 `LogPosition` | §4.1、§6.2 |
| 四种“过期” | `Ticket.Close(Expired)`：负责人失联或审批超时，或 lane 等待期间到期（H6，STS 过期步，在 `Prepared` 之前） | `DecisionStep::Expire(Deadline)`：程序规则时限 | 见表下 | §6.2、§6.1、§6.5 |
| 两种“拒绝” | `VenueRejected`：写已发出，venue 拒了；执行事实、终态之一 | `DecisionRejected`：审批人否决，或 STS 链否决（参数不合规、允许集合、冷却、放行门等）；单据关闭，从未进入 `Prepared` | 前者在链上，后者在单据上 | §6.5、§6.2、§6.3 |
| 两种 schema 身份 | `payload_schema`：观察记录的契约载荷按哪份 schema 读，随 `StreamDecl` 声明 | 意图参数 schema：写意图的参数按哪份 schema 校验，随写能力的 `CapabilityProof` 声明，每版意图带它 | 前者标观察的载荷，核心不校验；后者在输入约束步校验意图参数，校验后参数原样交给集成 | §2.2、§6.2、§8.1 |
| 上游的两种 `Refused` | `handshake` 返回 `Refused`：上游明确拒绝集成的身份或配置，整个集成登记 `Halted`，等运维动作 | `read`/`backfill` 返回 `Refused`：上游拒绝这一次读请求，集成照常运行，不是 gap | 都只在上游给出明确拒绝时返回；不可达、超时一律 `Unavailable` | §8.2、§8.3 |
| 离线 / 待处理 | `Connecting`：没有会话，会自己恢复（传输失败、上游暂时不可达） | `Halted`：没有会话，不会自己恢复（投影不合法、契约不兼容、身份被拒） | 二者都不发写（发出前门等待）；只有后者要 `restart_integration` 或 `rotate_credential` | §7.2、§6.5 |
| 读的“拿不到” | 不调用集成、不记 gap：`UnknownTarget`：来源未登记；`Unavailable{source_state}`：来源从未有过声明版本，或此刻没有已建立的会话；`Unsupported`：会话有效声明说该流不支持此读，或流不在会话有效声明里；`Unconfirmed`：会话有效声明说能力未知；`InvalidRequest`：请求不合 schema 或 schema 身份不一致 | 调用之后：`Unavailable{gap}`：渠道失败（已记 `Gap{origin: Channel}`）；`Pending`：`deadline` 到而调用在途（之后照常记结论或 gap，不是 gap；只限发出它的核心实例，以所带 `instance_id` 辨认）；`Refused`：上游明确拒绝这次请求，记读结论记录，不改能力 | 空回答是 `Answered`，不属“拿不到”；判定顺序与结果见 §8.5 一次性读 | §8.2、§8.5 |
| 账户的三个名字 | `WriteLaneKey`：核心路由键，不外露 | `account_ref`：对外账户引用，稳定、不复用 | 第三个 `label`：只给人看，可重复、可改 | §2.2 |
| 名义数据等级 / 记录上的等级 | 流声明的名义等级：读之前告诉下游“通常是什么” | 公共载荷里上游报告的实际等级 | 不一致时以记录为准；声明不担保逐条 | §2.2 |
| 两种“证据” | `CapabilityProof`：venue 有这个能力（握手结果） | `ResolutionEvidence`：我的尝试发生了没（对账结果） | 前者进 `Projection.capabilities`，后者进链 | §2.2、§6.5 |
| 形状 / 投影 | 上游形状：上游对象长什么样，只在集成里被消费，从不越过集成 | 投影：有几个作用域、几条流、各支持什么；UTA 定 schema、集成填、核心按它路由，并把声明部分经读模型 `sources` 交给解释层翻成对外概念 | 账户只是特例；订单、持仓、流、渠道都如此；外部下游不见投影本身 | §2.2 |
| 载荷 / 原始负载 | 载荷：集成的消费结论，契约 schema，程序与钩子解释 | 原始负载：集成所消费的上游原文，只作证据 | 前者是结论，后者是出处；两者一起进 `Evidence` | §2.1、§6.5 |
| 读模型 / 归因后的订单观察 | 读模型：核心对记录的非权威 fold（种类与输入见 §8.5），规则不引用 | 集成产出的带出处记录，钩子可读 | 都描述“订单现在什么状态”；一个是解释、一个是记录 | §4.4、§6.2 |
| 归因字段的归属 | 记录归观察侧：`attribution` 落在订单/成交观察记录上 | 响应归效应侧：读它的处理器（lane 决议匹配、读模型归因）注册在效应侧 | 由谁填：集成填；IO 壳在回执与取证的观察记录上，只对由该条自己的关联证据确定属于这次尝试的记录补 `FromAttempt(AttemptRef)`，同一回应里的其余记录不继承；填不出记 `Unattributed` | §5.3、§2.1、§6.5 |
| 锚点 / 处理器字段 | 锚点：缺失 = 畸形记录，链路不成立 | 处理器字段：缺失 = 处理器不触发，不是错误 | 前者闭合、入口即验；后者开放、按注册表 | §2.1 |
| 入站处理器 / 出站处理器 | 集成进来的字段出现 → 做什么（§2.1） | 程序出去的请求出现 → 做什么（§6.1） | 同一形状，方向相反；后者必须声明读/写 | §2.1、§6.1 |
| `Pooled` 组合子 / 原生 op | `Pooled`：值树里的读侧组合子，核心暴露给可选子系统的唯一接口 | 原生 op：子系统提供的注册表黑盒，要求输入是 `Pooled` 的 | 前者属核心代数，后者属可选子系统 | §4.5、§8.7 |
| 窗口 / delta | 触发时可见的完整 `LogPosition` 区间，在段池里物化为若干段的有序引用集；不向保留边界登记，越界得 `BeyondRetention` | 本次推进的增量记录 | 记录渐进不要求算法渐进；增量在节点粒度 | §4.3、§4.5、§8.7 |
| `Journal` / 段池 | 记录的载体，持久于 SQLite，由保留语义管理 | 为高性能计算设计的运行期快照，由 `Pooled` 洗入，永不持久 | 两套存储，互不派生；只共用 `LogPosition` 标定 | §4.1、§4.5、§7.4 |
| 读副作用 / 写副作用 | 读：不改变世界，可重试、可批、可丢，结果总可判定 | 写：改变世界，一次，可能 `Undetermined` | 与对象轴（§3.2）正交；对账取证是读 | §3.4 |
| `Program` 值 / 程序运行时 | JSON 值树，核心解释它 | 受监督子进程（宿主协议；Wasm 是走同一协议的替代宿主），解释器的宿主 | 运行时选择不改变值；预算靠宿主不靠类型 | §4.3、§6.1、§8.6 |
| `Transfer` / 协作 | 换负责人：单据始终只有一个负责人 | 协作：核心之外（另起单据、给负责人建议） | 单据不支持共同编辑 | §6.2 |
| lane 队首阻塞 / 锁 | 通讯协议语义：后续写的含义依赖队首结果 | 锁：消费者自己持有的互斥 | UTA 不是消费者；队列有序才重要 | §6.4、§6.8 |
| `Gap{origin}` 三种来源 | `Source`：来源断代，流有缺口（流上的记录） | `Delivery`：慢消费者或 conflated，某订阅的投递有缺口；是订阅表里的状态，不在流上 | 第三种 `Channel`：UTA 自己那次读调用（取证、回填、一次性读）的渠道不可用，是那次调用的结果，不计入 `gaps` | §4.2、§7.5、§8.3、§6.6 |
| 三种去重 | 投递去重：同一条记录被同一订阅者重复看到，按 `LogPosition` 去重（§4.2） | 按执行计数：同一笔执行经不同渠道成为不同记录，按 `execution_id` 与修订计一次（§8.1） | 第三种是回填与实时的边界：按范围不重叠（`live_from`），不逐条去重（§8.4）；三者互不替代，按执行计数不以 venue seq 为键 | §4.2、§8.1、§8.4 |
| 解释层 / 解释器 | 解释层：核心之外把核心清洗成对外接口的一层，面向下游 | 解释器：核心内对值树的解释，包括五个 fold、程序的解释①② 与宿主 | 前者是接口清洗，不含抽象；后者属设计中心 | §0.1、§2.5、§4.3、§6.1 |
| 两种“下游” | 核心内的“交给下游”：解释载荷的程序、钩子与消费方（§2.2） | 核心之外的下游：Alice、CLI 使用者、外部客户程序，只经解释层接触核心 | 前者说的是载荷的解释权，后者说的是接触核心的途径 | §2.2、§0.1、§8.5 |
| “快照” | 核心快照：`fold_state` 的重启加速点，只供重启恢复，不对外读（§7.4、§7.5、P15） | 读模型 `Snapshot`：`read_model` 的一次读取结果，带 `as_of`（§8.5） | 段池也称“运行期快照”（§8.7）；旧 UTA 的账户快照（A36–A38）是组合视图，属业务，UTA 不提供（§10.6） | §7.4、§8.5、§10.6 |
| `Starting` | 操作结果：核心启动第 5 步开放下游会话之前，除 `handshake` 外的操作一律返回它（§8.5 会话与 principal） | 流 readiness：会话已建立、该流在当前流 epoch 里尚未声明 `live_from`（§8.4） | 前者按整个核心、只在启动期；后者按集成 × 流、每次重连与每个会话内新开的流 epoch 都会经过；开放之后某集成的流处在 `Starting` 不使任何操作返回 `Starting` | §7.2、§8.4、§8.5 |

**四种“过期”的后两种：**

- 第三种：尝试记录 `Expired(deadline)`。已放行，但这次写交出之前 `deadline` 已过，IO 壳不发并结束等待；在崩溃恢复窗口与在发出前门等待会话或能力期间可达。写可能已交出之后不再有 `Expired`，结果未知时结束等待的唯一出口是 `Abandoned`。
- 第四种：写调用的时限。它在集成里：集成在自己的时限内得不到回执时返回 `NoResponse`，不是终态，只是 `Undetermined` 的原因之一；核心没有调用时限。

## 11.4 研究与调查报告索引

每个报告一行：路径 / 它是什么的一手出处 / 支撑的章节或条目 ID。本索引只作证据出处，不复述设计结论。

- 核心证据在 `design/research/*` 与 `design/investigation/*`。
- 可选子系统的证据在 `design/hpc-derivation/research/*`，由其文档 §12 逐篇索引。
- 本设计不改动它们。

| 路径 | 一手出处 | 支撑的章节 / 条目 ID |
|---|---|---|
| `design/research/fp-00-synthesis.md` | fp-01–fp-05 五份 FP 调查的综合索引与三把尺子（统一 litmus、五种关联、能力三阶段） | §0.1；§10.1 大对象行 |
| `design/research/fp-01-haskell-finance-cases.md` | Haskell 金融/多 provider 生产系统（Haxl、Composing Contracts、Marlowe、cardano-ledger STS、Mercury 等） | §2.5（M9/M10 闭合构造子）、§4.3、§6.1（M7）、§6.3；§10.1 值树/程序/`EffectRequest`/泛型 Embed 行 |
| `design/research/fp-02-scala-jvm-cases.md` | Scala/JVM 交易与 provider 栈（gvolpe/trading、Fetch、Stitch、ZIO、fs2 等） | §0.1（命题 12）、§2.2（命题 1/2）；§10.1 大对象行 |
| `design/research/fp-03-effect-composition-and-open-providers.md` | 效应组合与开放 provider 的理论/库（tagless final、Free/DTC、Servant、能力三阶段、reify+event sourcing） | §2.2（命题 3/8）、§2.5（条目 2/4/5）、§6.3（命题 6）、§4.4（命题 1）；§10.1 泛型 Embed / 类型级 capability / 全局 effect enum 行 |
| `design/research/fp-04-base-types-and-domain-primitives.md` | 基础类型与域原语（safe-money、Squants、DMMF、Incremental、幂等、位置/时钟） | §2.1（命题 15/16）、§2.3（命题 10/11）、§2.6；§10.1 信封解析行 |
| `design/research/fp-05-streams-incremental-frp.md` | 观察侧流/增量/FRP（fs2、Incremental、Salsa、Differential、Materialize、Pine Script） | §2.4（案例 13⑤）、§3.1（命题 12）、§4.1（命题 12）、§7.7；§10.1 增量引擎 / 两侧共表 / 快照重建行 |
| `design/research/fp-06-reconciliation-and-in-doubt.md` | 写边界 / in-doubt / 对账的一手出处（2PC 先例、四面泄漏、命题 1–6） | §6.5–§6.7；§10.1 两阶段行；§10.5 #8/#17 |
| `design/hpc-derivation/research/fp-07-*.md` … `fp-12-*.md`（含 `fp-11-appendix-*.md`） | 可选子系统的一手证据（类型导出与外部编译、段池选库、算法层与 SIMD、数组中间层、原语集实证、L2 订单簿 demo） | hpc-derivation/design.md §12 逐篇索引；本设计只经 §8.7、Q24/Q30 引用 |
| `design/investigation/alice-consumers.md` | Alice 消费面真实入口（SDK/路由/UI/connector/CLI） | §1.3.1（A 表）、§1.4、§1.6.4；S10/S11；§8.5 操作集与读模型集合 |
| `design/investigation/existing-capabilities.md` | 旧 UTA 可观察行为、后台任务、持久化与 20 条缺陷 | §1.1（O11）、§1.3.1；C9–C14；Q8/Q17/Q19/Q20 |
| `design/investigation/venue-capabilities.md` | venue 能力矩阵（推送流/幂等键/回读/续传游标；限额与 pacing） | §1.1（F6/F7）、§1.2（P1）、§1.6.1；§10.1 写批处理行 |
| `design/investigation/rust-feasibility.md` | Rust 生态可行性（tonic/Windows UDS、Wasmtime 快照缺口、fuel/epoch、gRPC 流、rust_decimal） | §1.6.2、§7.1、§8.6、§7.7；§10.1 持久化引擎 / 线缆编码 / 程序隔离运行时行 |
