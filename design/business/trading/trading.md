# 交易连接

```vdm
module Trading
exports
  types
    ExecutionId;
    OrderId;
    struct FillStatus;
    struct FillObs;
    struct OrderObs
  functions
    EffectiveExecutions: set of FillObs -> set of FillObs;
    MaximalOrderStates: set of OrderObs -> set of OrderObs
definitions
types
  -- 来源分配的成交执行身份
  ExecutionId = token;

  -- 来源分配的订单身份
  OrderId = token;

  -- 一笔成交的有效性：有效或已被来源作废
  FillStatus = <Valid> | <Voided>;

  -- 一条成交观察：来源对一笔成交在某个修订上的说法
  FillObs ::
    execId   : ExecutionId  -- 执行身份
    revision : nat          -- 来源给出的修订号
    status   : FillStatus   -- 有效或作废
    qty      : nat;         -- 本笔成交量

  -- 一条订单状态观察：来源对一张订单在某个修订上的说法
  OrderObs ::
    orderId  : OrderId  -- 订单身份
    revision : nat;     -- 来源给出的状态修订号

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
end Trading
```
