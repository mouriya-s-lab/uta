# fp-03 · 效应组合与开放 provider 集合：FP 理论与库层面的已知解法、代价与批评

> 本报告是**理论/库层面**的文献调查，不含 UTA 设计。它回答维护者三问——"副作用怎么消费、谁和谁能关联在一块、抽象怎么做"——的**已发表理论基础**：当 provider 集合可扩展、能力只部分已知时，函数式编程界用哪些机制把 operation、provider 身份、capability 证据、effect 作用域、执行记录关联起来；这种关联在哪一阶段成立；组合与更换解释器之后又保留什么语义。所有论断均来自本次实际打开的一手来源（论文、官方文档、作者博客、公开仓库源码），逐条标注出处与 OBSERVED/INFERENCE。仓库引用固定在 commit，PDF 引用给 canonical URL + 页/节。

## 摘要

本组案例中**最常见的设计中心是"一个小核心 = 一族开放的 operation/effect + 一个可替换的解释器"**：要么把操作表示成开放和式（`f :+: g`/`Union r`/effect operation/command·event log），由一个自然变换 `f ~> g` 或 handler 赋予语义；要么把操作表示成 effect row 的成员资格证据（tagless 约束、`:>`/`Member`/`Has`、capture set、类型级 row），由 instance/handler/capability 值选择实现。它们**避免"业务对齐的大对象"的共同手法**：核心是 operation 的*组合*而非字段的*罗列*；状态是 event/operation 流的 `fold`/projection，可重建可替换，而不是一个被就地改写、装满 provider 字段的巨型 record（条目 7 直接反驳"大对象"，条目 2/8 用 coproduct/row 让"加 provider 不改核心"）。三条一致结论：(1) "谁和谁能关联"不是一种关系而是至少五种，分别由类型参数、dictionary、row 成员证明、capability token、request key、log record 承载，验证阶段各异；(2) 纯类型级方案只能表达编译期/装配期已知的能力，对"运行期握手才知道、且部分未文档化"的能力矩阵一律退化为 existential/refinement 或运行期错误——**没有一种机制能把运行期未知冒充静态保证**；(3) 每种机制的"可组合"都带公开代价：tagless 无律（De Goes）、Free/Freer 左结合 bind 的 O(n²)（Karpov/Kiselyov）、代数效应 handler 顺序 non-commutativity 与 multi-shot 资源泄漏（King/Hillerström/OCaml manual）、event sourcing 的 schema 演化与 replay 重放外部副作用（Fowler/Nubank）。

## 范围与筛选标准

- **纳入**：对"开放 provider 集合 × 异构能力 × 可组合副作用"给出可判定机制的 FP **理论与库**，且能提供一手类型签名/定义与作者原话。八个条目覆盖两大家族：*final/约束式*（条目 1、4 约束面、6、8 的 constraint/row）与 *initial/reify 式*（条目 2、3、7），以及*调度/契约式*（条目 5）。
- **排除**：UTA 协议字段、provider 选型、重试/幂等/背压的具体设计、库推荐与迁移方案——这些是设计不是调查。金融行业实证案例、FRP/增量计算、Scala/JVM 工程案例、基础类型（money/time/id）分别属兄弟报告 fp-01/fp-04/fp-05/fp-02，本报告只在可迁移命题里交叉引用，不重复。
- **证据纪律**：仓库给 commit-pinned permalink + path + 行号；PDF 给 URL + 页/节；博客给标题 + 日期 + URL。每条结论标 OBSERVED（来源直述）或 INFERENCE（据来源的推断）。不把"interpreter 可换"说成"语义等价"，不把 `Monad` laws 当作业务 operation 的 law。

## 分析框架（贯穿全部条目的统一透镜）

维护者三问在文献里对应三组可判定的技术问题。为避免八节退化成八篇库简介，每个条目都用同一套透镜自检；透镜本身是研究工具，**不是 UTA 设计**。

### 统一 litmus（每条目自检，纯研究用）

设 provider `P` 支持 `Read + Write`、`Q` 支持 `Read + Stream`。对每个机制问四件事：
1. **能力子集拒绝**：`Q.Write` 在哪一阶段被拒绝——编译期、interpreter 构造期，还是运行期？
2. **可组合并行**：`P.Read` 与 `Q.Read` 两个独立读取能否被自动批处理/并行；机制凭什么判定它们无依赖？
3. **写—结果关联**：一次 `P.Write` 如何与它的回执/审计记录关联起来（谁承载这条关联）？
4. **replay 语义**：重放程序时 `Write` 会不会被再次执行（reify 的是语法、决策，还是外部结果）？

### "谁和谁能关联"的五种关系（每条目逐项写清承载物与验证阶段）

维护者的"谁和谁能关联"不是一种关系，至少五种，每种由不同东西承载：

| 关系 | 典型承载物 | 验证阶段 |
|---|---|---|
| R1 operation ↔ capability | type-class 约束 / effect-row 成员 / GADT 构造子 / capability 值 | 通常编译期 |
| R2 request/resource ↔ provider 身份 | 类型参数 / phantom / DataSource 键 / 生成的 nominal 类型 | 编译期或运行期键 |
| R3 多 effect ↔ 同一因果/并发作用域 | Applicative vs Monad 结构 / handler 作用域 / correlation id | 运行期结构，静态划界 |
| R4 program ↔ handler/interpreter | natural transformation `f ~> g` / handler / instance 选择 | interpreter 构造期 |
| R5 intent ↔ 执行结果/审计记录/replay 身份 | 不可变 event/log 记录 + causation/correlation id | 运行期，持久化 |

### 能力"何时已知"的三阶段

UTA 的能力矩阵**部分未文档化**，所以"能力在什么时候可知"是核心判据：
- **C-compile**：编译期即由类型固定（type-class instance 存在性、row 成员）。
- **C-construct**：装配 interpreter/handler 时由值固定（record of functions、handler 表）。
- **C-runtime**：只能在与 venue 握手/探测后才知道（幂等键保留期、续传游标有无……大量属此类）。

关键结论线索：**纯类型级方案（条目 8 的 row/constraint/dependent witness）只能表达 C-compile / C-construct；对 C-runtime 的能力，类型系统要么退化为 existential/refinement + 运行期错误，要么根本无法静态保证。** 任何把 C-runtime 说成静态保证的论述都是错误的。

### 开放世界的两条轴（expression problem）

"可扩展"必须拆成两个方向，分别判定要不要改既有代码：
- **轴 A（加 provider/interpreter）**：新增一个数据源/后端/解释器，既有 operation 代数与程序是否不动。
- **轴 B（加 operation/effect）**：新增一个操作/能力，既有 provider 的 interpreter 是否不动。

这正是 Wadler 的 expression problem。final 编码擅长轴 A（加 instance）、对轴 B 靠加新 type class；initial（Free/DTC）用 coproduct 同时开放两轴但付性能代价；生成式（amazonka）把轴 A 交给 codegen。逐条目给出它在两轴上的机制性表现。

---

## 逐条目分析

### 条目 1 · Tagless Final（final encoding）

**设计中心**：把 DSL 的每个操作表达为一个以结果 type constructor `repr`/`F[_]` 为参数的 type class 方法；程序对"能力集合"多态；解释器 = type class 的 instance。**syntax = 接口，semantics = instance**。

**核心类型（OBSERVED，Carette/Kiselyov/Shan, *Finally Tagless, Partially Evaluated*, JFP 2009, https://okmij.org/ftp/tagless-final/JFP.pdf, §2.1 p.7）**：

```haskell
class Symantics repr where
  int :: Int -> repr Int
  add :: repr Int -> repr Int -> repr Int
  lam :: (repr a -> repr b) -> repr (a -> b)
  app :: repr (a -> b) -> repr a -> repr a
```

论文原话（OBSERVED）："*the functions that construct object terms are methods in a type class `Symantics` … The class is so named because its interface gives the syntax of the object language and its instances give the semantics.*" 'tagless' 的论证：final 编码"represent object programs using ordinary functions rather than data constructors"，于是"*The result has no tags: the interpreter patently uses no tags and no pattern matching*"（§1，p.4），无需 GADT 即静态排除 ill-typed 项。Kiselyov 主页给出 final↔initial 的双射 `itf :: Symantics repr => IR repr t -> repr t`，二者同构。Scala 化（OBSERVED，De Goes 文中）：`trait Console[F[_]] { def putStrLn(s: String): F[Unit]; val getStrLn: F[String] }`，程序 `def prog[F[_]: Console: Monad]: F[String]`。

**谁和谁能关联 / 抽象层**：能力 = **type class 约束**；程序对能力集合多态 `f :: (Console m, Logger m) => m a`。"谁能关联" = 谁在同一约束集合里、被同一 type variable `F`/`repr` 统一（R1/R3，C-compile）。R2 provider 身份由选定的 `F` + 其 dictionary 承载（C-construct；类型擦除后运行期无法反查是哪个 provider，INFERENCE）。R4 program↔interpreter = 传入的 instance/implicit，绑定很早（Warski："Interpretation happens when the expression is created"，OBSERVED）。R5 intent↔审计/replay：**无 reified program 值**，只有流经 `F[A]` 的值——这是相对 Free 的明确劣势。

**组合律 / 语义歧义**：多约束组合**无内在 handler 顺序歧义**——所有操作直接 denote 进同一 `F`，顺序由 `F` 的 `flatMap` 唯一决定（相对代数效应的优势，INFERENCE）。但**缺 laws 时无法做通用推理**。

**已发表批评（逐字，OBSERVED）**——John A. De Goes, *The False Hope of Managing Effects with Tagless-Final in Scala* (2019, https://degoes.net/articles/tagless-horror)：
- "*In Scala, the Cats Effect type class hierarchy provides many type classes that are explicitly designed to capture side-effecting code. These type classes include Sync, Async, LiftIO, Concurrent, Effect, and ConcurrentEffect. Methods that require one of these type classes can literally do anything they want, without constraints, even assuming a working social contract.*"（Sync Bloat）
- "*Scala does not track side-effects, no matter how much we wish otherwise. We can perform console effects anywhere, even without the provided Console[F]*"；结论："*Stated more forcefully and for Scala code, effect parametric reasoning is a lie.*"
- "*Without lawful operations, there is no abstraction, only layers of indirection masquerading as abstraction.*"（Fake Abstraction）

Adam Warski, *Free and tagless compared* (2017, softwaremill.com)：final tagless "*requires making everything generic in the resulting container F[_]*"，且"pattern matching and optimization is possible, but harder to implement"。Fabio Labella (SystemFw) gist：final tagless "avoid this roundtrip by … abstracting over whatever monad you are going to use in the end. If however you also want to modify this data structure, then Free is easier."

**开放世界 + litmus**：轴 A **additive**（新 provider = 对既有 type class 写新 `instance`，type class 开放）；轴 B 作为**新 type class** 引入亦 additive（JFP contribution #4："*extending the language does not invalidate terms already encoded*"），但**编辑既有 type class 加方法则其所有 instance 都要更新**。litmus：① `Q.Write` = **编译期**缺 `Write[Q]` instance → 不过（但按 De Goes，这不阻止某 instance 内部私自做 IO）；② 自动批处理**不由编码提供**（无 reified 结构，需 Applicative 显式表达或退回 Free/Haxl）；③ 写—结果仅靠值线程 + interpreter 内部日志；④ replay = 重跑 Scala 表达式 = **会**再次执行 Write。C-runtime 能力**无法**表为静态约束，只能退回运行期错误或降级为值级 `Option`/existential（此时不再是约束）。

### 条目 2 · Free / Freer monads 与 Data types à la carte

**设计中心**："程序即带回复延续的纯语法树"——Swierstra 用 `f :+: g` + `:<:` 开放地拼接指令，Freer 用 existential reply continuation + open `Union`/`Member` 移走 Functor 约束。解决"能力集合如何组合"，不自动解决 provider 身份/动态能力/replay 语义。

**核心类型（OBSERVED）**——Swierstra, *Data types à la carte*, JFP 2008, https://webspace.science.uu.nl/~swier004/publications/2008-jfp.pdf, §2–§4, pp.424–427：

```haskell
data Expr f = In (f (Expr f))
data (f :+: g) e = Inl (f e) | Inr (g e)
foldExpr :: Functor f => (f a -> a) -> Expr f -> a
class (Functor sub, Functor sup) => sub :<: sup where inj :: sub a -> sup a
inject :: (g :<: f) => g (Expr f) -> Expr f
```

Kmett `ekmett/free`@8cce810 的自然变换解释器：`foldFree :: Monad m => (forall x. f x -> m x) -> Free f a -> m a`（`src/Control/Monad/Free.hs#L330-L333`；注意一手 API 命名为 `foldFree`，非 `Data.Foldable.foldMap`）。Kiselyov & Ishii, *Freer Monads, More Extensible Effects*, Haskell 2015, https://okmij.org/ftp/Haskell/extensible/more.pdf, §2.4–§3.2：

```haskell
data FFree f a where
  Pure   :: a -> FFree f a
  Impure :: f x -> (x -> FFree f a) -> FFree f a       -- Functor 约束被移除
data Eff r a where
  Pure   :: a -> Eff r a
  Impure :: Union r x -> Arrs r x a -> Eff r a          -- Arrs = FTCQueue，续延队列
class Member t r where inj :: t v -> Union r v; prj :: Union r v -> Maybe (t v)
send :: Member t r => t v -> Eff r v
```

论文原话（OBSERVED）："*The monad instance for FFree f no longer needs the Functor or any other constraint on f.*" `freer-simple`@5304190 公开 API：`interpret :: forall eff effs. (eff ~> Eff effs) -> Eff (eff ': effs) ~> Eff effs`（`Control/Monad/Freer.hs#L262-L267`）。

**谁和谁能关联 / 抽象层**：R1 operation↔capability = `g :<: f`/`inject`、`Member t r`/`inj`（row-membership dictionary，C-compile）。R2 provider 身份：`:+:`/`Union`/`Member` **只承载 effect 标签，不承载 provider identity**（INFERENCE：要关联须放进 operation value/handler dictionary/token 或运行期 existential）。R3 多 effect↔因果：bind 树/`Arrs` 续延承载**程序顺序**，无独立性证明/因果 id/并发 scope。R4 program↔interpreter = `foldFree` 的 `f ~> m`、`interpret`（C-construct）；Swierstra 称 terms 是"pure Haskell values"可 inspect/transform（§7）——**这是"程序即数据、解释器可换"的审计/回放原型**。R5 intent↔审计/replay：续延不含 request key/log，审计需 handler 显式写记录，replay 需另一 handler 用记录回复；对真实 IO handler 再 fold 会**再次执行 Write**。

**组合律 / 语义歧义**：Free/Freer 的 monad 结构由构造保证（"a monad by the very construction, just like list is a monoid by construction"），但**这不是 Read/Write 的业务 law**。解释器顺序可换仅在特例成立——论文原话："*Since the reader and writer effects commute, the order of the interpreters can be switched without affecting the result*"，且"*handlers impose the order*"（§3.2）；state/exception/log 不应据此默认可交换。DTC 重复 summand 注入有歧义：`Val :+: Val` 选 `Inl` 或 `Inr`"*There is no reason to prefer one over the other*"（§4）。

**已发表批评（逐字，OBSERVED）**：
- Kiselyov & Ishii, *more.pdf* p.4 §2.6：**"*Free (and freer) monads are certainly elegant and insightful, but poorly performing.*"** 左结合 bind 处理 n 个请求为 O(n²)；`FTCQueue` 版把它改成 linear。
- van der Ploeg & Kiselyov, *Reflection without Remorse*, https://okmij.org/ftp/Haskell/zseq.pdf, p.10 §6.3：**"*many functors give rise to performance problems when using a (non-adapted) free monad*"** 以及 **"*the performance problem of iteratees did not go away by formulating it as a free monad*"**。
- Mark Karpov, *Free monad considered harmful* (2017-09-27, https://markkarpov.com/post/free-monad-considered-harmful)：**"*Free monads can't possibly compete with this approach in terms of performance.*"**（"this approach" = final-tagless/`mtl` 风格）；并称 Free 是"*somewhat tedious and inefficient (unless you're careful)*"。

**开放世界 + litmus**：轴 A/轴 B **都开放**——新 provider = 新 functor/label + `:<:`/`Member` 注入 + 新 interpreter，既有 coproduct algebra/`Eff` 核心不改；新 operation = 新独立 label 加入开放和式，但**某个 handler 必须新增 case、`run` 前须消费掉**（塞进既有 GADT 则所有穷尽 interpreter 都要改）。litmus：① `Q.Write` = **编译期**无 `Member Write '[Read,Stream]` → `send Write` 构造失败（若能力握手后才知，静态 row 只是过近似，须 existential/runtime error）；② 两 Read 可被 interpreter 批处理但**不因 `Eff` 类型自动发生**（`foldFree` 顺序消费；"同类型 ≠ 可并行"）；③ 写—结果由续延回复类型承载，request-key/审计不在类型内；④ replay 用真实 IO interpreter **会重做 Write**，用 replay handler 拦截并以 log 回复才不重做。

### 条目 3 · 代数效应与 handler

**设计中心**：**effect = operations + equations**；程序只声明操作接口（effect constructor），语义由动态作用域里最近的 **handler** 用被捕获的 delimited continuation `k`（resumption）给出（effect deconstructor）。"执行 = 在自由代数上做 fold，handler = 从自由代数到某模型的唯一同态"。

**核心类型（OBSERVED）**——Plotkin & Pretnar, *Handlers of Algebraic Effects*, ESOP 2009, DOI 10.1007/978-3-642-00590-9_7, §1："*algebraic operations and effect handlers are dual: the former could be called effect constructors … the latter … effect deconstructors*"；handling "*amounts to composing it with a unique homomorphism guaranteed by universality*"。Koka（Leijen, *Type Directed Compilation of Row-Typed Algebraic Effects*, POPL 2017, §2.1/§4）：`effect exc { raise(s:string) : a }`，`catch : (action:()→⟨exc|e⟩a, h:string→e a)→e a`；归约规则 `handle{h}(X_op[op(v)]) → e[x↦v, resume↦λy.handle{h}(X_op[y])]`——resumption 把上下文定界捕获并**重装回同一 handler**（deep handling）。OCaml 5（Sivaramakrishnan et al., PLDI 2021, arXiv:2104.00250）：`effect Yield : unit` + `perform`/`continue k`/`discontinue k exn`。Unison abilities："*Abilities are Unison's implementation of algebraic effects*"：`structural ability Store v where get:v / put:v->()`，`modifyStore : (v -> v) ->{Store v} ()`。

**谁和谁能关联 / 抽象层**：抽象 = **"声明操作接口"（静态，在 effect row / ability set / 约束里）与"选择解释"（动态，最近 enclosing handler）分离**，分层由 delimited continuation + 动态作用域实现（非 typeclass 字典）。R1：row membership（label 在 `⟨…⟩`/`{…}` 里，C-compile）+ 动态装入的 handler 子句。R2 provider 身份：Eff 用 effect instance/resource（`new E` 生成 fresh 实例）承载，Koka/OCaml 用"最近 enclosing handler"（运行期动态作用域）。R3 多 effect↔作用域：handler 定界的 `handle{…}(…)` 边界即作用域（运行期）。R4 program↔handler：`with h handle c` 词法包裹但**动态最近解析**。R5 intent↔审计/replay：传给 `resume k` 的值 + handler 自记录；**replay 身份是 handler 策略，不是类型**。Koka Lemma 4.b："*effect types cannot be discarded (except through handlers)*"。

**组合律 / 语义歧义（本条目重点：handler 顺序 non-commutativity）**——Leijen 2017 §2.4 可执行输出（OBSERVED）：程序 `surprising` 同用 `state⟨int⟩` 与 `amb`（多-shot）：
- state 在 amb 外层（全局共享）：`state(0,{amb(surprising)})` → `([False,False,True,True,False], 2)`；
- amb 在 state 外层（每分支局部）：`amb({state(0,surprising)})` → `[(False,1),(False,1)]`。
逐字："*we can compose them in two ways, giving rise to two different semantics … If we change the order of the handler, we effectively make the state local to each ambiguous resumption.*" hasura/eff@7d7f9ab 可复现 `Writer+NonDet` 两序不同：`[(Sum 3,(Sum 3,True)),(Sum 4,(Sum 4,False))]` vs `(Sum 6,[(Sum 3,True),(Sum 4,False)])`。**结合律一般不可表达**（Plotkin/Pretnar §7 OBSERVED）："*a general associativity of the handling construct … may not be expressible, as the model needed for H … may not be definable.*" state/exception 交互也要显式 rollback handler（§6.8："*the standard exception handler, extended to state, is not correct*"）。

**已发表批评（逐字，OBSERVED）**：
- Alexis King, hasura/eff README@7d7f9ab：**"*Other approaches to scoped operations (including those taken in mtl, fused-effects, and polysemy) have behavior that changes depending on handler order, and some combinations can lead to nonsensical results.*"** 动机是"*eff provides a consistent semantics for such operations regardless of handler order*"。
- 资源泄漏（multi-shot / non-resuming）——Daniel Hillerström, *A Compiler for Multi-shot Effect Handlers* (2016, https://www.dhil.net/blog/posts/2016-05-08-a-compiler-for-multi-shot-effect-handlers.html)：**"*Multi-shotness is not local to the handler that calls resume twice. It propagates through the captured continuation and affects all handlers and resources inside it.*"** OCaml 5 manual（https://ocaml.org/manual/5.2/effects.html）明言未 resume 的 continuation 会保留 fiber 内存与其持有资源，须以 `discontinue` 展开；Koka tour 的 file handler："*it would fail to close the file handle if an exceptional effect inside `action` is used (i.e. or any operation that never resumes)*"，须引入 `finally`/`initially`。工业实现为效率放弃 multi-shot——OCaml PLDI'21 §5.2："*Since our primary use case is concurrency, continuations will be resumed at most once … Multicore OCaml optimises fibers for one-shot continuations.*"

**开放世界 + litmus**：轴 A **强**（同一 effect 写新 handler：eff 的 `runFileSystemIO` vs `runFileSystemPure`）；轴 B **弱**（Koka HANDLE 规则要求 handler 覆盖 signature 全部 operation，新增操作使既有 handler 不再穷尽——但新增**独立 effect（新 row label）纯加性**）。litmus：① `Q.Write` = 若 Write 是独立 effect 不在 Q.action 的 row → **编译期**类型错误；若共享 effect 而 Q handler 省了 Write 子句 → **运行期** `Unhandled`（代数效应本身不静态编码"Q 无 Write"，除非每 provider 给不同 effect）；② 两 Read 天然顺序，批处理须写累积 operation 的 handler；③ 操作返回值经 `resume k` 传回，handler 是唯一能同时 emit 审计的单点；④ replay handler 可把 Write 解释成 no-op/日志，但 replay 身份是策略非类型。C-runtime：无匹配 handler 的 operation 在运行期 `Unhandled`（OCaml EffUnHn），**不能谎称静态安全**。

### 条目 4 · Haskell 效应库：能力子集的三种表达

**设计中心**：不是 provider object，而是 **effect row/sum 的 membership constraint/evidence + 可替换 handler/carrier**。本条只取"一个 provider 只支持能力子集"这一面，比较三种表达。

**核心类型（OBSERVED，均私有 clone 定 SHA）**：
```haskell
-- mtl@22fdb9e：能力作为约束 + transformer stack
class Monad m => MonadReader r m | m -> r where ask :: m r; local :: (r->r) -> m a -> m a
class Monad m => MonadState  s m | m -> s where get :: m s; put :: s -> m ()
-- effectful@a8aa96a：Eff 环境 + :> membership + Handler 值
newtype Eff (es :: [Effect]) a = Eff (Env es -> IO a)
class (e :: Effect) :> (es :: [Effect]) where reifyIndex :: Int
interpret :: (DispatchOf e ~ Dynamic) => EffectHandler e es -> Eff (e : es) a -> Eff es a
-- polysemy@cf2efec：Sem + Member proof + interpret
newtype Sem r a = Sem { runSem :: forall m. Monad m => (forall x. Union r (Sem r) x -> m x) -> m a }
class Member (t :: Effect) (r :: EffectRow) where membership' :: ElemOf t r
-- fused-effects@3f2d942：Has + sum membership + carrier Algebra
class Monad m => Algebra sig m | m -> sig where alg :: Functor ctx => Handler ctx n m -> sig n a -> ctx () -> m (ctx a)
type Has eff sig m = (Members eff sig, Algebra sig m)
```

**谁和谁能关联（三种表达，本条目核心）**：
- **能力作为约束（constraint）**：`(Reader Cfg :> es, Log :> es) => Eff es A` / `Member`/`Has`/mtl class。能力子集 = 约束子集（C-compile 缺 `Write` 即缺 dictionary/proof 不过）。但 `Member`/`:>`/`Has` **只证明 syntax/effect 存在，不证明 provider 身份或外部 API 真支持**。
- **能力作为值（record of functions / capability object）**：`data RW m = RW { readF::K->m V, writeF::W->m R }`（INFERENCE，非库原生 provider API）；P 持 `RW`，Q 只构 `RS`（无 `writeF`）。effectful 的 `EffectHandler` 是函数值、`Eff` 是"*essentially a `ReaderT` over `IO` on steroids*"，逼近值化 handler，但**无原生"Q record 没 Write 字段"的 provider 类型**。
- **能力作为证据（GADT witness）**：effectful 官方 `data FileSystem :: Effect where ReadFile :: FilePath -> FileSystem m String; WriteFile :: FilePath -> String -> FileSystem m ()`；`send :: (e :> es) => e (Eff es) a -> Eff es a`。构造子在**值层**携带参数/返回类型，`:>`/`Member` dictionary 在**类型层**携带 row membership proof。

R2 provider 身份：四库核心 API **无 provider-id**，须额外 value/token/existential（C-construct 或握手后）。R5 intent↔审计/replay：`Eff es a`/`Sem r a` 只给结果类型，request key/log/replay policy 须额外值/effect，Write 重放是否再执行**无库级保证**（INFERENCE）。

**组合律 / 语义歧义**：mtl 的 `lift` forwarding 使 stack 顺序影响 Error/State 交互（transformer semantics，非业务 law）。effectful 的 `Eff` 是 IO-backed concrete env，Haddock："*Allows the effects to be handled in any order*"，Error/State 文档："*no matter the order of effects, state updates made within the `catchError` block before the error happens always persist, giving predictable behavior*"——**这是该 effect 实现的语义，不是任意 handler/Write 可交换**。四库 row/constraint 都不自动把两 Read 批处理/并行。

**已发表批评（逐字，OBSERVED）**：
- Andrzej Rybczak, effectful benchmarks README（GHC 9.2.4）：**"*The deep version is 50 times slower than the reference implementation!*"**（指 mtl deep）；**"*`fused-effects` exhibits similar behavior as `mtl` … It augments them with additional machinery … which seems to add even more overhead though.*"**；**"*`polysemy` is based on free monads just as `freer-simple` and performs similarly, though with a much higher initial overhead.*"**
- Torsten Schmits, polysemy README（自我修正）：**"*Previous versions of this README mentioned the library being zero-cost, as in having no visible effect on performance.*"**；优化"*don't work in bigger, multi-module programs*"，导致"*visible drop in performance*"。
- Rob Rix, fused-effects README：**"*Unlike `mtl`, effects are automatically available regardless of where they occur in the signature; in `mtl` this requires instances for all valid orderings of the transformers (O(n²) of them, in general).*"**；又承认"*new effects take more code to define in `fused-effects` than `polysemy`*"。

**开放世界 + litmus**：轴 A——effectful/polysemy 新增 handler、fused 新增 carrier，通常不改调用者；mtl 需 newtype/instances；value record 只需新增 provider 值。轴 B——effectful/polysemy 要新增 GADT + 支持它的 interpreter case，fused 要新增 syntax/carrier；**既有 provider 不会自动获得新操作**；mtl 加 class method 要补所有实例（O(n²) instance 耦合）。C-runtime：四库 row/constraint 都不能静态更新 provider 事实，须 existential/refinement/value/token 或 runtime error。

### 条目 5 · 多 provider 副作用组合的库级范式：Haxl 调度语义 + Servant"一契约多解释"

**设计中心**：Haxl——**用 `Applicative`/`Monad` 两组合子把"哪些副作用能合并/并行、哪些必须顺序"编码进控制流**（`<*>` = 同一 round 可批处理/去重，`>>=` = 依赖顺序边界），provider 以开放世界 `DataSource` typeclass instance 接入；Servant——**把 API 提升为一个类型（契约）**，让 server/client/mock/docs 各作为一个 type class instance 解释同一契约，加后端 = 加 interpreter。

**核心类型（OBSERVED）**——Marlow et al., *There is no Fork*, ICFP 2014, https://simonmar.github.io/bib/papers/haxl-icfp14.pdf；Haxl@b33c1c1：
```haskell
newtype GenHaxl u w a = GenHaxl { unHaxl :: Env u w -> IO (Result u w a) }   -- Core/Monad.hs:441
-- Applicative <*> 两参都探测，两侧 Blocked 则合并进同一 round（论文 Fig.3）：
--   Blocked br1 c <*> Blocked br2 d = Blocked (br1 <> br2) (c <*> d)
class (DataSourceName req, StateKey req, ShowP req) => DataSource u req where     -- Core/DataSource.hs:94
  fetch :: State req -> Flags -> u -> PerformFetch req
data SchedulerHint (req :: Type -> Type) = TryToBatch | SubmitImmediately        -- :139
data BlockedFetch r = forall a. BlockedFetch (r a) (ResultVar a)                 -- :187
dataFetch :: (DataSource u r, Request r a) => r a -> GenHaxl u w a               -- Core/Fetch.hs:231
```
Servant@4f9e7c0（WGP'15, https://www.andres-loeh.de/Servant/servant-wgp.pdf，作者明说是 expression problem 一例）：
```haskell
data (path :: k) :> (a :: Type)          -- Servant/API/Sub.hs:25   infixr 4 :>
data a :<|> b = a :<|> b                  -- Servant/API/Alternative.hs:25
class HasServer api context where { type ServerT api (m :: Type -> Type) :: Type; route :: ... }
class RunClient m => HasClient m api where { type Client (m :: Type->Type) (api :: Type) :: Type; ... }
class HasDocs api where docsFor :: Proxy api -> (Endpoint, Action) -> DocOptions -> API
class HasServer api context => HasMock api context where mock :: Proxy api -> Proxy context -> Server api
```

**谁和谁能关联 / 抽象层**：Haxl——同一 round（一层 `Blocked`）内经 `<*>`/`traverse` 组合的请求"能关联在一块"→ 被 `br1 <> br2` 合并、按 `DataSourceName` 分组、按 request key（`Eq`+`Hashable`）去重/缓存；`>>=` 划 round 边界 = 依赖顺序。**"谁能并/谁必须顺序"不是配置项，而是作者用组合子写出来的**（R3 由 monad 结构承载，运行期由 scheduler 验证）。R5 replay 身份 = 持久化的 `DataCache`。Servant——契约（类型）与其多解释彻底解耦，同一 API 类型同时约束 server 与 client 使二者天然兼容（R4 由 `HasX` dictionary，C-compile；加后端 = 加 instance 不动契约）。

**组合律 / 语义歧义**：Haxl 的 `Applicative` **故意违反** `<*> = ap`（§5.4："*Arguably we broke the rules… This is clearly not the case for our Applicative instance.*"），正当性依赖外部前提：`dataFetch` 无可观测副作用、请求可去重幂等（**不是业务 operation 的 law**，INFERENCE）。Servant 的歧义在 interpreter **完整性**：每加 combinator 必须给每个 `HasX` 补 instance，否则编译期缺 instance 报错；且多 interpreter **可换 ≠ 语义等价**（mock 随机、docs 生成文本、server 真执行）。

**已发表批评（逐字，OBSERVED）**：
- Marlow et al. §5.4：**"*Therefore, dataFetch should not have any observable side-effects—all our requests must be read-only.*"**（一旦引入写，`<*>` 的乱序/并行/去重/replay 全不再安全）；§1："*When the business logic is only concerned with reading data … the programmer doesn't care about the order in which data accesses happen…*"
- Servant WGP'15 §3.3：**"*We are losing some kind-safety in this way, which is unfortunate. … the program will still fail at compile time. But the error messages may be worse than they would be otherwise, and we lack the extra guidance that stronger kinds would provide.*"**

**跨语言移植处理异构**：Fetch(Scala)@290c0a4 `trait DataSource[F[_],I,A]{ def fetch(id:I):F[Option[A]]; def batch(ids):F[Map[I,A]]; def maxBatchSize:Option[Int]; def batchExecution:BatchExecution }`——用类型参数 + 抽象 `F[_]`，把"该源批量能力"显式为**构造期已知**配置。Muse(Clojure)@8db4d5d `defprotocol DataSource/BatchedSource/LabeledSource`——无静态类型，`satisfies? BatchedSource` **运行期**判定是否可批处理（能力何时已知下移到运行期反射）。

**开放世界 + litmus**：轴 A **YES**（Haxl 新数据源 = 新 `DataSource` instance；Servant 新解释 = 新 `HasX` + instances，ships-with 解释"in no way privileged"）。轴 B——Haxl 新增某源请求构造子须改**该源** `fetch`，但**新增一类副作用（写）根本不可表达**；Servant 新增 combinator 须给**每个** interpreter 补 instance。litmus：① `Q.Write`：Haxl 写不在 DSL 里，"拒绝"是设计/运行期的（**不做 per-provider 写能力静态检查**）；Servant 若契约无 `Post`/`Put` combinator 则无 handler 槽/client 函数 = 编译期不存在（但这是"契约缺 endpoint"非"Q 无写能力"证明）；② 两 Read：Haxl 同 round 同源 → batch+dedup（**能**）；Servant 两 client 函数各自独立 IO（**不能**）；③ Haxl 的 `w` write-monoid 是可 memo 的日志累加器**非对 provider 的效果写**（勿混）；④ replay：Haxl 用满 cache 重跑"*will not fetch any data*"，因全读幂等故写本不存在。

### 条目 6 · Capability-passing / object-capability：Scala 3 capture checking / `CanThrow` / Odersky capabilities

**设计中心**：**副作用的"许可"是一个一等程序变量（object capability），"哪个闭包/值捕获了哪些许可"由类型上的 capture set 静态追踪**。effect 不再是结果值上的加法，而是"闭包捕获了某 capability"这一事实——于是 `map` 签名不变即 effect-polymorphic（"effect polymorphism for free"）。

**核心类型（OBSERVED；记法：论文 arXiv:2207.03402 用前缀 `{c}T`、universal `*`；nightly 文档@2e0fea3 用后缀 `T^{c}`/`->{c}`、`T^`=`T^{any}`，universal 更名 `any`）**：
```
-- capturing type：论文 "A capturing type is of the form {c₁,…,cₙ} T"；文档 "T^{c₁,…,cᵢ}"，"T^ is a shorthand for T^{any}"
-- 函数箭头即 capture 载体：A -> B 纯；A ->{c,d} B 捕获 c,d；A => B ≡ A ->{any} B
erased class CanThrow[-E <: Exception]                                   -- canthrow.md:58
infix type $throws[R, +E <: Exception] = CanThrow[E] ?=> R              -- canthrow.md:71
def usingLogFile[T](op: FileOutputStream^ => T): T                       -- basics.md:48
class Source[X >: CapSet <: CapSet^]                                      -- polymorphism.md:101
sealed trait Capability / trait SharedCapability extends Capability, Classifier / trait ExclusiveCapability
```
`throw Exc()` 处编译器"*will check that there is a capability of type `CanThrow[Exc]` … It's a compile-time error if that's not the case.*" `CanThrow` "*are compile-time only artifacts; they do not have a runtime footprint*"（erased）。

**谁和谁能关联 / 抽象层**：**能力作为值 + 类型级追踪（capture set）** 的组合（对照条目 4 的 constraint/value/evidence：这里能力靠**普通程序变量 token** 承载、抽象在**捕获该变量的闭包类型的 capture set** 上，方法签名不需加 effect 参数）。R1：capability token 值，类型是非空 capture set 的 capturing type；`throw`/副作用调用点要求作用域内有对应 given/参数（C-compile）。R2 provider 身份：手上具体 capability **值的身份**，capture set 把 provenance 钉在类型（`l: Logger^{fs}` 表示保留了 `fs`），验证 = 编译期 subcapturing `{l} <: {fs} <: {any}`。R3 多 effect↔作用域：共享的 local `any` 与 **level**（词法嵌套构成 subcapturing 层级），并发由 separation checking 处理 exclusive/shared。R4 program↔handler：`try` 为 body 合成 `CanThrow[…]`、`boundary` 提供 `Label`（C-compile），但 **capability 只授权不执行**。R5 intent↔审计/replay：**本机制不承载**（capability 是许可非 request key/log record，且常 erased；属条目 5/7 职责）。

**组合律 / 语义歧义**：subcapturing 是偏序（子集 ∪ + derivation），"*capabilities cannot be created from nothing*"（§3.1 sc-var）。scoped capability 防逃逸：local `any`/level 阻止持局部能力的闭包写进外层可变变量或返回（`var esc: File^ = null; esc = f // error`）。**关键语义歧义**（OBSERVED，arXiv §2）："*capabilities can delimit what effects can be performed at any point in the program but they by themselves don't perform an effect… For that, one needs a library or a runtime system.*"——即持有 `CanThrow[E]` 与真正抛/handler 顺序/resume 语义是两层；handler 顺序、状态×异常交互这类 monadic 组合律 capture checking **本身不表达**。capture tunneling 精度取舍：泛型实例化时 capture "tunnel through"，外层类型不显式反映内部捕获（潜在困惑）。

**已发表批评（逐字，OBSERVED）**：
- 官方文档 `canthrow.md`：**"*But as it stands, it does not give us enough mechanism to enforce the absence of capabilities for arguments to higher-order functions.*"**（L235-244）；**"*The only loophole arises for scoped capabilities - here we have to verify manually that these capabilities do not escape.*"**（L274，作者自评仅"95% static checking"）。
- `basics.md#L18`：**"*capture checking is still highly experimental and unstable, and it evolves quickly.*"**
- VirtusLab, *Comparing effect systems in Scala*：**"*It's still too early to tell how this approach to effect management will scale in larger and more complex scenarios however.*"**；**"*both Gears and Caprese itself are still in very active development and a lot of elements haven't landed yet.*"**（Caprese 目标据 scaladays 2025 talk 摘要："track effects, authority, resource usage, and lifetimes through capabilities"，缓解"what color is your function?"——仅凭搜索摘要，未逐字打开原 slide/视频。）

**开放世界 + litmus**：轴 A **不用改**（OBSERVED，核心命题："*map is classified as pure … the capabilities required by the argument are also capabilities required by the whole expression … we get effect polymorphism for free*"；"*map is already effect polymorphic even though we did not change its signature at all*"）——新 provider = 提供新 capability 值/given。轴 B = 加新 `trait X extends SharedCapability`（或 `CanThrow[NewE]`），只有需要它的签名去 demand，既有泛型代码不改（比 sealed ADT 穷尽 match 在两轴上都开放）。litmus：① `Q.Write`：仅当 Write 建模为需 `WriteCap` token 且 Q 不交出该 token → **编译期**拒绝（前提是"Q 无 Write"C-compile/C-construct 已知；C-runtime 握手才知则**无法静态拒绝**，退化运行期错误）；② capture checking 不表达批处理/去重（是条目 5 职责），并行安全由 separation checking；③④ **不表达**（`CanThrow` erased，能力不携带幂等键/执行历史，replay/幂等属 runtime/事件溯源层）。

### 条目 7 · Interpreter 分层与执行/审计分离：reify-then-execute + event sourcing

**设计中心**：先把意图/事实 reify 成不可变 command/instruction/event，再由可替换解释器执行；state 只是 event 流的 `fold`/projection。**这直接反驳"把 provider 字段列成大对象"**——一等对象是有顺序和身份的 log，state 是可重建、可缓存、可替换的派生值。

**核心类型（OBSERVED）**——Wlaschin, *Domain Modeling Made Functional*（media.pragprog.com/titles/swdddf/modeling.pdf, pp.7–10）："*In this book, we will model workflows and other processes as function types.*" `type ValidateOrder = UnvalidatedOrder -> Result<ValidatedOrder, ValidationError list>`，`Result` "*documents that ValidateOrder might have 'error effects'*"；`ContactInfo = EmailOnly | PostOnly | EmailAndPost` 目标是"*make illegal states unrepresentable*"。**workflow = pipeline of functions + `Result`/choice types**（分析非原文术语）。Bjarnason, *Purely Functional I/O in Scala* slides："*return a value to the caller that describes how we want to interact with the I/O system … embed an I/O scripting language*"：`sealed trait IO[F[_],A]`，`Req[F[_],I,A](i: F[I], k: I => IO[F,A])`，`def runIO[G[_]:Monad](f: F ~> G): G[A]`——同一 `ask` 程序既可 `ConsoleEffect: Console ~> Id` 执行、也可 `PureConsole: Console ~> State`。Jet/Equinox@9c8e8a7（F# event sourcing，"*distilled from Jet.com systems dating all the way back to 2013*"，"*Not a framework*"）`samples/Store/Domain/InventoryItem.fs`：`interpret : Command -> State -> Event[]`、`evolve : State -> Event -> State`、`fold = Array.fold evolve`；Tutorial："*State is never stored directly*"、"*Fold is folding the evolve function over all events to get the current state*"。

**谁和谁能关联 / 抽象层**：R1：`F`/`IO[F,A]` 的指令类型参数在 C-compile 界定可构造 operation，`F ~> G` 是 interpreter dictionary（C-construct）；普通 `Free[ProviderOp,A]` 无 capability-row，不能仅凭 reification 保证某 provider 支持某 operation。R2 provider 身份：Equinox 用 `Category`/`StreamId`/`StreamName` + `Cart.Events.ContextInfo={time;requestId}` 把请求键放进事件值——**身份若重要必须是命令/事件值或 stream key，不从 state record 字段相等推断**。R3 多 effect↔因果：Free `Req(i,k)` 续延显式保留先后；Equinox 用 stream version/token 做乐观并发边界；Nubank"*we have a correlate ID to know if you have seen that*"。R4 program↔handler：`runIO`/`runFree` 的 `F ~> G`、Equinox `ICategory`/`Decider`（可换 handler，但不据此断言语义等价）。R5 intent↔审计/replay：Equinox `Committed=(StreamName*ITimelineEvent[])`，事件值可带 `requestId`；但 **Tutorial 不持久化 Command，保存的是 Event**——"事件引用命令/审计结果"只在事件 record 明确带 request/correlation key 时成立。Nubank 承认缺口："*We don't see every request that happens before the database write and the events are effectively lost.*"

**组合律 / 语义歧义**：`fold` 是按 log 顺序左折叠 `evolve`；若 `evolve` 纯且事件流/代码/版本相同，同一流 replay 得同一 state（INFERENCE）；但**事件一般不交换**——Nubank "*ordering matters*"、"*interleaving of events creates a combining explosion*"（穷尽单测不可行）。**不要把 Monad laws 当成业务 entries 的 commutativity law**。retry：Equinox 冲突时用新 state 重跑 `interpret/decide`，若混入外部 I/O/时间/非幂等则重试非纯重算（Wlaschin："*Result … is only for expected control-flow, not for unexpected situations*"，"*Don't use Result to reinvent exceptions*"）。**执行 vs 审计/replay**：重新 fold Event 不重调原 Command、不应重发 Write；但重跑 subscriber/handler 或把旧 command 交给 production interpreter 可能重发副作用。

**已发表批评（逐字，OBSERVED）**：
- Martin Fowler, *What do you mean by 'Event-Driven?'* (2017)：**"*Event sourcing does have its problems. Replaying events becomes problematic when results depend on interactions with outside systems. We have to figure out how to deal with changes in the schema of events over time. Many people find the event processing adds a lot of complexity to an application*"**；*Event Sourcing*："*if these events cause update messages to be sent to external systems, then things will go wrong because those external systems don't know the difference between real processing and replays.*"；*Memory Image*："*it's too slow to replay all the events*"、"*Migrating the event log is often more hassle*"。
- Nubank QCon transcript：**"*any message that goes on Kafka has to be idempotent because you can consume the same message more than once*"**（不依赖 exactly-once）。

**开放世界 + litmus**：轴 A——新 provider 只实现既有 `F`/`ICategory`/event contract，新增 `F ~> G` 或 store adapter 并 wiring，既有 `decide/evolve` 不改（INFERENCE）。轴 B——对闭合 `Command`/`Event` DU，新增 case 必须更新 `interpret/decide`、`evolve/fold`、codec 和每个 handler（exhaustive match 强迫漏项显现，代价是既有 interpreter 改动重编译）。litmus：① `Q.Write`：精确 instruction algebra 可构造期拒绝，开放 `ProviderOp` sum 通常只能构造期/runtime `Result` 拒绝（ES 不提供 capability proof）；② 两 Read：同一 stream 默认保序，跨 stream/显式并发解释器才并行且合并规则需另定义；③ `Transact(decide)` 回传 result，accepted Event 由 stream version/timeline/`requestId` 关联；④ 纯 `evolve/fold` 只重建 state，**重跑 command 或未屏蔽 subscriber 会再 Write**（Fowler：replay 模式须由 gateway 抑制外部更新）。

### 条目 8 · 异构能力矩阵的类型级方案

**设计中心**：把关系压到四种载体——Beam 的 backend/constraint 字典、Amazonka 的生成 request/response 名称、row 的字段成员资格、Idris 的 effect/resource 状态索引。都能编译期拒绝一部分非法组合，**都不能凭静态类型证明一个会变化且未文档化的外部 provider 永远支持某操作**。

**核心类型（OBSERVED，均定 SHA）**：
```haskell
-- Beam@77d0bca：backend 作为约束的"行"，syntax class 作为"能力列"
class (BeamBackend be, Monad m) => MonadBeam be m | m -> be where
  runReturningMany :: FromBackendRow be x => BeamSqlBackendSyntax be -> (m (Maybe x) -> m a) -> m a
class HasSqlValueSyntax expr ty where sqlValueSyntax :: ty -> expr
outerJoin_ :: BeamSqlBackendSupportsOuterJoin be => ...   -- 可选列 = 可选 constraint
-- Amazonka@b562aa3：provider 描述(botocore) → 生成 request/response 类型
class (Typeable a, Typeable (AWSResponse a)) => AWSRequest a where
  type AWSResponse a :: Type
  request  :: (Service -> Service) -> a -> Request a
  response :: MonadResource m => ... -> m (Either Error (ClientResponse (AWSResponse a)))
-- rows：能力矩阵作为 row，字段成员资格 = 谁支持谁
data Rec :: (u -> Type) -> [u] -> Type                    -- Vinyl@b1de389
newtype Row a = R [LT a]; (r :: Row k) .! (t :: Symbol)   -- row-types@f49248b：.+ 要求 disjoint
-- Idris：dependent effect/resource state index
Eff  : (m:Type->Type) -> (es:List EFFECT) -> (a:Type) -> Type
EffM : (m:Type->Type) -> (es:List EFFECT) -> (es':List EFFECT) -> (a:Type) -> Type  -- 状态迁移
putM : y -> EffM m [STATE x] [STATE y] ()                 -- Brady ICFP'13 §2.2.1
```

**谁和谁能关联 / 抽象层**：provider = 行（`be`/生成模块/`Rec`/`es`），能力 = 列（syntax class/request shape/label/effect）。R1 "谁支持谁"由 instance 存在性或 row 成员资格在 **C-compile** 判定（Beam 缺 syntax instance → 拒绝；`r .! #cap`/`EffElem` proof）。R2 provider 身份：Beam `be` fundep、Amazonka 生成模块名、row 的 provider 参数、effect/resource label。R5 intent↔审计/replay：四族签名**均不自动给出** request key/log/replay 身份，须值/key/record 另编码。

**组合律 / 语义歧义**：Vinyl `rappend` 保 type-level list 顺序（非交换），row-types `.+` disjoint union、`.\/`/`.\\` 明确冲突策略；这些是字段并/交/覆盖，**不是 handler 顺序/并发/异常语义**。Idris `(>>=) :: EffM m es es' a -> (a -> EffM m es' es'' b) -> EffM m es es'' b` 只说输出 resource list 进入下一步，不赋业务交换律；重复同类 state 默认选较早 effect："*In practice, the earlier effect is chosen. While clearly defined, this is unlikely to be the desired behaviour*"。已有 instance 也不证明远端版本永不变化。

**已发表批评（逐字，OBSERVED）**：
- Beam 官方文档（`docs/user-guide/queries/basic.md:L106-L109`）：**"*These types may seem incredibly complicated. Indeed, the safety that beam tries to provide requires these scary-looking types.*"**
- Amazonka README（Brendan Hay 等，`README.md:L170-L178`）：**"*Service configurations generated in this way are intended as examples only and the resulting … should be manually verified and curated as necessary.*"**
- row-types 官方示例（`examples/Examples.lhs:L192-L215`）：**"*GHC is a little finicky about the type operators and constraints -- indeed, some slight modifications to the signature can easily cause type checking to fail.*"**；Vinyl `Data/Vinyl/Core.hs:L26-L35`：**"*This usually helps with runtime performance, but can slow down compilation time.*"**
- Edwin Brady, ICFP 2013 §6：**"*We make no attempt to infer effect types*"**；**"*there is a small interpreter overhead since EffM is represented as an algebraic data type, with an associated interpreter*"**；**"*mixing control and resource effects is a challenge.*"**

**开放世界 + litmus**：轴 A——Beam 新 backend = 新包实现 `BeamBackend`+syntax instances+`MonadBeam`（既有 `be`-polymorphic 查询不改，代价是大量 syntax 字典）；Amazonka 新 service = 新 botocore 描述 + 新生成 package（核心 `AWSRequest` 不改，代价是上游模型/人工核对）；rows 新 provider = 新 row 值（核心 row algebra 不加 case）；Idris 新 interpreter = 新 `Handler e m`。轴 B——Beam 新 syntax class 要改核心与每个支持它的 backend；Amazonka 跨服务 operation 要改 generator/core/templates 重生成；rows 新 operation = 给各 row 加一列 + 消费者要求 membership；Idris 要扩 `Effect`/`EffElem`。litmus 汇总：

| 机制 | Q.Write 如何拒绝 | 两 Read / Write 关联 | Write replay |
|---|---|---|---|
| Beam | 缺 syntax/constraint 时 C-compile；未建模的业务能力不拒绝 | 查询值可关联；批处理/并行、audit key 是 runtime semantics | `run`/外部 DB 可再次执行，无静态防重 |
| Amazonka | 无生成 request 时构造期/C-compile；权限/漂移 runtime `Error` | `AWSResponse a` 只绑响应 shape | `send` 重发即可能重复，幂等不在 `AWSRequest` |
| rows | 缺 row membership 时 C-compile；动态 record 可能 runtime | field 可携带 key/dictionary，但 row algebra 不给因果/并行 | record 类型不决定 replay |
| Idris | 缺 `EffElem` C-compile；握手后须 existential/refinement | effect list/resource 可证明范围与状态迁移，handler 才定并行/审计 | handler 可纯 replay 或真实重做，类型不替业务决策 |

综合（INFERENCE）：相对条目 4/6，row/type-level capability 的独特优势是把"provider 支持哪些列"作为可组合的 membership/subset 证明，**新增 provider 不需中心 `case`**；相对代价是类型推导、错误信息、编译时间和运行时 witness 边界。四种机制都解决"可表达的静态形状/状态"，**没有一种单独解决开放 provider 的真实能力、因果关联、审计不可伪造或 replay exactly-once**。

---

## 横向对比矩阵

取值受控：`native`（机制原生表达）/`encoded`（可编码但需额外机制）/`convention`（靠约定/纪律）/`unsupported`（表达不了）/`n/a`。"性能""学习成本"仅在有同口径一手证据时给方向，否则标"未建立"。

| 方案 | 开放世界·轴A(加provider) | 开放世界·轴B(加operation) | 能力子集表达 | 审计/回放 | 解释器可换 | 性能(一手证据) | 学习成本 | 语义歧义(主要来源) |
|---|---|---|---|---|---|---|---|---|
| 1 Tagless final | native（加 instance） | encoded（加新 type class；改既有 class 贵） | 约束子集(C-compile) | unsupported（无 reified 程序，replay 重执行副作用） | native（换 instance≠语义等价） | 通常优于 Free（Karpov/Warski 定性） | 中 | 无 handler 顺序歧义；但无 laws→无法通用推理(De Goes) |
| 2 Free/Freer + DTC | native（新 functor+`:<:`/`Member`） | native（新 label；塞旧 GADT 则全改） | row-membership 证据(C-compile) | native（程序即数据可 inspect/replay，副作用安全需 replay handler） | native（`f~>g`） | 左结合 bind O(n²)；FTCQueue 转 linear(Kiselyov/RwR) | 中高 | 解释器顺序只在特例可换；重复 summand 注入歧义 |
| 3 代数效应+handler | native（同 effect 新 handler） | encoded（新独立 effect 加性；给旧 effect 加 op 须改各 handler） | effect-row 成员(C-compile) | encoded（replay 是 handler 策略非类型） | native（handler） | multi-shot 贵，工业退化 one-shot(OCaml'21) | 高 | **handler 顺序 non-commutativity**；结合律不可表达；multi-shot 资源泄漏(King/Hillerström) |
| 4 effectful/polysemy/fused/mtl | native（handler/carrier） | encoded（新 GADT+interpreter case） | constraint / value / GADT-evidence 三选 | encoded（须额外 effect/log） | native | mtl deep 慢50×；polysemy 高初始开销(effectful bench) | 中高 | transformer/handler 顺序影响 Error×State |
| 5 Haxl / Servant | native（`DataSource`/`HasX` instance） | Haxl:写不可表达 / Servant:每 interpreter 补 instance | Haxl:请求类型；Servant:契约 combinator | Haxl:cache 为 replay 身份(须只读幂等) / Servant:unsupported | Haxl:n/a / Servant:native(一契约多解释) | Haxl 自动 batch/dedup(设计目标) | 中高 | Haxl 故意违反`<*>=ap`(须只读)；Servant 错误信息差 |
| 6 Scala3 capability/CanThrow | native（新 capability 值/given；`map` 签名不变即多态） | native（新 `Capability` trait） | capability 值 + capture set(C-compile) | unsupported（capability 只授权不执行，`CanThrow` erased） | encoded（capability 只界定不执行语义） | 未建立(erased 无运行期足迹) | 高(实验性/不稳定) | capture tunneling 精度；防逃逸不完整(95%) |
| 7 reify + event sourcing | native（新 `F~>G`/store adapter） | encoded（新 Command/Event case 改 decide/evolve/codec） | encoded（精确 algebra 才 C-compile） | **native（不可变 log + fold；一等审计）** | native（`F~>G`/store） | Memory Image:全量 replay 太慢(Fowler) | 中 | 事件不交换、顺序敏感；replay 重放外部副作用(Fowler/Nubank) |
| 8 beam/amazonka/rows/Idris | native（新 backend/service/row/handler） | encoded（新 syntax/生成/列/effect） | backend-constraint / codegen / row-membership / dependent-witness | unsupported（签名不给 request key/replay） | native | Vinyl:编译期变慢；EffM interpreter 开销(Brady) | 高(类型级) | 字段并交覆盖≠effect 语义；codegen 依赖上游描述 |

跨全表的两条不变结论：**(i) 无一方案原生表达 C-runtime 未文档化能力**——全部退化为 existential/refinement 或运行期错误；**(ii) 审计/回放的"一等"程度与"程序是否被 reify 成不可变数据"正相关**——条目 2/7 native，条目 1/6/8 unsupported（除非另加 log 层）。

## 对 UTA 的可迁移命题

格式："案例 X 在条件 Y 下用机制 Z 解决了问题 W；UTA 若满足 Y 可迁移，否则不可。" 不设计 UTA、不发明字段/对象/状态机，只挂案例出处。

1. **[效应簇的设计中心]** 条目 7（Equinox/Nubank/Wlaschin）在"效应低频、不可丢、有资金后果、须可审计"条件下，用机制"intent/decision/attempt/回执 reify 成不可变、带 correlation/causation key 的 append-only log，state = 该 log 的 `fold`/projection"解决了"副作用怎么消费 + 谁和谁能关联 + 避免大对象"（Nubank"*you never do an update in place and lose that history*"）。UTA 效应簇满足该条件 → **可迁移设计中心是"log + fold"而非"装满 provider 字段的 record"**；观察簇（高频可丢）不满足"不可丢/须审计" → 不可照搬，属另一机制族（条目 5 调度）。

2. **[谁和谁能关联的分层承载]** 五案例（条目 2/3/5/6/7）一致表明"谁和谁能关联"须拆成五种关系分层承载：operation↔capability 用类型证据、intent↔result/audit 用 log record + correlation id（条目 7 R5）、request↔provider 身份用值级 key（条目 5 Haxl request key、条目 7 stream key）。UTA 只要接受"关联不是一个大对象的字段互指，而是分层承载物"→ **可迁移"关联即证据/键/记录"的分解**；若坚持把关联塞进单一 object 字段 → 五案例均判为反模式。

3. **[能力矩阵不可静态假装]** 全部八条目一致：C-runtime、部分未文档化的能力**无法**表为静态保证（条目 4/6/8 明列，Beam"scary-looking types"也只覆盖已建模 syntax）。UTA 的 venue 能力矩阵按 F6"部分未文档化" → **可迁移命题是"能力声明必须是值级、运行期可获得、可为 unknown 的证据（条目 5 Muse `satisfies?` 运行期反射是范式），而非编译期类型约束"**；若强行做成类型级 capability → 与 F6 冲突，八案例均判不可。

4. **[写副作用不能套用读的自动组合]** 条目 5（Haxl §5.4"*all our requests must be read-only*"）在"纯读、可去重幂等"条件下用 `Applicative`=并行/`Monad`=顺序解决自动批处理/去重；一旦引入写则乱序/去重/replay 全不安全。UTA 观察簇（高频可丢读）满足条件 → **可迁移 Applicative/Monad 双语义调度**；效应簇（有资金后果的写、F5 结果不可判定）**不满足** → 不可用 Haxl 式自动组合，须回到条目 7 的显式 log/顺序（对应域内"unknown 不得自动重试"）。

5. **[解释器可换 ≠ 语义等价，且 replay 会重放外部写]** 条目 2/3/7 一致：程序 reify 后可换 test/prod/replay 解释器（条目 2 `f~>g`、条目 7 `F~>G`），但 Fowler"*external systems don't know the difference between real processing and replays*"。UTA 若要"重启后恢复/回放程序状态"（H9） → **可迁移"replay 只重算纯决策、外部写由 gateway/边界抑制"（条目 7）**；若期望 replay 自动重发 venue 写而幂等 → 与 F5 及案例批评冲突，不可。

6. **[组合语义歧义要显式定序，不能靠默认]** 条目 3（Koka amb/state 两序两义、Plotkin/Pretnar 结合律不可表达）与条目 7（Nubank"*ordering matters*""*interleaving … combining explosion*"）表明多效应组合无默认可交换语义。UTA 的"消费有序与否由程序声明（await-all/latest+窗口/ordered，B5）" → **可迁移命题是"顺序语义必须由程序显式声明、由机制显式定界，禁止依赖 handler/interpreter 装配顺序的隐式默认"**；任何"默认可交换"假设被条目 3/7 直接证伪。

7. **[capability-passing 授权与执行分离，但不承载审计]** 条目 6（Scala3）在"想让 effect 信息由类型检查而非 monad 编码"条件下用 object capability + capture set 解决授权/防逃逸，但"*capabilities … by themselves don't perform an effect*"且 `CanThrow` erased 不承载 request/audit 身份。UTA 的授权（C11 principal × 账户 × 操作种类）与 H7 信任模型 → **可迁移"能力作为授权 token、作用域受限"的思路（概念，非 Scala 语法）**；但审计/关联（S8"谁、何时、依据什么"）**不可**由 capability 承载，须回条目 7 的 log record。

8. **[开放 provider 的加法性由 coproduct/row/instance 提供]** 条目 2（`:+:`/`Member`）、条目 5（`DataSource` 开放 typeclass）、条目 8（row membership、backend constraint、codegen）一致：加新 provider 不改核心，只加 instance/label/row/生成包。UTA 面对开放 venue 集合（Alpaca…将来还加） → **可迁移"provider = 开放实例，核心对其多态；加 provider 是 additive"**；但对偶代价（加新 operation 种类要改各 provider 的 interpreter，即轴 B）是这些机制共有的、UTA 无法回避的 expression-problem 取舍。

## 未覆盖与开放问题

- **未逐字打开**：Caprese 的 scaladays 2025 talk/slide 原件与 reddit 讨论仅凭搜索摘要（条目 6）；本环境禁用 `read` 的 URL 读取，PDF/HTML 均经 `curl`/搜索获取，个别动态页未得 archive 锚点。这些均未作为主要依据。
- **本报告未展开、属兄弟报告**：金融行业实证（Marlowe/DAML/Tsuru 等）→ fp-01；Scala/JVM 具体工程案例（ZIO/fs2/Fetch-Stitch）→ fp-02；money/quantity/time/id/ordering/error/unknown 的基础类型定义 → fp-04；FRP/增量指标计算（TradingView/pinescript 式增量、事件时间/差分）→ fp-05。本报告只在可迁移命题里交叉引用。
- **理论未收口**：C-runtime 能力的 existential/refinement **构造模式**（探测后如何构造带证明的 dependent pair，把运行期事实抬进类型）八条目均点到未展开——这是"部分未文档化能力"最需要的一手证据缺口，值得单独 spike（条目 6/8 提出但无成熟一手实现可引）。
- **未做**：跨机制的同口径性能基准（effectful bench 只覆盖 Haskell 四库，无 Free vs tagless vs 代数效应 vs event-sourcing 的可比数字）；multi-shot 资源安全的形式化保证（条目 3 只有 one-shot 工业实现与 Hillerström 的 leaky-abstraction 定性）。

## 来源清单（编号、URL、打开状态、本地路径）

仓库均由子代理 `git clone` 到私有临时目录并定 commit 后 `read`/`grep`；PDF 经 `curl` 下载本地后 `read` 全文（`read` 的 URL 直读被环境禁用）。

1. Carette, Kiselyov, Shan. *Finally Tagless, Partially Evaluated*. JFP 2009. https://okmij.org/ftp/tagless-final/JFP.pdf ——打开成功（§1 pp.3-5, §2.1 p.7, contrib #3/#4 p.6）。
2. Kiselyov. *Tagless-final style* 主页. https://okmij.org/ftp/tagless-final/index.html ——打开成功。
3. De Goes. *The False Hope of Managing Effects with Tagless-Final in Scala* (2019). https://degoes.net/articles/tagless-horror ——打开成功（全文逐字）。
4. Warski. *Free and tagless compared* (2017). https://softwaremill.com/free-tagless-compared-how-not-to-commit-to-monad-too-early/ ——打开成功（medium 镜像被 Cloudflare 拦截，用官网原文等价替代）。
5. Labella (SystemFw). *[DRAFT] Free vs Final tagless*. https://gist.github.com/SystemFw/2aa8eefa997022e5fbf195c98f7586aa ——打开成功。
6. Swierstra. *Data types à la carte*. JFP 2008. https://webspace.science.uu.nl/~swier004/publications/2008-jfp.pdf ——打开成功（§2–§7 pp.424-435）。
7. Kiselyov & Ishii. *Freer Monads, More Extensible Effects*. Haskell 2015. https://okmij.org/ftp/Haskell/extensible/more.pdf ——打开成功（§2–§4 pp.1-7）。
8. van der Ploeg & Kiselyov. *Reflection without Remorse*. https://okmij.org/ftp/Haskell/zseq.pdf ——打开成功（p.10 §6.3）。
9. `lexi-lambda/freer-simple` @`5304190c1deae1fa8905144ed79774e90d9c7247` ——打开成功（README、`Control/Monad/Freer*.hs`、`Data/OpenUnion*.hs`、`Data/FTCQueue.hs`）。
10. `ekmett/free` @`8cce810392abbde362a8c6be3a48d85f5f9b2c29` ——打开成功（`src/Control/Monad/Free.hs`）。
11. Karpov. *Free monad considered harmful* (2017). https://markkarpov.com/post/free-monad-considered-harmful ——打开成功（逐字）。
12. Plotkin & Pretnar. *Handlers of Algebraic Effects*. ESOP 2009, DOI 10.1007/978-3-642-00590-9_7 ——打开成功（§1, §6.8, §7）。
13. Bauer & Pretnar. *Programming with Algebraic Effects and Handlers*. arXiv:1203.1539 ——打开成功（§1–§2）。
14. Leijen. *Type Directed Compilation of Row-Typed Algebraic Effects*. POPL 2017 ——打开成功（§2.1, §2.4, §3.1, §4）。
15. Sivaramakrishnan et al. *Retrofitting Effect Handlers onto OCaml*. PLDI 2021, arXiv:2104.00250 ——打开成功（§1, §3.1, §4.2.4, §5.2）。
16. OCaml 5 manual — Effect handlers. https://ocaml.org/manual/5.2/effects.html ——打开成功（资源泄漏/`discontinue` 讨论）。
17. Hillerström. *A Compiler for Multi-shot Effect Handlers* (2016). https://www.dhil.net/blog/posts/2016-05-08-a-compiler-for-multi-shot-effect-handlers.html ——打开成功（逐字）。
18. Unison Language Reference — Abilities and ability handlers. https://unison-lang.org/docs/language-reference/abilities-and-ability-handlers ——打开成功（抓取 2026-09-16）。
19. `koka-lang/koka` @`3ac4f001ab7277b484d661fdbada1aaf8d01ecbf` ——打开成功（`doc/spec/tour.kk.md`）。
20. `hasura/eff` @`7d7f9ab77f3c7f473d52457e41e9b6e1870d72f6` ——打开成功（README、`Control/Effect.hs`、`test/Control/EffectSpec.hs`）。
21. `haskell-effectful/effectful` @`a8aa96a2ba978db2f86502432792444e133ea469` ——打开成功（`Internal/{Monad,Effect}.hs`、`Dispatch/Dynamic.hs`、`Error/Static.hs`、README、benchmarks/README）。
22. `polysemy-research/polysemy` @`cf2efec15ae57a46a81694d361da55da8d7b1d73` ——打开成功（`Internal.hs`、`Internal/Union.hs`、`Internal/Combinators.hs`、README）。
23. `fused-effects/fused-effects` @`3f2d942d701238173923cc066e583ecb91fbdcaf` ——打开成功（`Control/Algebra.hs`、`Control/Effect/Sum.hs`、README）。
24. `haskell/mtl` @`22fdb9e46bb097fe79a76f146257d1f202e51ae2` ——打开成功（`Control/Monad/{Reader,State}/Class.hs`）。
25. Marlow, Brandy, Coens, Purdy. *There is no Fork*. ICFP 2014. https://simonmar.github.io/bib/papers/haxl-icfp14.pdf ——打开成功（§1, §4, §5.4, §6.2）。
26. `facebook/Haxl` @`b33c1c1f132fa1d704cc0f7b6233f9c86bb50afe` ——打开成功（`Core/DataSource.hs`、`Core/Monad.hs`、`Core/Fetch.hs`）。
27. `47degrees/fetch` @`290c0a4e991553e98c4a7359247195feaed03162` ——打开成功（`datasource.scala`）。
28. `kachayev/muse` @`8db4d5de82a8acccb4486cc7cb6de045a0df9328` ——打开成功（`protocol.cljc`）。
29. Mestanogullari, Hahn, Arni, Löh. *Type-level Web APIs with Servant*. WGP 2015. https://www.andres-loeh.de/Servant/servant-wgp.pdf ——打开成功（§1.2, §3.3, §4.2–4.3, §6）。
30. `haskell-servant/servant` @`4f9e7c0dc76bf6ea0423d4d7b80d6a9acd6d31a6`；`servant-mock` @`f6c354abe8df43440baf2b2aa9d6abf5df42d094` ——打开成功（`HasServer/HasClient/HasDocs/HasMock/:>/:<|>/Get`）。
31. Odersky, Boruch-Gruszecki, Lee, Brachthäuser, Lhoták. *Scoped Capabilities for Polymorphic Effects*. arXiv:2207.03402 ——打开成功（§1–§4, §3.1, Prop 3.1, Lemma 4.15）。
32. Scala 3 官方文档仓库 `scala/scala3` @`2e0fea38441dd29dd887a712c49f055159f0d8a4` ——打开成功（capture-checking `basics.md`/`polymorphism.md`/`scoped-capabilities.md`、`canthrow.md`、`classifiers.md`、`advanced.md`）。
33. VirtusLab. *Comparing effect systems in Scala* (PDF). https://s3.eu-central-1.amazonaws.com/images.virtuslab.com/Comparing_effect_systems_in_Scala_aaa8acacba.pdf ——打开成功。
34. SoftwareMill. *The future of effects in Scala*. https://softwaremill.com/the-future-of-effects-in-scala/ ——打开成功。
35. Caprese: slideshare "Capabilities for Resources and Effects" + scaladays 2025 talk ——**仅凭搜索摘要，未逐字打开原 slide/视频**（未作主要依据）。
36. Wlaschin. *Domain Modeling Made Functional*（出版方摘录）. https://media.pragprog.com/titles/swdddf/modeling.pdf ——打开成功（pp.7-10）。
37. Wlaschin. fsharpforfunandprofit.com：*recipe-part1/part2*、*designing-with-types-making-illegal-states-unrepresentable*、*against-railway-oriented-programming*、DDD talk 页 ——打开成功。
38. Bjarnason. *Purely Functional I/O in Scala* slides https://blog.higher-order.com/assets/scalaio.pdf；*Free monads and free monoids* https://blog.higher-order.com/blog/2013/08/20/free-monads-and-free-monoids/ ——打开成功。
39. `jet/equinox` @`9c8e8a7797bf74726502ba036ee6577cdabed424` ——打开成功（README、`samples/Store/Domain/{InventoryItem,Cart}.fs`、`src/Equinox/{Decider,Category}.fs`、`src/Equinox.MemoryStore/MemoryStore.fs`、`samples/Tutorial/Counter.fsx`）。
40. Nubank QCon transcript (Wible/Ferreira). *Architecting a Modern Financial Institution*. https://www.infoq.com/presentations/nubank-architecture/ ——打开成功。
41. Fowler. *Event Sourcing* https://martinfowler.com/eaaDev/EventSourcing.html；*What do you mean by 'Event-Driven?'* https://martinfowler.com/articles/201701-event-driven.html；*Memory Image* https://martinfowler.com/bliki/MemoryImage.html ——打开成功。
42. `haskell-beam/beam` @`77d0bca3c1cda8364d7a7cf95881cd826feed8ec` ——打开成功（`beam-core/Database/Beam/Backend/SQL*.hs`、`Query*.hs`、`docs/user-guide/queries/basic.md`）。
43. `brendanhay/amazonka` @`b562aa3f24845e34b95748daae671860017426be` ——打开成功（`amazonka-core/.../Types.hs`、`gen/src/Gen/AST/Data*.hs`、`amazonka-s3/.../GetObject.hs`、README）。
44. `VinylRecords/Vinyl` @`b1de389c079191fe414321594b1b95e60f9ab033`；`target/row-types` @`f49248b6f175cc43fcc801e26c07a2f90e29b3c5`；`milessabin/shapeless` @`5d93df4b97d50469b06a37b09e98d7344d3d2d8f` ——打开成功。
45. Brady. *Programming and Reasoning with Algebraic Effects and Dependent Types*. ICFP 2013. https://www.type-driven.org.uk/edwinb/papers/effects.pdf ——打开成功（pp.2-7, 11）。

**统计**：8 组研究、45 条来源；逐字批评原话覆盖反 tagless final（De Goes）、反 free monad（Karpov/Kiselyov）、反代数效应（Hillerström/OCaml manual/King）三方向各 ≥1 处。未能逐字打开原件仅 1 条（#35 Caprese talk/slide），未作主要依据。
