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
      LogPosition renamed LogPosition,
  from Effect
    types
      LaneKey renamed LaneKey;
      AttemptId renamed AttemptId;
      IntentPayload renamed IntentPayload;
      PrepareOutcome renamed PrepareOutcome
    functions
      Blockers renamed Blockers
    operations
      Prepare renamed Prepare;
      Resolve renamed Resolve,
  from Ticket
    types
      TicketId renamed TicketId;
      Principal renamed Principal
    operations
      Draft renamed Draft;
      Release renamed Release
exports
  types
    ExecutionId;
    OrderId;
    struct OrderOp;
    struct FillObs;
    struct OrderObs
  functions
    EffectiveExecutions: set of FillObs -> set of FillObs;
    MaximalOrderStates: set of OrderObs * set of (OrderObs * OrderObs) -> set of OrderObs
  operations
    SubmitOrderIntent: Principal * OrderOp * LaneKey * IntentPayload * set of LogPosition ==> TicketId;
    ReplaceOrder: Principal * SourceId * CapabilityKey * LaneKey * IntentPayload * IntentPayload * set of LogPosition ==> seq of TicketId;
    CancelBlockingOrder: LaneKey * AttemptId * IntentPayload * set of LogPosition ==> PrepareOutcome;
    AttributeObservation: AttemptId * OrderObs ==> ()
definitions
types
  -- 上游在单作用域内给出的成交执行身份
  ExecutionId = token;

  -- 上游订单身份
  OrderId = token;

  -- 交易协议的四类写操作
  OrderOp = <Place> | <Cancel> | <Replace> | <Close>;

  -- 一条成交观察副本：用于按执行身份与修订号去重计数
  FillObs ::
    execId   : ExecutionId  -- 成交执行身份
    revision : nat          -- 修订号
    voided   : bool         -- 是否为作废修订
    qty      : nat;         -- 本笔执行的成交量

  -- 一条订单状态观察副本
  OrderObs ::
    orderId     : OrderId       -- 订单身份
    fromAttempt : [AttemptId]   -- 若由集成证明归属于某次本地尝试则填写
    terminal    : bool;         -- 是否已处于上游终态

functions
  -- 成交计数判定：同一执行身份取唯一最大修订且排除作废与同修订冲突
  EffectiveExecutions: set of FillObs -> set of FillObs
  EffectiveExecutions(fills) ==
    is not yet specified;

  -- 订单状态偏序判定：按来源定序证据求极大观察集合，不可比时不伪造唯一最新状态
  MaximalOrderStates: set of OrderObs * set of (OrderObs * OrderObs) -> set of OrderObs
  MaximalOrderStates(obs, newer) ==
    is not yet specified;

operations
  -- 场景：按交易操作种类发起订单单据
  SubmitOrderIntent: Principal * OrderOp * LaneKey * IntentPayload * set of LogPosition ==> TicketId
  SubmitOrderIntent(actor, op, lane, intent, basis) ==
    is not yet specified;

  -- 场景：改单——来源支持原子 Replace 时起一张单据，不支持时组合 Cancel 与 Place 两张单据
  ReplaceOrder: Principal * SourceId * CapabilityKey * LaneKey * IntentPayload * IntentPayload * set of LogPosition ==> seq of TicketId
  ReplaceOrder(actor, source, replaceCap, lane, cancelIntent, placeIntent, basis) ==
    is not yet specified;

  -- 场景：撤单目标精确指向当前通道阻塞头尝试时，传入该阻塞头越顶进入写边界
  CancelBlockingOrder: LaneKey * AttemptId * IntentPayload * set of LogPosition ==> PrepareOutcome
  CancelBlockingOrder(lane, blockingAttempt, cancelIntent, basis) ==
    is not yet specified;

  -- 场景：带本地尝试归因的订单观察到达时，驱动 Effect`Resolve 收敛结果未知的尝试
  AttributeObservation: AttemptId * OrderObs ==> ()
  AttributeObservation(attempt, obs) ==
    is not yet specified;
end Trading
```
