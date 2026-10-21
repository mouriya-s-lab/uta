# 交易连接

```vdm
module Trading
imports
  from Source
    types
      SourceId renamed SourceId;
      CapabilityKey renamed CapabilityKey;
      InstrumentId renamed InstrumentId;
      ClientKey renamed ClientKey,
  from Mapping
    types
      CapabilityStatus renamed CapabilityStatus
    operations
      CurrentCapability renamed CurrentCapability,
  from Observation
    types
      LogPosition renamed LogPosition
    operations
      ByClientKey renamed ByClientKey,
  from Write
    types
      AttemptRef renamed AttemptRef;
      Attempt renamed Attempt
    operations
      Attribute renamed Attribute;
      AttemptOf renamed AttemptOf,
  from Ticket
    types
      TicketId renamed TicketId;
      TicketRecord renamed TicketRecord
    operations
      Draft renamed Draft
exports
  types
    ExecutionId;
    OrderId;
    struct OperationKind;
    struct FillStatus;
    struct FillObs;
    struct OrderObs
  functions
    EffectiveExecutions: set of FillObs -> set of FillObs;
    MaximalOrderStates: set of OrderObs -> set of OrderObs
  operations
    ReplaceOrder: TicketRecord * TicketRecord * CapabilityKey ==> seq of TicketId;
    AttributeObservation: AttemptRef ==> bool
definitions
types
  -- 来源分配的成交执行身份
  ExecutionId = token;

  -- 来源分配的订单身份
  OrderId = token;

  -- 交易写的操作种类：封闭集合
  OperationKind = <Place> | <Cancel> | <Replace> | <Close>;

  -- 一笔成交的有效性：有效或已作废
  FillStatus = <Valid> | <Voided>;

  -- 一条成交观察
  FillObs ::
    execId   : ExecutionId  -- 执行身份
    revision : nat          -- 来源给出的修订号
    status   : FillStatus   -- 有效或作废
    qty      : nat;         -- 本笔成交量

  -- 一条订单状态观察
  OrderObs ::
    orderId  : OrderId      -- 订单身份
    pos      : LogPosition  -- 所在观察位置
    revision : nat;         -- 来源给出的状态修订号

functions
  -- 有效成交集合：同一执行身份取唯一最大修订，排除作废项与同修订号冲突项
  EffectiveExecutions: set of FillObs -> set of FillObs
  EffectiveExecutions(fills) == is not yet specified;

  -- 订单状态的极大观察集合：同一订单按修订号取极大元，不可比时保留多值
  MaximalOrderStates: set of OrderObs -> set of OrderObs
  MaximalOrderStates(obs) == is not yet specified;

operations
  ReplaceOrder: TicketRecord * TicketRecord * CapabilityKey ==> seq of TicketId
  ReplaceOrder(cancelOrReplaceDraft, placeDraft, replaceCap) == is not yet specified;

  AttributeObservation: AttemptRef ==> bool
  AttributeObservation(ref) == is not yet specified;
end Trading
```
