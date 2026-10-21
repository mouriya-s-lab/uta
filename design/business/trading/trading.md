# 交易连接

```vdm
module Trading
imports
  from Source
    types
      CapabilityKey renamed CapabilityKey
    operations
      SessionCapability renamed SessionCapability,
  from Observation
    types
      LogPosition renamed LogPosition,
  from Write
    types
      AttemptRef renamed AttemptRef;
      CancelHead renamed CancelHead
    functions
      KeyOf renamed KeyOf
    operations
      AttemptOf renamed AttemptOf,
  from Ticket
    types
      TicketId renamed TicketId;
      Proposal renamed Proposal
    operations
      Draft renamed Draft
exports
  types
    ExecutionId;
    OrderId;
    struct OperationKind;
    struct TradeDraft;
    struct FillStatus;
    struct FillObs;
    struct OrderObs
  functions
    EffectiveExecutions: set of FillObs -> set of FillObs;
    MaximalOrderStates: set of OrderObs -> set of OrderObs
  operations
    ReplaceOrder: TradeDraft * TradeDraft * TradeDraft * CapabilityKey ==> seq of TicketId;
    DraftCancelOfHead: TradeDraft * AttemptRef ==> bool
definitions
types
  -- 来源分配的成交执行身份
  ExecutionId = token;

  -- 来源分配的订单身份
  OrderId = token;

  -- 交易写的操作种类：封闭集合
  OperationKind = <Place> | <Cancel> | <Replace> | <Close>;

  -- 一张交易单据的起草：操作种类与单据提议
  TradeDraft ::
    op       : OperationKind  -- 这张单据做什么
    proposal : Proposal;      -- 单据提议

  -- 一笔成交的有效性：有效或已作废
  FillStatus = <Valid> | <Voided>;

  -- 一条成交观察；由业务从映射抽出的字段解出
  FillObs ::
    execId   : ExecutionId  -- 执行身份
    revision : nat          -- 来源给出的修订号
    status   : FillStatus   -- 有效或作废
    qty      : nat;         -- 本笔成交量

  -- 一条订单状态观察；由业务从映射抽出的字段解出
  OrderObs ::
    orderId  : OrderId      -- 订单身份
    pos      : LogPosition  -- 所在观察位置
    revision : nat;         -- 来源给出的状态修订号

functions
  -- 判定：哪些成交观察当前有效——同一执行身份只取最大修订，作废项与同修订号内容冲突项都不算；两个实现必须得出同一集合
  EffectiveExecutions: set of FillObs -> set of FillObs
  EffectiveExecutions(fills) ==
    {f | f in set fills &
       f.status = <Valid>
       and not exists g in set fills & g.execId = f.execId and g.revision > f.revision
       and not exists h in set fills &
             h.execId = f.execId and h.revision = f.revision and h <> f};

  -- 判定：每个订单当前可信的状态观察——按来源修订号取极大元，修订号相同时保留多值而不伪造唯一最新状态
  MaximalOrderStates: set of OrderObs -> set of OrderObs
  MaximalOrderStates(obs) ==
    {x | x in set obs &
       not exists y in set obs & y.orderId = x.orderId and y.revision > x.revision};

operations
  -- 场景：改单。来源此刻支持原子改单时只起改单单据；明确不支持时显式起撤单与下单两张独立单据；能力未确认或离线时不起单
  ReplaceOrder: TradeDraft * TradeDraft * TradeDraft * CapabilityKey ==> seq of TicketId
  ReplaceOrder(replace, cancel, place, replaceCap) ==
    cases SessionCapability(replace.proposal.source, replaceCap):
      <Supported> -> (Draft(replace.proposal); return [replace.proposal.id]),
      <Unsupported> -> (Draft(cancel.proposal); Draft(place.proposal);
                        return [cancel.proposal.id, place.proposal.id]),
      others -> return []
    end
  pre replace.op = <Replace> and cancel.op = <Cancel> and place.op = <Place>
      and replace.proposal.source = cancel.proposal.source
      and cancel.proposal.source = place.proposal.source
      and card {replace.proposal.id, cancel.proposal.id, place.proposal.id} = 3;

  -- 场景：起草撤阻塞头的撤单单据。目标必须是已过屏障、等待仍在进行、同一来源的尝试；单据以撤阻塞头方式准入并指向它的调用方键
  DraftCancelOfHead: TradeDraft * AttemptRef ==> bool
  DraftCancelOfHead(cancel, head) ==
    cases AttemptOf(head):
      nil -> return false,
      a -> if a.barriered and a.waiting = <Active> and a.intent.source = cancel.proposal.source
           then (Draft(mu(cancel.proposal, admission |-> mk_CancelHead(KeyOf(head))));
                 return true)
           else return false
    end
  pre cancel.op = <Cancel>;
end Trading
```
