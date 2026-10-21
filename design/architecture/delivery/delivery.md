# 订阅投递连接

```vdm
module Delivery
imports
  from Observation
    types
      StreamId renamed StreamId;
      Cursor renamed Cursor;
      Entry renamed Entry
    functions
      Earlier renamed Earlier
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
    Ack: SubscriptionId * StreamId * Cursor ==> ();
    CursorOf: SubscriptionId * StreamId ==> Cursor
definitions
types
  -- 一份持久订阅的身份；核心造出，使用方不读结构
  SubscriptionId = token;

  -- 投递缺口：该订阅在某条流上没有收到晚于 low、不晚于 high 的记录；流本身并不缺它们
  DeliveryGap ::
    stream : StreamId  -- 哪条流
    low    : Cursor    -- 缺口起点（不含）
    high   : Cursor;   -- 缺口终点（含）

  -- 向消费者交出的项：流上的记录，或先于记录交出的投递缺口
  Item = Entry | DeliveryGap;

  -- 一份订阅：订的逻辑流、各流已确认游标、尚未被确认越过的投递缺口
  Subscription ::
    streams : set of StreamId         -- 订阅的逻辑流
    cursor  : map StreamId to Cursor  -- 各流最后确认的游标
    gaps    : set of DeliveryGap;     -- 已写下、尚未被确认越过的缺口

  -- 订阅表：游标覆盖且只覆盖所订的流，缺口都在所订的流上且尚未被确认越过
state Subscriptions of
  subs : map SubscriptionId to Subscription  -- 全部持久订阅
  inv mk_Subscriptions(s) ==
        forall id in set dom s &
          dom s(id).cursor = s(id).streams
          and forall g in set s(id).gaps &
                g.stream in set s(id).streams
                and Earlier(g.low, g.high)
                and Earlier(s(id).cursor(g.stream), g.high)
  init s == s = mk_Subscriptions({|->})
end

operations
  -- 建立订阅：新身份，各流游标从第一代起点之前开始，无缺口
  Subscribe(streams: set of StreamId) id: SubscriptionId
  ext wr subs
  post id not in set dom subs~
       and subs = subs~ munion
             {id |-> mk_Subscription(streams, {s |-> mk_Cursor(1, 0) | s in set streams}, {})};

  -- 删除订阅；删除两次与一次效果相同
  Unsubscribe(id: SubscriptionId)
  ext wr subs
  post subs = {id} <-: subs~;

  -- 在跳过之前把缺口写在订阅上
  RecordGap(id: SubscriptionId, gap: DeliveryGap)
  ext wr subs
  pre id in set dom subs
      and gap.stream in set subs(id).streams
      and Earlier(gap.low, gap.high)
      and Earlier(subs(id).cursor(gap.stream), gap.high)
  post subs = subs~ ++ {id |-> mu(subs~(id), gaps |-> subs~(id).gaps union {gap})};

  -- 场景：交出某流上游标之后的记录；游标已落在保留边界之前时，先写下并交出缺口，再交出边界之后的记录
  Deliver: SubscriptionId * StreamId ==> seq of Item
  Deliver(id, stream) ==
    let cur = subs(id).cursor(stream), bound = RetentionOf(stream) in
      if Earlier(cur, bound)
      then let gap = mk_DeliveryGap(stream, cur, bound) in
             (RecordGap(id, gap);
              return [gap] ^ ReadFrom(stream, bound))
      else return ReadFrom(stream, cur)
  pre id in set dom subs and stream in set subs(id).streams;

  -- 确认某流消费到某游标：游标只前进，被越过的缺口随之消失
  Ack(id: SubscriptionId, stream: StreamId, upTo: Cursor)
  ext wr subs
  pre id in set dom subs
      and stream in set subs(id).streams
      and not Earlier(upTo, subs(id).cursor(stream))
  post subs = subs~ ++
         {id |-> mu(subs~(id),
                    cursor |-> subs~(id).cursor ++ {stream |-> upTo},
                    gaps |-> {g | g in set subs~(id).gaps &
                                g.stream <> stream or Earlier(upTo, g.high)})};

  -- 某订阅在某流上最后确认的游标
  CursorOf(id: SubscriptionId, stream: StreamId) r: Cursor
  ext rd subs
  pre id in set dom subs and stream in set subs(id).streams
  post r = subs(id).cursor(stream);
end Delivery
```
