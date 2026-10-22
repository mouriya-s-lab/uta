# 观察日志连接

```vdm
module Observation
imports
  from Source
    types
      SourceId renamed SourceId;
      StreamName renamed StreamName;
      RawPayload renamed RawPayload
exports
  types
    struct DerivedId;
    struct ClientKey;
    Fields;
    struct Decoded;
    struct External;
    struct Derived;
    struct Origin;
    struct StreamId;
    struct Cursor;
    struct LogPosition;
    struct EntryKind;
    struct Entry
  functions
    After: LogPosition * Cursor -> bool;
    Earlier: Cursor * Cursor -> bool
  operations
    OpenEpoch: StreamId * [RawPayload] ==> ();
    Append: StreamId * [Decoded] * [RawPayload] * [LogPosition] ==> [LogPosition];
    CheckBasis: set of LogPosition ==> bool;
    AdvanceRetention: StreamId * Cursor ==> ();
    ReadFrom: StreamId * Cursor ==> seq of Entry;
    RetentionOf: StreamId ==> Cursor;
    ByClientKey: ClientKey ==> set of LogPosition
definitions
types
  -- 派生流产出者的身份；由业务给出，观察侧不解释
  DerivedId = token;

  -- 核心为一次外部写铸造、随请求交给来源、在来源回声的记录里带回的调用方键
  ClientKey = token;

  -- 从来源原文抽出的、供核心与业务使用的字段集；观察侧不解释
  Fields = token;

  -- 一条记录经映射抽出的字段：核心需要的部分，其余留在原文
  Decoded ::
    clientKey : [ClientKey]  -- 记录回声的调用方键，用于归因到本地写尝试
    fields    : Fields;      -- 其余抽出的字段

  -- 外部来头：记录来自某个外部来源
  External ::
    source : SourceId;  -- 哪个来源

  -- 派生来头：记录由核心内的某个产出者算出
  Derived ::
    producer : DerivedId;  -- 哪个产出者

  -- 流的来头：外部来源，或核心内的派生产出者
  Origin = External | Derived;

  -- 一条逻辑流的身份：来头与流名；不含分代
  StreamId ::
    origin : Origin       -- 流的来头
    name   : StreamName;  -- 流名

  -- 流上的一个游标：某一代内已到达的位置号；位置号 0 表示该代尚无记录到达
  Cursor ::
    epoch : nat1  -- 分代号
    seqNo : nat;  -- 该代内的位置号

  -- 一条记录的顺序身份：所在流、分代与该代内由核心分配的位置号
  LogPosition ::
    stream : StreamId  -- 所在流
    epoch  : nat1      -- 分代号，每次来源连续性中断开新一代
    seqNo  : nat1;     -- 代内位置号，由核心按追加先后分配

  -- 记录类别：正常数据、开启新一代的来源连续性中断、当前映射解不出的记录
  EntryKind = <Data> | <SourceGap> | <Unmapped>;

  -- 观察流上的一条不可变记录
  Entry ::
    pos        : LogPosition     -- 顺序身份
    kind       : EntryKind       -- 类别
    decoded    : [Decoded]       -- 映射抽出的字段；解不出或缺口记录时为空
    raw        : [RawPayload]    -- 来源原文；派生记录可无
    supersedes : [LogPosition];  -- 派生记录修正时所取代的旧位置

  -- 观察日志：各流的当前代与流末、全部记录与保留边界
state Journal of
  head      : map StreamId to Cursor    -- 各已开代流的当前代与该代最后分配的位置号
  entries   : map LogPosition to Entry  -- 全部在册记录
  retention : map StreamId to Cursor    -- 各流保留边界：不晚于它的位置已删
  inv mk_Journal(h, e, r) ==
        (forall p in set dom e &
           e(p).pos = p
           and p.stream in set dom h
           and (p.epoch < h(p.stream).epoch
                or (p.epoch = h(p.stream).epoch and p.seqNo <= h(p.stream).seqNo)))
        and (forall p in set dom e &
               (p.seqNo = 1 and is_External(p.stream.origin)) => e(p).kind = <SourceGap>)
        and (forall p in set dom e &
               p.stream in set dom r => After(p, r(p.stream)))
  init j == j = mk_Journal({|->}, {|->}, {|->})
end

functions
  -- 判定：一个位置是否晚于一个游标——先比代，同代比位置号
  After: LogPosition * Cursor -> bool
  After(p, c) ==
    p.epoch > c.epoch or (p.epoch = c.epoch and p.seqNo > c.seqNo);

  -- 判定：一个游标是否早于另一个游标
  Earlier: Cursor * Cursor -> bool
  Earlier(a, b) ==
    a.epoch < b.epoch or (a.epoch = b.epoch and a.seqNo < b.seqNo);

operations
  -- 为逻辑流开新一代：代号比当前代大 1，首次开代为 1；外部流的首条记录是来源连续性中断，派生流不写
  OpenEpoch(stream: StreamId, raw: [RawPayload])
  ext wr head wr entries
  post let e = if stream in set dom head~ then head~(stream).epoch + 1 else 1 in
         (is_External(stream.origin)
          and head = head~ ++ {stream |-> mk_Cursor(e, 1)}
          and entries = entries~ munion
                {mk_LogPosition(stream, e, 1) |->
                   mk_Entry(mk_LogPosition(stream, e, 1), <SourceGap>, nil, raw, nil)})
         or (is_Derived(stream.origin)
             and head = head~ ++ {stream |-> mk_Cursor(e, 0)}
             and entries = entries~);
  -- 在流上追加一条记录：流尚未开代时不追加；已开代时位置在当前代流末之后，解出字段的是数据记录，没解出的是解不出记录；取代位置必须在同一流上
  Append(stream: StreamId, decoded: [Decoded], raw: [RawPayload], supersedes: [LogPosition]) r: [LogPosition]
  ext wr head wr entries
  pre supersedes <> nil => supersedes.stream = stream
  post (stream not in set dom head~ and r = nil and head = head~ and entries = entries~)
       or (stream in set dom head~
           and r = mk_LogPosition(stream, head~(stream).epoch, head~(stream).seqNo + 1)
           and head = head~ ++ {stream |-> mk_Cursor(r.epoch, r.seqNo)}
           and entries = entries~ munion
                 {r |-> mk_Entry(r, if decoded = nil then <Unmapped> else <Data>,
                                 decoded, raw, supersedes)});

  -- 一组依据位置是否仍然新鲜：全部在册、没有被取代、所在代仍是该流当前代；任一项为假即不可逆
  CheckBasis(basis: set of LogPosition) r: bool
  ext rd head rd entries
  post r = (forall p in set basis &
              p in set dom entries
              and not (exists e in set rng entries & e.supersedes = p)
              and p.epoch = head(p.stream).epoch);

  -- 推进某流的保留边界并删去不晚于它的记录；边界只前进，倒退时不变
  AdvanceRetention(stream: StreamId, bound: Cursor)
  ext wr retention wr entries
  post let forward = stream not in set dom retention~ or Earlier(retention~(stream), bound) in
         (forward
          and retention = retention~ ++ {stream |-> bound}
          and entries = {p | p in set dom entries~ & p.stream <> stream or After(p, bound)} <: entries~)
         or (not forward and retention = retention~ and entries = entries~);

  -- 按位置先后读出某流上晚于游标的全部在册记录，跨代连续
  ReadFrom(stream: StreamId, since: Cursor) items: seq of Entry
  ext rd entries
  post elems items = {e | e in set rng entries & e.pos.stream = stream and After(e.pos, since)}
       and (forall i, j in set inds items &
              i < j => After(items(j).pos, mk_Cursor(items(i).pos.epoch, items(i).pos.seqNo)));

  -- 某流当前保留边界；未推进过的流为第一代起点之前
  RetentionOf(stream: StreamId) r: Cursor
  ext rd retention
  post (stream in set dom retention and r = retention(stream))
       or (stream not in set dom retention and r = mk_Cursor(1, 0));

  -- 带有某调用方键回声的全部在册记录位置
  ByClientKey(key: ClientKey) r: set of LogPosition
  ext rd entries
  post r = {p | p in set dom entries &
              entries(p).decoded <> nil and entries(p).decoded.clientKey = key};
end Observation
```
