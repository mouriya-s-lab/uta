# 订阅投递连接

```vdm
module Delivery
imports
  from Observation
    types
      StreamId renamed StreamId;
      LogPosition renamed LogPosition;
      Entry renamed Entry
    operations
      ReadFrom renamed ReadFrom;
      RetentionOf renamed RetentionOf
exports
  types
    SubscriptionId;
    struct DeliveryGap;
    struct Item;
    struct Subscription
  operations
    Subscribe: set of StreamId ==> SubscriptionId;
    Unsubscribe: SubscriptionId ==> ();
    Deliver: SubscriptionId * StreamId ==> seq of Item;
    Ack: SubscriptionId * LogPosition ==> ();
    CursorOf: SubscriptionId * StreamId ==> nat
definitions
types
  -- 一份持久订阅的身份；核心造出，使用方不读结构
  SubscriptionId = token;

  -- 投递缺口：该订阅在某条流上没有收到 [low, high] 的记录；流本身并不缺它们
  DeliveryGap ::
    stream : StreamId  -- 哪条流
    low    : nat1      -- 起点（含）
    high   : nat1;     -- 终点（含）

  -- 向消费者交出的项：流上的记录，或先于记录交出的投递缺口
  Item = Entry | DeliveryGap;

  -- 一份订阅：订的流、各流已确认位置、尚未被确认越过的投递缺口
  Subscription ::
    streams : set of StreamId      -- 订阅的流
    cursor  : map StreamId to nat  -- 各流最后确认的位置号，0 为尚未确认
    gaps    : set of DeliveryGap;  -- 已写下、尚未被确认越过的缺口

  -- 订阅表
state Subscriptions of
  subs : map SubscriptionId to Subscription  -- 全部持久订阅
  inv mk_Subscriptions(s) ==
        forall id in set dom s &
          dom s(id).cursor = s(id).streams
          and forall g in set s(id).gaps &
                g.stream in set s(id).streams and s(id).cursor(g.stream) < g.high
  init s == s = mk_Subscriptions({|->})
end

operations
  Subscribe: set of StreamId ==> SubscriptionId
  Subscribe(streams) == is not yet specified;

  Unsubscribe: SubscriptionId ==> ()
  Unsubscribe(id) == is not yet specified;

  Deliver: SubscriptionId * StreamId ==> seq of Item
  Deliver(id, stream) == is not yet specified;

  Ack: SubscriptionId * LogPosition ==> ()
  Ack(id, pos) == is not yet specified;

  CursorOf: SubscriptionId * StreamId ==> nat
  CursorOf(id, stream) == is not yet specified;
end Delivery
```
