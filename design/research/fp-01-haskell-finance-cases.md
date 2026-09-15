# FP 调查 01：Haskell（及近邻）金融/交易/多 provider 生产系统的设计中心

> 本报告只做**案例调查与横向对比**，不做 UTA 设计。所有论断来自本次实际打开的一手来源（论文本地副本、公开仓库 clone、官方文档）；打不开的来源已在正文与来源清单中显式降级。类型签名、术语、引用保留原文。

## 摘要（≤10 行）

本组案例中最常见的设计中心是**一个小的、封闭或半封闭的核心代数（ADT / GADT / 组合子库），它把"要发生什么"表示为一个可被多种解释器消费的数据值，而不是一个把字段列全的大 object**——`Contract`（Composing Contracts / Marlowe）、`GenHaxl`+`DataSource`（Haxl）、`Work_` DAG（Standard Chartered Mu）、payout 深嵌 DSL（Barclays FPF）、`Update` monad（DAML）、`STS` 转移系统（cardano-ledger）无一例外都是"先定义一个能组合的核心值，再写多个解释器（pricing / lifecycle / 调度 / 授权 / 账本落地）去消费它"。它们避免"业务对齐的大对象"的共同手法有三条：①**深嵌入（deep embedding）**——领域构造被具化成 AST/值，业务语义留给解释器，核心类型只有个位数构造子；②**provider/后端异构不进核心类型**，而是外置成 type class 实例（`DataSource u req`、`STS`）、能力字典或独立解释器，核心代数对后端是"开放世界"；③**执行与描述分层**——一个纯的"描述"层（Contract/Work/Update/BlockedFetch）先构造出来、可静态分析、可序列化，再由一个"解释/调度"层在 IO 边界消费。副作用不是散落在业务对象的方法里，而是被收敛成"核心值 → 解释器"这一条边。下面逐案例给出真实类型签名与出处。

## 范围与筛选标准

- **纳入**：有一手证据（论文/官方文档/公开源码）能同时给出①生产采用规模②核心类型签名的案例。Haskell 为主，含 Haskell 派生语言（Mu、DAML）与 Cardano 生态（Marlowe/ledger 用 Haskell 写成）。
- **证据分三档**：①生产采用证据（规模/年限/业务线，不是"某公司用了 Haskell"）②设计/类型证据（真实签名+出处）③后悔/局限证据（**必须作者原话**；无则标注"未找到一手后悔记录"）。
- **不强行归一**：同一生态里若存在两个平行核心（Marlowe 的 `Contract` 语义 vs cardano-ledger 的 `STS` 账本转移；Composing Contracts 论文模型 vs LexiFi 商业产品），分开陈述，不硬凑成一个"设计中心"。
- **反例标注**：若来源本身是"把字段列成巨型 object"的设计，明确指出。
- **本仓库源码一律未读**；域理解仅来自 `uta-design.md` §1 与附录 B，其中的设计细节不作为依据。

---

## 案例 1：Facebook / Meta **Haxl**（多 provider 副作用组合的核心案例）

> 源码：`git clone --depth 1 https://github.com/facebook/Haxl`，commit `b33c1c1f132fa1d704cc0f7b6233f9c86bb50afe`（本次 clone 于 /tmp/haxl，已打开）。论文：Marlow, Brandy, Coens, Purdy, *There is no Fork: an Abstraction for Efficient, Concurrent, and Concise Data Access*, ICFP 2014（abstract 页已打开，PDF 仅凭搜索摘要）。

### ① 做什么、规模与生产使用证据

Haxl 是 Meta 的 Haskell EDSL，用于**从多个异构外部数据源并发、批量、去重地取数**。动机（ICFP 2014 abstract，`simonmar.github.io/bib/nofork-2014_abstract.html`，已打开）："motivated by a Facebook service whose business logic queried several external sources to identify content and take appropriate actions"——即反垃圾/滥用检测（Sigma 规则引擎）在线上大规模运行。它把"看起来是顺序的业务逻辑"自动变成并发+批处理+缓存的执行。这是本组里与 UTA"多 provider + 统一内部协议 + effect 消费"形状最接近的一个。

### ② 设计中心：`GenHaxl` monad + `DataSource` type class（两个类型合成一个中心）

核心不是一个 object，而是**一对**：一个描述计算的 monad `GenHaxl`，和一个把请求抽象成"可被批量调度的值"的类 `DataSource`。

```haskell
-- Haxl/Core/Monad.hs:441-442
newtype GenHaxl u w a = GenHaxl { unHaxl :: Env u w -> IO (Result u w a) }

-- Haxl/Core/Monad.hs:694-705  —— 一次求值的三态结果
data Result u w a
  = Done a
  | Throw SomeException
  | forall b . Blocked
      {-# UNPACK #-} !(IVar u w b)   -- 阻塞在哪个待填变量上
      (Cont u w a)                   -- 继续（morally b -> GenHaxl u w a）
```

`GenHaxl` 求值一步只会得到三种结果之一：`Done`、`Throw`、或 `Blocked ivar cont`（"我卡在某个尚未取回的数据上，这是我的续延"）。整套并发就是围绕 `Blocked` 展开的：调度器把所有当前 `Blocked` 的分支收集起来，一起发出去取数，再用回填的结果唤醒续延。

### ③ 多 provider / 异构能力怎么表达：type class 实例 + `TypeRep` 键的动态映射（开放世界）

provider 异构**完全在类型级用 type class 表达，核心 monad 对它一无所知**：

```haskell
-- Haxl/Core/DataSource.hs:94-107
class (DataSourceName req, StateKey req, ShowP req) => DataSource u req where
  fetch :: State req -> Flags -> u -> PerformFetch req
  schedulerHint :: u -> SchedulerHint req          -- :109
  classifyFailure :: u -> req a -> SomeException -> FailureClassification  -- :115

-- Haxl/Core/DataSource.hs:154-167  —— 一个数据源可以用四种方式取数
data PerformFetch req
  = SyncFetch  ([BlockedFetch req] -> IO ())
  | AsyncFetch ([BlockedFetch req] -> IO () -> IO ())
  | BackgroundFetch ([BlockedFetch req] -> IO ())   -- "最好的一种，并发度最高"
```

每个 provider 定义自己的 GADT 请求类型 `req :: Type -> Type`（`req a` 表示"这个请求返回 `a`"）并实现 `DataSource`。运行时把不同 provider 的待办请求存进一个**动态类型的、按请求类型 `TypeRep` 分桶**的映射：

```haskell
-- Haxl/Core/RequestStore.hs:54  —— 核心：按 TypeRep 分桶，开放世界，运行时才知道用哪些源
newtype RequestStore u = RequestStore (Map TypeRep (BlockedFetches u))

-- Haxl/Core/RequestStore.hs:62-64
data BlockedFetches u =
  forall r. (DataSource u r) => BlockedFetches [BlockedFetch r] [BlockedFetchInternal]
```

存在量化 `forall r. DataSource u r` 把"任意实现了 `DataSource` 的 provider"装进同一个容器——**新增一个 venue = 新增一个 type class 实例，核心调度代码零改动**。这正是"provider 异构不进核心类型"的教科书写法。

### ④ 副作用怎么描述/组合/解释/消费：描述层 `BlockedFetch`/`ResultVar` + 调度层 `runHaxl`（两层）

副作用被切成"未执行的请求描述"和"填回结果的回调"两半：

```haskell
-- Haxl/Core/DataSource.hs:187  —— 请求 + 结果槽，存在量化抹掉返回类型差异，可放进同一 list
data BlockedFetch r = forall a. BlockedFetch (r a) (ResultVar a)

-- Haxl/Core/DataSource.hs:194-198  —— 结果槽就是一个"把 Either 异常或结果塞回去"的回调
newtype ResultVar a =
  ResultVar (Either SomeException a -> Bool -> Maybe DataSourceStats -> IO ())
```

**谁关联谁**：`BlockedFetch (r a) (ResultVar a)` 把"请求"和"它的结果该填到哪个 `IVar`"绑成一对，类型系统保证 `r a` 的结果类型与 `ResultVar a` 一致（DataSource.hs:170-186 的注释专门解释这点）。**解释分两层**：
1. **描述/收集层**：`GenHaxl` 求值遇到 `dataFetch` 就产生 `Blocked`，把 `BlockedFetch` 塞进 `RequestStore`（`Fetch.hs:272-276`：`TryToBatch` 入库、`SubmitImmediately` 立即发）。
2. **调度/执行层**：`runHaxl`（`Run.hs:55-58`）跑一个循环 `schedule`（`Run.hs:72-117`），一轮里把所有 `Blocked` 攒够后调 `performFetches`（`Fetch.hs:445-494`），把同一 provider 的请求交给它的 `PerformFetch`，按 sync/async/background 三类分别驱动（`Fetch.hs:739-749`：`sync_fetches`/`async_fetches`/`fully_async_fetches`）。结果通过 `putResult`（`DataSource.hs:211-212`）填回 `ResultVar`，唤醒续延。

**批处理是数据源实现的性质，不是每个调用点的性质**（ICFP abstract 原文："batching is a property of the data-source implementation, not of each caller"）。并发由 `Applicative <*>` 暴露：`<*>` 的两侧独立、都可 `Blocked`，于是被同一轮收集（论文核心论点）。去重/缓存由 `Env` 里的 `dataCache`（`Monad.hs:184`）保证同一逻辑请求只取一次。

### ⑤ 基础类型设计

- **请求身份**：`type Request req a = (Eq (req a), Hashable (req a), Typeable (req a), ...)`（`DataSource.hs:130-136`），请求要可比较、可哈希——因为要做缓存键与去重。
- **结果/失败**：结果统一走 `Either SomeException a`（`ResultVar` 的定义）；一次求值的三态是 `ResultVal a w = Ok a (Maybe w) | ThrowHaxl SomeException (Maybe w) | ThrowIO SomeException`（`Monad.hs:616-619`）——**区分了"Haxl 内 throw 的异常"与"IO 抛的异常"**，这是有意的错误建模分层（`Monad.hs:716-754` Note [Exception] 讨论为何用 `Throw` 构造子而非直接 `throwIO`）。
- **待填变量**：`IVar`（`Monad.hs:507-518`），`IVarContents = IVarFull (ResultVal a w) | IVarEmpty (JobList u w)`——空的 IVar 携带一串等它的续延（write-once 语义，但注释指出 `Haxl.Core.Parallel` 有合法的多次写）。
- **无 money/time 领域类型**：Haxl 是取数框架，不定义金额/时间；这些留给业务的 `req`/结果类型。

### ⑥ 已知问题 / 已承认的局限（源码 TODO 与注释，非 retrospective 后悔）

源码内注释/TODO 是维护者**当场承认的局限与工程债**（不是事后 retrospective 后悔叙述——Haxl 无公开 post-mortem）：
- **时间源不对**：`Monad.hs:13-14` TODO："timing: we should be using clock_gettime(CLOCK_MONOTONIC) instead of getCurrentTime, which will be affected by NTP and leap seconds."
- **AsyncFetch 的异常语义可能错**：`Fetch.hs:508-510` 注释："this might be wrong: if the outer 'fio' throws an exception, then we don't know whether we have executed the inner 'io' or not"——这正是 UTA F5"发出后不可判定"的同构问题，作者在框架层承认没解决干净。
- **`RequestStore` 动态类型的 `unsafeCoerce`**：`RequestStore.hs:82` 用 `unsafeCoerce` 合并同类型请求，注释承认"the dynamic type check here should be unnecessary, but if there are bugs in Typeable or Map then we'll get an error"。
- **BackgroundFetch 无法精确计量分配**：`Fetch.hs:615-618` 注释承认对 background fetch 的内存归因是近似的。

---

## 案例 2：Standard Chartered Bank **Mu / Cortex**（Haskell 方言，量化定价与风险基础设施）

> 论文（读者出处）：Atze Dijkstra, José Pedro Magalhães, Pierre Néron, *Functional Programming in Financial Markets (Experience Report)*, Proc. ACM Program. Lang. 8, ICFP, Article 244 (2024), DOI `10.1145/3674633`；公开 PDF `dreixel.net/research/pdf/fpfm.pdf`。**定位一律用 §节号 / Listing 号 / 页码**（下文引用即如此，可复现）；本报告作者已 curl 此 PDF 逐页读全文。

### ① 做什么、规模与生产使用证据（一手，非常强）

Mu 是 SC 银行 Markets 部门（2023 年营业收入 30 亿美元，§1 摘要）自研的 Haskell 方言；生态叫 **Cortex**。规模（§2.2，已打开）："Mu code (over 7 million lines), C++ (under 2 million lines), Haskell (around 500 thousand lines), and some F#"，2008 年起；"thousands of users ... over one hundred write functional code"；每日 CI 构建、5000+ 测试。覆盖定价/风险 API、CLI、亚秒 REST 服务、批处理、GUI、分布式（"price one trade locally, or millions of trades across thousands of cloud nodes"）。

### ② 设计中心：`Work_` —— 一个可共享、可多后端执行的**计算 DAG 代数**

Mu 的定价/风险编排核心不是"把一笔交易的所有字段列出来的 object"，而是 **Work API：一个把"相互依赖的计算"表示成 DAG 的三构造子代数**（§3.1，Listing 2，appendix，已打开）：

```haskell
-- Listing 2「Work internals」(ICFP 2024, Supplementary Material A, p.13)
data Work_ = WorkPure Fun
           | WorkApp  Work_ Work_
           | WorkIO   (Any -> IO Any) Work_
newtype Work a = Work { unWork :: Work_ }
type    WorkPool   = [Work_]
type    WorkResult = Map Key Any
toKey :: Work_ -> Key
```

三构造子：常量 `WorkPure`、把函数应用到另一 Work 上（`WorkApp`，建立依赖）、把 IO 计算提升进来（`WorkIO`）。**Mu 没有存在量化**，作者用 `Any`/`Fun` 两个"擦除类型"的桥接类型手工模拟本该用 existential 写的 GADT（§3.1 原文："In the absence of existential quantification in Mu, we make use of two specific types, Any and Fun ... mimicking the typed ADT we could write directly if Mu had existential quantification"）。类型安全的外壳是 `Work a`（Listing 3）：`workApp :: Work (a -> b) -> Work a -> Work b`——**这是一个 Applicative 结构**，和 Haxl 的 `<*>` 暴露独立性异曲同工。

### ③ 多 provider / 后端异构怎么表达：**执行方法与描述解耦**（值级 + 类型驱动）

Work API 的明确设计目标（§3.1 原文）："separate the concerns between the definition of a sequence of inter-dependent computations (represented as a DAG) and the execution of these computations." 同一个 `WorkPool` 可以被三种"provider"（执行后端）消费：顺序、并行（每节点一线程/进程）、或 Shepherd 作业调度器。**后端不进核心 `Work_` 类型**，是外置的 `runWork` 系列解释器。此外 QuickRisk 用 type class（`Collect`、`Workable`，Listing 6）做**类型驱动的最小工作流生成**：从请求的输出类型反推需要哪些 `Stage`，只构造必要的节点。

### ④ 副作用怎么描述/组合/解释：`SafeIO` 分层 + DAG 节点共享（多层）

- **effect 分级**：Mu 把只读效应放进 `SafeIO` 而非 `IO`（§2.3.9 原文）："we have the function putStrLn :: String -> IO () but readFile :: String -> SafeIO String"；`io :: SafeIO a -> IO a` 提升，`perform :: SafeIO a -> a` 消费（"with fewer caveats than the infamous unsafePerformIO"）。这是**在类型上区分"可重跑的读"与"有后果的写"**——与 UTA 观察簇/效应簇的二分同构。
- **谁关联谁**：`WorkApp` 边即依赖；节点用 `Key` 标识、跨 `WorkPool` 共享（§3.1"Sharing of nodes ... avoids duplication of (sub-)work"），"重算一部分只触发依赖部分重算"（§1 摘要）——增量计算内建在 DAG 而非业务对象里。
- **解释分层**：runQR（`runQR :: forall r. (Collect r, Workable r) => Inputs -> Parameters -> SafeIO r`，§3.2）→ toWorkPool → runWork（三种后端之一）→ WorkResult。描述（纯 DAG）与执行（IO/并行/调度）严格分离。

### ⑤ 基础类型设计

- **交易表示**：直接用 Composing Contracts 的变体（§3.2 原文）："a representation of financial contracts as described by Peyton Jones et al. [9]. We use a variant of this representation"——**每种金融产品是一个不同的 Mu datatype**（§6.1"each different kind of financial product ... is modelled by a different Mu datatype"），不是一个巨型 union object。
- **数据即关系**：不用 list 而用 **Relations**（§2.3.4）——"abstract first class objects with a pure API ... tabular structure with named and typed columns ... no material ordering"；"millions of rows and over one hundred columns"。有代数律、便于重构。
- **风险数据的三分类**（§4 Risk schemas）：每列归为 `Position`（唯一标识）/ `Coordinates`（n-维空间，如 IR delta 的 (currency, curve, tenor) 3-space）/ `Value`（要求"additive over positions"以便任意聚合良构）。类型级编码，自动派生 SQL。
- **序列化**：§2.3.6"(De-)serialisation is supported by default for mostly everything"——任何部分完成的计算可存盘、跨 OS/架构重启（这是他们分布式的基础）。
- **字符串**是原生 UTF-8 `std::string`，无 `Char` 类型（§2.3.4）。

### ⑥ 已知问题 / 后悔（作者原话）

作者**明确宣称"see no significant downsides"**（§6.2）——这本身是一个数据点：一个 7M 行、16 年的生产系统作者不认为 FP 有重大代价。但论文诚实列了 Mu 语言层的具体局限（§2.3，一手）：
- **缺 existential / GADT / rank-2 类型**（§2.3.7："it lacks existential quantification, GADTs, and rank-2 (and higher) types – but ... having GADTs would be useful"）——直接导致 Work API 要用 `Any`/`Fun` 手工擦除类型。
- **编译慢**（§2.3.8）：whole-program 编译，"compilation times of 5 minutes are frequent ... largest applications can take 10 minutes"，且影响程序设计决策。
- **动态模块导入引入运行时类型错误**（§2.3.8）："leading to potential runtime type errors (which would have otherwise been compile-time errors)"。
- **内存调试困难**（§2.3.5）：C++ 引用计数与 GHC GC 两套内存视图无统一，"can make it sometimes challenging to track identifiers when debugging"。
- **无形式语义**（§2.3.1）："Mu does not have formal semantics ... choices ... informed by theory but driven by pragmatism"，最坏情况是运行时错误顺序不同。

---

## 案例 3：Barclays Capital **Functional Payout Framework (FPF)**

> 论文（读者出处）：Simon Frankau, Diomidis Spinellis, Nick Nassuphis, Christoph Burgard, *Commercial uses: Going functional on exotic trades*, JFP 19(1):27–45, 2009, DOI `10.1017/S0956796808007016`；作者托管公开 HTML 全文 `www2.dmst.aueb.gr/dds/pubs/jrnl/2008-JFP-ExoticTrades/html/FSNB08.html`（本报告作者已打开全文）。**定位以 Abstract / 章节名 / Figure 号为准**；下文括注的"行 N"是该公开 HTML 的抽取段落序号，仅为作者复核辅助、非权威出处，读者请按图/节定位。

### ① 做什么、规模与生产使用证据（一手，强）

FPF 是"a Haskell application that uses an embedded domain specific functional language to represent and process exotic financial derivatives"（Abstract）。关键点：不止定价，而是**用多种解释（multiple interpretations）**对同一份 payout 脚本做生命周期分析、现金流事件识别、生成人读数学文档、生成下游定价引擎的代码/参数、校验与录入。规模（§"Experience"）："20,000 lines of Haskell"（框架）+ "30,000 lines" 交易脚本，"more than 400 trade types (including variants)"，"around twenty direct users (traders and structurers)"，2-3 人年。

### ② 设计中心：**payout 的深嵌入 DSL —— 一组核心原语组合子**（非 Turing 完备）

设计中心是"payout function"——用一组核心原语组合出来的、编译期产生 AST 的深嵌 DSL（**Figure 1「The core FPF primitives」**）：

```haskell
-- Figure 1「The core FPF primitives」(JFP 2009)
(+), (-), (*), (/) :: Double -> Double -> Double   -- 算术
min, max, pow       :: Double -> Double -> Double
abs, log, exp       :: Double -> Double
observe :: Asset -> Date -> Double                 -- 资产价格观察 ★核心
cond    :: Bool -> a -> a -> a                     -- 条件
map     :: (a -> b) -> List a -> List b
zipWith :: (a -> b -> c) -> List a -> List b -> List c
foldl   :: (a -> b -> a) -> a -> List b -> a
foldr   :: (a -> b -> b) -> b -> List a -> b
mapAccumL :: (a -> b -> (a, c)) -> a -> List b -> (a, List c)
name    :: String -> a -> a                        -- 给子表达式挂元数据
```

`observe :: Asset -> Date -> Double` 是把"外部市场观察"引入的唯一原语——**观察是核心代数的一等构造，不是散落字段**。所有值必须是 `FPFVal` 实例（§2.2）："the type class representing all values that can exist in FPF ... prevents applying FPF primitives to non-FPF values ... allows us to uniformly traverse FPF programs"。**语言故意非 Turing 完备**（§2.2，无一般递归）："The fact that the language is not Turing-complete is not an impediment ... simplifies analysis"——这换来可静态分析。

### ③ provider / 解释异构：**deep embedding 的一份原语 → 多个后端**（开放世界的解释器集合）

FPF 明确产**多后端**（§ back-ends 综述）："multiple back-ends from a single trade description. These back-ends provide different interpretations or analyses ... range from a concrete price through to a user interface schema or barrier risk report." 机制是两步（§ deep embedding）：①脚本在 Haskell 编译器下用 deep embedding 跑一遍，产出**类型正确的 AST 并序列化**（"Haskell's type system guarantees the AST is correctly typed"）；②AST 被任意后端读入处理，**无需重编译**，"Code from multiple back-ends can live in the same executable without type clashes"。核心 `Double` 在不同解释下可以是真浮点、也可以是 AST 或注释集："our 'Double' may represent ... everything from an actual floating point number through to a syntax tree or set of annotations"——**同一核心类型被多态复用于多个解释**。

### ④ 副作用怎么描述/组合/解释：**纯 payout 描述 + 解释器族**（描述/解释两层）

FPF 的"副作用"就是各解释器产出的工件（价格、现金流事件、文档、下游代码）。payout 本身是纯的、可组合、可作静态分析的 DAG（legacy 用"payout DAG"、有 sharing）。组合性是核心卖点："Compositional construction is a natural mapping of the problem domain, and encourages code reuse by composing new trades from existing trade components." 元数据通过 `name` 原语挂在子表达式上、由静态分析提取——**关联信息进 AST 注释，不进业务对象**。

### ⑤ 基础类型设计

- **`FPFVal` type class**：FPF 世界的"值宇宙"边界（§2.2），所有原语受它约束。
- **`Asset`、`Date`、`Double`、`Bool`、`List a`**：核心原语的类型（Figure 1）。刻意用**自定义类型而非语言内建**，因为同一 `Double` 在不同 embedding 下含义不同。
- **序列化用 Haskell-like 文本而非 XML**："Using a Haskell-like text data structure rather than XML we were able to control the representation fully"——刻意提高第三方直接解析的门槛，只暴露翻译工具。

### ⑥ 已知问题 / 后悔（作者原话）

- **磁盘/可执行文件膨胀**（§ Performance）："executables coming in at 5 megabytes each ... FPF turned out to be unexpectedly disk hungry ... something of a weakness"。
- **朴素求值的性能陷阱**：AST 生成的 DAG 有大量共享，"Naïve evaluation processes the tree fully"——不处理共享会爆。
- **points-free 组合子库对量化分析师不可读**（§5.2 "withpoints"）："The points-free approach, while elegant, can make code unreadable, especially if it is written by quantitative analysts moonlighting as functional programmers."——所以他们**放弃**了纯 point-free 组合子库，改用更朴素的函数式风格。这是一条明确的"人因代价导致的设计回退"（作者 retrospective）。
- **对比 LexiFi/MLFi**（§ Related work）：他们把 MLFi 当 COTS 备选，"but in the end the risks in integrating the ... product"——最终自研。

## 已入表案例的统一 litmus 自检（OBSERVED / INFERENCE）

> 检查框架借自并行工作流的 effect-composition litmus，仅用于**逐案对照源码/论文实际说了什么**，源未覆盖处标"来源未覆盖"，不从现代术语反推。litmus 设定：provider P 支持 Read+Write，Q 支持 Read+Stream。

**能力"何时已知"三阶段** / **开放世界双轴（expression problem）**：

| 案例 | Q.Write 何时被拒 | 两个独立 Read 能否批处理/并行 | Write↔审计/结果关联 | replay 是否重放 Write | 能力已知阶段 | 加 provider 改旧码？ | 加 operation 改旧 provider？ |
|---|---|---|---|---|---|---|---|
| **Haxl** | **构造期（类型级）**：`req a` 是每 provider 自己的 GADT，Q 的请求类型里根本没有 Write 构造子 → 写不出（OBSERVED，DataSource.hs GADT+existential） | **能**：`<*>` 独立两侧同轮收集、按 provider 批处理（OBSERVED，ICFP abstract + Fetch.hs performFetches） | Haxl 是取数框架，无写审计概念——**来源未覆盖**（OBSERVED：无 Write/audit 类型） | Haxl 缓存去重保证同一请求一轮内只发一次；跨 run 无 replay 语义（OBSERVED，dataCache）；写的幂等**未覆盖** | **compile-time**（req 类型 + DataSource 实例齐备即定） | **否**：新 provider = 新 `DataSource u req` 实例，核心 `runHaxl`/`RequestStore` 零改（OBSERVED） | **否**（对既有 provider）：新 effect = 新 req 类型 + 新实例；但消费方要用到得改调用点（INFERENCE） |
| **SC Mu** | Read/Write 分级靠 `SafeIO` vs `IO`（OBSERVED §2.3.9）；但这是**效应种类**分级，非 per-provider 能力拒绝——Q 不支持 Write 这类**运行期能力**在 Mu 里无静态表达证据（来源未覆盖） | **能**：`Work_` DAG 独立节点可并行/分布式执行（OBSERVED §3.1） | Work 有 `Key` 标识、`WorkResult :: Map Key Any` 关联输入↔结果（OBSERVED Listing 2）；金融审计链**来源未覆盖** | Work 节点可序列化后在另一机重启（OBSERVED §2.3.6）；`WorkIO` 重放语义**来源未覆盖** | 混合：effect 分级 compile-time；具体后端选择 interpreter 构造期（runWork 选择） | **否**：新执行后端 = 新 `runWork` 解释器，核心 `Work_` 不改（OBSERVED §3.1） | **是（可能）**：`Work_` 是封闭三构造子，新增 effect 种类需改核心 ADT（INFERENCE，缺 GADT 佐证 §2.3.7） |
| **Barclays FPF** | Q.Write 无对应原语即写不出——**构造期**，因语言是封闭原语集深嵌（OBSERVED Figure 1，无写原语，payout 是纯映射） | **能**（INFERENCE）：payout 是纯 AST，独立子表达式可并行求值；论文未直接讨论批处理 | `name :: String -> a -> a` 把元数据挂子表达式、静态分析提取（OBSERVED §metadata/name）；无外部写审计（payout 无副作用） | payout 确定性、无外部 Write → replay 无副作用（OBSERVED：非 Turing 完备纯 DSL） | **compile-time**：脚本编译期产出类型正确 AST（OBSERVED §deep embedding） | **否**：新解释器读同一 AST，无需重编译、无类型冲突（OBSERVED §back-ends） | **是**：新原语要加进核心原语集（OBSERVED §"we slowly changed the primitives available"） |

**"谁和谁能关联"五类关系承载物**（逐案，OBSERVED 除非标注）：

- **operation↔capability**：Haxl=type param（`req a` 的构造子即能力，编译期）；FPF=封闭原语集（构造期）；Mu=`SafeIO`/`IO` 类型分级（compile-time，仅读写二分）。
- **request/resource↔provider 身份**：Haxl=`TypeRep` 键（`Map TypeRep (BlockedFetches u)`，运行期分桶但类型驱动）；Mu/FPF **来源未覆盖**（单体解释，无多 provider 路由）。
- **多 effect↔同一因果/并发作用域**：Haxl=一轮 `performFetches` 即一个并发作用域（OBSERVED Run.hs schedule 循环）；Mu=一个 `WorkPool`/`Key` 共享域；FPF=一棵 payout AST。
- **program↔handler/interpreter**：三者都是"纯描述值 → 外置解释器"，handler 不进核心类型（Haxl `PerformFetch`、Mu `runWork`、FPF 后端）。
- **intent↔执行结果/审计/replay 身份**：Haxl=`BlockedFetch (r a) (ResultVar a)` 把请求与结果槽类型级绑定（OBSERVED DataSource.hs:187）；金融级 intent→审计→replay 三案**均来源未覆盖**（都不是"不可丢、有资金后果的写"域）——**这正是 UTA 效应簇相对这三案的独特之处，需在 DAML/Marlowe/ledger 案例里找对应**。

---

## 案例 4：**Composing Contracts**（Peyton Jones/Eber/Seward, ICFP 2000）+ LexiFi 商业化

> 论文（读者出处）：Simon Peyton Jones, Jean-Marc Eber, Julian Seward, *Composing Contracts: An Adventure in Financial Engineering*, ICFP 2000, SIGPLAN Not. 35(9):280–292, DOI `10.1145/357766.351267`；canonical PDF `www.cs.tufts.edu/comp/150FP/archive/simon-peyton-jones/contracts.pdf`（本报告作者已逐页打开、核对 Figure 2/3）。**定位以 Figure 号 / §节号 / ACM 页码为准**；下文"行 N"是该 PDF 的抽取段落序号，仅为作者复核辅助、非权威出处。LexiFi 官方材料由委派 curl 打开；MLFi 论文 PDF 返回 `AccessDenied` **未打开**，不作依据。

### ① 做什么、规模与生产使用证据

这是"设计中心 = 组合子代数"的**原型论文**。摘要（Abstract）："We introduce a combinator library that allows us to describe such contracts precisely, and a compositional denotational semantics that says what such contracts are worth." **生产证据须降级**：论文 §5.3 只提"two partial implementations"，无客户/上线/交易量。生产化证据来自 **LexiFi**（Eber 联合创办）：官方 FAQ 自述"Used in many highly demanding production environments for more than 15 years"，About 页"25 YEARS TRACK RECORD / 50 CLIENTS WORLDWIDE"（供应商自述，非可审计第三方案例）。SC Mu（案例 2）§3.2 亦证实工业界直接复用此模型："a representation of financial contracts as described by Peyton Jones et al. [9]"。

### ② 设计中心：`Contract` + `Obs a` 双代数（本报告作者从 canonical PDF 核出完整原语签名）

论文正文只给出截断 `data Contract`（§5.3），但 **Figure 2「Primitives for defining contracts」完整列出原语签名**（ICFP'00 §3.4，pp.283–284，已核对）：

```haskell
-- Figure 2「Primitives for defining contracts」(ICFP'00 §3.4, pp.283-284)
zero     :: Contract                              -- 无权利无义务，无限 horizon
one      :: Currency -> Contract                  -- 立即收 1 单位 k
give     :: Contract -> Contract                  -- 权利义务互换（对手方）
and, or  :: Contract -> Contract -> Contract
truncate :: Date -> Contract -> Contract          -- 限制 acquisition 的最晚日
then     :: Contract -> Contract -> Contract
scale    :: Obs Double -> Contract -> Contract    -- 按 observable 缩放
get      :: Contract -> Contract                  -- 在 horizon 处 acquire
anytime  :: Contract -> Contract
-- Figure 3「Primitives over observables」(ICFP'00 §3.3, pp.283-284)
konst    :: a -> Obs a
lift     :: (a -> b) -> Obs a -> Obs b
instance Num a => Num (Obs a)                      -- observable 可做算术
type Days = Double
```

`Obs a`（§3.3）："a value of type Obs d represents a time-varying quantity of type d"。**两个正交代数**：`Contract`（权利/义务的组合）与 `Obs a`（客观可观测量）。设计哲学（§3.2 "Discount bonds"）："by separating them we significantly simplify the semantics and enrich the algebra of contracts"——刻意把复合原语（`zcb = scaleK x (get (truncate t (one k)))`）拆成独立小原语。这与 Barclays FPF、Marlowe 同源。

### ③ provider / underlying 异构：**封闭核心构造子 + 值级/模型级开放 observable**

核心 `Contract`/`Obs` 构造子固定；underlying 通过 observable 注入且**开放**（行"an arbitrary number of other primitive observables"）：`libor :: Currency -> Days -> Days -> Obs Double`、`noonTempInLA :: Obs Double`。估值时的 provider 是 `Model`（利率演化/汇率演化/underlyings，§5.3）。[INFERENCE] 无 `Provider`/capability/evidence 类型，无 provider 缺失的错误类型——不能把数据源拒绝/批处理/重试语义推回此代数。LexiFi 官方同构确认（CDL 博客）："mapping of abstract underlyings to actual underlyings is outside the scope of the DSL"。

### ④ 副作用怎么描述/组合/解释：**纯合约描述 → 抽象估值语义 → 具体数值模型**（两/三层）

合约是权利/义务描述，非 `IO`。解释分层（§4-5）：抽象层 `E_k[[·]] : Contract -> PR R`，`PR a = DATE -> RV a`（value process），复合合约的值 = 子合约值的组合（compositional）；具体层选 lattice/Monte-Carlo/PDE 解释过程，`eval :: Model -> Currency -> Contract -> ValProc`。同一描述可驱动 schedules、back-office execution、risk analysis（§2）——多解释器族，与 FPF 一致。**无 IO/事件日志/replay 类型**。

### ⑤ 基础类型设计

抽象 `Currency`、抽象 `Date`、`type Days = Double`、`Obs a`；语义层 `RV a`（random variable）/`PR a`（process）；实现层 `type ValProc = (TimeStep, [Slice])`、`type Slice = [Double]`。**金额是 `Double`/`Obs Double`，非 `Money`/`Quantity` record**。无 `Id`/结构化 `Error`/`Unknown` 结果；"签约时可能未知"仅是 observable 的语义描述，非显式 ADT。LexiFi 简化语法同层级：`cash_flow(T, C, e)`、`S(T)`、`if e then c1 else c2`（正负号表收付方向）。

### ⑥ 已知问题 / 后悔（作者原话）

- **原语集不足、选原语难**（§3.6，作者原话）："not enough to describe all the contracts that are actively traded, and we are extending the set"；"Identifying the 'right' primitive combinators is quite a challenge."
- **过程树二次膨胀 / sharing 是开放问题**（§5.3）：memo function 方案作者不满意，"we simply identify it as an important open problem that deserves further study"；抽象语义数学细节自嘲"arcane"。
- **LexiFi 性能压力原话**（FAQ）："pricing is never fast enough. Never."；symbolic description "is never directly executed for pricing purposes"，必须 advanced compilation，Monte-Carlo payoff code 执行 millions of times。
- LexiFi 明确 scope 外（CDL 博客）：party↔real party、underlying↔real underlying 的映射在 DSL 外；counterparty failure、corporate events 等 extraordinary events 不纳入 nominal behavior——**这正是 UTA 最关心的"外部不可判定/归因"被这套纯代数排除在外**。

---

## 案例 5：Digital Asset **DAML**（Haskell 派生语言，`Update` monad + 授权模型）

> 源码：`git clone https://github.com/digital-asset/daml`，commit `f3b4b347208490fa8b97d67c0ba84fa6317c3663`（委派 clone，已按 commit 打开）。permalink 均带 commit。

### ① 做什么、规模与生产使用证据

DAML 是智能合约语言 + 工具链：`.daml → Daml-LF → .dar`，Canton participant 评估 Daml-LF（The Daml Compiler，`flags.rst`）。**生产规模证据须降级**：本次打开的仓库/文档无客户名/年限/交易量；只能证"可部署的语言/ledger 运行链"（Sandbox = 单 Participant + Synchronizer 的 Canton）。

### ② 设计中心：`Update a`（可解释的 ledger 事务 action 代数）+ template 授权边界（相邻核心）

```haskell
-- DA/Internal/LF.daml#L146-L167  (commit f3b4b34)
data Update a = Update Opaque       -- 运行时表示 Opaque
instance Action Update where (>>=) = primitive @"UBind"
instance Applicative Update where pure = primitive @"UPure"
-- Daml-LF AST 列出真实 action 构造子 (daml-lf-ast/.../Base.hs#L674-L776):
--   UPure | UBind | UCreate | UExercise | UFetch | UGetTime | ...
create   : t -> Update (ContractId t)      -- Template/Functions.daml#L45-L115
fetch    : ContractId t -> Update t
exercise : ContractId t -> c -> Update r
```

`Update a` 注释："an `Action` to update or query the ledger" 返回 `a`——**不是 IO，是一个可被 ledger 解释器消费的事务描述**。template 的 `signatory`/`observer`/`choice ... controller ... do` 是资源与授权边界（相邻核心，不与 `Update` 硬合并）。[边界] 运行时 `Update` 是 `Opaque`，AST 的 `UBind/UCreate...` 是编译器侧节点，不能伪造完整 free-monad 定义。

### ③ provider 异构 / 授权：**类型级 action 形状 + 值级 Party/authorization evidence**

类型级：`Update a`、`ContractId t`、choice 参数/结果保证 action 与资源形状；值级：Party 集合、contract id、ledger time、controller/signatory 值。**Authorization 是 evidence-like 运行条件**（glossary.rst）：`create` 需 signatories、`exercise` 需 choice controllers、`fetch` 需 stakeholder；"authorization 只来自当前 choice 的 signatories/controllers，不从更早 execution context 继承"。[INFERENCE] backend 异构（Canton 拓扑/LF interpreter）不由 `Update` 携带 provider 名承载；材料未承诺各 backend 语义等价。

### ④ 副作用怎么描述/组合/解释：**Update do-block 组合 → LF interpreter 授权检查 → transaction commit/abort**

choice body 里 `create/fetch/exercise/getTime` 用 bind/do 组合成一个 ledger update；**谁关联谁**（glossary）："A Daml update is the composition of update actions"，所有 contained actions 须 well-authorized，required parties 与 action 关联。授权缺失 → abort 整个 transaction（"nothing in it will be committed"）——**原子性 + 授权是核心不变量**。`Script a`（`runScript : () -> Free ScriptF (a,())`）是另一层测试/客户端编排，不是 Canton 内部 Update 语义。

### ⑤ 基础类型设计（强，类型驱动精度）

`Party = Party Opaque`、`Date/Time = Opaque`、`ContractId a = ContractId Opaque`（**类型索引指向 template `a`**）；数值是**类型级精度** `data Numeric (n : Nat) = Numeric Opaque`（precision 固定 38、scale n∈0..37），`type Decimal = Numeric 10`（GHC/Types.daml#L180-L201）。**unknown 建模**：`lookupByKey : k -> Update (Optional (ContractId c))`——"不存在"是 `Optional` 而非命名 error；`abort : Text -> m a` 以文本失败（非 error ADT）。核心无 `Money`/`Currency`（那在 DAML Finance 层，本次未取）。

### ⑥ 已知问题 / 后悔（维护者原话）

- **Script 与 Daml 纠缠过深**（PROJECTS.md，roadmap 原话）："Daml script is too tightly entangled with Daml. We would like to completely separate it such that Daml scripts are not a dependency of Canton."
- **contract keys 在 Canton 3.x 不支持**（contract-keys.rst）："Contract keys are not supported in this release of Canton 3.x"；unique keys 不计划支持，因多 subnetwork 随时连断时无已知基础设施级唯一性方案——**直接对应 UTA F6"幂等/回读语义按 venue 变化、部分不可保证"**。
- **时间不单调**（time.rst，维护者警告）：ledger/record time 都不保证单调、"fuzzy"，"不应把 getTime 返回值解释为 precise timestamp"——对应 UTA F11。

---

## 案例 6：**Marlowe**（Cardano 金融合约 DSL）

> 源码：`marlowe-cardano` commit `99f432d8ef9dbd1b52b7fa089254de15913b490f`、`marlowe`（Isabelle 规范）commit `7b5b1e900ec53a8eb18747992bec73470704dfcb`（委派 clone，已打开）。

### ① 做什么、规模与生产使用证据

多方金融合约 DSL，含 Haskell 参考语义、Isabelle 形式规范、Plutus 链上实现、Runtime/CLI。**采用证据（有边界）**：一手案例文档记录 mainnet 链上运行——revenue-based loan "run on the Cardano blockchain: that involved 8 transactions"，English Auction "18 transactions"；IOG 公告 "deployed to the Cardano mainnet"（语境是 early adopters 探索）。[边界] 无总用户/合约数/TVL/累计量，不能外推为持续商业规模。

### ② 设计中心：**闭合的 6 构造子 `Contract` 代数 + `Value`/`Observation`/`Action` 三子代数**

```haskell
-- marlowe-cardano .../V1/Semantics/Types.hs#L292-L308 (commit 99f432d)
data Contract = Close
              | Pay AccountId Payee Token (Value Observation) Contract
              | If Observation Contract Contract
              | When [Case Contract] Timeout Contract
              | Let ValueId (Value Observation) Contract
              | Assert Observation Contract
data Value a = AvailableMoney AccountId Token | Constant Integer
             | AddValue (Value a) (Value a) | MulValue (Value a) (Value a)
             | ChoiceValue ChoiceId | Cond a (Value a) (Value a) | ...  -- #L191-L208
data Observation = AndObs .. | ValueGE (Value Observation) (Value Observation)
                 | ChoseSomething ChoiceId | TrueObs | FalseObs | ...     -- #L219-L230
data Action = Deposit AccountId Party Token (Value Observation)
            | Choice ChoiceId [Bound] | Notify Observation                -- #L254-L258
```

**封闭 sum（六种建合约方式），故意有限、可静态分析、可穷尽、必终于 `Close`**（Isabelle 规范 continuation-based）。与 Composing Contracts 同族但**更封闭**（Marlowe 无 observable 开放注入，全部枚举），因为要上链做验证器。

### ③ provider 异构：**无 provider 类型；异构轴是值级资产与参与者**

`Party = Address Network Ledger.Address | Role TokenName`、`Token = Token CurrencySymbol TokenName`、`Accounts = Map (AccountId, Token) Integer`。[OBSERVED] 无 `Provider`/`Interpreter` typeclass；操作集合是**封闭 ADT**（加操作要改语言+解释器），参与者/资产身份是**开放的字节/ledger 值**；无类型级 provider capability 静态拒绝。

### ④ 副作用怎么描述/组合/解释：**纯 reduction → `Payment` intent → 链上验证器实际转账**（三层）

effect 单独建模：`ReduceEffect = ReduceWithPayment Payment | ReduceNoPayment`，`Payment = Payment AccountId Payee Token Integer`。入口是**纯函数**：
```haskell
-- .../V1/Semantics.hs#L814-L833
computeTransaction :: TransactionInput -> State -> Contract -> TransactionOutput
```
成功返回 warnings/payments/new state/continuation，失败返回 `Error TransactionError`。**关键分层**（Isabelle SemanticsTypes.thy#L343-L363）：内部 `Account` payee 更新内部 accounts；外部 `Party` payee "the actual transfer is expected to occur outside of the semantics"——**纯语义只产出 payment intent，真正外部转账在语义之外由链验证**。Cardano 把同一解释器当 validation script。这正是 UTA"核心只消费统一协议、真实 venue 交互在集成层"的同构。

### ⑤ 基础类型设计（强）

`Money = Plutus Value`；`Token = Token CurrencySymbol TokenName`；`AccountId = Party`；`Timeout = POSIXTime`；`TimeInterval = (POSIXTime, POSIXTime)`；`ChoiceId = ChoiceId BuiltinByteString Party`；`Bound = Bound Integer Integer`；`ada = Token adaSymbol adaToken`（Ada 以 Lovelace 百万分之一计）。**结果/错误显式 ADT**：`TransactionOutput { txOutWarnings, txOutPayments, txOutState, txOutContract } | Error TransactionError`，`TransactionError` 枚举 ambiguous interval/no match/interval error/useless transaction/hash mismatch——**枚举失败原因，无 catch-all**（对应 UTA C13）。

### ⑥ 已知问题 / 后悔（维护者原话）

- **audit：validator 曾基于 "optimistic assumptions" 省略检查以降 Plutus 成本**（response-to-audit-report.md#L38-L60），后补 state 不变量；且改动 "have not been re-audited"（L14）——修复 ≠ 重新审计保证。
- **表达/资源硬限**（example-contracts.md#L141-L167）：大合约在 Preview 因 transaction-size limit、在 Pioneers 因 execution memory/steps "fails to execute"；English Auction 未 merkleize 时 990 MB/~940k `Case`，merkleize 后 9.8 MB/1150 cases——**封闭穷尽代数的代价：状态空间爆炸，靠 merkleize 分片绕过**。

---

## 案例 7：**cardano-ledger** 的 `STS`（Small-Step Transition Systems）框架

> 源码：`cardano-ledger` commit `2c33b4f858c0e62b300d121996a479f505d8c0e5`（委派 clone，已打开）。**与案例 6 是同生态两个平行核心**：Marlowe 是合约语义，STS 是账本状态转移，不归一。

### ① 做什么、规模与生产使用证据

Cardano Ledger 的 "formal specifications, executable models, and implementations"。**采用证据**：changelog 明确变更进入每个 `cardano-node` release（自 8.0 起），contributing 称其 "a core Cardano component with downstream impact across the whole ecosystem"——与生产节点发布链路相连。[边界] 无节点数/交易量/年限统计。

### ② 设计中心：**`STS a` type class —— 一族可组合嵌套的状态转移系统**

```haskell
-- libs/small-steps/.../Transition/Extended.hs#L208-L258 (commit 2c33b4f)
class (Ord (PredicateFailure a), Show (PredicateFailure a),
       Monad (BaseM a), Typeable a) => STS a where
  type State a :: Type
  type Signal a :: Type
  type Environment a :: Type
  type BaseM a :: Type -> Type   ; type BaseM a = Identity
  type Event a :: Type           ; type Event a = Void
  type PredicateFailure a :: Type
  initialRules    :: [InitialRule a]
  transitionRules :: [TransitionRule a]
  assertions      :: [Assertion a]
type TransitionRule sts = Rule sts 'Transition (State sts)   -- #L169-L171 (是 synonym，非 data)
data TRC sts = TRC (Environment sts, State sts, Signal sts)  -- #L152-L171
```

[委派更正] 源码里**没有** `data TransitionRule` 或独立 `type family Environment/State/...`——它们是 STS class 的 **associated types**，`TransitionRule` 是 type synonym。[INFERENCE] 最简 `BaseM ~ Identity` 特化下一条规则概括为 `(Environment, State, Signal) -> Either [PredicateFailure] State`（Simple.hs#L17-L28），但真实 API 保留 rule type/monad/非空 failure/可选 events。

### ③ provider / era 异构：**type-level 开放世界，associated types + era 参数化**

异构轴是不同 `s`/`era` 的 instance：`STS (DELEG era)` 把 `State`→`CertState era`、`Signal`→`TxCert era`、`Environment`→`DelegEnv era`、`PredicateFailure`→`ShelleyDelegPredFailure era`（Deleg.hs#L169-L187）；后者是**该规则封闭的 sum**。规则嵌套由 `Embed sub super` 的 `wrapFailed`/`wrapEvent` 承载；`ledgerTransition` 先 `trans @(EraRule "DELEGS" era)` 再 `trans @(EraRule "UTXOW" era)`（Ledger.hs#L315-L354）。[INFERENCE] 加系统/era = 加 `STS` instance，通用 interpreter 不枚举 provider（开放世界）；但加新 signal operation 仍须由相应 STS 的 `Signal`/rule 表达（expression problem 另一轴）。

### ④ 副作用怎么描述/组合/解释：**规则 AST（free-monad 风格）→ `BaseM` interpreter → node 消费 state/failures/events**（三层）

STS rule 不是裸函数而是 `Rule = F (Clause ...)` 的 free-monad 规则 AST；`Clause` 提供 `Lift (BaseM sts) a | GetCtx | SubTrans | Writer [Event sts] | Predicate | Label | IfFailureFree`（Extended.hs#L323-L359）。**`BaseM` 是可替换的解释 monad**（默认 `Identity`）。`applySTS` 默认 `ValidateAll`/`EPDiscard`，给 `State` 或 `NonEmpty PredicateFailure`；子 STS 失败经 `wrapFailed` 进父、事件经 `wrapEvent` 进父事件流（`SubTrans` 递归调子 interpreter）。[INFERENCE] event 是框架显式输出，不是业务副作用；外部 IO 边界不在 STS 定义内。

### ⑤ 基础类型设计

`IRC sts = IRC (Environment sts)`、`TRC sts = TRC (Environment sts, State sts, Signal sts)`；**失败结果显式** `Either (NonEmpty (PredicateFailure s)) (State s)`，可选事件经 `EventReturnType`/`STSResult { stsResultState, stsResultFailures, stsResultEvents }`。具体 failure sum 携带 `Coin`、credential、slot 等领域值（Deleg.hs#L126-L158）。`Ord (PredicateFailure a)` 是 class 约束。

### ⑥ 已知问题 / 后悔（维护者原话）

[OBSERVED] **未找到 STS 设计后悔或 post-mortem**（诚实标注，不推导）。仅有工程/人因成本原话：contributing 称 "security-critical project"、review "time-expensive"（CONTRIBUTING.md#L5-L13）；profiling 文档作者 Tim Sheard 称其 "an intricate dance between nix, cabal, and ghc"，重编译 "takes a very long time (greater than 30 minutes)"（HowToProfileLedger.md）。

---

## 案例 8：**Tsuru Capital**（Haskell 高频期权做市，历史）

> 证据等级：**中/弱**。交易核心类型未公开；公开的是 iteratee/Franz 工具链。源码 commit 见证据表。

### ① 做什么、规模与生产使用证据

HCAR 21 版（2011）："high-frequency market-making on options markets"，在 Kospi 200 上跑套利型流动性提供，交易软件 "developed entirely in Haskell"。Tsuru 博客："We build our internal systems for live trading and offline analysis in Haskell"，iteratee 遍布系统、补丁 "use in production every day"。[边界] 无订单量/延迟/账户数。**当前官网（2024-11 更新）："In-house software is mainly developed in Rust"**——历史 Haskell 事实不能外推当前全栈。

### ② 设计中心：**未找到交易核心的一手类型**（诚实降级）

HCAR 只公开语言与业务用途，无订单/报价/持仓/结算代数。可审计的**相邻**公开中心是 iteratee 流处理（`tsurucapital/iteratee` commit `352448d`）：
```haskell
data Stream c = EOF (Maybe SomeException) | Chunk c            -- Base.hs#L45-L93
newtype Iteratee s m a = Iteratee { runIter :: forall r. (a -> Stream s -> m r)
     -> ((Stream s -> Iteratee s m a) -> Maybe SomeException -> m r) -> m r }
type Enumeratee sFrom sTo m a = Iteratee sTo m a -> Iteratee sFrom m (Iteratee sTo m a)
```
以及 Franz append-only 日志/回放容器（`tsurucapital/franz` commit `6d27cd3`）。这些说明公开工程中心是"流 + 可组合转换/消费"，**不能据此声称它是交易核心**。

### ③–⑤（简）

**③ provider 异构：未知**（无 broker/exchange adapter 公开）；iteratee 的异构是流形状/控制消息（`seek`）异构。Franz `FranzPath = FranzPath HostName PortNumber FilePath | LocalFranzPath FilePath`（封闭 local/remote）。**④ 副作用**：iteratee 的 `m` 参数把消费逻辑参数化于外层 monad，`psequence_` 为每个分析 iteratee `forkIO`；Franz 异步消费 `type Response = Either Contents (STM Contents)`（`Right`=服务端内容就绪前阻塞）。**⑤ 基础类型**：`Item = Item { seqNo :: !Int, indices :: !(U.Vector Int64), payload :: !ByteString }`、`ItemRef = BySeqNum !Int | ByIndex !IndexName !Int`、`FranzException = MalformedRequest .. | StreamNotFound .. | ...`；**无 Money/Quantity/Currency/订单类型公开**。

### ⑥ 已知问题 / 后悔（作者原话）

[OBSERVED] **未找到交易核心后悔/post-mortem**；当前 Rust 陈述无迁移原因说明，不推导为后悔。Franz 维护者写的是取舍："不用 Kafka" 因异常关机后启动慢、clustering 有时让整体可靠性更差（README#L51-L58）；writer 必须集成以保证 "no server failure blocks the application"。

---

## 案例 9：**Mercury**（银行；`hs-temporal-sdk` 的 Workflow/Activity durable-execution 核心）

> 证据等级：**强**（生产规模 + SDK 类型）。源码 `MercuryTechnologies/hs-temporal-sdk` commit `2270909b35944823fe57543009b661877bc19319`；官方 Haskell Blog（Ian Duncan，Mercury Stability 工程师）。**注意**：SDK 是 durable-execution 基础设施，非 Mercury 支付账本核心（后者未公开）。

### ① 做什么、规模与生产使用证据（强）

官方 Haskell Blog："over 300,000 businesses"、2025 处理 "$248 billion" transaction volume、年化收入 "$650 million"、~1500 员工、**约 2 million 行 Haskell**、运行多年经历 hypergrowth/SVB 危机（五天 +$2B 存款、8700 客户）。工程组织多为入职前没写过 Haskell 的 generalists。SDK README："in production use at Mercury"（但对外部用户不保证稳定）。

### ② 设计中心：**确定性 `Workflow` monad + 隔离副作用的 `Activity` 外壳**（直接对应 UTA 的意图/effect 分层）

```haskell
-- Temporal/Workflow/Internal/Monad.hs#L68-L84 (commit 2270909)
newtype Workflow a = Workflow { unWorkflow :: ContinuationEnv -> InstanceM (Result a) }
-- Temporal/Activity/Definition.hs#L169-L189
newtype Activity env a = Activity { unActivity :: ReaderT (ActivityEnv env) IO a }
-- typed 注册（existential codec）:
data KnownWorkflow args result =
  forall codec. FunctionSupportsCodec codec args result => KnownWorkflow ...  -- Definition.hs#L77-L86
```

**`Workflow` 是对 event history 的纯函数（可重放），`Activity` 是唯一能做 IO 的外壳**——"The workflow orchestrates; the activities execute"（Blog）。`MonadIO Workflow` 被 `TypeError` 拒绝（Monad.hs#L230-L235），错误信息要求用 `executeActivity`/`executeLocalActivity`。**这是"描述层禁止 IO、执行层隔离 IO"在类型层的强制**。

### ③ provider 异构 / ④ 副作用 / replay（对 UTA 最相关）

**③** 工作单元异构在类型级用 `ActivityRef` 的关联类型 `ActivityArgs f`/`ActivityResult f` + existential codec 表达；Worker 侧按运行时 `Text` 名把定义放进 `HashMap`（`Definitions env`，开放值级集合）。无具体支付 provider capability 协商。**④ + replay**：`startActivity : Workflow (Task result)`，`Task a = Task { waitAction :: Workflow a, cancelAction :: Workflow () }`（等待/取消作为可组合值）。Temporal 记录每步 event history，**worker 崩溃后重放确定性前缀再继续；重试/超时/取消由平台处理**（Blog#L57-L60）。横切副作用用 `WorkflowInboundInterceptor`/`OutboundInterceptor`（有 `Semigroup`/`Monoid` 组合），不改每个 workflow。`unsafeAsyncEffectSink :: IO () -> Workflow ()` 仅供 metrics/logging（源码警告不得用于 workflow logic）。

### ⑤ 基础类型设计（强）

newtype 隔离标识：`WorkflowId/RunId/ActivityType/ActivityId/TaskQueue`（`TaskQueue` 文档明确"不保证 ordering"）；时间 `UTCTime`（`now :: Workflow UTCTime`）、`Duration = Duration { durationSeconds :: !Int64, durationNanoseconds :: !Int32 }`。**多层显式错误**：`Codec.decode :: fmt -> Payload -> IO (Either String a)`、`RetryState`（封闭 ADT）、`ApplicationFailure`/`ActivityFailure`（携带 retry/payload/activity id）、workflow 终态 `WorkflowExitSuccess a | WorkflowExitFailed SomeException | ...`。Activity 文档建议 critical side effects 用 **idempotency keys**（Definition.hs#L143-L167）——**直接对应 UTA C1/幂等键**。

### ⑥ 已知问题 / 后悔（作者原话，强）

- **采用前痛点**（Blog#L57）：过去用 cron/background worker 驱动的 DB 状态机、重试/超时散落，"fragile ... source of a disproportionate share of our operational incidents"；采用 Temporal 是 "one of the better infrastructure decisions"。
- **类型编码的代价**（同作者原话，Blog#L75-L133）："not always clear if the cognitive load is net-reduced" 的近似陈述——类型编码有认知开销、引入 rigidity、需求变更更难改，全部不变量编码会导致跨 50 个模块、数周重构；总结 "has not been painless"。
- SDK 仍 heavy development、外部不保证稳定（README）。

---

## 案例 10：**Bitnomial**（受监管数字资产交易所；类型安全 API/认证边界）

> 证据等级：**中**。监管时间线 + 员工访谈 + 公开 API/认证库源码；matching/clearing/settlement 核心未公开。源码 commit 见证据表。

### ① 做什么、规模与生产使用证据

官方 About 可核验时间线：2014 创立、2020 CFTC DCM approval、2022 FCM registration、2023 DCO approval、2025 首个 U.S. perpetual futures、2026 被 Payward（Kraken 母公司）收购。matching engine 在 AWS US-East-2（API Overview）。员工访谈（Serokell）："backend is built entirely in Haskell"，支撑 "millions of trading accounts"。[边界] 官方页无账户/吞吐数字。

### ② 设计中心：**未找到 matching/clearing/ledger 核心公开类型**（诚实降级）；公开中心是类型级 API contract

`bitnomial/servant-jsonrpc`（commit `bfc17a8`）把方法名/参数/错误/结果放进类型参数：
```haskell
data JsonRpc (method :: Symbol) p e r                        -- JsonRpc.hs#L191-L204
data JsonRpcResponse e r = Result Word64 r | Ack Word64 | Errors (Maybe Word64) (JsonRpcErr e)
data JsonRpcErr e = JsonRpcErr { errorCode :: Int, errorMessage :: String, errorData :: Maybe e }
type RpcHandler (JsonRpc method p e r) m = p -> m (Either (JsonRpcErr e) r)  -- server#L115-L174
```
[边界] 这是 API contract + response outcome 的公开可审计中心，**不证明交易引擎内部用同一代数**。

### ③–⑤（简）

**③ provider/协议异构**：值级多协议接入（DMA binary BTP、FIX 4.4 dropcopy、REST、WebSocket、CQG indirect）；API DSL 里部署的 endpoint 集合是封闭类型树 `RawJsonRpc JSONRPC (Add :<|> Multiply :<|> Print)`（加 endpoint = 扩展类型树）。认证 `HmacAuth hashAlgorithm id`：`hashAlgorithm`/`id` 是类型参数，key 查找是值级效果 `verifySignature :: .. -> (id -> m (Maybe AuthKey)) -> ..`——**类型级算法/身份选择 + 值级 evidence 注入**同时可见。**④ 副作用**：handler 返回 `Either (JsonRpcErr e) r`；HMAC middleware 先构 digest 再 effectfully 取 key 验签，失败用 `HmacError` 报 operator。**⑤ 基础类型**：`Request p = Request { method :: String, params :: p, requestId :: Maybe Word64 }`；`AuthKey` 是 opaque `ByteString` newtype（避免 `Show` 泄密）；`HmacError = UnknownIdentity | NoSignature id | UnknownKey id | InvalidSignature id | UnsignableRequest String`。**这些是 transport-level unknown/error，非金融域 unknown 结果**；无 `Money`/`Quantity`/`Currency` 公开。

### ⑥ 已知问题 / 后悔（员工原话）

- **第三方 bindings 难做对**（访谈原话）："relatively large set ... looking at you, FIX"；GHC eventlog 知识传播不足使高性能 Haskell "a little harder ... than it should be"。
- **类型安全认知成本**（原话）："It's not always clear if the cognitive load is net-reduced"；建议把 singleton/type-level complexity 封装在边界，仅当防止不可恢复的安全/数据损坏时才付复杂度。
- [OBSERVED] **未找到交易失败 post-mortem 或 matching-engine 性能基准**。

---

## 横向对比表

| 案例 | 设计中心类型 | provider/后端异构表达 | 副作用解释层数 | unknown/失败建模 | 生产规模证据（一手） |
|---|---|---|---|---|---|
| **Haxl** | `GenHaxl u w a`（monad）+ `DataSource u req`（class）；`Result = Done\|Throw\|Blocked` | type class 实例 + `Map TypeRep (BlockedFetches u)` 动态分桶，存在量化（开放世界） | 2 层：描述(`BlockedFetch`/`RequestStore`) → 调度(`runHaxl`，sync/async/background 三驱动) | `Either SomeException a`；`ResultVal = Ok\|ThrowHaxl\|ThrowIO`；AsyncFetch 异常"发出与否不可判定"作者承认未解决 | Meta 反滥用 Sigma 线上；ICFP 2014 |
| **SC Mu/Cortex** | `Work_ = WorkPure\|WorkApp\|WorkIO`（计算 DAG）+ `Work a` 壳 | 执行后端外置（顺序/并行/Shepherd），核心不含后端；`Collect`/`Workable` 类型驱动生成 | 2 层：纯 DAG 描述 → `runWork`；effect 分级 `SafeIO`/`IO` | 未定义统一 unknown；错误随各解释器 | 7M 行 Mu，2008 起，千级用户，30 亿美元业务线；ICFP 2024 |
| **Barclays FPF** | payout 深嵌 DSL：核心原语组合子（`observe`/`cond`/`name`/fold），`FPFVal` 约束 | 一份 AST → 多解释器族（pricing/lifecycle/doc/codegen），开放世界；`Double` 多态跨解释 | 2 层：纯 payout AST（可序列化）→ 后端解释 | 无 unknown（payout 确定性映射） | 20k 行框架 + 400 交易类型，~20 用户；JFP 2009 |
| **Composing Contracts** | `Contract`（10 原语）+ `Obs a` 双代数 | 封闭核心构造子 + 值级/模型级开放 observable（`libor`/`Model`），无 provider 类型 | 2–3 层：纯合约 → 抽象 `PR a=DATE→RV a` → 具体 lattice/MC/PDE | 无 unknown ADT（"签约时可能未知"仅语义） | 论文级；LexiFi 商业化 15+ 年（供应商自述）；ICFP 2000 |
| **DAML** | `Update a`（ledger 事务 action 代数）+ template `signatory`/`controller` 授权 | 类型级 action 形状 + 值级 Party/authorization evidence；backend=Canton 拓扑/LF interpreter | 3 层：`Update` do-block → LF interpreter 授权检查 → transaction commit/abort | `lookupByKey → Update (Optional ..)`；`abort :: Text → m a`（文本失败） | 可部署 Canton/LF 运行链；无客户/规模一手数字；commit f3b4b34 |
| **Marlowe** | 闭合 6 构造子 `Contract` + `Value`/`Observation`/`Action` 三子代数 | 无 provider 类型；异构轴=值级 `Party`/`Token`/`Accounts`（封闭操作、开放资产身份） | 3 层：纯 `computeTransaction` → `Payment` intent → 链上验证器实际转账 | `TransactionOutput \| Error TransactionError`（枚举失败原因，无 catch-all） | mainnet 部署 + 8/18-tx 案例（无总量）；commit 99f432d |
| **cardano-ledger STS** | `class STS a`（associated types `State/Signal/Environment/PredicateFailure` + rule AST） | type-level 开放世界，era 参数化 instance；`Embed sub super` 规则嵌套 | 3 层：规则 free-monad AST → `BaseM` interpreter → node 消费 state/failures/events | `Either (NonEmpty (PredicateFailure s)) (State s)`；`STSResult{state,failures,events}` | 进入每个 cardano-node release（≥8.0）；无量级数字；commit 2c33b4f |
| **Tsuru Capital** | **交易核心未公开**；相邻公开=`Iteratee s m a`/`Enumeratee`/Franz 日志 | 未知（无 broker adapter 公开）；Franz `FranzPath` 封闭 local/remote | 流处理层：`Enumerator→Enumeratee→Iteratee→run`，`m` 参数化外层 | `Stream c = EOF (Maybe SomeException) \| Chunk c`；`FranzException` 枚举 | HCAR 2011 全 Haskell 交易；现官网主用 Rust；证据中/弱 |
| **Mercury** | `Workflow a`（确定性、可重放）+ `Activity env a`（隔离 IO 外壳） | 类型级 `ActivityArgs/Result` + existential codec；Worker `HashMap Text` 值级开放 | 平台层：`Workflow` 纯函数 over event history → `executeActivity` → Activity IO；崩溃重放确定性前缀 | `RetryState`/`ApplicationFailure`/`ActivityFailure`；`WorkflowExit{Success\|Failed}`；建议 idempotency keys | ~2M 行 Haskell，300k 企业，$248B/2025 交易额；commit 2270909 |
| **Bitnomial** | **matching/clearing 核心未公开**；公开=类型级 `JsonRpc method p e r` API contract | 值级多协议（BTP/FIX/REST/WS/CQG）+ 封闭 endpoint 类型树；`HmacAuth` 类型级算法/身份 + 值级 key | API 层：`RpcHandler = p → m (Either (JsonRpcErr e) r)`；HMAC middleware | `JsonRpcResponse = Result\|Ack\|Errors`；`HmacError` 枚举（transport-level，非金融 unknown） | CFTC DCM/DCO 时间线；"millions of trading accounts"（员工访谈）；证据中 |

**读表要点**：①十案例无一以"业务对齐大 object"为中心，全部是**小核心代数/monad + 外置解释器**。②真正建模了"不可丢、有资金后果的写 + 外部不可判定 + 审计/replay"（UTA 效应簇）的只有 **Marlowe（payment intent 出语义外）、DAML（授权+原子 abort）、Mercury（durable execution + 重放 + 幂等键）** 三家；Haxl/Mu/FPF/Contracts 都是纯读/纯计算域，UTA 不能从它们直接抄效应簇。③provider 异构**无一进核心类型**：要么 type class 实例（Haxl/STS），要么值级资产/参与者（Marlowe/Contracts），要么执行后端外置（Mu）。④"运行期才知道的能力"（UTA F6）在这些系统里**基本没有对应**——它们的能力多在 compile-time/构造期已知；DAML 的 contract-keys "3.x 不支持"、时间不单调是最接近 UTA F6/F11 的一手承认。

---

## 对 UTA 的可迁移命题

> 格式：案例 X 在条件 Y 下用机制 Z 解决了问题 W；UTA 若满足 Y 可迁移，否则不可。不发明 UTA 字段/对象。逐条挂案例出处。

**观察簇 / provider 异构 / 增量计算**

- **M1（Haxl）**：多异构数据源、逻辑想写成顺序但要并发批量（Y）下，用 `DataSource u req` type class + `Map TypeRep (BlockedFetches u)` 动态分桶（Z）解决"新增 provider 不改核心调度"（W）。UTA 满足"provider 开放、核心只消费统一内部协议、每 provider 独立声明取数方式"（B6/P1）可迁移；若核心类型要枚举全部 venue 则不可。
- **M2（Haxl）**：用 `Blocked ivar cont` + `ResultVar` 回调（Z）把未完成外部效果表示成一等续延+结果槽，解决"结果未回时如何挂起顺序逻辑"（W）。UTA 观察消费声明 await-all/latest+窗口/ordered（B5）可迁移两层结构；但 `AsyncFetch` 异常"inner io 是否执行不可判定"（Fetch.hs:508-510）恰是 F5 unknown 的同构未解——迁移时不能指望框架判定，必须显式建模 unknown（C1）。
- **M3（SC Mu）**：相互依赖的计算、要多执行后端且增量重算（Y）下，用 `Work_` 三构造子 DAG + `Key` 共享 + 描述/执行解耦（Z）解决"编排与执行耦合、全量重算"（W，对应 B3 不接受每 bar 全量重跑）。UTA 若把程序指标计算建模成可共享节点依赖图，可迁移"纯依赖图 + 多后端解释 + 只重算脏依赖"；前提计算可表达为 acyclic 依赖。
- **M4（SC Mu / DAML / Mercury）**：三家都用**类型分级区分"可重跑的读"与"有后果的写"**——Mu `SafeIO` vs `IO`、DAML `Update`（受授权约束）vs 纯计算、Mercury `Workflow`（禁 IO，`MonadIO` 被 TypeError 拒）vs `Activity`（唯一 IO 外壳）（Z）。UTA 观察簇/效应簇二分（§1.5）若在类型层落地，可迁移"effect 分级不靠约定靠类型"；Mercury 的 `TypeError` 拒绝 IO 是最强形式。

**效应簇 / 意图-尝试-回执-对账 / unknown / 审计 / replay**（UTA 最缺、最需要的部分）

- **M5（Marlowe，最相关）**：合约要上链验证、"决定发生什么"与"真正转账"必须分离（Y）下，用**纯 `computeTransaction` 产出 `Payment` intent、真实转账"outside of the semantics"由链验证器执行**（Z）解决"核心不碰协议、只消费统一内部表示"（W，正对应 B6）。UTA 满足"集成层洗协议、核心消费统一内部协议、意图≠对 venue 的调用"（B6/P6/P8）可迁移这种"纯语义产出 intent、执行在边界"的分层；且 Marlowe `TransactionError` 枚举失败原因、无 catch-all（对应 C13 无"其他→rejected"）。
- **M6（DAML）**：多方、写必须原子且需授权证明（Y）下，用 **`Update` do-block 组合 + 每 action 授权检查 + 缺授权 abort 整个 transaction + `signatory`/`controller` 关联"谁授权谁"**（Z）解决"副作用怎么关联、谁和谁能关联"（W，正对应 B7/K8/S8）。UTA 满足"每个意图带 principal 与依据、授权按 (principal,账户,操作种类)"（C3/C11）可迁移"授权是随 action 携带的 evidence、不从更早上下文继承"的模型；DAML 的 `Optional`（而非 `Unknown`）与 `abort :: Text` 则是**反面参照**——UTA 的 unknown 有资金后果，不能压成 `Optional`/文本失败（对应 C1/C13）。
- **M7（Mercury，最相关）**：不可丢的写、崩溃要恢复、重试/超时/取消要统一（Y）下，用 **`Workflow`（对 event history 的纯函数，可重放确定性前缀）+ `Activity`（隔离副作用）+ idempotency keys + `RetryState`/`ActivityFailure` 显式建模**（Z）解决"手写分布式状态机 fragile、重试/超时散落"（W——Mercury 原话 "disproportionate share of our operational incidents"，正对应 UTA O11 的 E5/E19/E20）。UTA 满足"程序状态跨重启恢复、意图先持久化再尝试、unknown 不自动重试"（H9/C1）可迁移"确定性重放 + 副作用隔离在 Activity + 关键写用幂等键"；但 Mercury 靠平台记录 event history 保证重放，UTA 的 Rust/Wasm 无 live snapshot（rust-feasibility 证据缺口）——迁移前提是**程序逻辑状态显式可序列化**，否则不可。
- **M8（cardano-ledger STS）**：账本状态转移要可组合、可嵌套、失败要结构化聚合（Y）下，用 **`STS a` 的 `(Environment,State,Signal) → Either [PredicateFailure] State` + `Embed` 嵌套 + `PredicateFailure` 封闭 sum**（Z）解决"每个规则独立可测、子规则失败结构化上浮"（W）。UTA 的效应簇状态机（意图→决定→尝试→回执→对账）若建模成带环境的转移系统，可迁移"规则=纯转移 + 失败是枚举的 `PredicateFailure` 而非异常"；`fail-closed`（规则否决记 P7）与 STS 的 `Either [PredicateFailure]` 同构（对应 C12）。

**设计中心方法论 / 人因（贯穿）**

- **M9（Composing Contracts / Marlowe / FPF，三证一致）**：同一描述要被多用途消费（Y）下，都用 **deep embedding：把领域构造具化成小代数值/AST，业务语义留给解释器族**（Z）解决"每加下游用途就改业务对象"（W，正对应维护者"每写一个巨大 object 都是维护天坑"）。UTA 若让集成层洗出的统一内部协议是**可被多个 effect 消费者解释的核心值**（B6/B7）可迁移。三家一致的取舍：**核心代数越小越封闭，静态分析/穷尽/可预算越强**（Marlowe 6 构造子上链、FPF 非 Turing 完备、Contracts 刻意拆小原语）——对 UTA 的不可信 AI 程序（H2/C4）尤其相关。
- **M10（FPF / Mercury / Bitnomial，人因代价一致）**：FPF 放弃 point-free 组合子（量化分析师读不懂）、Mercury "全部不变量编码→跨 50 模块数周重构 / has not been painless"、Bitnomial "not always clear if cognitive load is net-reduced 建议把类型级复杂度封装在边界"（Z 的反面）。迁移命题：**核心代数要可组合，但面向不可信/低水平作者（AI 程序，H2）的那层语法必须朴素，类型级复杂度封装在边界**，否则组合性红利被人因代价吃掉。
- **M11（DAML 反例）**：DAML `lookupByKey → Optional`、`abort :: Text`、contract-keys "Canton 3.x 不支持"、时间"不保证单调、不应解释为 precise timestamp"（time.rst）——**这些恰是 UTA F6/F11/F5 的一手佐证：一个成熟 ledger 系统也承认幂等/唯一性/时间语义按后端变化且部分做不到**。UTA 不可把 unknown 当 `Optional`、不可假设 venue 时间单调（对应 F11/C6）。

## 未覆盖与开放问题

- **效应簇的完整一手样板仍有限**：只有 Marlowe/DAML/Mercury 真正建模了"有资金后果的写 + 授权 + 恢复"，但三家都不完全等于 UTA 场景——Marlowe/DAML 是**链上确定性验证**（有全局账本共识，UTA 没有），Mercury 靠**平台记录 event history**（UTA 的 Rust/Wasm 无 live snapshot）。UTA F5"发出后不可判定 unknown"在这三家里都被"链共识/平台重放"消解掉了，**没有一家直接给出 UTA 式 unknown（既非成功也非失败、不得盲重试）的成熟建模**——这是本组调查的最大缺口。
- **"There is no Fork" 论文 PDF 正文**未逐页打开（URL read 禁用，curl PDF 未逐页读）；Haxl 论文级论断以官方 abstract 页 + 源码为准，源码结论均 pin commit+行号。
- **LexiFi MLFi 论文**（MLFiPaper.pdf）curl 返回 `AccessDenied` **未打开**；LexiFi 部分仅用官方网页 + Composing Contracts 论文的 lineage 佐证，规模为供应商自述、非第三方可审计。
- **Tsuru / Bitnomial / Mercury 的交易/账本核心类型未公开**：三案的②设计中心只能给"相邻公开系统"的真实签名，核心本身诚实标注"未公开"。**未找到一手后悔**的项：STS 设计后悔、Tsuru 迁移 Rust 原因、Bitnomial 交易 post-mortem、Mercury 支付账本 post-mortem——均已在各案例标注，不推导。
- **能力矩阵"运行期才知道"（UTA F6）无强对应**：本组系统的能力多在 compile-time/构造期已知；最接近的是 DAML contract-keys 的版本级不支持声明。若有其他工作流负责 Scala/effect-system/streaming 案例（见 peer `ScalaJvmCases`/`StreamsIncrementalFRP`/`EffectCompositionTheory`），运行期 capability 探测的证据宜在那里补。

## 来源清单

| # | 来源 | URL | 打开状态 | 本地路径/commit |
|---|---|---|---|---|
| S1 | facebook/Haxl 源码 | github.com/facebook/Haxl | ✅ clone 并逐文件读 | /tmp/haxl @ `b33c1c1f132fa1d704cc0f7b6233f9c86bb50afe` |
| S2 | Marlow et al., *There is no Fork*, ICFP 2014 | simonmar.github.io/bib/nofork-2014_abstract.html | ⚠️ abstract 页经搜索摘要打开；PDF 正文未逐页读 | — |
| S3 | Dijkstra/Magalhães/Néron, *Functional Programming in Financial Markets*, ICFP 2024 | dreixel.net/research/pdf/fpfm.pdf | ✅ 全文已打开（curl→PDF） | /tmp/scmu.pdf |
| S4 | Frankau et al., *Going functional on exotic trades*, JFP 2009 | www2.dmst.aueb.gr/dds/pubs/jrnl/2008-JFP-ExoticTrades/html/FSNB08.html | ✅ 作者托管 HTML 全文已打开 | /tmp/fpf.html |
| S5 | Peyton Jones/Eber/Seward, *Composing Contracts*, ICFP 2000 | www.cs.tufts.edu/comp/150FP/archive/simon-peyton-jones/contracts.pdf | ✅ canonical PDF 逐页打开、核对 Figure 2/3 | /tmp/cc1.pdf |
| S6 | *How to write a financial contract*（修订章节，标注版本边界用） | www.cs.utexas.edu/~novak/pj-eber.pdf | ✅ 已打开（委派） | — |
| S7 | LexiFi 官方：CDL 博客 / Algebra / FAQ / About / Lifecycle | lexifi.com/blog/structured-thoughts/contract-description-language/ 等 | ✅ 已打开（委派 curl） | — |
| S8 | LexiFi MLFiPaper.pdf | lexifi.com/files/resources/MLFiPaper.pdf | ❌ curl 返回 `AccessDenied`，未打开，非依据 | — |
| S9 | Digital Asset DAML 源码/文档 | github.com/digital-asset/daml | ✅ clone 按 commit 读（委派） | @ `f3b4b347208490fa8b97d67c0ba84fa6317c3663` |
| S10 | DAML 官方 Updates reference | docs.daml.com/daml/reference/updates.html | ✅ 已打开（委派） | — |
| S11 | Marlowe specification（Isabelle + Haskell 参考 + audit response） | github.com/input-output-hk/marlowe | ✅ clone 按 commit 读（委派） | @ `7b5b1e900ec53a8eb18747992bec73470704dfcb` |
| S12 | Marlowe on Cardano（语义/类型/示例） | github.com/input-output-hk/marlowe-cardano | ✅ clone 按 commit 读（委派） | @ `99f432d8ef9dbd1b52b7fa089254de15913b490f` |
| S13 | Marlowe go-live 公告 | marlowe.iohk.io/blog/marlowe-goes-live-... | ✅ 已打开（委派 curl）；仅用于部署事实 | — |
| S14 | cardano-ledger 源码（small-steps + Shelley rules + docs） | github.com/IntersectMBO/cardano-ledger | ✅ clone 按 commit 读（委派） | @ `2c33b4f858c0e62b300d121996a479f505d8c0e5` |
| S15 | Tsuru: HCAR 21st ed.（2011） | haskell.org/communities/11-2011/html/report.html | ✅ 已打开（委派 curl） | — |
| S16 | Tsuru: "Iteratees at Tsuru" 博客 | blog.kfish.org/2011/09/iteratees-at-tsuru.html | ✅ 已打开（委派 curl） | — |
| S17 | Tsuru 官网 / iteratee / franz 源码 | tsurucapital.com；github.com/tsurucapital/{iteratee,franz} | ✅ 已打开（委派） | iteratee @ `352448d`；franz @ `6d27cd3` |
| S18 | Mercury: "A Couple Million Lines of Haskell" | blog.haskell.org/a-couple-million-lines-of-haskell/ | ✅ 已打开（委派 curl） | — |
| S19 | Mercury hs-temporal-sdk 源码/README | github.com/MercuryTechnologies/hs-temporal-sdk | ✅ clone 按 commit 读（委派） | @ `2270909b35944823fe57543009b661877bc19319` |
| S20 | Bitnomial 官方 About/Exchange/API + Serokell 员工访谈 | bitnomial.com/{about,exchange}；serokell.io/blog/...bitnomial | ✅ 已打开（委派 curl） | — |
| S21 | Bitnomial servant-jsonrpc / wai-middleware-hmac-auth 源码 | github.com/bitnomial/{servant-jsonrpc,wai-middleware-hmac-auth} | ✅ clone 按 commit 读（委派） | servant-jsonrpc @ `bfc17a8`；hmac-auth @ `ee3f8b0` |
