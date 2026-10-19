# 交易业务接入

```vdm
module Trading
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
      LogPosition renamed LogPosition,
  from Effect
    types
      LaneKey renamed LaneKey;
      AttemptId renamed AttemptId;
      IntentPayload renamed IntentPayload;
      PrepareOutcome renamed PrepareOutcome
    operations
      LaneBlockers renamed LaneBlockers;
      Prepare renamed Prepare;
      Resolve renamed Resolve,
  from Ticket
    types
      TicketId renamed TicketId;
      VersionId renamed VersionId;
      Principal renamed Principal;
      TicketRecord renamed TicketRecord;
      ReleaseOutcome renamed ReleaseOutcome
    operations
      Draft renamed Draft;
      Decide renamed Decide;
      Release renamed Release
exports
  types
    ExecutionId;
    OrderId;
    struct FillStatus;
    struct FillObs;
    struct OrderObs
  functions
    EffectiveExecutions: set of FillObs -> set of FillObs;
    MaximalOrderStates: set of OrderObs * set of (OrderObs * OrderObs) -> set of OrderObs
  operations
    SubmitOrderIntent: TicketRecord * AttemptId * RefHolder * SourceId * CapabilityKey ==> ReleaseOutcome;
    ReplaceOrder: TicketRecord * TicketRecord * SourceId * CapabilityKey ==> seq of TicketId;
    CancelBlockingOrder: AttemptId * RefHolder * LaneKey * AttemptId * IntentPayload * set of LogPosition ==> PrepareOutcome;
    AttributeObservation: AttemptId * OrderObs ==> ()
definitions
types
  -- 上游在单作用域内给出的成交执行身份
  ExecutionId = token;

  -- 上游订单身份
  OrderId = token;

  -- 成交修订的有效性状态：有效成交或作废修订
  FillStatus = <Valid> | <Voided>;

  -- 一条成交观察副本：用于按执行身份与修订号去重计数
  FillObs ::
    execId   : ExecutionId  -- 成交执行身份
    revision : nat          -- 修订号
    status   : FillStatus   -- 有效或已作废
    qty      : nat;         -- 本笔执行的成交量

  -- 一条订单状态观察副本
  OrderObs ::
    orderId     : OrderId       -- 订单身份
    fromAttempt : [AttemptId]   -- 若由集成证明归属于某次本地尝试则填写
    revision    : nat;          -- 来源给出的状态修订号或序号

functions
  -- 判定有效成交集合：同一执行身份取唯一最大修订，排除作废项与同修订号内容冲突项
  EffectiveExecutions: set of FillObs -> set of FillObs
  EffectiveExecutions(fills) ==
    {f | f in set fills &
       f.status = <Valid>
       and not exists g in set fills &
             g.execId = f.execId and g.revision > f.revision
       and not exists h in set fills &
             h.execId = f.execId and h.revision = f.revision and h <> f};

  -- 判定订单状态的极大观察集合：按来源定序偏序关系求极大元，不可比时保留多值而不伪造唯一最新状态
  MaximalOrderStates: set of OrderObs * set of (OrderObs * OrderObs) -> set of OrderObs
  MaximalOrderStates(obs, precedes) ==
    {x | x in set obs &
       not exists y in set obs &
         y.orderId = x.orderId and y <> x and mk_(x, y) in set precedes};

operations
  -- 场景：发起交易单据、完成版本批准并放行进入受控写边界
  SubmitOrderIntent: TicketRecord * AttemptId * RefHolder * SourceId * CapabilityKey ==> ReleaseOutcome
  SubmitOrderIntent(draft, attemptId, holder, source, cap) ==
    (Draft(draft);
     let - = Decide(draft.id, draft.version, <Approved>) in
       return Release(draft.id, attemptId, holder, source, cap));

  -- 场景：改单——来源支持原子改单时只起一张单据，不支持时显式组合撤单与下单两张单据
  ReplaceOrder: TicketRecord * TicketRecord * SourceId * CapabilityKey ==> seq of TicketId
  ReplaceOrder(cancelOrReplaceDraft, placeDraft, source, replaceCap) ==
    cases CurrentCapability(source, replaceCap):
      <Supported> ->
        (Draft(cancelOrReplaceDraft);
         return [cancelOrReplaceDraft.id]),
      others ->
        (Draft(cancelOrReplaceDraft);
         Draft(placeDraft);
         return [cancelOrReplaceDraft.id, placeDraft.id])
    end;

  -- 场景：撤单目标精确指向当前通道阻塞头集合中的尝试时，携带当前阻塞头集合越顶进入写边界
  CancelBlockingOrder: AttemptId * RefHolder * LaneKey * AttemptId * IntentPayload * set of LogPosition ==> PrepareOutcome
  CancelBlockingOrder(cancelAttemptId, holder, lane, targetBlockingAttempt, cancelIntent, basis) ==
    let currentBlockers = LaneBlockers(lane) in
      if targetBlockingAttempt in set currentBlockers
      then return Prepare(cancelAttemptId, holder, lane, cancelIntent, basis, currentBlockers)
      else return <Blocked>;

  -- 场景：当带本地尝试归因的订单观察到达时，作为 Found 证据收敛该尝试的结果未知状态
  AttributeObservation: AttemptId * OrderObs ==> ()
  AttributeObservation(attempt, obs) ==
    if obs.fromAttempt = attempt
    then Resolve(attempt, <Found>)
    else skip;
end Trading
```
