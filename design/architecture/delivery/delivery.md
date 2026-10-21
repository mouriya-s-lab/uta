# 订阅投递连接

```vdm
module Delivery
imports
  from Observation
    types
      StreamId renamed StreamId;
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
  functions
    GapBefore: StreamId * nat * nat1 -> [DeliveryGap];
    Ordered: DeliveryGap * seq of Entry -> seq of Item
  operations
    Subscribe: set of StreamId ==> SubscriptionId;
    Unsubscribe: SubscriptionId ==> ();
    RecordGap: SubscriptionId * DeliveryGap ==> ();
    Skip: SubscriptionId * DeliveryGap ==> seq of Item;
    Deliver: SubscriptionId * StreamId ==> seq of Item;
    Ack: SubscriptionId * StreamId * nat ==> ();
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
                g.stream in set s(id).streams
                and g.low <= g.high
                and s(id).cursor(g.stream) < g.high
  init s == s = mk_Subscriptions({|->})
end

functions
  -- 判定：游标之后到保留边界之前有没有已被删去的位置；有则为那段缺口。两个实现必须算出同一段
  GapBefore: StreamId * nat * nat1 -> [DeliveryGap]
  GapBefore(stream, cursor, bound) ==
    if cursor + 1 < bound
    then mk_DeliveryGap(stream, cursor + 1, bound - 1)
    else nil;

  -- 判定：交出的次序——缺口先于它之后的记录，消费者先知道少了什么再收到后续记录
  Ordered: DeliveryGap * seq of Entry -> seq of Item
  Ordered(gap, entries) == [gap] ^ entries;

operations
  -- 建立订阅：新身份，各流游标从 0 起，无缺口
  Subscribe(streams: set of StreamId) id: SubscriptionId
  ext wr subs
  post id not in set dom subs~
       and subs = subs~ munion {id |-> mk_Subscription(streams, {s |-> 0 | s in set streams}, {})};

  -- 删除订阅；删除两次与一次效果相同
  Unsubscribe(id: SubscriptionId)
  ext wr subs
  post subs = {id} <-: subs~;

  -- 在跳过之前把缺口写在订阅上
  RecordGap(id: SubscriptionId, gap: DeliveryGap)
  ext wr subs
  pre id in set dom subs
      and gap.stream in set subs(id).streams
      and gap.low <= gap.high
      and subs(id).cursor(gap.stream) < gap.high
  post subs = subs~ ++ {id |-> mu(subs~(id), gaps |-> subs~(id).gaps union {gap})};

  -- 场景：跳过一段已被删去的位置：先把缺口写在订阅上，再把缺口与它之后的记录一起交出
  Skip: SubscriptionId * DeliveryGap ==> seq of Item
  Skip(id, gap) ==
    (RecordGap(id, gap);
     return Ordered(gap, ReadFrom(gap.stream, gap.high)))
  pre id in set dom subs and gap.stream in set subs(id).streams
      and subs(id).cursor(gap.stream) < gap.high;

  -- 场景：交出某流上游标之后的记录；游标已落入保留边界之下时先跳过缺口
  Deliver: SubscriptionId * StreamId ==> seq of Item
  Deliver(id, stream) ==
    cases GapBefore(stream, CursorOf(id, stream), RetentionOf(stream)):
      nil -> return ReadFrom(stream, CursorOf(id, stream)),
      gap -> return Skip(id, gap)
    end
  pre id in set dom subs and stream in set subs(id).streams;

  -- 确认某流消费到某位置号：游标只前进，被越过的缺口随之消失
  Ack(id: SubscriptionId, stream: StreamId, seqNo: nat)
  ext wr subs
  pre id in set dom subs
      and stream in set subs(id).streams
      and seqNo >= subs(id).cursor(stream)
  post subs = subs~ ++
         {id |-> mu(subs~(id),
                    cursor |-> subs~(id).cursor ++ {stream |-> seqNo},
                    gaps |-> {g | g in set subs~(id).gaps &
                                g.stream <> stream or g.high > seqNo})};

  -- 某订阅在某流上最后确认的位置号
  CursorOf(id: SubscriptionId, stream: StreamId) r: nat
  ext rd subs
  pre id in set dom subs and stream in set subs(id).streams
  post r = subs(id).cursor(stream);
end Delivery
```
