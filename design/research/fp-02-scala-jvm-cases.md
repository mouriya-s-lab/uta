本组案例中最常见的设计中心是“可组合的计算载体 + 明确的解释/运行边界”（`ZIO[R,E,A]`、`Fetch[F,A]`、`Stitch[T]`、`Stream[F,O]`、FSM/Petri net），它们把 provider、能力和业务数据放进参数、代数、Group/Layer/Shape 或事件中，而不是把所有业务字段对齐成一个巨型 object。
- 这不是“抽象越多越好”：每个载体都把一部分关系交给解释器、资源 scope、消息 envelope 或 provider-local metadata；缺少这些关系时不能从类型名推导出审计、幂等或 replay 保证。
- 最强的批处理设计来自 Stitch/Fetch：先聚合同一 provider 的独立请求，再在解释阶段决定 batch、并发、缓存和资源边界。
- 最强的依赖/错误设计来自 ZIO：`R`、`E`、`A` 三轴贯穿 effect，`ZLayer` 负责构造和供给服务；但它仍不是金融领域的 unknown-outcome 或 ledger 模型。
- 最强的业务流程设计来自 Baker 与 `gvolpe/trading`：Petri-net/事件或 command-state-event FSM 作为中心，事件和状态转换驱动消费，而不是共享一个跨服务可变对象。
- 基础类型案例（Scala 3 opaque、Iron、refined）解决的是名义隔离和边界验证，不自动解决 provider capability 或副作用关联。
- 规模事实与机制事实严格分开；本文没有把 library README 的宣传、示例测试或 GitHub stars 当作生产流量证据。

## 范围与筛选标准

- 本报告只读本次实际打开的一手材料：固定提交的公开仓库源码/README/docs、作者博客/论文/演讲页面、作者或维护者公开 issue/PR/release notes。
- 本仓库只读取了用户允许的域事实章节；没有读取任何 `src/`、`services/`、`packages/`、`ui/` 等源码，也没有把域文件中的设计细节当作依据。
- `[观察]` 表示原文或源码直接写出；`[推断]` 表示由已引文机制推出，不能扩大为作者承诺；`[未找到]` 表示本次打开的材料中没有该类型或证据。
- “生产使用”分三级：公司/作者明确说生产并给出规模；官方 adopters/README 自述但没有产品规模；仅有 examples、教学应用或本地部署。
- 六问中的“设计中心”按一个最小组合载体来找；若系统实际是成对中心（如 `ZIO` + `ZLayer`、`Stream` + `Pull/Resource`），明确记录分工，不强行删掉互补中心。
- 没有运行这些候选项目的 build、lint、test、benchmark，也没有启动 Pulsar、Kafka、Postgres、Redis 或 Akka 集群；验证是对固定快照逐段打开、交叉检查路径和行号。
- 本报告不提出 UTA 的字段、对象、状态机或 API；最后的迁移命题只使用“案例 X 在条件 Y 下用机制 Z 解决问题 W；若条件不成立则不可迁移”的证据条件句。

## 逐案例分析

### 案例 1：Gabriel Volpe 的 `gvolpe/trading`——command/state/event FSM 驱动的事件流

#### ①系统做什么、规模与生产使用证据

- [观察] README 将它定义为书中 `Functional event-driven architecture: Powered by Scala 3` 的 reference application，而不是宣称交易生产系统（[S1] `README.md:L1-L10`）。
- [观察] 系统拆成 domain、core、feed、processor、snapshots、alerts、WS server、tracing 等模块；README 明确列出约 9 个模块和各服务职责（[S1] `README.md:L200-L258`）。
- [观察] Feed 生成随机 `TradeCommand`/forecast；Processor 消费命令并生成状态和 `TradeEvent`；Snapshots 消费事件并保存 `TradeState`；Alerts 消费事件并发出 `Alert`；WS server 将 Alert 发给订阅者（[S1] `README.md:L232-L258`）。
- [观察] 运行入口是 Docker Compose、Pulsar、Redis、Prometheus/Grafana 及本地 feed；部署文档是 local minikube，示例多数单副本（[S1] `README.md:L178-L198`；`ops/deployment.md:L1-L3,L42-L82`）。
- [观察] CI 有镜像构建和固定 smoke：启动 Pulsar/Redis，发布有限 TradeCommands，检查 WS alerts（[S1] `.github/workflows/ci-images.yml:L41-L53`；`.github/workflows/ci-smokey.yml:L36-L52`）。
- [边界] 本次没有找到生产用户、真实订单量、吞吐、延迟、SLO 或 benchmark；README 的 “In production, one would configure topic compaction” 是未来配置说明，不是生产证据（[S1] `README.md:L306-L322`）。因此它是高价值教学/参考系统，不应被写成已验证的交易生产系统。

#### ②设计中心：`FSM` 与 command–state–event 代数

- [观察] 最小中心是 `case class FSM[F[_], S, I, O](run: (S, I) => F[(S, O)])`；`runS` 可丢弃输出，`FSM.id` 表达纯状态转换（[S1] `modules/lib/src/main/scala/trading/lib/FSM.scala:L6-L11`）。
- [观察] `TradeEngine.fsm` 的真实签名是 `FSM.id[TradeState, TradeCommand | SwitchCommand, (EventId, Timestamp) => TradeEvent | SwitchEvent]`（[S1] `modules/core/src/main/scala/trading/core/TradeEngine.scala:L14-L16`）。
- [观察] On 状态中 Create/Update/Delete 改变 `TradeState` 并返回 `CommandExecuted` 构造器；Off 状态返回 `CommandRejected`；Switch 命令产生 Started/Stopped/Ignored（[S1] `TradeEngine.scala:L17-L41`）。
- [观察] `eventsFsm` 接受 `TradeEvent | SwitchEvent`，把 `CommandExecuted` replay 回状态，Started/Stopped 改状态，Rejected/Ignored 不改状态（[S1] `TradeEngine.scala:L43-L61`）。
- [观察] `TradeCommand`、`TradeEvent`、`Alert` 是 Scala 3 `enum`，带 `Codec.AsObject`/`Show` 等派生；它们被 pattern-match、replay、路由和 correlation tracing，不能只当 wire DTO（[S1] `TradeCommand.scala:L11-L47`；`TradeEvent.scala:L12-L31`；`Alert.scala:L9-L31`）。
- [判断] 这里的“单一中心”应准确写作“typed command–state–event 协议族由 FSM 组合”，而不是单独声称 `TradeEvent` 是全部中心；传输边界和业务转换分层，且 `TradeEvent` 嵌入完整 `TradeCommand`，带来 envelope 与 command 的耦合。

#### ③多 provider / 异构能力

- [观察] `Producer[F,A]` 暴露 `send(a)`、带 properties、带 transaction 和两者同时存在的四个入口（[S1] `modules/lib/src/main/scala/trading/lib/Producer.scala:L14-L18`）。
- [观察] `Consumer[F,A]` 扩展 Acker，提供 `receiveM: Stream[F,Msg[A]]`、按 MsgId rewind、payload-only `receive` 和 `lastMsgId`；Acker 有单个、批量、事务 ack/nack（[S1] `Consumer.scala:L18-L22`；`Acker.scala:L3-L7`）。
- [观察] local provider 用 `Queue[F,Option[A]]`，关闭时发送 `None`；Pulsar provider 编码 JSON、支持 MsgId、ack/nack、dead-letter 和 transaction；Kafka provider 关闭 auto-commit 并批量 commit offset（[S1] `Producer.scala:L20-L77`；`Consumer.scala:L49-L132`）。
- [观察] 同一个编译接口并不保证相同语义：local 的 rewind/ack/nack 是 no-op；Kafka 的 `receiveM(id)` 忽略 id、`lastMsgId` 永远 `None`、transaction overload 落到普通 ack；Pulsar 才有真实 broker transaction/dead-letter（[S1] `Consumer.scala:L49-L132`；`Producer.scala:L20-L77`）。
- [观察] Shard/Compaction typeclass 将命令、事件、Alert 映射到 symbol/status/correlation 等 broker key；AppTopic 是 sealed topic 描述并区分持久/非持久 topic（[S1] `Shard.scala:L12-L48`；`Compaction.scala:L10-L47`；`AppTopic.scala:L5-L62`）。
- [判断] provider 能力主要是值级实现选择和统一 trait；Txn、rewind、metadata 没有被 capability evidence 精确标记，真实 broker 差异到运行期才显现。新增 transport 可实现 trait，但新增业务 operation 仍要增加相应 ADT、FSM 分支和 provider 映射。

#### ④副作用如何描述、组合、解释与消费

- [观察] Processor 的输入是 `Either[Consumer.Msg[TradeCommand], Consumer.Msg[SwitchCommand]]`，主 stream 用 `evalMapAccumulate(TradeState.empty)(fsm.run)`；每条消息在 Pulsar transaction 中运行纯 FSM、生成 event id/time、发送事件，再 ack 输入（[S1] `processor/Engine.scala:L13-L45`；`processor/Main.scala:L16-L27`）。
- [观察] Feed 通过 `Producer[IO,...]` 注入 effectful producer，生成 UUID/timestamp 后发送并 sleep；ForecastFeed 用三个 stream `parJoin(3)` 表达 Register→Publish→Vote 的并发模拟（[S1] `feed/Feed.scala:L17-L63`；`feed/ForecastFeed.scala:L21-L57`）。
- [观察] Alerts 将 TradeEvent、SwitchEvent、PriceUpdate 合成 nested Either/`union2`，再 `evalMapAccumulate`；CommandExecuted replay 后计算价格变化，发送 Alert 和可选 PriceUpdate，并在同一 transaction 中 ack（[S1] `alerts/Main.scala:L16-L42`；`alerts/Engine.scala:L30-L37,L48-L103`）。
- [观察] `app-id` property 防止 PriceUpdate 被自己再次消费；Snapshots 从 Redis 最新状态/last MsgId rewind，再 merge SwitchEvent 与 Tick；WS 为每 SocketId 消费 Alert 并按 symbol 过滤（[S1] `alerts/Engine.scala:L58-L63,L96-L101`；`snapshots/Main.scala:L20-L48`；`ws-server/Handler.scala:L15-L85`）。
- [观察] tracing 独立消费 command/event/alert，以 CorrelationId 将三种消息配对；关联载体是 payload correlation id，而不是隐含的共享对象（[S1] `tracing/Main.scala:L34-L82`；`tracing/fsm/trading.scala:L16-L65`）。
- [判断] 纯状态转换在 `FSM.id`，IO、clock、UUID、broker、ack、Redis、日志、WS frame 留在解释/消费边界；这是避免业务字段巨型对齐的实际做法，但它只提供 provider-local transaction/MsgId 关系，不自动提供跨 provider audit/replay。

#### ⑤基础类型

- [观察] `Timestamp` 是 opaque `Instant`；`Quantity` 是 `NumNewtype[Int]`；`Price` 是 `NumNewtype[BigDecimal]`；CommandId、AlertId、EventId、CorrelationId、SocketId、AuthorId 等是基于 UUID 的 `IdNewtype`（[S1] `domain.scala:L15-L76`）。
- [观察] `Newtype[A]` 要求底层 Eq/Order/Show/Circe Encoder/Decoder，并派生 Scala Ordering；`IdNewtype.unsafeFrom` 对坏 UUID 直接抛异常；`NumNewtype` 只提供底层 Numeric 加减（[S1] `Newtype.scala:L17-L53`）。
- [观察] `TradeState` 是 `status: TradingStatus, prices: Map[Symbol, Prices]`；Prices 内含 ask/bid maps/high/low，数量为 `Int`（[S1] `TradeState.scala:L13-L16,L66-L78`）。
- [观察] 时间 Eq/Order 只按 `getEpochSecond`，因此源码定义丢掉了 sub-second equality/order；消息 ordering 依赖 Pulsar ShardKey/KeyShared，Kafka provider 还固定 key（[S1] `OrphanInstances.scala:L11-L24`；`Shard.scala:L12-L19`）。
- [未找到] 没有 Money/Currency/Amount 或带货币的 quantity/rounding；`Price` 只是 BigDecimal 别名，不能推出多币种金额正确性。
- [观察] 拒绝原因是 `Reason = Newtype[String]`；运行失败是 Throwable/Logger，JSON decoder 是 Either；唯一明确的 Unknown 是 `ForecastTag.Unknown`，不是统一 unknown provider-result ADT（[S1] `domain.scala:L66-L67`；`ForecastTag.scala:L8-L13`；`processor/Engine.scala:L36-L38`）。

#### ⑥已知问题、后悔、性能与人因代价

- [观察] Feed 源码承认随机实现会产生大量 `AuthorNotFound`，并建议更真实的 ForecastFeed；README 还说 Tyrian yarn 路线不可保证复现，docker-compose 的 feed 因 sbt-native-packager 问题被注释（[S1] `feed/Feed.scala:L52-L53`；`README.md:L84-L101`；`docker-compose.yml:L258-L285`）。
- [观察] 作者 PR #179 把 Tyrian WebSocket 实现称为 “temporary replacement ... with a hacky bug-fix”（[S1-P179]）。
- [观察] PR #55 中作者写过 “Yes, this is ugly but I didn't know how to do this better”，并说自己是 Scala.js newbie，因 linking/compilation errors 困惑数小时（[S1-P55]）。
- [观察] PR #122 修复 Pulsar transactional support，PR #114 更新 deduplication fix；这些证明事务/dedup 是维护边界，但没有被写成完整 post-mortem（[S1-P122]；[S1-P114]）。
- [未找到] 没有该项目真实生产事故、GC/延迟/吞吐复盘；公开证据是教学部署和上述工具、人因、协议维护成本。

### 案例 2：47deg Fetch——以 `Fetch` AST 把独立数据访问延迟到 batching interpreter

#### ①系统做什么、规模与生产证据

- [观察] README 定义 Fetch 为 “A library for Simple & Efficient data access in Scala and Scala.js”，目标是把有延迟成本的数据库或 web service 查询组合起来（[S2] `README.md:L1-L7,L37-L45`）。
- [观察] 真实示例有 http4s Users/Posts、GitHub API、Doobie/H2；GitHub 示例带 Authorization，组合 OrgRepos、Languages、Contributors，并用 `mapN` 与 `.batchAllWith`（[S2] `fetch-examples/src/test/scala/Http4sExample.scala:L59-L163`；`GithubExample.scala:L105-L232`；`DoobieExample.scala:L98-L176`）。
- [边界] GitHub 示例被 `ignore`，其余也是 test fixtures；本次 README、docs、源码和 examples 没有生产部署量、吞吐、用户或 SLA，因此不冒充生产系统。

#### ②设计中心与实际定义

- [观察] provider identity 是 `Data[I,A]`：`name: String`，`identity: Int` 由 universal hash 得到（[S2] `datasource.scala:L29-L41`）。
- [观察] provider 合约是：

```scala
trait DataSource[F[_], I, A] {
  def data: Data[I, A]
  implicit def CF: Concurrent[F]
  def fetch(id: I): F[Option[A]]
  def batch(ids: NonEmptyList[I]): F[Map[I, A]] = // parallel fetch fallback
  def maxBatchSize: Option[Int] = None
  def batchExecution: BatchExecution = InParallel
}
```

- [观察] 组合中心是自定义 `sealed trait Fetch[F[_], A]`，内部持有 `F[FetchResult[F,A]]` 的 delayed run；有 `FetchOne`、`Batch`、`BlockedRequest`、`FetchResult.Done/Throw` 等节点和 `Monad[Fetch[F,*]]`，但固定快照中没有 `cats.Free` 或名为 `Free` 的类型（[S2] `fetch.scala:L27-L75,L258-L363`）。
- [判断] 准确称呼是“Fetch 自己的 free-like request AST/Monad”，不要把它误报成 Cats Free。
- [观察] 解释链是 `Fetch.apply` 建 RequestMap → 以 `Data.Identity` 聚合 → 同 id 合为 `FetchOne`、多 id 合为 `Batch` → `performRun` 处理 blocked round → provider batch/fetch → 缓存并恢复 continuation（[S2] `fetch.scala:L238-L255,L385-L461,L584-L654`）。

#### ③多 provider / 异构能力

- [观察] 同一 `Fetch` 可含多个 `DataSource`；不同 `Data` identity 不合并，但各 provider entry 在 fetch round 中并行（[S2] `fetch.scala:L238-L255,L611-L639`）。
- [观察] `F/I/A` 表达 effect、输入 identity、结果类型；Users 的 `UserId -> User` 与 Posts 的 `UserId -> List[Post]` 是不同 provider/result 形状（[S2] `datasource.scala:L43-L71`；`Http4sExample.scala:L59-L122`）。
- [观察] batch 不是强制的真实批接口：默认实现把单项 fetch 并行化；实现者可以覆写 batch、限制 `maxBatchSize`、选择 `Sequentially` 或 `InParallel`（[S2] `datasource.scala:L43-L71,L159-L161`；`fetch.scala:L657-L789`）。
- [判断] 新增 provider 是开放的：实现 DataSource 即可；新增 read/write/stream operation 没有独立 capability ADT，不能仅由 `Fetch[F,A]` 拒绝“不支持的写”。

#### ④副作用、组合、解释与消费

- [观察] `F[_]` 承载副作用，`DataSource` 要求 `Concurrent[F]`；`Fetch.liftF` 对 `F[A]` 做 attempt，失败封成 `UnhandledException`，成功变为 Done（[S2] `fetch.scala:L385-L461`）。
- [观察] `map2/product/flatMap` 保留依赖关系；独立分支在解释时按 DataSource 聚合，跨 provider 并行，flatMap 后续 round 才能看到前一结果（[S2] `fetch.scala:L260-L363,L584-L654`）。
- [观察] `InMemoryCache` key 是 `(Data[Any,Any], DataSourceId[Any])`；`run` 默认每次新 cache，`runCache`/`runAll` 才把 cache 跨 execution 带出（[S2] `cache.scala:L25-L79`；`fetch.scala:L463-L609`）。
- [观察] README 明确 shared cache 没有 automatic invalidation；使用者要自行知道何时重新读取（[S2] `README.md:L249-L307`）。
- [观察] `batchAcrossFetches` 是独立 Resource wrapper：队列收集 callback，在窗口内按 provider/group identity 去重并调用底层 batch（[S2] `datasource.scala:L90-L156`）。
- [观察] 日志有 `Round`、`Request(start,end,duration)`；这是解释阶段的可观察 timing，不是业务审计或 unknown write 结果（[S2] `log.scala:L21-L56`）。

#### ⑤基础类型

- [未找到] core/docs/examples 没有 Money、Quantity、Currency、Amount 或 BigDecimal 金融模型；I/A 是泛型，例子里的 domain case class 由调用者定义。
- [观察] 时间只用于 `FiniteDuration` batching window、`Clock[F]` 和日志 `Long` start/end；不是业务时间轴或有效期类型（[S2] `datasource.scala:L90-L156`；`log.scala:L31-L45`）。
- [观察] 业务 identity 是任意 `I`，而 `Data.Identity = Int` 是 provider descriptor 的 hash；两者不能混同（[S2] `datasource.scala:L29-L41`）。
- [观察] ordering 只有 `BatchExecution.Sequentially/InParallel`；源码没有全局业务 ordering contract。
- [观察] `FetchException` 是 sealed trait，下有 `MissingIdentity[I,A]` 与 `UnhandledException`；missing 可转 Option，外部异常仍是 Throwable（[S2] `fetch.scala:L51-L64,L657-L789`）。
- [未找到] 没有 `Unknown` ADT；也没有 write 的 indeterminate/unknown result。

#### ⑥已知问题、后悔、性能与人因代价

- [观察] issue #306 的讨论纠正 README：每次 run 都创建新 cache，用户容易误以为跨 run 会自动缓存（[S2-I306]）。
- [观察] issue #305 记录随机重复 key 被同一 run 的 DataCache dedup；用户询问如何并行但不按 ID 优化，说明 dedup 对非幂等/随机操作可能是意外语义（[S2-I305]）。
- [观察] PR #538 记录跨请求 batching 在 timeout 恰落于 queue pop/ref update 之间的 deadlock，修复使用 `F.uncancelable`（[S2-I538]）。
- [观察] CHANGELOG 把 caching 文档和 batch execution 作为修复主题（[S2] `CHANGELOG.md:L230-L237`）。
- [未找到] 没有正式 post-mortem、生产 benchmark 或作者 regret；不能把 README 的窗口示例当性能报告。

### 案例 3：Twitter Stitch——`Stitch[T]` + `Group` 的 RPC DAG、批处理与缓存

#### ①系统做什么、规模与生产证据

- [观察] Twitter 官方 README 将 Stitch 定义为 “Scala library for elegantly and efficiently composing RPC calls to services”，自动批同一 data source、并发多 source，并支持既有 service interfaces（[S3] `README.md:L1-L19`）。
- [观察] README 自述 “This project is used in production at X.”；官方 `WhyUseStitch` 更具体地说：用于 `>200` projects，从低流量到服务 `hundreds of millions of operations per second`，并有 many services 使用多年（[S3] `README.md:L13-L19`；`docs/WhyUseStitch.rst:L6-L37`）。这是维护者自述，不是本次独立压测。
- [观察] Jake Donham 作者页面和其 paper 说 Stitch built at Twitter、用于 tens of existing services；paper §5 单独描述的是 Strato 的 tens of teams/hundreds of microservices，不能把 Strato 规模转写为 Stitch 规模（[S4] 作者页；[S5] PDF p.8 §4、p.10 §5）。

#### ②设计中心与实际定义

- [观察] `sealed abstract class Stitch[+T]` 是 lazy computation；atomic RPC call 不立即执行，直到整个 computation blocked，以便 batching/rewrite；`map/flatMap`、`join/collect/traverse` 分别表达变换、依赖、并发/批处理（[S3] `Stitch.scala:L24-L121`）。
- [观察] provider 批处理中心是 `trait Group[C,T] { def runner(): Runner[C,T] }`；`SeqGroup[K,V]` 运行 `Seq[K] => Future[Seq[Try[V]]]`，`MapGroup` 运行 `Seq[K] => Future[K => Try[V]]`，并带 maxSize/maxConcurrency/bucket 形状（[S3] `Group.scala:L5-L113`）。
- [观察] `Stitch.call[C,T](call:C, group:Group[C,T]): Stitch[T]` 将一个原子 call 交给 Group；`Stitch.run` 返回 Twitter `Future[T]`（[S3] `Stitch.scala:L1594-L1622,L1765-L1784`）。
- [观察] 作者论文 §4 给出解释 loop：syntax tree → traverse atomic calls → deduplicate/batch → invoke services → wait RPC → substitute results → simplify → repeat to constant（[S5] PDF p.8 §4，抽取文本 L246-L256）。
- [判断] Stitch 的单一中心是 lazy `Stitch[T]`，`Group/Runner` 是 provider-specific interpreter；不要把 Group 当业务大对象。

#### ③多 provider / 异构能力

- [观察] `Group` 的 equality/hash 是 batching key；equal Group 共享 runner，不同 Group 分开；context 必须纳入 Group/call key，否则会错误合批（[S3] `Group.scala:L5-L23`；`docs/Groups.rst:L112-L140`）。
- [观察] Seq、Map、BucketedSeq、BucketedMap 表达不同 batch result shape、最大 batch size、最大并发和 bucket packing（[S3] `Group.scala:L25-L113`）。
- [观察] Service Adapter 隐藏 Group/`Stitch.call`，让业务方只调用 `adapter(arg): Stitch[result]`；无 batch API 的服务仍可用 Group 包装单项 Future（[S3] `docs/ServiceAdapters.rst:L10-L49`）。
- [判断] C/T 与 Seq/Map shape 在编译期；Group equality 和 runner 参数在一次 run 的解释构造期；RPC 失败、能力不匹配与完成时机在 runtime。没有 read/write/stream capability matrix，也没有 runtime capability handshake。

#### ④副作用、组合、解释与消费

- [观察] 创建 Stitch 不执行；必须 `Stitch.run`。Run 每轮 simplify 同步节点、收集 Group、`runner.run()` 发 Future batch，回调再 loop，直至 root constant（[S3] `Stitch.scala:L76-L94`；`Runner.scala:L78-L369`）。
- [观察] `flatMap/rescue` 是依赖边，会减少可见的批处理窗口；`join/collect/traverse` 让独立 RPC 同时可被 dedup/batch；官方 EfficientQueries 明说 false dependency 会增加 latency（[S3] `docs/EfficientQueries.rst:L6-L78`）。
- [观察] `runCached` 是单次 run local cache；`StitchCache` 是跨 run 的异步 in-process cache，并转移 passed Stitch 的 ownership，调用者必须运行 returned Stitch（[S3] `docs/StitchCache.rst:L6-L78`；`cache/StitchCache.scala:L6-L155`）。
- [观察] `ValueCache` 只缓存成功值，不 deduplicate in-flight requests；普通 StitchCache 可合并同 key in-flight request，失败需要 eviction（[S3] `cache/ValueCache.scala:L5-L38`；`StitchCache.scala:L93-L155`）。
- [观察] 同一 Stitch 重复 run 的行为是 undefined；`respond/onSuccess/onFailure` 等副作用只有被 run 的 computation 包含时才消费，异步 computation 可能在 run 返回后继续（[S3] `Stitch.scala:L164-L243,L1579-L1592,L1765-L1784`）。

#### ⑤基础类型

- [未找到] Stitch core/docs/source 没有 Money、Quantity、Currency、BigDecimal 或统一 Unknown 领域类型。
- [观察] `C` 是任意 call key（示例可为 Long user id）；provider identity 由 Group equality/hash 承载，不是独立的 ProviderId ADT（[S3] `Stitch.scala:L1594-L1622`；`Group.scala:L5-L23`）。
- [观察] time 是 `com.twitter.util.Duration`/Timer、`within`、`sleep` 和可返回 `(Try[T], Duration)` 的 helper；这是超时/计时基础设施，不是业务时间轴。
- [观察] SeqRunner/MapRunner 会在 batch 结果中重建输入关系，但 Advanced 文档明确没有“which keys will end up in which batches”的 ordering guarantee（[S3] `docs/Advanced.rst:L10-L47`）。
- [观察] 错误是 Twitter `Try`/Future，加上 `NotFound`、`Timeout`、`StitchInvalidState`、`StitchNonLocalReturnControl`；NotFound 可转 Option，未命中不是 Unknown（[S3] `Stitch.scala:L17-L21,L314-L328`；`docs/FailureHandling.rst:L6-L64`）。

#### ⑥已知问题、后悔、性能与人因代价

- [观察] `WhyUseStitch` 承认本地运行有额外性能损失，但 RPC batching 通常值得；Arrows 更快但更难用（[S3] `docs/WhyUseStitch.rst:L22-L37`）。
- [观察] 作者 paper §4.1 写出真实取舍：monadic/applicative 语法树造成 memory allocation/CPU cost；一个 Twitter service 为性能改 arrows，后来为 readability 改回 monadic interface（[S5] PDF p.8–9 §4.1，抽取文本 L260,L289-L293）。
- [观察] Running 文档警告 Stateful Stitch 不可随意 rerun；无锁重跑会出现 missing data 或 never executes，`synchronizedRef` 虽安全却带额外同步成本（[S3] `docs/Running.rst:L40-L109`）。
- [观察] StitchCache ownership、失败 eviction 和 returned Stitch 规则构成人因/维护成本（[S3] `docs/StitchCache.rst:L6-L22`；`StitchCache.scala:L130-L155`）。
- [未找到] 没有 Stitch 专门事故 post-mortem 或作者 regret；Strato paper 中用户对 DSL feature/docs/support 的 frustration 只归于 Strato，不能转写成 Stitch 代价。

### 案例 4：ZIO 2.x——`ZIO[R,E,A]`、`ZLayer` 与金融生产上下文

#### ①系统做什么、规模与生产证据

- [观察] ZIO 源码定义 `ZIO[R,E,A]` 是 immutable effect，描述 async/concurrent workflow；形式上可看作 `ZEnvironment[R] => Either[E,A]`，Runtime 才执行，fibers 支持高并发（[S6] `ZIO.scala:L33-L55`）。
- [观察] 官方 adopters 页面只说是“partial list of companies happily using ZIO in production to craft concurrent applications”，列 Bank of America、CurrencyCloud、Tinkoff、Tranzzo、Unit 等，但不提供产品、版本、团队或规模（[S7] `docs/adopters.md:L7-L9,L24-L25,L38-L39,L94-L97`）。
- [观察] T‑Bank 官方 Scala 页面提供了强金融上下文：T‑Business 法人结算/工资支付/账户/operations feed，最多约 15 million client operations/month；T‑Investments 管用户资产、资产操作和交易所信息，tens of thousands requests/second，并列出 ZIO、Cats、Kafka、K8S 及多类数据库；Scala 开发者超过 250 人（[S8] `/tmp/tbank-scala-page.txt:L45-L49,L57-L75,L121-L129,L163-L181`）。
- [边界] T‑Bank 页面没有把 ZIO 逐服务绑定到 T‑Business 或 T‑Investments；可说“公司生产 Scala stack 包含 ZIO 且公司公开了这些金融规模”，不能说这些系统全部由 ZIO 实现。

#### ②设计中心与实际定义

- [观察] 主要载体是 `sealed trait ZIO[-R,+E,+A]`；R 是所需环境，E 是 typed failure，A 是 success value（[S6] `ZIO.scala:L33-L55`）。
- [观察] 互补中心是 `sealed abstract class ZLayer[-RIn,+E,+ROut]`；它是“recipes for producing bundles of services”，构造可以 effectful/resourceful，默认共享（[S6] `ZLayer.scala:L27-L44`）。
- [观察] `provideEnvironment`/`provideLayer` 将环境供给 effect；`build` 产出带 Scope 的 environment，最后由 Runtime 解释（[S6] `ZIO.scala:L1271-L1283`；`ZLayer.scala:L124-L141`）。
- [判断] 不应把 ZIO 和 ZLayer 合并成一个职责：前者是 computation carrier，后者是 provider graph/constructor。

#### ③多 provider / 异构能力

- [观察] 服务通过 Tag/`ZEnvironment` 取出；`ZEnvironment.union/unionAll` 合并服务，右侧碰撞优先（[S6] `ZEnvironment.scala:L271-L294`）。
- [观察] Layer 的 `++`/`zipWithPar` 组合异构输入输出，结果环境是 `ROut1 with ROut2`，输入是 `RIn with RIn2`；`fromZIO` 把 effect 构造成 service（[S6] `ZLayer.scala:L57-L77,L400-L418,L858-L872`）。
- [判断] 新 provider 可作为 Layer 进入同一环境；但是 ZIO 类型没有自动证明外部 provider 语义等价、Read/Write/Stream capability 或 runtime handshake。

#### ④副作用、组合、解释与消费

- [观察] `provideLayer` 在 scope 中 `layer.build(scope).flatMap(r => self.provideEnvironment(r))`；`ZIO.service[A:Tag]`/`serviceWithZIO` 从环境取服务（[S6] `ZIO.scala:L1274-L1283,L4795-L4832,L5759-L5768`）。
- [观察] `acquireRelease` 注册 finalizer，acquire/release 默认不可中断；`ZIO.scoped` 在 scope 结束时关闭资源（[S6] `ZIO.scala:L2735-L2758,L4749-L4761`；`docs/reference/resource/scope.md:L72-L108`）。
- [观察] `fork` 的 fiber 默认绑定父 scope，`forkDaemon` 脱离父 scope；`zipPar/race` 失败时会关联中断另一侧，race 可能等待 loser 清理（[S6] `ZIO.scala:L759-L822,L1310-L1328`；`docs/reference/fiber/fiber.md:L15-L59`）。
- [观察] `Cause` 区分 `Fail[E]`、`Die(Throwable)`、`Interrupt(FiberId)`，并以 `Then`/`Both` 表达顺序/并行错误；typed E 不等于没有 defect 或 interrupt（[S6] `Cause.scala:L27-L41,L984-L1066`；`docs/reference/error-management/typed-errors-guarantees.md:L15-L50`）。
- [判断] ZIO 的解释闭环是 `ZLayer.build(scope)` → `provideEnvironment` → Runtime；它为服务依赖、资源和并发提供通用关系，但没有金融 write unknown、audit identity 或 replay policy。

#### ⑤基础类型

- [观察] 核心基础三轴是 `R/E/A`，另有 `ZEnvironment`、Tag、Scope 和 Cause；它们是 effect/runtime 基础设施，不是金额/数量领域模型（[S6] `ZIO.scala:L33-L55`；`ZLayer.scala:L27-L44`）。
- [未找到] ZIO core/docs 没有 Money、Currency、Quantity、业务 Time、业务 Id、业务 Ordering 或金融 Unknown/indeterminate outcome 定义。
- [观察] checked error 是 E，defect 是 Throwable，interrupt 携带 FiberId；这些错误变体可穷举消费，但外部 provider unknown 不会自动成为某一变体（[S6] `Cause.scala:L984-L1066`）。

#### ⑥已知问题、后悔、性能与人因代价

- [观察] ZIO docs 对裸 fiber 使用给出明显警告：优先 `zipPar`/`raceWith` 等高层组合；daemon 改变生命周期，误用会泄漏或脱离父 scope（[S6] `ZIO.scala:L764-L779`；`fiber.md:L47-L59`）。
- [观察] race 可能等待 loser clean termination；Layer 默认 sharing、环境 collision preference 和 Scope 都是需要理解的语义成本（[S6] `ZIO.scala:L1310-L1328`；`ZLayer.scala:L35-L42`；`ZEnvironment.scala:L278-L294`）。
- [作者原话] De Goes 在《A Brief History of ZIO》中说教学中 monad transformers “painful”，用户为 higher-kinded types 牺牲 type inference，并把 ZIO 从 “purely functional IO monad” 改成更易销售的 async/concurrent programming；作者原文已打开（[S9] `https://degoes.net/articles/zio-history`，`/tmp/degoes-zio-history.html:L181-L217`）。
- [作者原话] De Goes 的 ZIO Environment 文章把 tagless-final 的代价命名为 “Massive Ramp-Up”“Big Bang”“Tedious Repetition”“Completely Uninferrable”，并称 ReaderT 最高约 4x、ReaderT+EitherT 最高约 8x；这些是作者估计而非本次 benchmark（[S10] `/tmp/degoes-zio-environment.html:L323-L415,L619-L623,L765-L799`）。
- [未找到] 没有 ZIO 金融事故 post-mortem、金融 benchmark 或维护者对 T‑Bank 具体部署的 regret。

#### 设计比较：tagless-final、Free 与 ZIO environment 的一手原话

- [作者原话] De Goes 先给 Free 的定义：“A Free monad is basically just a way to stuff a sequential computation in a data structure, so you can inspect that data structure and ‘interpret’ it later”；并说 `Free f a` 是由 operational algebra 描述的 program（[S11] `/tmp/degoes-modern-fp.html:L204-L239`）。
- [作者原话] 同一作者的 tagless-final 示例是 `trait Console[F[_]]` 与 `def program[F[_]: Console: Monad]: F[String]`；他明确列出类型类参数的 ramp-up、big-bang refactor、重复 context bounds 与 inference 限制（[S10] `/tmp/degoes-zio-environment.html:L258-L343,L357-L415`）。
- [作者原话] 他随后给出 `ZIO[R,E,A]`，称它只用额外环境参数和 `provide/accessM` 就可达到比 tagless-final 更低的学习成本，并主张 fully inferable、modular、incremental（[S10] `L623-L683,L765-L812,L897-L911`）。
- [作者原话] Fabio Labella 的公开 gist 给出 `trait KVS[A]`、`Free[Instr[_],A]`、`foldMap[G](translator: F ~> G): G[A]`，并强调 “A Free monad is always translated into another Monad; the other Monad then does what's needed to produce an A”；他还说副作用要等 Task/IO 明确 run（[S12] `/tmp/fabio-free-deb56/Free conversation.md:L59-L79,L105-L145,L217-L250,L314-L320`）。
- [判断] 这三者不是同一层竞争：Free 的中心是可检查的 instruction program；tagless-final 的中心是按 `F` 参数化的 capability interface；ZIO 的中心是带 environment/error/result 的 effect carrier。作者原话支持“选择取舍”，不支持把任一者宣传成金融正确性方案。

### 案例 4A：Adrian Filip 的 money application / `MultiLaneSequencer`

#### ①系统、规模与生产证据

- [观察] 作者博客说曾开发处理 loans、deposits、monthly payments、reports 的 money application，并担心同时付款、deposit liquidation 和 potential double-spend（[S13] `https://adrianfilip.com/2020/04/07/moving-from-kotlin-spring-reactor-arrow-to-scala-zio/`，开头及 “money application” 段）。
- [观察] 作者没有给公司名、部署环境、RPS 或客户数；“several orders of magnitude more concurrency than expected real world usage”是测试叙述，不是生产 benchmark（[S13] 同文 Testing/Conclusions 段）。

#### ②设计中心

- [观察] `MultiLaneSequencer` 的核心签名是 `sequence[R,E,A](lanes: Set[Lane], programId: UUID, program: ZIO[R,E,A]): ZIO[R,E,A]`（[S14] `MultiLaneSequencer.scala:L24-L26`）。
- [观察] 实现中心是 ZIO STM `TMap[Lane,List[UUID]]`；`occupyLanes` 追加 id，`waitUntilFree` 检查每 lane head/`STM.retry`，`bracket_` 后 release（[S14] `L34-L81`）。

#### ③异构能力

- [观察] 任意 `ZIO[R,E,A]` 可被排入任意 Lane；博客用全局 CLIENTS lane 或具体 Client lane 区分全局串行与不同 client 并行（[S13] concurrency/lanes 段；[S14] `L24-L81`）。
- [未找到] 没有多个 provider/interpreter、capability token 或 provider matrix；不能从 `R,E,A` 推断金融 provider 语义。

#### ④副作用组合与消费

- [观察] lane 占用和释放是 STM transaction，资源释放由 `bracket_` 绑定 program 生命周期；Recorder 以同一个 `programId` 记录 requests/responses（[S14] `L42-L81`）。
- [观察] 作者把 randomness 生成的 effect id 改为调用方传入 UUID；ZIO Runtime 消费最终 effect，但仓库没有 ledger commit、持久审计或 replay interpreter（[S13] Testing/Conclusions；[S14] `L24-L81`）。

#### ⑤基础类型

- [观察] 真实显式值是 `Set[Lane]`、`Lane`、`UUID programId`、`List[UUID]` ordering queue；money 只出现在博客场景，而非类型定义（[S14] `L24-L81`）。
- [未找到] 没有 Money/Currency/Quantity/Amount/Time、unknown provider result 或金融 error ADT；UUID 只承担 program identity。

#### ⑥代价

- [作者原话] 作者说旧 Kotlin/Reactor/Arrow 方案有 “tricky logic”、bug 后果严重且难找；Scala+ZIO STM 方案 “easier to test, easier to compose, and easier to understand”（[S13] Kotlin/Scala comparison 段）。
- [边界] 这是作者的主观可维护性比较和测试经验，不是独立生产事故 post-mortem；也不能把 lane sequencing 自动推广成资金正确性。

### 案例 4B：Decathlon Digital POSLog——ZIO + Spark 的生产规模邻近案例

#### ①系统、规模与生产证据

- [观察] Decathlon 作者文章说 POSLog 覆盖 1700+ stores、70 countries、70+ websites/marketplaces，单条 transaction 有约 340 fields；Bronze→Silver 验证用 Databricks/Delta（[S15] `https://medium.com/decathlondigital/why-combine-asynchronous-and-distributed-calculations-to-tackle-the-biggest-data-quality-challenges-2e04dfc51401`，POSLog/validation 段）。
- [观察] ZIO World 2023 schedule 将它描述为 deployed scalable Data Validation solution，处理 millions of XML transaction records；这是零售数据工程，不是银行/交易所/支付生产系统（[S16] `https://www.zioworld.com/`，Retail Transactions Validation at Scale 项目描述）。

#### ②设计中心

- [观察] ZIO 是 `R,E,A` carrier；领域中心是 permissive `Option[String]`-heavy untyped input 转 typed `RetailTransaction`，`Validation.validateWith` 递归验证并返回 `Validation[Error,ValidObject]`（[S15] 文章 “What is ZIO?”、“POSLog transactions validation”、“ZIO Prelude” 段）。

#### ③异构能力

- [观察] Spark/Hadoop/Kubernetes 承载分布式计算，ZIO 同时启动两个 Spark DAG；validators 通过 nested `validateWith` 组合，不是 provider capability registry（[S15] distributed calculations/validation 段）。
- [边界] 没有公开 provider identity、Read/Write/Stream capability 或 runtime handshake；不能把 Spark 与 ZIO 当可互换金融 provider。

#### ④副作用组合与消费

- [观察] Spark actions 用 `ZIO.attempt`，两个 DAG 用 fiber fork/join；数据库连接可用重试 schedule；错误累积到 `ValidationError`，包含 fieldLog、transactionId、cause，并计划写 DynamoDB 供监控告警（[S15] Spark/ZIO/ValidationError 段）。
- [观察] 这是“坏数据与有效数据分离”的消费闭环；文章没有 request→provider→audit identity、replay 或 idempotency 设计。

#### ⑤基础类型

- [观察] untyped model 使用 `Option[String]`；typed model 有 `Double amount/percent`、`Int sequenceNumber`、字符串和 `transactionId`；没有领域金额/货币封装（[S15] data model/validation 段）。
- [未找到] 没有 Money/Currency/Quantity/业务时间/ordering/unknown provider outcome。

#### ⑥代价

- [观察] 作者强调 immutability 避免 locks/deadlocks，ZIO 并行提升 Spark 资源利用；没有公开该部署的吞吐/延迟成本、事故复盘或 regret（[S15] concurrency/resource 段）。
- [边界] 只能迁移“验证错误累积和执行层分离”的命题，不能迁移金融 correctness 结论。

### 案例 5：Typelevel FS2/Cats Effect 与 Kafka、HTTP、Postgres provider

#### ①系统做什么、规模与生产证据

- [观察] Cats Effect README 定义为 high-performance、asynchronous、composable framework，提供 typed/resource-safe concurrency；官方 concepts 说明 JVM/JS foundational runtime，fiber 约 150 bytes、可创建 tens of millions 是容量性说明（[S17] Cats Effect `README.md:L8-L10`；`docs/concepts.md:L18-L30`）。
- [观察] FS2 README 定义为 purely functional、effectful、polymorphic stream-processing library，目标是 compositionality、resource safety、speed（[S18] `README.md:L8-L17`）。
- [未找到] 本次没有找到金融生产部署、吞吐、SLA 或真实资金系统一手证据；fs2-kafka、http4s、Skunk 的 docs/examples 也不等于金融生产。

#### ②设计中心

- [观察] FS2 公共载体是 `final class Stream[+F[_],+O]`，底层为 `Pull[F,O,Unit]`；文档称 Stream 是 effectful computation 的 description，consumer pull 才逐步控制 effects（[S18] `Stream.scala:L46-L66,L166-L178`）。
- [观察] `sealed abstract class Pull[+F[_],+O,+R]` 是纯不可变 process description；R 是正常终止结果，Pull 可 fail/cancel/never（[S18] `Pull.scala:L36-L44,L111-L122`）。
- [观察] Cats Effect 的互补生命周期载体是 `sealed abstract class Resource[F,+A]`，描述 action+finalizer；`use` 在 success、failure、cancellation 后释放，内部 ADT 有 Allocate/Bind/Pure/Eval（[S19] `Resource.scala:L32-L45,L153-L208,L1166-L1181`）。
- [判断] 不能把 Pull 或 Resource 假称为 Stream 的别名：Stream 是组合输出载体，Pull 是步进/解释结构，Resource 是 acquire/use/release 语义。

#### ③provider / 异构能力

- [观察] Stream 有 `translate[F2,G](F2 ~> G)`，Resource 有 `mapK[G](F ~> G)`；provider 可在 FunctionK/Resource 边界换 effect（[S18] `Stream.scala:L3037-L3046`；[S19] `Resource.scala:L404-L414`）。
- [观察] `Stream.resource(Resource[F,O])` 把资源纳入 stream scope；`fromPublisher` 提供第三方 publisher 接缝，chunkSize 控制 request/memory（[S18] `Stream.scala:L3918-L3966,L4123-L4155`）。
- [观察] fs2-kafka 将 `KafkaConsumer` 与 `KafkaProducer` 分离：Consumer 输出 `Stream[F,CommittableConsumerRecord]`；Producer 的核心是 `produce(records): F[F[ProducerResult]]`（[S20] `KafkaConsumer.scala:L46-L90,L522-L610`；`KafkaProducer.scala:L34-L51`）。
- [观察] Skunk 将 `Query[A,B]` 与 `Command[A]` 分开，由 Encoder/Decoder 连接 SQL 与 Scala type；Session/PreparedQuery/PreparedCommand 通过 Resource/Stream 消费（[S21] `Query.scala:L39-L46`；`Command.scala:L35-L40`；`Session.scala:L65-L117,L158-L174`）。
- [观察] http4s 的 `EntityBody[F] = Stream[F,Byte]`，client `run(req): Resource[F,Response[F]]`；backend 只需实现 Resource/Stream 边界（[S22] `package.scala:L25-L37`；`Client.scala:L36-L53`）。
- [判断] P/Q provider 的静态差异由 Kafka Consumer/Producer、Skunk Query/Command 体现；FS2 本身没有 Read/Write capability tag，也没有跨 Kafka/HTTP/DB 的业务 coproduct。

#### ④副作用、组合、解释与消费

- [观察] Pull 以 immutable Chunk 输出，chunk-level 可避免 boxing；错误/中断发生在 chunk 之间；pull-based 编译器决定 effect 何时执行（[S18] `Pull.scala:L46-L84`）。
- [观察] `compile.drain` 只构造目标 effect，返回前 stream 尚未执行；真正副作用由 Cats Effect Runtime 执行；`compile.resource` 把 stream lifetime 继续暴露成 Resource（[S18] `Stream.scala:L5226-L5241,L5349-L5437`）。
- [观察] Kafka consumer 是 `Resource` 建 actor/fiber/queues，再由 `Stream.resource` 暴露；ConsumerRecord 携带 topic/partition/offset，CommittableOffset 的 commit 与消费值关联（[S20] `KafkaConsumer.scala:L458-L528`；`CommittableConsumerRecord.scala:L19-L49`；`CommittableOffset.scala:L18-L43`）。
- [观察] Kafka producer 的两层 F 分离 buffer 与等待 metadata；transaction Resource 在成功 commit、error/cancel abort，嵌套 transaction 会 deadlock（[S20] `KafkaProducer.scala:L34-L51,L69-L90`）。
- [观察] http4s `Client.run` 的 Resource callback 结束后 connection release，之后读 body 会 error；`streaming` 将 Resource response 转 Stream 后消费 body（[S22] `Client.scala:L47-L53,L78-L101`）。
- [观察] Skunk cursor 是 Resource-backed stream，按 chunk execute/fetch；Query 文档实际链是 prepare → stream → evalMap → compile.drain（[S21] `PreparedQuery.scala:L85-L100`；docs Query `L154-L181`）。
- [判断] 共同闭环是 provider Resource/Stream → Pull/Chunk → compile → target effect Runtime；Kafka offsets/metadata、Skunk Completion 是 provider-local result/association，不是统一 audit/replay record。

#### ⑤基础类型

- [未找到] FS2/Cats Effect/http4s/Skunk 核心没有 Money、Currency、Quantity 或统一业务 ID 模型；F/O/A、Byte、A/B 是抽象参数。
- [观察] 时间是 `FiniteDuration`/Temporal/timeout；ordering 主要是 stream/chunk 顺序、Kafka topic-partition offset 顺序或 Skunk chunk fairness，不是业务 event-order ADT（[S18] `Pull.scala:L384-L388`；[S20] `KafkaConsumer.scala:L46-L64`；[S21] `PreparedQuery.scala:L29-L36`）。
- [观察] error 方面 Pull 区分 failure/cancellation/never，Resource 有 `ExitCase` Succeeded/Errored/Canceled；http4s 有 EntityStreamException/DecodeResult；Kafka 有 deserialization/commit errors；Skunk 有 session invalidation/EofException（[S18] `Pull.scala:L36-L44`；[S19] `Resource.scala:L1183-L1210`；[S22] `Message.scala:L255-L295`）。
- [未找到] 没有统一 Unknown/indeterminate provider result；Replay/幂等只能由 Kafka/应用层另行定义。

#### ⑥已知问题、后悔、性能与人因代价

- [观察] FS2 文档警告 concurrent combinators 通常不传播 open scopes，network socket stream 误用可能泄漏；`streamNoScope` 明确可能造成 leak（[S18] `Stream.scala:L95-L102`；`Pull.scala:L302-L323`）。
- [观察] Cats Effect 承认最简单 async FFI 不能在 callback 注册处执行 F effects，也不能注册 cancellation finalizer；`timeoutAndForget` 明确违反 backpressure 并故意泄漏 fibers（[S19] `Async.scala:L32-L63`；`IO.scala:L865-L883`）。
- [观察] fs2-kafka docs 说 at-least-once 下并发、过滤、批处理和 out-of-order commit “can be challenging”，逐条 commit 有 substantial performance implication；transactional producer 共享会成为 bottleneck，嵌套 transaction deadlock（[S20] consumer docs `L281-L321,L351-L352`；transactions docs `L41-L45`）。
- [观察] Skunk 作者承认 Resource lifetime-managed object 要 discipline 避免 leaks，decoder 与 query 分离会变成 maintenance issue；chunk size 在 fairness 与 throughput 间取舍（[S21] package docs `L20-L22`；Query docs `L100-L107`；`PreparedQuery.scala:L29-L58`）。
- [未找到] 没有 FS2/Cats Effect/fs2-kafka/http4s/Skunk 针对这些机制的正式 post-mortem 或作者 regret。

### 案例 6：Akka Graph/Actor 与 ING Baker——Petri-net 编排的金融邻近系统

#### ①系统做什么、规模与生产使用证据

- [观察] Akka README 宣称下载超过 1 billion、服务 thousands of systems，支持 millions concurrent users、terabyte stream processing、low-latency read/write 和 99.9999% availability；这是官方项目/商业宣传，不能替代单一客户的审计证据（[S23] `README.md:L1-L14`）。
- [观察] Akka `Source` 是一个有一个 open output 的 stream processing steps 集合，materialization 转成 Reactive Streams Publisher；`Graph` 的 shape 是外部可见 inlets/outlets（[S24] `Source.scala:L29-L40`；`Graph.scala:L18-L35`）。
- [观察] ING Baker 的演讲以 ING Bank、全球 40+ 国家、12 systems、27 steps、2 minutes–6 hours 的业务流程为背景；它明确展示 checking/savings account、customer onboarding 等金融流程（[S25] `ScaleByTheBay-18-Nov-2017.md:L4-L25,L129-L136`）。
- [边界] Baker 的公开 repo 是 ING 的开源库与演示文档；上述演讲给出 ING 的场景规模和架构背景，但没有给 Baker 的生产 TPS/SLA。

#### ②设计中心：Recipe → Compiler → Petri net → RecipeInstance

- [观察] Baker 明确把 business process specification 与 runtime implementation 分开：`Type→Value`、`Ingredient→IngredientInstance`、`Event→EventInstance`、`Interaction→InteractionInstance`、`Recipe→RecipeInstance`（[S26] `docs/sections/reference/main-abstractions.md:L1-L15`）。
- [观察] `Recipe` 的 Scala 定义是 interactions、sensory/checkpoint events、subRecipes、sieves、failure strategy、receive/retention duration 的 immutable case class（[S26] `baker-recipe-dsl/.../scaladsl/Recipe.scala:L18-L47`）。
- [观察] `InteractionDescriptor` 最小合约是 name、input ingredients、output events、required events、predefined values、renames、maximum count、failure strategy、reprovider（[S26] `InteractionDescriptor.scala:L6-L82`）。
- [观察] RecipeCompiler 将 recipe 编译成 Petri-net representation，保存 marking、transitions、validation errors，并以 SHA-256 截断 hash 计算 recipe id（[S26] `execution-semantics.md:L26-L31`；`CompiledRecipe.scala:L22-L65`）。
- [判断] Baker 的中心不是“所有业务字段的 state object”，而是可视化、可验证、可持久化的 process graph；Ingredients 是流程数据，Interactions 是 effectful boundary，Events 是异步边界。

#### ③多 provider / 异构能力

- [观察] Interaction 可以 query microservice、发 event broker message、await message、生成文件或做 transformation；实现以 Scala/Java/Kotlin DSL 注入 runtime（[S26] `main-abstractions.md:L202-L225`）。
- [观察] Baker 自有 `Type` 是 sealed ADT：primitive（Bool、Int16/32/64、BigInt、Float、Date、ByteArray、CharArray）、List、Option、Enum、Record、Map；Record/Map 用字段/值集合表达跨节点可序列化 schema（[S26] `Type.scala:L3-L48,L91-L127`；docs types `L35-L64`）。
- [观察] custom type adapter 可通过 reference.conf 注册；默认适配 Java primitive/enum/List/Map/BigDecimal/POJO 与 Scala case class/List/Map/Option/BigDecimal（[S26] `baker-types-and-values.md:L76-L115`）。
- [观察] 类型是 runtime recipe-time schema，不是 Scala compile-time capability proof；`Value.isInstanceOf/validate` 在运行时检查，unknown event 或 invalid output 由 `validateInteractionOutput` 报错（[S26] `Value.scala:L9-L62`；`RecipeRuntime.scala:L49-L86`）。
- [判断] 新 interaction/provider 可开放加入，但运行时 adapter/type registry 是值级/构造期发现；它没有 P(Read+Write)/Q(Read+Stream) 的静态矩阵。

#### ④副作用、组合、解释与消费

- [观察] 执行循环是 EventInstance 提供 ingredients → 匹配所有 required ingredients/events → 执行 InteractionInstance → 产生 EventInstance → 重复直到没有 interaction（[S26] `execution-semantics.md:L3-L19`）。
- [观察] compiler 用 Petri net 处理 ingredient 被多个 interaction 使用、AND/OR precondition、firing limit；同一个 ingredient 通过 token duplication 进入多个 transition（[S26] `execution-semantics.md:L26-L65`）。
- [观察] ingredients immutable，重复提供时 latest value overrides；internal event 与 sensory event 底层相同，只是触发来源不同（[S26] `concepts.md:L6-L30`）。
- [观察] `RecipeRuntime.eventSourceFn` 把 EventInstance 的 providedIngredients 加入 state，并追加 EventMoment；interaction task 调 InteractionManager、验证 output、发 internal event 与 runtime event log（[S26] `RecipeRuntime.scala:L24-L32,L193-L248`）。
- [观察] ING 演讲写明 Cassandra persistent storage、ingredients encrypted by default、state recovered automatically；技术失败 exponential backoff，且“Works well with idempotent services”（[S25] `ScaleByTheBay-18-Nov-2017.md:L99-L115`）。
- [观察] event listeners 不是 primary business logic：不 graceful shutdown 时可能 miss events；用于 logging/tracing 的 listener 也应接受此边界（[S26] `docs/archive/event-listener.md:L3-L20`）。
- [观察] Baker/ Akka 的 stream 侧 `GraphStage` 以 Shape + `createLogic` + materialized value 解释 graph；Actor `Behavior[T]` 收消息并返回 next Behavior；`ActorRef.tell` 是 at-most-once（[S24] `GraphStage.scala:L31-L58,L103-L115`；`Behavior.scala:L21-L43,L72-L114`；`ActorRef.scala:L24-L32`）。

#### ⑤基础类型

- [观察] Baker `Type.Date` 定义为 UTC ISO-8601、millisecond precision，底层技术上等同 Int64；Type 另有 arbitrary precision `IntBig`/`FloatBig`、Enum/Record/Map（[S26] `Type.scala:L159-L196`；docs types `L35-L64`）。
- [观察] `Value` 有 NullValue、PrimitiveValue、RecordValue、ListValue；NullValue 对应 null/void/none/empty，RecordValue 是 String→Value map（[S26] `Value.scala:L154-L219`）。
- [观察] 业务 process id 是用户选择的 String；recipe id 是 hash；event/ingredient identity 主要是 name 字符串；没有统一 money quantity type（[S26] `dictionary.md:L8-L12`；`baker-types-and-values.md:L27-L33`）。
- [未找到] Baker 本次源码/docs 没有 Money/Currency/Amount/Quantity；ING 示例有 IBAN、account、customer 等名字，但没有金额精度、舍入、币种或未知回执定义。
- [观察] 失败分为 technical exception（抛异常、可 retry）与 functional failure（返回事件）；invalid output、unknown event、missing field 有字符串 validation error，未形成统一 Unknown result（[S26] `concepts.md:L32-L66`；`RecipeRuntime.scala:L54-L86`）。

#### ⑥已知问题、后悔、性能与人因代价

- [观察] Baker 开源文档 `awaitCompleted` 修复记录公开了一个 race：interaction output EventTransition 异步执行但不在 `instance.jobs` 中，completion check 提前返回，导致 `reservedItems` 缺失（[S26] `docs/awaitCompleted-race-condition-fix.md:L19-L59`）。
- [观察] 该文档给出的验证是 buggy branch stress test 约 80% failure（4/5 runs），fix branch 20 iterations 0 failure；文档状态为 Ready for review，不应冒充已合并 release 的 SLA（[S26] `awaitCompleted-race-condition-fix.md:L107-L171`）。
- [观察] 修复用 in-memory `inFlightEventTransitions` counter；作者明确列出 non-persistent、actor restart 时丢失，依赖 event sourcing replay 恢复，生产改动约 44 行（[S26] `awaitCompleted-race-condition-fix.md:L72-L103,L175-L189,L243-L275`）。
- [观察] Baker migration 文档承认 stateful cluster binary-incompatible change 需要 downtime，rolling deploy 未测试且不推荐（[S26] `docs/archive/migration-guide.md:L7-L21`）。
- [观察] Akka `Source.preMaterialize` 会引入 buffer，upstream error 变 cancellation 且丢 error details；ActorRef at-most-once 也不能被误写成 reliable delivery（[S24] `Source.scala:L94-L103`；`ActorRef.scala:L18-L32`）。
- [判断] Baker 的真实代价是异步 completion、持久事件 sourcing、schema/type adapter、集群升级和 idempotent interaction discipline；这是比“一个 process state object”更重但可审计的边界。

#### Akka/Pekko 近邻：PayPal squbs 的平台化分层

- [观察] squbs 原则把 business logic 与 lifecycle/infrastructure 分开，使用消息传递和 loosely coupled cubes，尽量不让 application 直接知道 admin/health/shutdown 初始化（[S27] `docs/principles_of_the_squbs_design.md:L1-L24,L26-L56`）。
- [观察] pipeline 是 HTTP 与 service/client 之间的 BidiFlow，统一承载 logging、metrics、tracing、auth 等基础设施；`RequestContext` 包装 request/response 与 context（[S27] `docs/pipeline.md:L1-L17,L51-L62,L90-L118`）。
- [观察] PerpetualStream 针对 Kafka/JMS consumer 和 HTTP 多 stream consolidation，启动时运行、停止时 graceful，不丢 message；materialized value 以 Future 或 `(KillSwitch,Future)` 作为消费边界（[S27] `docs/perpetualstream.md:L1-L7,L25-L87`）。
- [未找到] 当前 squbs 文档没有可复核的 PayPal 金融吞吐、事故 post-mortem 或具体业务领域类型；且当前仓库已迁移到 Pekko，不能把平台文档写成 Akka 生产指标。

### 案例 7：Scala 3 opaque/enum/union/match 与 Iron/refined——基础类型而非 provider 系统

#### 7A Scala 3 语言构造的六问

- ① [观察] opaque、enum、union、match types 是 compiler/type-system feature；官方例子是 Logarithm、permissions、Color/Option、`UserName | Password`、`Elem[X]`，没有生产系统或部署规模（[S28] `opaques.md:L7-L113`；`enums.md:L7-L47`；`union-types.md:L7-L31`；`match-types.md:L7-L59`）。
- ② [观察] opaque 的精确定义是 `opaque type T >: L <: U = R`；作用域内透明等于 R，外部抽象；`Logarithm` 用 `Double`，由 `apply/safe` 和 extension `toDouble/+/*` 构造/消费（[S28] `opaques-details.md:L21-L33`；`opaques.md:L7-L60`）。
- ③ [观察] union `A | B` 表示 unrelated alternatives，enum/ADTs 表示 sealed closed cases，match type 是 `S match { case P => T }` 的 compile-time reduction；这些表达值/类型异构，不表达 provider capability、provider identity 或 runtime handshake（[S28] `union-types-spec.md:L32-L62,L176-L229`；`match-types.md:L61-L152`）。
- ④ [观察] 语言构造没有 effect/interpreter；`safe` 的 Option 只是 smart-constructor result，enum pattern match 是值级操作，match type reduction 是 compiler 行为；不能从 `ordinal` 推导业务 ordering/audit/replay。
- ⑤ [观察] 具体底层值是 Double/Int/String/Option/Either/ordinal；SIP 虽把 units-of-measure、Id/Password 列为动机，但本次打开的官方材料没有 Money/Currency/Quantity 可用定义、舍入、time/id/order/Unknown domain model（[S29] opaque SIP `L14-L18,L28-L52,L134-L173`）。
- ⑥ [观察] SIP 目标原话是 wrapper operations “must not create any extra overhead at runtime while still providing a type safe use at compile time”；同时承认 generic collections、arrays、function parameters、equals/hashCode 会触发 boxing，仍需要 benchmark 排除 extension method performance issue（[S29] `L28-L33,L134-L173,L896-L901,L955-L999`）。enum/match pages 未找到作者 post-mortem；match recursion 可报 `Recursion limit exceeded`（[S28] `match-types.md:L192-L222`）。

#### 7B Iron 的六问

- ① [观察] Iron 是 Scala 3 refined-type library，目标是 data validation、business data types、mathematics；README 列 adopters（Ledger、Lichess、gvolpe/trading 等）但没有规模或生产交易职责，故只作基础类型案例（[S30] `README.md:L7-L18,L91-L138`；`docs/_docs/overview.md:L7-L69`）。
- ② [观察] 中心是 `opaque type IronType[A,C] <: A = A`、别名 `:|[A,C]`，加 `Constraint[A,C]` 的 inline `test/message`；runtime polymorphism 由 `RuntimeConstraint[A,C]` 提供（[S30] `package.scala:L20-L45,L49-L101`；`Constraint.scala:L6-L33`；`RuntimeConstraint.scala:L5-L24`）。
- ③ [观察] C 可用 union/intersection 组合，Cats/Circe/Doobie/ZIO 是独立 integration modules；这是 predicate/integration provider 轴，不是 P/Q operation capability 或 provider registry（[S30] `docs/_docs/reference/constraint.md:L14-L35`；`modules/index.md:L7-L29`）。
- ④ [观察] compile-time literal 用 inline/macro；未知 runtime value 用 `refineUnsafe`、`refineEither`、`refineOption`；Cats `Validated`/EitherNec 与 ZIO Validation 消费验证错误（[S30] `RefinedType.scala:L15-L71`；`package.scala:L49-L101`；modules Cats/ZIO docs）。这些是 validation carriers，不是 effect interpreter/audit/replay。
- ⑤ [观察] Positive、Interval、Multiple 等 numeric constraints 覆盖 Int/Long/Float/Double/BigDecimal/BigInt；全树精确搜索没有 Money/Quantity/Currency/Amount/UnitOfMeasure，Iron main/src 也没有专用 time/ID/order model（[S30] `constraint/numeric.scala:L13-L177`；全树 negative search）。
- ⑥ [维护者原话] issue #174 解释 Constraint 会在每个 callsite inline，而 RuntimeConstraint 可减少 code size 并让 JVM 优化；PR #175 讨论指出 Either/Option 会 box primitive，继续 inline 可能增加 compile time/generated code，并评估 slight conversion inefficiency；issue #281 说复杂表达式的详细 compile errors “quickly becomes unusable”（[S30-I174]；[S30-P175]；[S30-I281]）。迁移文档还记录 compiler bug、Scala 3.6.3 要求和 2.x→3.x opaque 变化（[S30] `migration3-0.md:L5-L17,L39-L60`）。

#### 7C refined 的六问

- ① [观察] refined v0.11.4 是 type-level predicate/refinement library；README 有 Positive/Greater/Regex/URL/Interval 和项目列表，但没有生产规模或金融系统证据（[S31] `README.md:L8-L79,L281-L313`）。同一固定 tag 的 build.sbt 明列 Scala 3.3.8；README 仍只写 2.12/2.13，是文档陈旧矛盾（[S31] `build.sbt:L14-L16,L55-L60,L82`）。
- ② [观察] Scala 3 carrier 是 `infix opaque type Refined[T,P] = T`；`RefType[F]` 可替换 carrier；`Validate[T,P]` 定义 predicate semantics；runtime boundary 是 `refineV[P]`（[S31] `Refined.scala:L1-L13`；`RefType.scala:L15-L51`；`Validate.scala:L7-L28`；`package.scala:L8-L15`）。
- ③ [观察] Cats derivation 从 base encoder/decoder 与 Validate 派生 refined carrier；外部 integrations 可扩展，但 provider identity/Read/Write/Stream capability 不在 Refined 类型中（[S31] `cats/derivation.scala:L10-L35`；README integrations `L220-L279`）。
- ④ [观察] runtime `refineV` 通过 Either/unsafe exception 消费 Validate；Validated 可积累错误；这些是 decoder/error carriers，非副作用 interpreter，也无 write-result/audit/replay semantics（[S31] `RefinePartiallyApplied.scala:L5-L21`；Cats derivation `L22-L35`）。
- ⑤ [观察] 有 `PosInt` 等 numeric aliases；time 近邻是 `Month/Day/Hour/Minute/Second/Millis` 的 Int interval aliases；whole clone 没有 Money/Quantity/Currency/Amount/UnitOfMeasure，亦无专用 ID/ordering/Unknown（[S31] `types/numeric.scala:L1-L52`；`types/time.scala:L6-L40`；全树 negative search）。
- ⑥ [维护者/发布记录] release notes 给出 compile-time cost：0.8.4 避免 eval 后常见 predicates 约降 26%、组合后 up to 67%，refineV 约快 25%；0.8.6 说 `PosInt(1)` 曾比 `1: PosInt` 慢一个数量级后被优化；PR #149 称 reference types “zero runtime overhead ... only cause boxing for value types”，macro pitfalls 则要求 custom predicate/Validate 分离 compilation unit，否则 ClassNotFoundException（[S31] `notes/0.8.4.markdown:L1-L13`；`notes/0.8.6.markdown:L31-L37`；[S31-P149]；`modules/docs/macro_pitfalls.md:L8-L55`）。

## 横向对比与统一 litmus

| 案例 | 设计中心 | provider/能力表达 | 解释/消费边界 | 基础类型与 unknown | 生产/代价边界 |
|---|---|---|---|---|---|
| gvolpe/trading | `FSM[F,S,I,O]` + command/state/event | Producer/Consumer trait；Pulsar/Kafka/local 值级差异 | fs2 stream + FSM + transaction send/ack | opaque ids/time/price/quantity；无 Money；少数 Unknown | 教学/本地；事务、dedup、Scala.js 人因成本 |
| Fetch | `Fetch[F,A]` + RequestMap/Batch | `DataSource[F,I,A]`；batch 可覆写 | `performRun`/round/cache → F | 泛型 I/A；Missing/Throwable；无 Unknown | 无生产规模；cache、deadlock、dedup caveat |
| Stitch | `Stitch[T]` + equal `Group` runner | Seq/Map/Bucketed Group，runtime grouping | simplify → runner Future → repeat | Try/NotFound/Timeout；无 Money/Unknown | >200 projects 自述；allocation vs readability |
| ZIO | `ZIO[R,E,A]` + `ZLayer` | Tag/Environment union；Layer `++/zipWithPar` | scope build → provide → Runtime | typed E/Cause；无金融 unknown | T‑Bank stack/规模未逐服务绑定；fiber/layer成本 |
| FS2 stack | `Stream[F,O]` + `Pull`/`Resource` | FunctionK、Resource；Kafka Consumer/Producer、HTTP、SQL | pull Chunk → compile → Runtime | protocol metadata/Throwable；无统一 Unknown | 高性能/资源安全目标；scope/backpressure难点 |
| Baker | Recipe/Type/Value → compiled Petri net | interaction/adapter/runtime Type，开放值级 registry | Event/Ingredient token → Interaction → Event | Date/primitive/Record/Value；functional vs technical failure | ING 12 systems/27 steps背景；await race、升级 downtime |
| Akka/squbs | Graph Shape/Materialized value、Behavior | Shape、ActorRef message type、BidiFlow | materializer/actor mailbox/kill switch | Future/Done/Throwable；ActorRef at-most-once | 官方大规模宣传；preMaterialize/生命周期 caveat |
| Scala3/Iron/refined | opaque/ADT/predicate carrier | closed enum、union、constraint/integration | compiler 或 boundary validation | scalar predicates；无 money/provider unknown | compile/code-size/errors/macro cost |

### 统一 capability litmus（只总结材料，不提出 UTA 设计）

- **Q.Write 如何拒绝？** Fetch/Stitch/ZIO/FS2 Stream 本身不能从 `F/A/T/O` 静态拒绝一个 provider 的 Write；fs2-kafka 的 Consumer/Producer、Skunk 的 Query/Command 在 API 层确实分开，但 broker privilege、HTTP method/status、DB server capability 仍到 runtime。Baker 的 Interaction output contract 也不是 Read/Write capability proof。
- **两个独立 Read 能否批处理/并行？** Fetch 以 Data identity 合并并按 batch execution 解释；Stitch 以 Group equality 合 batch；FS2/kafka 以 Chunk、poll、partition streams 组合；Skunk 以 cursor/chunk；ZIO 只提供并行 effect/layer，不自动发现批处理机会。
- **Write 与结果/审计如何关联？** gvolpe/trading 以 CorrelationId、Pulsar transaction、MsgId 关联；Kafka 以 CommittableOffset/RecordMetadata/transaction local 关联；Baker 以 EventInstance/recipe process state 关联；Fetch/Stitch/FS2/HTTP/Skunk 没有统一业务 audit record。
- **Replay 是否避免再次执行 Write？** 本次打开材料没有任何 Fetch、Stitch、ZIO、FS2、Akka 或 Baker 通用 API 给出“replay 不再次执行 write”的保证；Stitch 重跑甚至可能 undefined，Fetch 每次 run 默认新 cache。任何更强结论都是超出证据的推断。
- **能力在哪个阶段确定？** Scala opaque/enum/union/match 与 Iron/refined literal 主要 compile-time；Fetch/Stitch Group、Baker TypeAdapter、ZLayer/Resource、Kafka/HTTP/Skunk resource 多在 interpreter/构造期；网络 broker、server、decode、quota、commit 和 unknown outcome 仍 runtime。
- **开放世界哪一轴开放？** Fetch 的新增 DataSource、Stitch 的新增 Group、ZIO 的新增 Layer、FS2 的新增 Resource/Stream provider 相对开放；新增 operation 通常要改各自 ADT/Group/DSL 或 provider adapter。sealed enum/union/match 是封闭 case 世界，不等于开放 provider registry。

## 对 UTA 的可迁移命题

1. **案例 Stitch：** 在条件“许多独立 RPC、同一后端有 batch API、调用方可以接受一次 run 的聚合窗口”下，Stitch 用 `Stitch[T]` + equal `Group` + simplify/runner loop 解决重复请求、批处理和跨后端并行；若条件不成立（依赖链、非幂等调用或需要固定批次顺序），该机制不可直接迁移，必须保留其明确限制。
2. **案例 Fetch：** 在条件“数据源能给出稳定 `Data` identity，单项读取可安全 dedup，批接口可能有 size/顺序策略”下，Fetch 用 `DataSource[F,I,A]`、RequestMap、cache 和 `Sequentially/InParallel` 解决跨 Fetch 的批处理；若读取非幂等、cache 需 TTL/invalidation 或 identity hash 不能代表业务身份，则不可迁移其默认 dedup/cache 语义。
3. **案例 ZIO：** 在条件“依赖服务可被命名/tag，构造与使用要分离，测试要增量替换”下，ZIO 用 `R` + `ZLayer` + `provide` 解决依赖传播与解释边界；若所需能力只能运行时握手、provider 语义不等价或要表达 unknown external write，则不可把 `R`/Layer 当作 capability evidence 或审计结果。
4. **案例 Fabio/De Goes Free：** 在条件“需要检查、变换、组合一组有限的 orthogonal instructions，并能提供自然变换 interpreter”下，Free 的 `Free[F,A]`/`F ~> G` 解决 program 与 handler 分离；若目标 operation 依赖运行期值并要求 aggressive batching，必须另有 applicative/优化解释器，不能直接照搬 sequential Free。
5. **案例 FS2/Cats Effect：** 在条件“消费者驱动需求、资源生命周期和 backpressure 是一等语义”下，`Stream[F,O]` + `Pull` + `Resource` 解决 provider stream 到 Runtime 的消费；若业务写结果不可判定或需要跨 stream replay/audit，FS2 的 compile/resource 语义不可充当该业务保证。
6. **案例 fs2-kafka：** 在条件“partition ordering、批量 offset commit 和 producer metadata 是 provider-local 约束”下，CommittableRecord/Offset、Chunk 与 transaction Resource 解决 Kafka 消费—提交关联；若必须跨 provider 获得统一 exactly-once/unknown 结论，则不可迁移，因为文档只给 Kafka-local semantics。
7. **案例 gvolpe/trading：** 在条件“业务转换可写成纯 `(state,input) -> (state,output)`，事件是跨进程边界，IO 只在边缘”下，`FSM[F,S,I,O]` + command/event ADT 解决 replay、状态转移和 effectful delivery 的分层；若事件 envelope 嵌入完整 command 或时间/金额模型不完整，则不可把该参考实现当金融领域模型。
8. **案例 Baker：** 在条件“流程由 interaction 输入、event 输出、ingredient 依赖和可持久化过程状态组成，且 interaction 可按幂等语义重试”下，Recipe→Petri net→RecipeInstance 解决可视化、验证、长期流程和事件 sourcing；若 latest-value override、异步 completion 或不幂等外部写不允许，则不可迁移其默认 retry/ingredient 语义。
9. **案例 Akka/squbs：** 在条件“消息 at-most-once 或 stream materialization 生命周期符合业务容忍度，基础设施应与业务解耦”下，Graph Shape/Materialized value、BidiFlow 和 PerpetualStream 解决组合、backpressure、shutdown 与横切基础设施；若 delivery 必须可靠或必须保留 error details，则不可使用 ActorRef/`preMaterialize` 的默认语义。
10. **案例 Scala 3 opaque/Iron/refined：** 在条件“约束是标量、边界可验证、底层表示复用且 provider 不由该类型负责”下，opaque + predicate dictionary + `Either/Validated` 解决名义隔离和输入验证；若需求是 Money/Currency/Quantity、外部 unknown write、provider capability 或 replay，则不可把 refinement 当领域 effect algebra。
11. **案例 De Goes 的反思：** 在条件“团队需要渐进测试、能接受具体 effect carrier、避免为每个方法重复高阶 typeclass bounds”下，ZIO environment 的局部 Layer 机制可减少 tagless-final 的 ramp-up/big-bang 成本；若目标是 library-level effect polymorphism 或需要严格 algebra laws，则不可把作者偏好写成普适替代定理。
12. **负面命题：** 任何案例都没有证明“一个统一大 object 能解决所有 provider/业务对齐”；恰恰是 Fetch/Stitch 用 provider key、Baker 用 tokens、trading 用 correlation/event、ZIO 用 environment、FS2 用 scope、Akka 用 shape 将关系分散到可组合载体。若条件缺少这些关系载体，不能从字段相似性推导设计中心。

## 未覆盖与开放问题

- 本次没有找到一手公开的 Scala 金融交易系统源码，能同时展示多 venue Read/Write capability、unknown write outcome、audit/replay identity 和增量观察消费；T‑Bank 页面给了生产规模但没有 per-service ZIO code。
- 本次没有找到 Fetch/Stitch/ZIO/FS2/Skunk 的正式金融 post-mortem；能打开的代价证据主要是维护者 caveat、issue/PR、作者性能取舍和 Baker 的 race-fix 文档。
- `gvolpe/trading` 的 Kafka/local provider overload 明显弱化 transaction/rewind 语义；没有运行 broker，故不能把静态派发推断为真实 exactly-once 行为。
- Baker 的 `awaitCompleted` 文档标记 Ready for review；其 stress 数字是该文档给出的验证记录，不等同于合并 release 的生产 SLA。
- ZIO official adopters 是 partial list，T‑Bank 公司页面不按产品绑定 ZIO；Bank of America/CurrencyCloud/Tinkoff 等条目不能扩写成“全公司所有服务都使用 ZIO”。
- Scala 3 opaque SIP 的 zero-overhead 是设计目标，Iron/refined 的 compile-time 数字来自各自维护者材料；本次没有在本机跑 benchmark，不能把它们写成统一 JVM 性能定律。
- 未覆盖的近邻案例包括 Scala Quantlib/券商内部 Scala 交易系统、effect-ts/TypeScript 运行时、更多 OCaml/F# 金融 DSL；本组任务已优先覆盖可打开且有具体源码链的 Scala/JVM 案例。
- 开放问题：哪些 provider-local metadata 应被视为业务 identity，哪些只可作为 transport hint；哪些 effect 可以重放，哪些 write 需要显式 non-replay semantics；这些都需要各系统额外的一手领域资料，本报告不补猜。

## 来源清单（固定快照、打开状态与本地路径）

1. **gvolpe/trading**；URL `https://github.com/gvolpe/trading/tree/e38afe019eda8c97185371de228002047a5ae890`；已打开固定 commit；本地 `/tmp/gvolpe-trading`；关键 `README.md`、domain/core/feed/processor/alerts/snapshots/ws/tracing 源码。
2. **gvolpe/trading PR #179**；URL `https://github.com/gvolpe/trading/pull/179`；已打开 PR 正文；本地缓存 `issue/pr`；用于 “hacky bug-fix” 原话。
3. **gvolpe/trading PR #55**；URL `https://github.com/gvolpe/trading/pull/55` 及 discussion/issuecomment anchors；已打开；本地缓存；用于 Scala.js linking/人因原话。
4. **gvolpe/trading PR #122/#114**；URL `https://github.com/gvolpe/trading/pull/122`、`/pull/114`；已打开；本地缓存；用于 transaction/dedup 修复主题。
5. **gvolpe/scalar-feda**；URL `https://github.com/gvolpe/scalar-feda/tree/de07909580fb3a7bf507fed649b442b8bf1075e0`；已打开固定 commit；本地 `/tmp/scalar-feda`；用于 PriceEvent/State/FSM 教学对照。
6. **47deg Fetch**；URL `https://github.com/47degrees/fetch/tree/290c0a4e991553e98c4a7359247195feaed03162`；已打开固定 commit；本地 `/tmp/47degrees-fetch`；关键 `datasource.scala`、`fetch.scala`、`cache.scala`、examples。
7. **Fetch issue #305**；URL `https://github.com/47degrees/fetch/issues/305#issuecomment-760391265`；已打开 issue/comment；本地缓存；用于 dedup/random source 代价。
8. **Fetch issue #306**；URL `https://github.com/47degrees/fetch/issues/306#issuecomment-760400904`；已打开 issue/comment；本地缓存；用于 cache 文档人因代价。
9. **Fetch PR #538**；URL `https://github.com/xebia-functional/fetch/pull/538#issuecomment-926032541`；已打开 issue/PR material；本地缓存；用于 timeout/deadlock 修复。
10. **Twitter Stitch**；URL `https://github.com/twitter/stitch/tree/2e32bf50221d4ba5ac0afe962f3196a421c19223`；已打开固定 commit；本地 `/tmp/twitter-stitch`；关键 Stitch/Group/Runner/cache/docs。
11. **Stitch paper / Strato**；URL `https://doi.org/10.1145/3241653.3241654`，作者链接 `https://drive.google.com/file/d/1aYupExDuAbUheDX4aycrxZDrc6stTcig/view`；已打开 PDF；本地 `/tmp/strato-paper.pdf`；引用 §4 p.8–9 与 §5 p.10。
12. **Jake Donham/Twitter University Stitch video**；URL `https://www.youtube.com/watch?v=VVpmMfT8aYw`；已打开页面 metadata；本地 `/tmp/VVpmMfT8aYw.metadata.txt`；无可用字幕，未伪造 transcript。
13. **John A. De Goes, A Brief History of ZIO**；URL `https://degoes.net/articles/zio-history`；已抓取并实际打开；本地 `/tmp/degoes-zio-history.html`；用于作者设计动机/痛点。
14. **John A. De Goes, Beautiful, Simple, Testable Functional Effects for Scala**；URL `https://degoes.net/articles/zio-environment`；已抓取并实际打开；本地 `/tmp/degoes-zio-environment.html`；用于 tagless-final/ZIO Environment 原话。
15. **John A. De Goes, A Modern Architecture for FP**；URL `https://degoes.net/articles/modern-fp`；已抓取并实际打开；本地 `/tmp/degoes-modern-fp.html`；用于 Free/orthogonal algebra/interpreter 原话。
16. **Fabio Labella/SystemFw Free conversation gist**；URL `https://gist.github.com/SystemFw/deb56c93e37af6a1fb1b48f878256b6b`；已打开固定 gist commit `6a4d4471a5979c0d22285292099e7ae5624c6615`；本地 `/tmp/fabio-free-deb56`；用于 `Free`/`foldMap`/natural transformation 原话。
17. **ZIO 2.x**；URL `https://github.com/zio/zio/tree/43d439febd7a595ef6bd0a5ce538ef607267c6ee`；已打开固定 commit；本地 `/tmp/zio-research-43d`（另有 `/tmp/scala-zio`）；关键 ZIO/ZLayer/ZEnvironment/Cause/docs。
18. **ZIO adopters**；URL `https://github.com/zio/zio/blob/43d439febd7a595ef6bd0a5ce538ef607267c6ee/docs/adopters.md`；已打开固定源码；本地 `/tmp/zio-research-43d/docs/adopters.md`；partial list，弱生产证据。
19. **T‑Bank Scala developers official page**；URL `https://www.tbank.ru/career/it/scala/`；已抓取并实际打开；本地 `/tmp/tbank-scala-page.txt`；金融规模和 ZIO stack；未作 per-service ZIO 绑定。
20. **Tinkoff functional effects article**；URL `https://medium.com/its-tinkoff/aspects-of-functional-effects-execution-in-scala-runtimes-89cf415c7153`；已打开文章章节；网页抓取路径未保留为 clone；用于 Operations Feed/ZIO runtime 语境；未把它扩写成完整业务审计设计。
21. **Adrian Filip money application**；URL `https://adrianfilip.com/2020/04/07/moving-from-kotlin-spring-reactor-arrow-to-scala-zio/`；已打开作者博客；网页抓取；用于 money app 语境和作者主观比较。
22. **Adrian Filip MultiLaneSequencer**；URL `https://github.com/adrianfilip/multilane/tree/eb827b2980cb51223a335e0e733154cfc066b074`；已打开固定 commit；本地 `/tmp/multilane-research-eb827`；用于 STM lane queue 源码。
23. **Decathlon Digital POSLog article**；URL `https://medium.com/decathlondigital/why-combine-asynchronous-and-distributed-calculations-to-tackle-the-biggest-data-quality-challenges-2e04dfc51401`；已打开作者/公司文章；网页抓取；用于 retail 邻近规模，非金融证据。
24. **ZIO World 2023 schedule**；URL `https://www.zioworld.com/`；已打开页面 schedule；网页抓取；用于 Decathlon talk 描述，未把无关视频当证据。
25. **Cats Effect**；URL `https://github.com/typelevel/cats-effect/tree/f219988a0c8821105da8a29e30d731bab39e2ba4`；已打开固定 commit；本地 `/tmp/evidence-cats-effect` 或 `/tmp/scala-cats-effect`；Resource/Async/IO docs。
26. **FS2**；URL `https://github.com/typelevel/fs2/tree/8aa47aba38454789ce206ad282ed30d45dcbae26`；已打开固定 commit；本地 `/tmp/evidence-fs2` 或 `/tmp/scala-fs2`；Stream/Pull docs/source。
27. **fs2-kafka**；URL `https://github.com/typelevel/fs2-kafka/tree/abebcaa3a84324df275984625d68070ff9a4ca8d`；已打开固定 commit；本地 `/tmp/evidence-fs2-kafka` 或 `/tmp/scala-fs2-kafka`；Consumer/Producer/commit/transaction source。
28. **http4s 0.23**；URL `https://github.com/http4s/http4s/tree/ff4e64f4a88da729e909912928d67b4ca63affe7`；已打开固定 commit；本地 `/tmp/evidence-http4s` 或 `/tmp/scala-http4s`；EntityBody/Client Resource source。
29. **Skunk 2.0**；URL `https://github.com/typelevel/skunk/tree/1045d3b2317ed21d7554c92676cc77edb3d1cd18`；已打开固定 commit；本地 `/tmp/evidence-skunk` 或 `/tmp/scala-skunk`；Session/Query/Command/cursor source。
30. **Akka core**；URL `https://github.com/akka/akka-core/tree/e0f2eff299576fbfc06052e5c6a28b51ef160aee`；已打开固定 commit；本地 `/tmp/akka-core-evidence`；Graph/Source/GraphStage/Behavior/ActorRef source。
31. **ING Baker**；URL `https://github.com/ing-bank/baker/tree/9c732d7ec48b5d04da21da64ea9194c59b13e502`；已打开固定 commit；本地 `/tmp/ing-baker-evidence`（另有 `/tmp/ing-baker`）；Recipe/Type/Value/Petri/runtime/docs。
32. **ING Baker race-fix document**；URL `https://github.com/ing-bank/baker/blob/9c732d7ec48b5d04da21da64ea9194c59b13e502/docs/awaitCompleted-race-condition-fix.md`；已打开仓库文档；本地 `/tmp/ing-baker-evidence/docs/awaitCompleted-race-condition-fix.md`；状态 Ready for review，按此限制解释。
33. **PayPal squbs/Pekko-neighbor**；URL `https://github.com/paypal/squbs/tree/86d6a7bf2e92ecc6f825f496a393c1db1e1ff293`；已打开固定 commit；本地 `/tmp/paypal-squbs-015-evidence`；pipeline、PerpetualStream、design principles；当前仓库迁移 Pekko，未作 Akka 生产指标。
34. **Scala 3 compiler/reference**；URL `https://github.com/scala/scala3/tree/2e0fea38441dd29dd887a712c49f055159f0d8a4`；已打开固定 commit；本地 `/tmp/scala3-uta-20260916`；opaque/enum/union/match reference。
35. **Scala opaque-types SIP**；URL `https://github.com/scala/docs.scala-lang/tree/f9b365a886739d2f7e56216e92f261fc1cf55b1f/_sips/sips/035-opaque-types.md`；已打开固定 docs commit；本地 `/tmp/scala-docs-uta-20260916`；目标、boxing、benchmark caveat。
36. **Scala SIP minutes**；URL `https://github.com/scala/docs.scala-lang/blob/f9b365a886739d2f7e56216e92f261fc1cf55b1f/_sips/minutes/2018-09-24-sip-minutes.md`；已打开固定 docs commit；本地 `/tmp/scala-docs-uta-20260916`；union types 讨论。
37. **Iron**；URL `https://github.com/Iltotore/iron/tree/b8bdda9f2f4cc3a77acfb5171614bf78644c56e2`；已打开固定 commit；本地 `/tmp/iron-uta-20260916`；opaque refined type、constraints、integrations、issues/PRs。
38. **refined v0.11.4**；URL `https://github.com/fthomas/refined/tree/f7b891288a021958310480636ef3905ed749cce9`；已打开 tag commit；本地 `/tmp/refined-v0114-uta-20260916`；Refined/Validate/time aliases/release notes。
