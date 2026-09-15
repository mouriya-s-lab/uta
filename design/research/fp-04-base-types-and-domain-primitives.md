# FP 生态中的基础类型、异构能力与组合式消费：真实案例调查

## 摘要

本组案例中最常见的设计中心是“可组合的值/程序/状态转换代数”（`Dense currency`、`Quantity`、`AsyncResult`、`m i j a`、`'a t`、`Rule/Validation`），它们把业务拆成有边界的构造器与解释器/处理器，而不是把字段对齐到一个巨大的对象。
- Haskell/F# 案例优先把不变量放进 sum/product 类型；Scala 案例多以 typeclass、数据类型与组合子承载关系。
- provider/能力差异通常是“解释器能否解释某个操作”的值级证据，或由模块/API 边界延迟到运行时；只有 indexed monad 等少数路径把状态转移约束到类型级。
- 观察/事件消费的核心不是“对象字段更多”，而是 DAG、`Rule`、`Validated`/`Ior`、watermark 和可解释的事件列表。
- 精确金额、单位、时间、因果顺序和未知结果都被当成独立原语；`Double`、`String`、`Bool` 与 `Maybe` 的滥用是反例。
- 这些材料不会给 UTA 设计方案；末节只给带前提的迁移命题。
- 证据分为 `[OBSERVED]`（本次打开的一手源码/作者文档）和 `[INFERENCE]`（由源码组合出的分析，不冒充作者原话）。

## 范围与筛选标准

1. **问题边界。** 只考察“值如何保持语义”“能力如何暴露”“副作用如何被组合和解释”“未知/部分结果如何表达”四个相交面。金融系统、交易工作流、流处理和通用 FP 原语都可入选，但必须有可读的一手实现或作者文档。
2. **设计中心判定。** 只有当一个核心类型/代数能解释主要组合方式时，才称为 design center：例如 `Quantity[A]` 约束同维度运算，`'a t` 是增量计算图节点，`m i j a` 是带状态索引的程序，`Validated[E,A]` 是可积累失败的应用结构。一个仅包含所有字段的 record/class 不算设计中心。
3. **案例固定六问。** 每个案例按①系统/规模，②设计中心，③异构能力，④副作用组合与消费，⑤基础类型，⑥已知问题/代价回答。找不到证据时明确写“未找到”，不以常识补齐。
4. **规模证据。** README 的项目目标、仓库发布/多语言包、论文明确写出的生产实现可作为 `[OBSERVED]`；“社区常用”“生产级”若源码没有原话不作结论。Cardano、CCXT、QuantLib、Flink/Dataflow 有公开规模或工程使用证据；DMMF 示例则明确标为未维护的样例。
5. **来源纪律。** 仓库均以本次 shallow clone 的 commit 固定；作者文章、官方文档、PDF 均已下载/打开。报告中的行号是本地文件行号；论文另附页码/章节。搜索摘要不作为主要依据。
6. **不构造 UTA。** 不定义 UTA 的字段、对象、状态机或 provider 接口。末节每条命题都用“案例 X 在条件 Y 下用机制 Z 解决问题 W；UTA 若满足 Y 可迁移，否则不可”的条件式表述。
7. **负例。** QuantLib 和 CCXT 保留为“成功的生产系统也会遇到的对象/统一结构成本”，不把它们的字段集合当推荐设计。

## 逐案例分析

### 案例 1：`safe-money`（Haskell）：类型级货币/单位与运行时序列化边界

#### ① 系统做什么、规模与生产使用证据

- `[OBSERVED]` `safe-money` README 将其定义为对“all world currencies including fiat currencies, precious metals and crypto currencies”的 type-safe、lossless 金额库，并列出 `aeson`、`cereal`、`serialise`、`store`、`xmlbf` 等序列化集成（`safe-money/README.md:1-26`）。
- `[OBSERVED]` 公开源码覆盖金额、单位尺度、汇率和序列化 existential wrapper；这是可复用库的证据，但本次没有打开采用它的交易所或银行生产系统，不能据此声称生产规模。
- `[INFERENCE]` 它适合把“精确值”和“外部动态值”分在两侧：核心运算使用类型级 currency/scale，序列化时才进入运行时字符串检查。

#### ② 设计中心是什么

- `[OBSERVED]` 核心值是 `newtype Dense (currency :: Symbol) = Dense Rational`（`safe-money/src/Money/Internal.hs:158-185`）。`Dense currency` 将 currency 作为类型参数，把精确 `Rational` 计算留在同一货币内。
- `[OBSERVED]` 离散单位是 `type Discrete currency unit = Discrete' currency (UnitScale currency unit)` 与 `newtype Discrete' (currency :: Symbol) (scale :: (Nat, Nat)) = Discrete Integer`（同文件:320-351）。
- `[OBSERVED]` 跨货币转换的中心代数是 `newtype ExchangeRate (src :: Symbol) (dst :: Symbol) = ExchangeRate Rational`，并为其提供 `Category` 组合、倒数和 `exchange :: ExchangeRate src dst -> Dense src -> Dense dst`（同文件:726-775, 858-875）。
- `[INFERENCE]` 因而 design center 不是 `Money {amount,currency,...}` 聚合对象，而是“带静态索引的精确量 + 可组合转换箭头”。

#### ③ 多 provider/异构能力如何表达

- `[OBSERVED]` 该库本身不是 provider adapter；不同 currency/unit 不是枚举在一个对象里，而是 `Symbol`/type family `UnitScale currency unit`。示例包括 `UnitScale "USD" "cent" = '(100,1)`、`"dollar" = '(1,1)`、XAU 的任意实践尺度（同文件:593-647）。
- `[OBSERVED]` 运行时未知货币通过 `SomeDense`、`SomeDiscrete`、`SomeExchangeRate` 进入；`withSomeDense :: SomeDense -> (forall currency. KnownSymbol currency => Dense currency -> r) -> r` 在边界重新引入 existential type（同文件:893-974）。
- `[INFERENCE]` 这是“开放的字符串输入、封闭的已验证内部值”分层，而不是预先枚举所有 venue。能力差异没有 provider-level 证据；本案例不能支持“类型自动发现 provider 能力”。

#### ④ 副作用如何描述、组合、解释与消费

- `[OBSERVED]` 核心模块是纯值构造/组合；`Category` 的 identity、associativity 和 rate multiplication 明确写入注释（同文件:739-771）。序列化是单独的 `Some*` 边界，不改变核心 `Dense` 的类型。
- `[OBSERVED]` `discreteFromDense` 返回 `(Discrete' currency scale, Dense currency)`，用余数保留不能落入离散单位的精确部分（同文件:541-569）；这不是把舍入副作用藏在金额对象中。
- `[INFERENCE]` 消费端应在“需要离散化/传输”的边界解释近似；核心代数的消费是普通函数应用。该库没有订单回执、重试或未知副作用语义，不能外推这些能力。

#### ⑤ 基础类型设计

- `[OBSERVED]` `Dense` 使用 `Rational`，文档明确警告转换 `Float`/`Double` 是有损的（同文件:179-183）。它故意不成为 `Fractional`，因为 `recip`/`/` 可能发散，编译期错误 `ErrFractionalDense` 要求调用方显式转成 `Rational`（同文件:221-235）。
- `[OBSERVED]` `Scale = Scale Rational`，构造器只接受正且非零的 rational；`GoodScale`/`UnitScale` 把合法尺度推进 type level（同文件:573-591, 645-719）。
- `[OBSERVED]` `ExchangeRate src dst` 的正、非零约束由安全构造器建立；`SomeDense` 的 `Ord` 明确警告不会跨货币比较金额，只用于排序容器（同文件:893-905）。
- `[INFERENCE]` 这些类型把 amount、currency、unit、conversion direction 和 ordering 分开，避免用一个 `Object` 同时承载不相容的维度。

#### ⑥ 已知问题、后悔、性能/人因代价

- `[OBSERVED]` 作者在源码中承认 `Rational` 到 `Float`/`Double` 会丢精度，并把有损转换责任交还调用方（同文件:179-183）。
- `[OBSERVED]` `Some*` wrapper 为了兼容序列化使用 `String`，其 `Ord` 语义不等于金融金额排序（同文件:893-905, 992-1007）。这是运行时边界的明确代价。
- `[OBSERVED]` `discreteFromDense` 必须选择近似策略，并返回余数以避免静默丢钱（同文件:541-569）。本次未找到作者针对大型系统吞吐量或 API 人因的 post-mortem。

### 案例 2：Squants（Scala）：维度代数、MoneyContext 与能力落到运行时

#### ① 系统做什么、规模与生产使用证据

- `[OBSERVED]` README 将 Squants 定义为 quantities/units/dimensional analysis 的 API，强调 immutable、thread-safe，并提供维度运算（`squants/README.md:1-8,74-111`）。仓库带有公开版本和 Sonatype 依赖配置，但本次未找到具体生产部署规模。
- `[OBSERVED]` `Money` 是库中对购买力的数量建模，README 还给出 `Price = Money / Quantity` 的组合（README:494-524）。
- `[INFERENCE]` 它是“可组合量值库”，不是交易执行系统；不能把 `MoneyContext` 当成 provider registry 的生产实现。

#### ② 设计中心是什么

- `[OBSERVED]` `abstract class Quantity[A <: Quantity[A]] extends Serializable with Ordered[A]`，要求 `value: Double`、`unit: UnitOfMeasure[A]`、`dimension: Dimension[A]`，并定义同维度 `+`、`-`、标量乘除与转换（`shared/src/main/scala/squants/Quantity.scala:16-83,260-273`）。
- `[OBSERVED]` `trait UnitOfMeasure[A <: Quantity[A]]` 给出单位构造与 conversion function（`UnitOfMeasure.scala:12-72`）。
- `[INFERENCE]` design center 是“Quantity + UnitOfMeasure + Dimension”的代数；Money 只是一个具体 quantity，不是所有业务字段的根类。

#### ③ 多 provider/异构能力如何表达

- `[OBSERVED]` Money 的 currency 是运行时 `Currency`：`abstract class Currency(val code: String, val name: String, val symbol: String, val formatDecimals: Int) extends UnitOfMeasure[Money]`，仓库提供 USD/EUR/JPY 等对象（`market/Money.scala:421-497`）。
- `[OBSERVED]` 跨币种运算不能在编译期判定；`+`/`-` 同币种路径直接运算，跨币种要通过隐式 `MoneyContext`，没有汇率时抛 `NoSuchExchangeRateException`（`Money.scala:19-51,82-130`）。
- `[OBSERVED]` `MoneyContext` 可携带 default currency/rates，更新频率由应用控制（README:567-654）。这是值级配置/运行时证据，不是 type-level provider capability。

#### ④ 副作用如何描述、组合、解释与消费

- `[OBSERVED]` 数量运算本身是纯方法；`mapAmount: BigDecimal => BigDecimal` 保持 currency，README 专门说明该路径保持 BigDecimal precision（`Money.scala:381-387`; README:338-346）。
- `[OBSERVED]` FX 的解释由 `MoneyContext` 的 rate 查找完成；缺 rate 是异常而不是 `Either`/命名结果（`Money.scala:19-45`）。
- `[INFERENCE]` 该库把“单位/汇率环境”作为解释器上下文，但没有事件关联、回执、重试或未知结果的效果代数；它只能说明局部量值如何消费，不能回答交易副作用编排。

#### ⑤ 基础类型设计

- `[OBSERVED]` 核心 `Quantity` 的 `value` 是 `Double`，而 Money 的主构造器使用 `BigDecimal`；generic `Numeric` 工厂路径会 `num.toDouble`（`Money.scala:393-405`）。
- `[OBSERVED]` Money 的 currency 是封闭的 Scala 对象目录；同维度比较/相加由 `Quantity` API 约束，README 明确说不同 dimension 不可比较/相等/相加（README:74-111）。
- `[OBSERVED]` `Price` 作为 `Money / Quantity` 的派生量展示了 product/quotient 组合，而不是一个含 amount/unit 的大记录（README:520-524）。

#### ⑥ 已知问题、后悔、性能/人因代价

- `[OBSERVED]` Squants 源码明确承认跨币种运算 compile-time 不可判定；缺汇率会抛异常（`Money.scala:19-45`）。
- `[OBSERVED]` API 同时存在 Double 与 BigDecimal；Double 路径有精度代价，README 的 `mapAmount`/格式化说明提醒精确值与展示舍入不同（README:338-346,555-565）。
- `[INFERENCE]` MoneyContext 是一个可用但隐式的全局/作用域假设；若多个 provider/账户同时存在，调用点仅凭类型看不出 rate 来源。本次未找到维护者对此做 post-mortem。

### 案例 3：F# Domain Modeling Made Functional：阶段类型、工作流与可选副作用

#### ① 系统做什么、规模与生产使用证据

- `[OBSERVED]` `DomainModelingMadeFunctional` 的源码是书籍配套示例；README 明确说代码“不 actively maintained”，`src/OrderTaking` 是完整 bounded context 的 workflow（`dmmf/README.md:1-8`）。因此它是教学/样例系统，不是生产规模证据。
- `[OBSERVED]` Wlaschin 的一手文章把简单状态机定义成“每个状态有自己的 type，再由 union class 表示全体状态”（`fsharp-site/.../designing-with-types-representing-states/index.md:151-175`）。
- `[INFERENCE]` 价值在于公开展示“每一步改变类型”的设计方法，而不是规模或吞吐量。

#### ② 设计中心是什么

- `[OBSERVED]` 状态中心示例为：

  ```fsharp
  type ActiveCartData = { UnpaidItems: string list }
  type PaidCartData = { PaidItems: string list; Payment: float }
  type ShoppingCart =
      | EmptyCart
      | ActiveCart of ActiveCartData
      | PaidCart of PaidCartData
  ```

  （作者文章:163-171）。
- `[OBSERVED]` 订单 workflow 的中心不是单一 `Order` record，而是阶段函数：`ValidateOrder = ... -> UnvalidatedOrder -> AsyncResult<ValidatedOrder, ValidationError>`、`PriceOrder = ... -> ValidatedOrder -> Result<PricedOrder, PricingError>`、`CreateEvents = PricedOrder -> OrderAcknowledgmentSent option -> PlaceOrderEvent list`（`PlaceOrder.Implementation.fs:44-76,105-118`）。
- `[INFERENCE]` design center 是“阶段化函数组合 + union state”，records 只承载某一阶段有效的 product，不是 giant object。

#### ③ 多 provider/异构能力如何表达

- `[OBSERVED]` 外部依赖以函数参数注入：`CheckProductCodeExists`, `CheckAddressExists`, `GetProductPrice`, `SendOrderAcknowledgment` 等；`ValidateOrder`/`PriceOrder` 的类型签名要求调用者提供具体能力（`PlaceOrder.Implementation.fs:25-76`）。
- `[OBSERVED]` provider 失败由 `AddressValidationError = InvalidFormat | AddressNotFound`、`ValidationError`、`PricingError` 等 ADT 表达，而不是把不同 provider 的字段塞进 record。
- `[INFERENCE]` 这是开放世界的值级 dependency injection：增加 provider 是传入另一个函数；增加业务操作则需要新阶段类型/错误类型，编译器暴露调用点。

#### ④ 副作用如何描述、组合、解释与消费

- `[OBSERVED]` 验证使用 `AsyncResult`，定价使用 `Result`；整体 `placeOrder` 在 `asyncResult` block 中依次 `let!` 验证、定价，再把 acknowledgement 结果映射到 event（`PlaceOrder.Implementation.fs:251-289,312-333,413-435`）。
- `[OBSERVED]` `SendResult = Sent | NotSent`；发送失败在此 workflow 不生成错误，而是 `AcknowledgeOrder ... -> OrderAcknowledgmentSent option`，`Sent` 产生 `Some event`，`NotSent` 产生 `None`（同文件:94-109,340-358）。
- `[OBSERVED]` `CreateEvents` 通过 `Option.map`、`listOfOption` 与 union event constructors 关联 `PricedOrder` 和 acknowledgment event（同文件:364-407）。
- `[INFERENCE]` 这里显式展示“谁和谁关联”：同一 `OrderId` 穿过阶段；消费是解释器函数把阶段结果变成事件列表，而不是在对象内部自动发通知。`NotSent -> None` 仍可能不足以表达未知，这是后文的风险。

#### ⑤ 基础类型设计

- `[OBSERVED]` `CheckedAddress = CheckedAddress of UnvalidatedAddress` 是解析后的证明；`HtmlString = HtmlString of string` 是避免任意 string 误用的 wrapper；`OrderQuantity`、`Price`、`BillingAmount` 在独立模块定义（`PlaceOrder.Implementation.fs:30-38,83-89,295-330`）。
- `[OBSERVED]` 状态数据按有效状态分开：`ActiveCartData` 没有 `Payment`，`PaidCartData` 没有 `UnpaidItems`；`makePayment : ShoppingCart -> float -> ShoppingCart` 对每个 union case 穷尽处理（作者文章:175-231）。
- `[OBSERVED]` 事件层以 `OrderAcknowledgmentSent option` 与 `PlaceOrderEvent list` 表达可选发送与一个订单可能产生的多个事件（实现:105-118,384-407）。

#### ⑥ 已知问题、后悔、性能/人因代价

- `[OBSERVED]` 作者承认业务逻辑“is complicated”，但认为复杂性必须被类型显式表达；类型变化会产生 breaking changes（Wlaschin 文章:144-160）。
- `[OBSERVED]` 作者特别警告生产代码不要用 `option.Value`，必须 match 处理两种情况（同文章:138-141）。
- `[OBSERVED]` 示例 README 明确未维护；不要把它当生产可靠性证明。
- `[INFERENCE]` `NotSent` 与 `None` 将“明确未发”“发送失败”“发出但无确认”压成同一路径；这正是效果/未知语义不能只靠 `Option` 的警示，而非该项目声称的完整方案。

### 案例 4：Haskell `indexed`：以索引约束状态转移

#### ① 系统做什么、规模与生产使用证据

- `[OBSERVED]` `indexed` 暴露 `IxFunctor`、`IxApplicative`、`IxMonad` 等接口，是 Haskell 的 indexed monad/functor 组合库（`indexed/Control/Monad/Indexed.hs:12-47`; `indexed/Data/Functor/Indexed.hs:20-50`）。
- `[OBSERVED]` 本次打开的源码能证明 API 存在，但没有库 README 或采用方给出生产规模；不要把它等同于 Cardano 的生产系统。

#### ② 设计中心是什么

- `[OBSERVED]` 核心签名是：

  ```haskell
  class IxApplicative m => IxMonad m where
    ibind :: (a -> m j k b) -> m i j a -> m i k b
  (>>>=) :: IxMonad m => m i j a -> (a -> m j k b) -> m i k b
  ```

  （`Control/Monad/Indexed.hs:25-35`）。
- `[INFERENCE]` `m i j a` 是“从索引 i 到索引 j、产出 a 的程序”；组合时中间索引 `j` 必须统一。因此 design center 是状态转移代数，而不是状态对象 record。

#### ③ 多 provider/异构能力如何表达

- `[OBSERVED]` `indexed` 不定义 provider；异构能力可由不同 `m` 实例/索引类型表达，但本源码未提供 capability discovery、open-world registry 或 provider negotiation。
- `[INFERENCE]` 它天然适合封闭的状态轨迹：新增状态/非法边会在类型组合处暴露；对于开放 provider，可将 provider 作为解释器参数，但这一步不是该库源码提供的现成机制。

#### ④ 副作用如何描述、组合、解释与消费

- `[OBSERVED]` `ibind` 只组合索引转换，不决定 IO 如何执行；`iapIxMonad`、`IxMonadZero`、`IxMonadPlus` 提供不同组合结构（`Control/Monad/Indexed.hs:40-47`）。
- `[INFERENCE]` 执行/解释分层是：先构造 `m i j a` 程序，后由具体 indexed monad interpreter 解释；消费方拿到 `a` 时只能沿合法索引走过来的路径继续组合。
- `[INFERENCE]` 它能表达“P.Read 后才能 P.Acknowledge”类顺序，但不自动表达 provider 的运行时断线、未知回执或慢消费者。

#### ⑤ 基础类型设计

- `[OBSERVED]` 核心基础类型是两个索引 `i/j/k`、结果 `a/b` 和组合箭头；没有内置 money/time/id/error 领域类型。
- `[INFERENCE]` 设计价值在于把 ordering/phase 作为 phantom/type-level index，而不是用 `Bool` 或字符串状态；错误结果需在外层用 `Either`/ADT 或具体 monad 建模。

#### ⑥ 已知问题、后悔、性能/人因代价

- `[OBSERVED]` 源码本身没有作者 post-mortem 或性能数字。
- `[INFERENCE]` 索引越细，构造合法程序的类型负担越大；开放世界 provider 能力若无法在编译期知道，就不能强行伪装成 indexed proof。该限制来自签名本身，不是库作者的负面评价。

### 案例 5：Cardano Ledger：小步状态转移、累积 PredicateFailure 与生产规格

#### ① 系统做什么、规模与生产使用证据

- `[OBSERVED]` Cardano Ledger 仓库自称包含 formal specifications、executable models 和 implementations，并列出 Byron 到 Conway 各 era 的设计/形式规格/实现；README 还说明 Conway 形式化已完成、旧 era 部分完成（`cardano-ledger/README.md:9-43`）。
- `[OBSERVED]` README 提供 Shelley/Mary/Alonzo/Babbage/Conway 的正式规格和测试目录（同文件:14-22,45-86），这是公开的工程/协议规模证据；它不是银行 provider API，但确实是运行中的账本规则模型。

#### ② 设计中心是什么

- `[OBSERVED]` Shelley UTxO 规则的失败代数是：

  ```haskell
  data ShelleyUtxoPredFailure era
    = BadInputsUTxO (NonEmptySet TxIn)
    | ExpiredUTxO (Mismatch RelLTEQ SlotNo)
    | ...
    | FeeTooSmallUTxO (Mismatch RelGTEQ Coin)
    | ValueNotConservedUTxO (Mismatch RelEQ (Value era))
    | WrongNetwork ...
    | UpdateFailure (EraRuleFailure "PPUP" era)
  ```

  （`eras/shelley/impl/src/Cardano/Ledger/Shelley/Rules/Utxo.hs:166-210`）。
- `[OBSERVED]` 通用小步语义以 `Rule sts ctx result`、`PredicateFailure sts`、`Event sts` 组织；`validate` 将 `Validation (NonEmpty (PredicateFailure sts))` 变为规则（`libs/small-steps/.../Extended.hs:239-375`）。
- `[INFERENCE]` design center 是“带规则上下文的状态转移/失败代数”，不是一个包含所有 ledger 字段的 mega-state API。

#### ③ 多 provider/异构能力如何表达

- `[OBSERVED]` Cardano 的差异主要是 era，而不是 provider：`PredicateFailure`、`Event`、`State` 通过 `STS` type family 与 `wrapFailed`/`wrapEvent` 嵌套到上层（`Extended.hs:209-274,716-723`）。
- `[OBSERVED]` 这是封闭且版本化的 era 枚举；新增 era 要增加对应的规则/类型实例，不是运行时任意加入 venue。
- `[INFERENCE]` 对 UTA 类场景可借鉴的是“能力/失败附着于具体规则和阶段”，不能直接迁移其封闭 era 假设到开放 provider 世界。

#### ④ 副作用如何描述、组合、解释与消费

- `[OBSERVED]` `STSInterpreter`/`RuleInterpreter` 返回 `(State s, [PredicateFailure s])`；`STSResult` 还同时携带 `stsResultEvents :: [Event s]`（`Extended.hs:530-547`）。
- `[OBSERVED]` `applySTSOptsEither`/`applySTS` 返回 `Either (NonEmpty (PredicateFailure s)) (State s)`，而 `applySTSIndifferently` 可保留 state 与所有 predicate failures（同文件:592-655）。
- `[OBSERVED]` `failOnNonEmptySet`、`failOnNonEmptyMap` 等组合子将多个验证问题保留为结构化集合（同文件:404-451）。
- `[INFERENCE]` 关联关系是 `Rule -> PredicateFailure/Event -> State`；解释器消费规则，审计/上层消费事件和失败，而不是 observer 在对象间隐式通知。

#### ⑤ 基础类型设计

- `[OBSERVED]` `Coin = Coin {unCoin :: Integer}` 表达交易输出价值，并派生 `Semigroup/Monoid/Group/Abelian`（`libs/cardano-ledger-core/src/Cardano/Ledger/Coin.hs:92-106`）。
- `[OBSERVED]` `TxId = TxId {unTxId :: SafeHash EraIndependentTxBody}`，`TxIn = TxIn !TxId !TxIx`，把交易身份与 UTxO 索引分开（`TxIn.hs:63-70`）。
- `[OBSERVED]` 失败携带 `SlotNo`、`Coin`、`Value era` 和 `Mismatch relation value`，错误的关系方向也成为类型参数/构造器的一部分，而不是字符串（`Utxo.hs:166-189`）。
- `[INFERENCE]` `NonEmpty` failure 与 typed mismatch 同时保留“有问题的集合”和“问题的比较关系”，适合解释与审计，但不能自动表示外部写入 unknown。

#### ⑥ 已知问题、后悔、性能/人因代价

- `[OBSERVED]` README 明确指出各 era 的形式化完成度不同，且仓库称自己为 pre-release software、通常不提供支持（`README.md:28-32,173-177`）。
- `[OBSERVED]` `applySTSInternal` 在多条规则中寻找无 failure 的结果，否则聚合 failures；这是一种为了规则树可解释性而付出的分支/累积复杂度（`Extended.hs:732-757`）。
- `[INFERENCE]` 封闭 era 的穷尽性提高一致性，但跨 era/协议升级会产生大范围类型改动；不要把它当成 provider 开放世界的直接答案。

### 案例 6：Jane Street `Bignum`（OCaml）：精确有理数与显式失败/特殊值

#### ① 系统做什么、规模与生产使用证据

- `[OBSERVED]` Jane Street `bignum` 的接口定义 arbitrary-precision rational numbers，提供 Core/Quickcheck/sexp 集成；仓库是 Jane Street 公共库（`bignum/src/bignum.mli:1-20`）。
- `[OBSERVED]` 本次没有在该公共仓库中找到 `Core.Money` 模块；因此这里只报告通用精确数原语，不能把它宣称为 Jane Street 的货币模型或交易系统。

#### ② 设计中心是什么

- `[OBSERVED]` design center 是抽象 `type t`，构造 `val create : num:Bigint.t -> den:Bigint.t -> t`，另有受 alert 保护的 `create_unchecked`（`bignum.mli:1-26`）。
- `[OBSERVED]` 运算、舍入、序列化、可表示性和特殊值均围绕该 `t` 的接口展开，而不是开放 record（同文件:28-179）。

#### ③ 多 provider/异构能力如何表达

- `[OBSERVED]` 没有 provider 概念。输入差异由 `of_float_decimal` 与 `of_float_dyadic` 显式分开；Quickcheck generator 甚至包含 denominator-zero 值（同文件:17-20,181-220）。
- `[INFERENCE]` 这是一种开放输入、严格解释函数的设计：不同来源的浮点语义必须在调用点选择，不能靠一个 `Float` 字段猜测。

#### ④ 副作用如何描述、组合、解释与消费

- `[OBSERVED]` 运算是纯值操作；`to_string_decimal_accurate : t -> string Or_error.t` 把不能有限十进制表示的情况变成结构化 `Or_error`，而 `to_string_accurate` 保证可 round-trip（`bignum.mli:120-179`）。
- `[OBSERVED]` 除零不抛异常，而返回 `inf/-inf/nan`；`inverse zero = infinity`（同文件:47-64）。
- `[INFERENCE]` 消费者必须显式选择是否接受特殊值、是否调用 `is_real/is_nan/is_infinite`；这把解释策略放在边界，而不是静默丢失信息。

#### ⑤ 基础类型设计

- `[OBSERVED]` numerator/denominator 使用 `Bigint.t`；`round` 的方向是 polyvariant `` `Down | `Up | `Nearest | `Zero | `Bankers ``，整数转换返回 `option` 或 `_exn` 两条路径（`bignum.mli:68-118`）。
- `[OBSERVED]` 有 `is_real`、`is_nan`、`is_infinite` 和精确/人类可读两套字符串表示（同文件:120-179）。
- `[OBSERVED]` 作者警告大多数 rational/decimal 不能精确表示为 float，并废弃模糊的 `of_float`，要求调用方在 decimal/dyadic 之间做明确决策（同文件:181-223）。

#### ⑥ 已知问题、后悔、性能/人因代价

- `[OBSERVED]` 文档警告如 `2 ** 8_000_000` 会占用至少一兆字节，超大数乘法本身很慢（`bignum.mli:55-57`）。
- `[OBSERVED]` “除零返回特殊值而非 error”和“Quickcheck 包含 denominator-zero”要求使用者理解 algebra 的特殊元素；这是明确的人因代价。
- `[INFERENCE]` Bignum 证明了精确 primitive 仍需选择异常语义、舍入语义和输入来源语义；它不能单独解决 money currency identity。

### 案例 7：Jane Street `Incremental`（OCaml）：计算 DAG、增量更新与观察消费

#### ① 系统做什么、规模与生产使用证据

- `[OBSERVED]` README 说 Incremental 用于输入改变时高效更新复杂计算，适用于大型 spreadsheet 计算、GUI view 和保持派生数据同步（`incremental/README.org:1-18`）。
- `[OBSERVED]` README 把它作为 Jane Street 库并链接完整 interface；本次没有打开具体生产应用代码，不能量化规模。

#### ② 设计中心是什么

- `[OBSERVED]` 核心是抽象计算节点 `type 'a t`，具备 `const/return/map/bind/join/if_/all/both/reduce_balanced`（`incremental/src/incremental_intf.ml:712-752`）。
- `[OBSERVED]` 可变输入与节点分开：`module Var` 有 `create/set/watch`，`Observer` 有 `value : 'a t -> 'a Or_error.t`、`Initialized/Changed/Invalidated`，全局 `stabilize` 解释更新（同文件:804-850）。
- `[INFERENCE]` design center 是有依赖边的 computation node/DAG，不是一个把 quote/book/bar/indicator 全部存进对象的 record。

#### ③ 多 provider/异构能力如何表达

- `[OBSERVED]` `'a t` 对 payload 参数化，provider 差异由输入 node 的 `'a` 类型和构建函数承担；源码没有 provider capability registry 或类型级“支持某 endpoint”机制。
- `[INFERENCE]` 新增数据源通常是新 `Var`/node；新增组合操作是 `map/bind/reduce`。这对开放观察源友好，但是否有流、游标或配额要由 node 构建边界另行表达。

#### ④ 副作用如何描述、组合、解释与消费

- `[OBSERVED]` `map/bind/both/all/reduce_balanced` 构造依赖图；`Var.set` 改变输入，`stabilize` 批量推进，`Observer.on_update_exn` 消费 `Initialized/Changed/Invalidated`（同文件:721-782,804-850）。
- `[OBSERVED]` `Observer.value` 返回 `Or_error.t`，因此无效化/错误不会被伪装成普通值（同文件:815-834）。
- `[INFERENCE]` 关联关系是“node 依赖 node，observer 依赖 node”；解释器是稳定化算法，不是 provider-specific object callback。其增量路径天然适合只重算受影响的节点，但源码没有金融语义或 unknown write。

#### ⑤ 基础类型设计

- `[OBSERVED]` `Update` 是 `Initialized of 'a | Changed of 'a * 'a | Invalidated`，另有 `Necessary`、`Unnecessary` 状态（同文件:825-845）。
- `[OBSERVED]` `reduce_balanced`、`unordered_array_fold` 和 `opt_sum` 暴露可增量聚合的组合接口，避免每次输入改变都全量重跑（同文件:748-790）。
- `[INFERENCE]` ordering/causal relation 在 DAG 中由依赖边表达；time/id/money 需作为 payload 的独立类型注入，不能从 `'a t` 自动获得。

#### ⑥ 已知问题、后悔、性能/人因代价

- `[OBSERVED]` README 只给用途和设计来源，没有公开性能数字或生产 post-mortem。
- `[INFERENCE]` `stabilize` 是显式消费边界，忘记稳定化会让观察者看到旧值；`Invalidated` 与 `Changed` 语义需要应用层谨慎处理。
- `[INFERENCE]` DAG 设计解决的是增量计算，不等于可靠消息队列：断线、持久化 gap、重放和写入 unknown 仍需另一层代数。

### 案例 8：Cats `Validated`/`Ior` + cats-effect `Clock`（Scala）：错误积累、部分成功与时间解释器

#### ① 系统做什么、规模与生产使用证据

- `[OBSERVED]` Cats core 提供通用数据类型和 typeclass；本次打开的是公开 `Validated.scala`/`Ior.scala`，没有打开某一交易系统采用证据，因此不声称生产规模。
- `[OBSERVED]` cats-effect kernel 的 `Clock[F[_]]` 是 effectful time typeclass，提供 monotonic/realTime/timed（`cats-effect/.../Clock.scala:25-61`）。
- `[INFERENCE]` 这组库展示的是可嵌入系统的组合代数，不是完整 adapter 框架。

#### ② 设计中心是什么

- `[OBSERVED]` `sealed abstract class Validated[+E,+A]` 的构造器是 `Valid[A]` 与 `Invalid[E]`（`cats/data/Validated.scala:31-46,843-845`）。
- `[OBSERVED]` `Ior` 是右偏的三路代数：`Left[A] | Right[B] | Both[A,B]`，并明确与 `Either` 的差异（`cats/data/Ior.scala:30-47,825-829`）。
- `[OBSERVED]` `Clock[F[_]]` 把时间读取作为 `F` 内效果，而不是让业务直接读全局 wall clock（`Clock.scala:25-56`）。

#### ③ 多 provider/异构能力如何表达

- `[OBSERVED]` provider 可用不同的 `F[_]` interpreter、不同 `E` error type 和不同 `Semigroup[E]` 组合；源码不枚举 provider。
- `[OBSERVED]` `Validated.ap/product` 要求 `Semigroup[E]` 来合并两个 Invalid，适合并行、互不依赖的验证（`Validated.scala:520-558`）。
- `[INFERENCE]` capability 是解释器/实例级而非 endpoint 字段；是否支持 monotonic clock、真实时钟或某种业务能力由 `Clock[F]`/其他 typeclass 实例提供，运行时握手信息仍需另建 ADT。

#### ④ 副作用如何描述、组合、解释与消费

- `[OBSERVED]` `Validated` 的 `product` 在两个输入都 Invalid 时用 `Semigroup` 合并错误；这表达“独立检查的全部诊断”，不是短路 `Either`（同文件:520-558）。
- `[OBSERVED]` `Ior.flatMap` 在 `Both(a1,b)` 后继续计算时，按 `Semigroup[A]` 合并左侧诊断；但 `toEither` 等方法会忽略 `Both` 的 A，源码特别提醒语义差异（`Ior.scala:38-45,658-667`）。
- `[OBSERVED]` `Clock.timed(fa)` 通过两次 `monotonic` 与 `fa` 的 `map3` 组合出 `F[(FiniteDuration,A)]`；monotonic 受 `<=` law 约束，realTime 是系统时间（`Clock.scala:33-56`）。
- `[INFERENCE]` 关联关系是“并行验证 -> accumulated error”，“可部分成功计算 -> Both”，“effect -> handler supplied by F”；消费端必须选择保留还是丢弃诊断。

#### ⑤ 基础类型设计

- `[OBSERVED]` `Validated[E,A]` 的 E/A 分离了错误与值；`Ior[A,B]` 额外允许同时存在二者，不把“有值但带警告”编码成空字符串/布尔标志。
- `[OBSERVED]` `Clock` 分出 `FiniteDuration` 的 monotonic 与 realTime，避免用单一 `Long` 混淆测量经过时间和日历时间（`Clock.scala:33-45`）。
- `[INFERENCE]` 这些类型没有 money/id/ordering 的金融定义；它们提供的是 error/time 组合的基础形状。

#### ⑥ 已知问题、后悔、性能/人因代价

- `[OBSERVED]` `Ior` 明确警告某些方法（如 `toEither`）会忽略 `Both` 左侧值；同一构造器在不同解释器方法中可能有信息损失（`Ior.scala:38-45`）。
- `[INFERENCE]` `Validated` 只能在检查独立时安全积累；依赖前一步输出的操作若硬套 `product`，会把顺序关系丢掉。
- `[INFERENCE]` `Clock` 抽象要求调用方选择 monotonic/realTime；这是额外的人因负担，但防止把系统时间当耗时计时器。

### 案例 9：Flink/Dataflow、Lamport、Pekko：事件时间、watermark 与因果顺序

#### ① 系统做什么、规模与生产使用证据

- `[OBSERVED]` Flink 官方概念文档描述分布式流处理的 processing time、event time、watermarks、多输入算子和乱序处理（`flink/docs/content/docs/concepts/time.md:40-165`）。
- `[OBSERVED]` Dataflow Model 论文明确把模型实现于 FlumeJava 与 MillWheel，并称 Cloud Dataflow 的重新实现大部分完成；论文主题是 unbounded/out-of-order 数据、正确性、延迟和成本（Akidau et al., PVLDB 8(12), 2015, abstract/contribution/§3–§4, pp.1792–1805）。
- `[OBSERVED]` Lamport 论文和 Pekko `VectorClock` 是因果排序原语；本次没有把它们宣称为交易系统生产部署。

#### ② 设计中心是什么

- `[OBSERVED]` Flink 的中心不是事件 record 字段，而是“event timestamp + watermark + window/trigger/accumulation”四件套：`Watermark(t)` 表示不应再有 `t' <= t` 的事件，operator 用 watermark 推进内部 event-time clock（Flink 文档:98-146）。
- `[OBSERVED]` Dataflow 论文把问题拆成四个互相可组合的维度：`What` 结果、`Where` event time、`When` processing time、`How` 早期结果与后续 refinement 的关系（论文 §2, pp.1793–1794）。
- `[OBSERVED]` Lamport 的设计中心是 `happened-before` 偏序和满足 Clock Condition 的逻辑时钟 `C(a)`；Pekko 则是 `VectorClock(versions: TreeMap[Node, Long])` 与 `Before|After|Same|Concurrent`（Lamport PDF pp.558–560；`pekko/.../VectorClock.scala:27-113`）。
- `[INFERENCE]` 核心是可解释的时间/因果代数，不是“事件对象加一个 status 字段”。

#### ③ 多 provider/异构能力如何表达

- `[OBSERVED]` Flink 多输入算子的当前 event time 取各输入 watermark 的最小值（Flink 文档:148-162）；这把异构输入的进度聚合定义成明确规则。
- `[OBSERVED]` Dataflow 支持多种 trigger、window 和 accumulation mode；trigger 可用逻辑组合（论文 §3–§4）。
- `[OBSERVED]` VectorClock 的 `Concurrent` 是开放运行时关系：当双方各有对方没有的版本时返回并发，而不是强行给出全序（`VectorClock.scala:27-58,175-200`）。
- `[INFERENCE]` provider capability 本案例不是“能否下单”，而是每条输入报告了什么进度/因果证据；新输入可加入，但聚合规则（例如 min watermark）是封闭的。

#### ④ 副作用如何描述、组合、解释与消费

- `[OBSERVED]` Dataflow 把 windowing（Where）、trigger（When）、accumulation（How earlier results relate to later refinements）分层并组合；论文明确说通过四件套获得 composability（§3–§4）。
- `[OBSERVED]` Flink processing time 延迟低但在异步/故障下 nondeterministic；event time 可处理乱序/历史重放，但要等待有限时间，因而仍有晚到边界（文档:45-88）。
- `[OBSERVED]` Lamport 的 IR1/IR2 通过本地递增与消息携带 timestamp 保持 `a -> b => C(a) < C(b)`（论文 pp.559–560）。
- `[INFERENCE]` 消费端不只是“取最新值”：它可以等待 watermark、保留早期结果并处理 refinement，或显式报告并发/缺口；这些都是解释器可观察的关系。

#### ⑤ 基础类型设计

- `[OBSERVED]` Flink 区分 machine system clock 的 processing time 与 event timestamp；watermark 是带 `t` 的进度声明。
- `[OBSERVED]` Lamport 的 `happened-before` 是偏序，concurrent 事件保持不可比；Pekko `Ordering` 封闭枚举 `After/Before/Same/Concurrent`，版本是 `Long`（Lamport pp.558–560；Pekko:27-58）。
- `[INFERENCE]` `gap`、late event、refinement、concurrency 不应压成同一 `Option`/`Bool`；本案例提供的是时间/顺序 primitive，不定义金额或外部写结果。

#### ⑥ 已知问题、后悔、性能/人因代价

- `[OBSERVED]` Flink 文档直接写出 processing time 的 nondeterminism、event time 等待乱序事件的 latency，以及有限等待意味着不可能无限保证 deterministic（Flink:57-88）。
- `[OBSERVED]` Dataflow 论文把 correctness、latency、cost 同时作为目标；trigger/accumulation 的组合增加可配置性，也增加解释负担（论文 abstract、§3–§4）。
- `[INFERENCE]` Vector clock 的 map 维度和合并成本随参与节点增长；本次没有读取 Pekko 性能数字，不能定量评价。

### 案例 10：Stripe/PayPal 幂等文档：未知写结果必须与重试分离

#### ① 系统做什么、规模与生产使用证据

- `[OBSERVED]` Stripe 官方 low-level error 文档描述 HTTP API 在网络中断、500、超时和 webhook correlation 下的客户端行为；PayPal 官方 idempotency 指南描述 capture 等写调用的 retry 语义（已下载并打开官方页面）。
- `[OBSERVED]` 这是直接面向支付 API 的生产协议文档，但文档本身没有给出内部类型定义或 provider 数量。

#### ② 设计中心是什么

- `[OBSERVED]` 文档没有公开代数/ADT 定义，不能声称它们以某个 FP 类型为设计中心。
- `[INFERENCE]` 从协议语义看，中心关系是“同一个 idempotency key 将请求与后续观察到的结果关联”，不是请求对象上加一个 `retryable: Bool`；这是协议抽象推断，不是 Stripe/PayPal 的代码类型。

#### ③ 多 provider/异构能力如何表达

- `[OBSERVED]` Stripe 公开文档描述 key 的 retention、参数一致性和 webhook local identifier；PayPal 用 `PayPal-Request-Id`，同 key 重试返回 latest status（Stripe low-level:135-155,196-225；PayPal idempotency:页面说明与示例）。
- `[INFERENCE]` 不同 provider 的 key header、保留期和 status 语义是值级协议能力；不能仅靠统一 `OrderResult` 假定相同。

#### ④ 副作用如何描述、组合、解释与消费

- `[OBSERVED]` Stripe 明确说 network error 时客户端不知道服务器是否收到请求；应以相同 key 和参数重试，不能换 key；500 也可能是 indeterminate，结果可能 reconcile/roll forward/rollback（官方 low-level 文档:150-193）。
- `[OBSERVED]` PayPal 示例说明 capture 请求超时但服务器已 capture；用原 `PayPal-Request-Id` 重试可取 latest status 而不重复 capture。
- `[INFERENCE]` “请求失败”与“外部副作用未知”是两个维度；`Either error a` 或 `Option a` 单独不能从类型名看出是否已提交。若使用 `Either e (Maybe a)`，三种构造虽有语法区别，业务语义仍需命名并验证；这是本报告的推论，不冒充官方定义。

#### ⑤ 基础类型设计

- `[OBSERVED]` 公开文档强调 idempotency key、参数一致性、server status、webhook local identifier；没有公开 money/time/id/error 的代码定义。
- `[INFERENCE]` 至少要将 correlation key、attempt/request、acknowledged result、unknown/indeterminate 与 reconciliation evidence 分成不同语义，但本段不提出 UTA 类型，只指出现有 API 文档中能观察到的关系。

#### ⑥ 已知问题、后悔、性能/人因代价

- `[OBSERVED]` Stripe 文档明确写 500 的 outcome 未必是客户端期待的 outcome，网络失败的请求可能已到达服务器；24 小时后 key 可能被移除，参数变化会报错（官方 low-level:170-225）。
- `[OBSERVED]` PayPal 文档提醒同 key 返回的是 latest status，不保证是 original status；这对审计和用户解释有成本。
- `[INFERENCE]` 盲重试、换 key 重试和把 timeout 当作拒绝都会引入资金风险；这不是 retry policy 的“边角情况”，而是副作用消费协议的核心。

### 案例 11（负例）：CCXT：统一交易 API 与巨大 TypedDict 的维护边界

#### ① 系统做什么、规模与生产使用证据

- `[OBSERVED]` CCXT README 写明支持 100+ cryptocurrency exchanges/prediction markets，提供 REST/WebSocket、跨交易所分析和统一 API，并支持 JavaScript/TypeScript/Python/C#/PHP/Go/Java/Rust（`ccxt/README.md:1-25,64-175,216-234`）。这提供了公开规模与多语言工程证据。
- `[OBSERVED]` 它是本报告所说的“多 provider 统一层”最直接的反例候选，但其成功不等于类型设计优越。

#### ② 设计中心是什么

- `[OBSERVED]` 统一中心是 `Exchange` 基类的标准方法族和返回结构；`Exchange.has` 用大量 method-name keys 标记 `True/False/None/'emulated'`（`python/ccxt/base/exchange.py:2715-2956`）。
- `[OBSERVED]` 统一返回值是宽 TypedDict，例如 `Trade` 有 `info, amount, datetime, id, order, price, timestamp, type, side, symbol, takerOrMaker, cost, fee`，`Order` 继续列出 status、price、filled、remaining、stopPrice、trades、fee 等（`python/ccxt/base/types.py:130-144,201-227`）。
- `[OBSERVED]` `MarketInterface` 也聚合 spot/margin/swap/future/option、fees、limits、precision、contract flags、outcomes 等（同文件:437-479）。
- `[INFERENCE]` 这是“统一结构 + 能力矩阵 + raw `info`”的对象中心设计；它并非本组推荐的代数 design center，正好用于识别 giant object 的代价。

#### ③ 多 provider/异构能力如何表达

- `[OBSERVED]` capability 主要值级表达：`has`/`features` 按方法名返回 `None`、`False`、`True` 或 `'emulated'`；基类未实现方法抛 `NotSupported`（`exchange.py:2715-2956,8395-8608`）。
- `[OBSERVED]` contributor 文档要求 unified method names/arguments 不能自由增删、所有返回结构必须符合 Manual，并要求市场 id 与 unified symbol 双向转换（`CONTRIBUTING.md:372-440,863-881`）。
- `[OBSERVED]` raw endpoint 仍通过动态 `api` JSON 注入 magic functions，参数是 dictionary（`CONTRIBUTING.md:920-924`）。
- `[INFERENCE]` provider 是开放世界，但新增 operation 主要要修改公共统一结构、所有语言生成物和大量 adapters；新增 provider 则要填 capability map 与 normalize raw data。

#### ④ 副作用如何描述、组合、解释与消费

- `[OBSERVED]` CCXT 的基类统一 method 负责发 HTTP/WS 调用、解析到宽结构、在不支持时抛 `NotSupported`；贡献规则要求尽量一个 unified method 一个 HTTP 请求（`CONTRIBUTING.md:353-359`）。
- `[OBSERVED]` 市场 symbol/id 转换和 `has` capability 检查把“调用什么、是否支持、如何解析”分散在 adapter/base class（`CONTRIBUTING.md:372-440`; `exchange.py:2715-2956`）。
- `[INFERENCE]` 关联关系是 method -> exchange subclass -> unified dict；失败、限流、unknown write 主要作为异常/响应约定存在，宽 `info: dict[str, Any]` 保留 provider 数据但把语义推迟到调用方。

#### ⑤ 基础类型设计

- `[OBSERVED]` `OrderSide`/`OrderType` 是字符串 Literal，`Num = None | str | float | int | Decimal`，`Str = str | None`；`OrderRequest` 的 `params: Any` 和各种 `info: dict[str, Any]` 是显式动态逃生口（`base/types.py:18-20,64-71,176-182`）。
- `[OBSERVED]` 统一时间要求为 integer UTC milliseconds（`CONTRIBUTING.md:637-640`）；但 amount/price/cost 同时允许多种 Num 表示。
- `[INFERENCE]` 这使跨 provider 接入实际可行，却无法由静态类型证明 currency、quantity、idempotency 或 result correlation 的业务不变量。

#### ⑥ 已知问题、后悔、性能/人因代价

- `[OBSERVED]` CCXT maintainer 在 issue #1027 中回应对 exchange quirks 的维护请求：“a single human and even a small team can't possibly have enough time to do it all” (`issue://ccxt/ccxt/1027`, @kroitor, 2018-01-12)。
- `[OBSERVED]` 同 issue 显示 Binance all-ticker endpoint 的 ML rate limit/weight 不能由客户端统一绕过；maintainer 说 “We can't really work around this on client side.”
- `[OBSERVED]` contributor 文档承认统一任务仍包含尚未全部实现的 margin、leverage、derivatives、conditional orders、transfers、positions 等（`CONTRIBUTING.md:45-62`）。
- `[INFERENCE]` 统一 object 把差异推入 `has`、`params`、`info`、运行时异常与文档；它降低首次接入门槛，却把能力语义和维护成本集中到统一层。

### 案例 12（负例）：QuantLib：Instrument/PricingEngine 对象层次与 observer 成本

#### ① 系统做什么、规模与生产使用证据

- `[OBSERVED]` QuantLib README 将其定义为面向 quantitative finance、modeling/trading/risk management 的 comprehensive framework，并称 free/open-source（`QuantLib/README.md:2,19-25`）。
- `[OBSERVED]` `Instrument`/`PricingEngine`/Observer 是公开源码中的长期核心；QuEP 5 以 option pricer 作为真实重构讨论对象（`ql/instrument.hpp:38-112`; `ql/pricingengine.hpp:33-74`; QuEP5）。
- `[INFERENCE]` 这是成熟金融库的架构证据，但本次未打开商业用户数量或生产部署统计，不能填入未经证实的规模数字。

#### ② 设计中心是什么

- `[OBSERVED]` `Instrument : public LazyObject` 暴露 `NPV`, `errorEstimate`, `valuationDate`, `pricingEngine`, `additionalResults`，并通过 `setupArguments`/`fetchResults` 与 engine 连接（`ql/instrument.hpp:44-112`）。
- `[OBSERVED]` `PricingEngine` 是 `Observable`，含抽象 `arguments`, `results`, `reset`, `calculate`；`GenericEngine<ArgumentsType,ResultsType>` 用两个可变结构保存参数与结果（`ql/pricingengine.hpp:33-74`）。
- `[OBSERVED]` QuEP 5 的 proposed `EuropeanOptionParameters` 多继承 `OptionParameters, UnderlyingParameters, MarketParameters`，`EuropeanOptionResults` 多继承 `OptionValue, OptionGreeks`，再通过 dynamic_cast 取 slot（QuEP5:84-157）。
- `[INFERENCE]` 它的 design center 是对象层次 + pricing engine delegation；但参数/结果的“可组合性”仍依赖继承、空 base class 和运行时 downcast，不是纯代数。

#### ③ 多 provider/异构能力如何表达

- `[OBSERVED]` provider 在此对应 analytic/finite-difference/Monte Carlo pricing engine；QuEP5 通过 `Handle<OptionPricingEngine>` 把不同 engine 注入同一 `EuropeanOption`（QuEP5:218-247）。
- `[OBSERVED]` 差异被放进 `Arguments`/`Results` 派生结构和 `dynamic_cast`；`Instrument` 接口需知道 NPV/valuationDate，而其他 result 以 `std::map<string, any>` 扩展（`instrument.hpp:51-62,102-124`）。
- `[INFERENCE]` 这是开放 engine、相对封闭 Instrument 接口的 runtime capability；新增结果 slot 不会由编译期穷尽性保证。

#### ④ 副作用如何描述、组合、解释与消费

- `[OBSERVED]` `Instrument` 的 LazyObject machinery 负责缓存/重算；engine `calculate()` 产出 results，`Instrument::fetchResults` 读取 NPV/额外结果（`instrument.hpp:76-100,114-124`; `pricingengine.hpp:58-74`）。
- `[OBSERVED]` QuEP5 的 critique 原话指出当前 option pricers “leads to mixing of concerns, loss of abstraction, and a number of unrelated class hierarchies”，并担心 “such hierarchies would be difficult to maintain” （QuEP5:19-27）。
- `[OBSERVED]` 作者提出 engine delegation 以分开 housekeeping/result caching、market data interpretation 和 calculation（QuEP5:218-247）。
- `[OBSERVED]` QuantLib 作者关于 Observer 的文章写出：通知异常要捕获，否则一个 observer 抛错会中断其余通知；原始 exception 会丢失（Observer blog:200-217）。

#### ⑤ 基础类型设计

- `[OBSERVED]` `Instrument` 的 NPV/error estimate 是 `Real`，valuation date 是 `Date`，additional results 是 `map<string,any>`；engine `arguments/results` 只要求 `validate/reset`（`instrument.hpp:51-62,76-124`; `pricingengine.hpp:45-55`）。
- `[OBSERVED]` QuEP5 旧 pricer 参数以 `double` 接收 underlying price/risk-free rate 等，即使上游是 `MarketElement`/`TermStructure`；作者把重复提取 scalar 列为缺点（QuEP5:56-76）。
- `[INFERENCE]` 这类 API 对传统数值模型方便，但 currency/quantity/time identity 与不可判定副作用不是类型级原语，而是 `double`, `Date`, `any` 加约定。

#### ⑥ 已知问题、后悔、性能/人因代价

- `[OBSERVED]` QuEP5 列出 class hierarchy 重复 housekeeping、duplicate data members、参数 scalar 提取重复、clone/setVolatility 维护负担（QuEP5:56-83）。
- `[OBSERVED]` QuantLib 作者的 Observer post-mortem 写明 C#/Java GC 线程造成 “random crashes as a notification was sent to a half-deleted object”；修复依赖 undocumented `boost::shared_ptr` hook，会拖慢代码且默认关闭（Observer blog:228-247）。
- `[OBSERVED]` 同文写道日期变化 “can easily trigger tens or hundreds of notifications”，CVA/XVA 使用者通过禁用 notifications 并显式重算规避；作者结论是 “In short, the problem is still not solved” （Observer blog:249-265）。
- `[INFERENCE]` 这正是“业务对象 + 隐式 observer 关联”把效果消费、生命周期、性能和错误传播耦合到一起的代价；不应通过再添加字段掩盖。

## 横向对比表

|案例|设计中心|异构能力|效果/消费|关键基础类型|主要代价|
|---|---|---|---|---|---|
|safe-money|`Dense currency`、`Discrete' currency scale`、`ExchangeRate src dst`|type-level currency/scale；`Some*` 运行时边界|纯转换/离散化；余数显式返回|`Rational`、正 scale、typed FX|序列化需 existential；Float 有损|
|Squants|`Quantity[A]` + `UnitOfMeasure` + `Dimension`|runtime Currency/MoneyContext|纯量值；FX 由 context 解释|`BigDecimal` Money 与 `Double` Quantity|缺 rate 异常；Double/隐式 context|
|F# DMMF|阶段函数 + union state|依赖函数/ADT error|`AsyncResult`/`Result`，阶段产事件|`CheckedAddress`、`Price`、`Option`/事件 list|样例未维护；`NotSent -> None` 不足|
|indexed|`m i j a`|索引/解释器；无 provider registry|由具体 monad 解释合法转移|状态索引、结果类型|开放世界能力难静态证明|
|Cardano|`Rule`/`STS` + `PredicateFailure`/`Event`|封闭 era type family|状态、事件、累积结构化失败|`Coin`、`TxId`、`TxIn`、`SlotNo`、`Value`|era 演进改动大；形式化复杂|
|Bignum|抽象 `type t`|输入构造器区分 float 语义|纯运算；`Or_error`/特殊值|Bigint rational、rounding polyvariant|大数成本；NaN/∞ 语义负担|
|Incremental|`'a t` DAG node|payload 参数化，能力在外部|`Var.set`→`stabilize`→Observer update|`Changed`/`Invalidated`/`Or_error`|不是持久消息/unknown 语义|
|Cats|`Validated`/`Ior`/`Clock[F]`|typeclass/interpreter|错误积累、Both、effectful time|`Semigroup[E]`、`FiniteDuration`|`Ior` 方法可能丢左值|
|Flink/Dataflow/因果时钟|watermark/window/trigger；偏序/向量时钟|多输入进度、Concurrent|增量结果/refinement/乱序|event/processing time、watermark、`Concurrent`|等待/延迟/成本和解释负担|
|Stripe/PayPal|协议关系（key↔attempt↔latest status）|provider-specific key/header/retention|unknown 与 reconcile 分开|idempotency key、status、webhook id|重试不能保证“未发生”|
|CCXT（负例）|统一 Exchange + 宽 dict|`has`/`features`/`params`/`info`|base method + runtime NotSupported|TypedDict + `Any`/多种 Num|维护者承认 quirks 无法全覆盖|
|QuantLib（负例）|Instrument + PricingEngine + Observer|engine subclass/runtime cast|lazy cache + observer notify|`Real`/`Date`/`any`|层次、生命周期、通知性能与异常丢失|

## 对 UTA 的可迁移命题

以下只陈述“案例中的条件—机制—问题”关系；不定义 UTA 的字段、对象或状态机。

1. **safe-money 命题。** 案例 safe-money 在“金额必须精确、货币/单位在核心路径可知、序列化才遇到动态名称”的条件下，用 `Dense currency`/`Discrete' currency scale`/`ExchangeRate src dst` 解决了跨货币误算与舍入丢失问题；UTA 若满足“核心消费阶段可保留 provider/资源身份且不必把未知名称当核心类型”，可迁移，否则不可。
2. **Squants 命题。** Squants 在“维度关系稳定、汇率可由作用域 context 提供”的条件下，用 `Quantity`/`Dimension` 和 `MoneyContext` 解决同维度运算与 FX 解释；UTA 若能保证 context 的来源、生命周期和隔离条件，才可迁移，否则隐式 context 会遮蔽 provider 差异。
3. **F# 阶段命题。** DMMF/Wlaschin 在“工作流阶段的有效字段集合不同、下一步只应消费已证明阶段值”的条件下，用 union state 和阶段函数解决业务对齐大对象与错误阶段调用；UTA 若满足阶段值可封闭/可穷尽，否则不可将开放 provider 世界硬编码成封闭 union。
4. **indexed monad 命题。** `indexed` 在“合法状态转移能在构造程序时知道”的条件下，用 `m i j a` 的中间索引一致性拒绝非法组合；UTA 若某种 effect 轨迹确实有稳定、可静态知道的前后状态，才可迁移，否则运行时 capability/unknown 不能伪装成 compile-time proof。
5. **Cardano 命题。** Cardano Ledger 在“规则集合/era 是版本化且封闭、失败需要可审计积累”的条件下，用 `Rule`、`PredicateFailure`、`Event` 与 `NonEmpty` 组合解释状态转移；UTA 若某一子协议也能封闭版本化，才可迁移，否则 provider 能力应保留开放解释边界。
6. **Bignum 命题。** Bignum 在“精度和输入来源的语义必须由调用点选择”的条件下，用抽象 rational、显式 rounding 和 `of_float_decimal`/`of_float_dyadic` 解决隐式浮点语义；UTA 若需要资金或数量计算并能承受显式选择，可迁移，否则不可把任意数值转换默认为安全。
7. **Incremental 命题。** Incremental 在“派生计算可建成稳定依赖 DAG、更新可按受影响节点重算”的条件下，用 `'a t`、`Var`、`Observer`、`stabilize` 解决每次输入全量重跑；UTA 若观察指标满足该 DAG 条件，可迁移，否则消息持久化、重放和 gap 不能由 Incremental 自动提供。
8. **Validated/Ior 命题。** Cats 在“验证项相互独立，错误可用 Semigroup 合并”时用 `Validated[E,A]` 累积诊断；在“值与诊断可同时存在且解释器保留 Both”时用 `Ior[A,B]`；UTA 若满足相应独立性/保留条件，可迁移，否则应避免把顺序依赖误作并行验证或让解释器丢弃诊断。
9. **Clock 命题。** cats-effect 在“耗时与日历时间必须区分、时间读取属于效果”的条件下，用 `Clock[F]` 分开 monotonic 与 realTime；UTA 若有窗口/超时/延迟语义且能注入时间解释器，可迁移，否则不可把系统 wall clock 当排序证明。
10. **Flink/Dataflow 命题。** Flink/Dataflow 在“输入乱序、进度可声明、早期结果需后续修正”的条件下，用 event time、watermark、window、trigger、accumulation 解决事件消费的确定性/延迟折衷；UTA 若各观察源能提供可解释进度证据，才可迁移，否则不能把收到最新消息当作完整性证明。
11. **Lamport/VectorClock 命题。** Lamport/Pekko 在“只需因果偏序、并发不应被伪造为全序”的条件下，用 happened-before、logical clock、vector clock 与 `Concurrent` 解决跨进程顺序；UTA 若关联语义只要求因果/并发判定，可迁移，否则不可把单一递增序号当全局真相。
12. **Stripe/PayPal 命题。** 支付 API 在“客户端可能不知道写是否已到达、服务端以同一 key 返回 latest status”的条件下，用 idempotency key 与 reconciliation 解决重复写风险；UTA 若写副作用同样不可判定且 provider 保证 key 语义，才可迁移，否则不可凭 timeout/异常推断外部未发生。
13. **CCXT 反命题。** CCXT 在“provider 数量极大、用户优先要共同最小 API、未统一能力必须继续可访问”的条件下，用统一宽结构、`has/features` 和 raw params 降低接入门槛；但维护者同时承认无法覆盖所有 quirks。UTA 若不满足“接受运行时动态逃生口与统一层维护成本”，不可迁移，尤其不能照搬 giant `Order/Market` object。
14. **QuantLib 反命题。** QuantLib 在“仪器对象共享 NPV/缓存接口、pricing engine 可替换”的条件下，用 `Instrument`/`PricingEngine` delegation 复用计算 machinery；但其作者记录了层次混杂、GC 生命周期崩溃、通知风暴和问题仍未解决。UTA 若必须进行异步、可断线、可重放消费，不可把 observer callback 当效果消费中心。
15. **解析边界命题。** Alexis King 在“外部输入先被解析、下游应依赖已证明结构”的条件下，用 `parseNonEmpty :: [a] -> IO (NonEmpty a)` 保留 proof，避免 `validateNonEmpty :: [a] -> IO ()` 丢失信息；UTA 若能在入口把可验证约束转成结构化值，可迁移，否则不可把不可信输入直接当内部 domain value。
16. **newtype 限制命题。** King 在“构造性枚举可表达不变量”时警告“On its own, a newtype is just a name. And names are not type safety.”；只有构造器隐藏、smart constructor 与测试构成 trust boundary。UTA 若选择 wrapper，必须满足该边界条件，否则不可把换名字当作能力/金额/结果安全。

## 未覆盖与开放问题

- 本次没有找到 Jane Street 公共仓库中的 `Core.Money` 一手定义；因此 OCaml 部分只覆盖 `Bignum` 和 `Incremental`，没有补写未经证实的金融模型。
- Scala 3 opaque/match/union types 的深入案例由同批次 `ScalaJvmCases` 任务覆盖；本报告只保留 Cats/Squants 与时间/错误原语，避免重复。
- 没有打开 OpenGamma Strata 的完整 Trade/Product 源码或 FIX 标准的维护评论；QuantLib 与 CCXT 已提供两个公开负例，但不能把它们的批评外推到所有金融对象模型。
- 没有找到公开的一手材料同时给出“多个交易 venue + effect handler + unknown write + durable subscription”完整系统；Stripe/PayPal 只证明未知写与幂等协议，Incremental/Flink 只证明观察/进度消费。
- 需要进一步考察的问题包括：开放 provider 能力如何在不退化为 `Any` 的边界解析；unknown 结果与 reconciliation evidence 的可组合代数；断线 gap 与 watermark/causal clock 的交叉语义；以及 capability 在编译期、解释器构造期、运行时握手期三个知识阶段的分层。
- 本报告没有运行本仓库 build/lint/test，也没有修改其他仓库文件；验证范围仅是打开并比对下列一手源码、文档与 PDF。

## 来源清单

> 状态含义：`已打开（clone）` = 本次 shallow clone 后用 `read/grep` 打开；`已下载并打开` = 通过官方 URL 下载到本地再用 `read` 打开；所有仓库 commit 均是本次实际读取的 `HEAD`。

1. **safe-money** — `https://github.com/k0001/safe-money`；已打开（clone `9960c6102a97786dc219b8ad78a1a978c1f63fef`）；本地 `/tmp/openalice-fp04/safe-money/safe-money/src/Money/Internal.hs`、`README.md`。
2. **Squants** — `https://github.com/typelevel/squants`；已打开（clone `29aa57f4a1958daf9a5a397cca8982b1654e164c`）；本地 `shared/src/main/scala/squants/Quantity.scala`、`UnitOfMeasure.scala`、`market/Money.scala`、`README.md`。
3. **Domain Modeling Made Functional** — `https://github.com/swlaschin/DomainModelingMadeFunctional`；已打开（clone `8153616b1dc0d5a0bb9e965cbe14a46b0dd4f3cf`）；本地 `src/OrderTaking/PlaceOrder.Implementation.fs`、`README.md`。
4. **F# for Fun and Profit（作者源码）** — `https://github.com/swlaschin/fsharpforfunandprofit.com`；已打开（clone `15b0a0d2b503aa297f4f5a789ebff00ba53e75b5`）；本地 `content/posts/designing-with-types-making-illegal-states-unrepresentable/index.md`、`.../designing-with-types-representing-states/index.md`。网站 canonical 路径在本次环境返回 404，故以作者仓库为一手打开来源。
5. **indexed** — `https://github.com/reinerp/indexed`；已打开（clone `cc46ad61557cd7c2e14556c3429d8b9b2abe3f59`）；本地 `Control/Monad/Indexed.hs`、`Data/Functor/Indexed.hs`。
6. **Cardano Ledger** — `https://github.com/IntersectMBO/cardano-ledger`；已打开（clone `2c33b4f858c0e62b300d121996a479f505d8c0e5`）；本地 `README.md`、`eras/shelley/impl/src/Cardano/Ledger/Shelley/Rules/Utxo.hs`、`libs/small-steps/src/Control/State/Transition/Extended.hs`、`libs/cardano-ledger-core/src/Cardano/Ledger/Coin.hs`、`TxIn.hs`。
7. **Jane Street Bignum** — `https://github.com/janestreet/bignum`；已打开（clone `61d7aa67d863dcc12cf5154469344bbccf05a2c8`）；本地 `src/bignum.mli`。
8. **Jane Street Incremental** — `https://github.com/janestreet/incremental`；已打开（clone `98b5750ec3c006641351bfd858a89136a5dbc52c`）；本地 `src/incremental_intf.ml`、`README.org`。
9. **Cats** — `https://github.com/typelevel/cats`；已打开（clone `f0c5f1b450b3250db4f6b819ebd9db4d583843bd`）；本地 `core/src/main/scala/cats/data/Validated.scala`、`Ior.scala`。
10. **cats-effect Clock** — `https://github.com/typelevel/cats-effect`；已打开（clone `f219988a0c8821105da8a29e30d731bab39e2ba4`）；本地 `kernel/shared/src/main/scala/cats/effect/kernel/Clock.scala`。
11. **Apache Flink** — `https://github.com/apache/flink`；已打开（clone `bd0a65afb943d56a8afb13c20a1ba57d1bfc30d5`）；本地 `docs/content/docs/concepts/time.md`。
12. **Dataflow Model paper** — Akidau et al., `https://www.vldb.org/pvldb/vol8/p1792-Akidau.pdf`；已下载并打开 `/tmp/openalice-fp04/dataflow-model.pdf`；引用 abstract、§2–§4、pp.1792–1805。
13. **Lamport, “Time, Clocks, and the Ordering of Events in a Distributed System”** — `https://lamport.azurewebsites.net/pubs/time-clocks.pdf`；已下载并打开 `/tmp/openalice-fp04/lamport-time-clocks.pdf` / `.txt`；引用 pp.558–560、§“Logical Clocks”。论文 DOI：`https://doi.org/10.1145/359545.359563`。
14. **Apache Pekko VectorClock** — `https://github.com/apache/pekko`；已打开（clone `6db9cafd59b8e3567106747557e23ee511390d02`）；本地 `cluster/src/main/scala/org/apache/pekko/cluster/VectorClock.scala`。
15. **Haskell time** — `https://github.com/haskell/time`；已打开（clone `24ddc629d3da321f4d6bf36ea7b329ffc7978032`）；本地 `lib/Data/Time/Clock/Internal/UTCTime.hs`、`lib/Data/Time/Clock.hs`。仅作为未展开的 UTC/nominal time 交叉证据。
16. **Alexis King, “Parse, don’t validate”** — `https://lexi-lambda.github.io/blog/2019/11/05/parse-don-t-validate/`；已下载并打开 `/tmp/openalice-fp04/parse-dont-validate.html` / `.txt`；引用 `NonEmpty`、parse/validate、shotgun parsing 段落。
17. **Alexis King, “Names are not type safety”** — `https://lexi-lambda.github.io/blog/2020/11/01/names-are-not-type-safety/`；已下载并打开 `/tmp/openalice-fp04/names-not-type-safety.html` / `.txt`；引用 newtype、constructive data、trust boundary 和维护代价段落。
18. **Stripe, low-level errors** — `https://docs.stripe.com/error-low-level`；已下载并打开 `/tmp/openalice-fp04/stripe-low-level.html` / `.txt`；引用 network error、500 indeterminate、idempotency key、webhook correlation 段落。
19. **PayPal idempotency** — `https://developer.paypal.com/reference/guidelines/idempotency/`；已下载并打开 `/tmp/openalice-fp04/paypal-idempotency.html`；引用 `PayPal-Request-Id` 与 timeout/capture 示例。
20. **CCXT** — `https://github.com/ccxt/ccxt`；已打开（clone `e1cd3a681bff89560ac3f8c68f317fcc34683e57`）；本地 `README.md`、`python/ccxt/base/types.py`、`python/ccxt/base/exchange.py`、`CONTRIBUTING.md`。
21. **CCXT maintainer issue #1027** — `https://github.com/ccxt/ccxt/issues/1027`；已打开 GitHub issue resource；引用 @kroitor 2018-01-12 对 quirks 维护范围的原话。
22. **QuantLib** — `https://github.com/lballabio/QuantLib`；已打开（clone `e34c461de96e1f3afe55b8d94f2e0d6fc418a399`）；本地 `README.md`、`ql/instrument.hpp`、`ql/pricingengine.hpp`。
23. **QuantLib QuEP 5（Luigi Ballabio）** — `https://www.quantlib.org/quep/quep005.html`；已下载并打开 `/tmp/openalice-fp04/quep005.html`；引用 abstract、Current implementation、Disadvantages、Proposed implementation、Conclusion。
24. **Implementing QuantLib Observer（Luigi Ballabio）** — `https://www.implementingquantlib.com/2017/09/odds-and-ends-observer.html`；已下载并打开 `/tmp/openalice-fp04/ql-observer.html`；引用 exceptions/lifetime/GC/notification storm/post-mortem 段落。
25. **thiserror（补充错误 ADT 对比）** — `https://github.com/dtolnay/thiserror`；已打开（clone `5a306c7d0a8588caaaaa6a7567aeb25c1c10719b`）；本地 `src/lib.rs`；本报告正文未展开为独立案例，避免偏离 Haskell/Scala/F# 主线。
26. **Scala 3 opaque types（未展开，供同批次交叉）** — `https://github.com/scala/docs.scala-lang`；已打开（clone `f9b365a886739d2f7e56216e92f261fc1cf55b1f`）；本地 `_overviews/scala3-book/types-opaque-types.md`；正文未展开，避免与 Scala JVM 案例重复。
