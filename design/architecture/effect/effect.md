# 受控写边界连接

```vdm
module Effect
imports
  from Source
    types
      SourceId renamed SourceId;
      CapabilityKey renamed CapabilityKey
    operations
      CurrentCapability renamed CurrentCapability,
  from Observation
    types
      RefHolder renamed RefHolder;
      LogPosition renamed LogPosition
    operations
      AcquireRef renamed AcquireRef;
      ReleaseRef renamed ReleaseRef
exports
  types
    LaneKey;
    AttemptId;
    IntentPayload;
    struct TerminalOutcome;
    struct Attention;
    struct InDoubt;
    struct Resolved;
    struct AttemptStage;
    struct Attempt;
    struct Evidence;
    struct PrepareOutcome;
    struct BarrierOutcome
  functions
    Blockers: LaneKey * map AttemptId to Attempt -> set of AttemptId
  operations
    Prepare: LaneKey * IntentPayload * set of LogPosition * [set of AttemptId] ==> PrepareOutcome;
    Barrier: AttemptId * SourceId * CapabilityKey ==> BarrierOutcome;
    CompleteSend: AttemptId * (TerminalOutcome | <NoResponse>) ==> ();
    Resolve: AttemptId * Evidence ==> ();
    Abandon: AttemptId ==> ()
definitions
types
  -- 串行化外部写的通道键；结构对架构不透明，由业务单元映射
  LaneKey = token;

  -- 一次外部写尝试的身份
  AttemptId = token;

  -- 写意图的载荷；对架构不透明
  IntentPayload = token;

  -- 已由权威证据确立的写尝试终态：上游受理、上游拒绝或确知未发出
  TerminalOutcome = <Accepted> | <Rejected> | <NotSent>;

  -- 结果未知时的本地等待状态：仍在阻塞等待，或已由操作员放弃本地等待
  Attention = <Waiting> | <Abandoned>;

  -- 写已越过发送屏障但上游结果未知；放弃等待只改变 attention，不产生终态结论
  InDoubt ::
    attention : Attention;  -- 本地是否仍在阻塞同通道后续普通写

  -- 写尝试的结果已由来源证据最终确立
  Resolved ::
    outcome : TerminalOutcome;  -- 最终确立的结论

  -- 一次写尝试的阶段：已准备未发、已落发送屏障、结果未知或已终结
  AttemptStage = <Prepared> | <Barriered> | InDoubt | Resolved;

  -- 一条不可抹除的外部写尝试记录
  Attempt ::
    holder : RefHolder           -- 对应在观察日志中登记 basis 引用的持有者身份
    lane   : LaneKey             -- 所属写通道
    intent : IntentPayload       -- 所执行的意图载荷
    basis  : set of LogPosition  -- 决策所依据的观察位置集合
    stage  : AttemptStage;       -- 当前阶段与结局

  -- 取证渠道返回的三种封闭结论：未查清（Inconclusive）不得当作未发生（Absent）
  Evidence = <Found> | <Absent> | <Inconclusive>;

  -- Prepare 的结局：成功建立尝试，或因通道存在未决阻塞头而被拦下
  PrepareOutcome = AttemptId | <Blocked>;

  -- 发送前过门与落屏障的结局：已落屏障可发送、因离线/能力未明而等待、或未发即过期
  BarrierOutcome = <Ready> | <Waiting> | <Expired>;

  -- 写边界的尝试表
state EffectLog of
  attempts : map AttemptId to Attempt  -- 尝试身份到尝试记录的映射
  init e == e = mk_EffectLog({|->})
end

functions
  -- 判定指定通道上当前阻塞后续普通写的尝试集合：处于 Prepared、Barriered 或 InDoubt(Waiting) 的尝试
  Blockers: LaneKey * map AttemptId to Attempt -> set of AttemptId
  Blockers(lane, attempts) ==
    is not yet specified;

operations
  -- 将获准意图交入写边界并锁住 basis 引用；普通放行要求通道无阻塞头，显式越顶要求精确指明当前阻塞头集合
  Prepare: LaneKey * IntentPayload * set of LogPosition * [set of AttemptId] ==> PrepareOutcome
  Prepare(lane, intent, basis, overrideBlockers) ==
    is not yet specified;

  -- 在真正调用外部写之前落盘发送屏障；已越过屏障的尝试永不重入
  Barrier: AttemptId * SourceId * CapabilityKey ==> BarrierOutcome
  Barrier(id, source, cap) ==
    is not yet specified;

  -- 记录单次外部写调用的返回：得业务回执/确知未发即终结并释放引用，无响应则进入 InDoubt(Waiting)
  CompleteSend: AttemptId * (TerminalOutcome | <NoResponse>) ==> ()
  CompleteSend(id, reply) ==
    is not yet specified;

  -- 用上游取证或归因证据收敛结果未知的尝试；即使已 Abandoned 仍可补全 Resolved 结论
  Resolve: AttemptId * Evidence ==> ()
  Resolve(id, evidence) ==
    is not yet specified;

  -- 放弃对结果未知尝试的本地等待以解除通道阻塞，并释放观察引用，但不改写外部结果结论
  Abandon: AttemptId ==> ()
  Abandon(id) ==
    is not yet specified;
end Effect
```
