# 观察日志连接

```vdm
module Observation
imports
  from Source
    types
      SourceId renamed SourceId;
      StreamName renamed StreamName;
      InstrumentId renamed InstrumentId;
      ClientKey renamed ClientKey;
      RawPayload renamed RawPayload
exports
  types
    struct DerivedId;
    struct RefHolder;
    Fields;
    struct Decoded;
    struct External;
    struct Derived;
    struct Origin;
    struct StreamId;
    struct LogPosition;
    struct EntryKind;
    struct Entry;
    struct Range;
    struct BasisValidity;
    struct RefOutcome;
    struct AppendOutcome
  operations
    OpenEpoch: Origin * StreamName * [RawPayload] * bool ==> StreamId;
    Append: StreamId * [Decoded] * [RawPayload] * [LogPosition] ==> AppendOutcome;
    CheckBasis: set of LogPosition * nat ==> BasisValidity;
    AcquireRef: RefHolder * set of LogPosition ==> RefOutcome;
    ReleaseRef: RefHolder ==> ();
    AdvanceRetention: StreamId * nat1 ==> RefOutcome;
    ReadFrom: StreamId * nat ==> seq of Entry;
    RetentionOf: StreamId ==> nat1;
    ByClientKey: ClientKey ==> set of LogPosition
definitions
types
  -- 派生流产出者的身份；由业务给出，观察侧不解释
  DerivedId = token;

  -- 登记保留引用的持有者身份；由使用方给出
  RefHolder = token;

  -- 从来源原文抽出的、供核心与业务使用的字段集；观察侧不解释
  Fields = token;

  -- 一条记录经映射抽出的字段：核心需要的部分，其余留在原文
  Decoded ::
    venueSeq   : [nat]           -- 来源连续序号；流声明带序号时由映射抽出
    instrument : [InstrumentId]  -- 记录所属品种
    clientKey  : [ClientKey]     -- 记录回声的调用方键，用于归因到本地写尝试
    fields     : Fields;         -- 其余抽出的字段

  -- 外部来头：记录来自某个外部来源
  External ::
    source : SourceId;  -- 哪个来源

  -- 派生来头：记录由核心内的某个产出者算出
  Derived ::
    producer : DerivedId;  -- 哪个产出者

  -- 流的来头：外部来源，或核心内的派生产出者
  Origin = External | Derived;

  -- 一条观察流的身份：来头、流名与分代；每次来源连续性中断开新一代
  StreamId ::
    origin : Origin      -- 流的来头
    name   : StreamName  -- 流名
    epoch  : nat1;       -- 分代号，从 1 起

  -- 一条记录的顺序身份：所在流与核心分配的位置号
  LogPosition ::
    stream : StreamId  -- 所在流
    seqNo  : nat1;     -- 流内位置号，由核心按追加先后分配

  -- 记录类别：正常数据、开启新一代的来源连续性中断、当前映射解不出的记录
  EntryKind = <Data> | <SourceGap> | <Unmapped>;

  -- 观察流上的一条不可变记录
  Entry ::
    pos        : LogPosition     -- 顺序身份
    kind       : EntryKind       -- 类别
    decoded    : [Decoded]       -- 映射抽出的字段；解不出或缺口记录时为空
    raw        : [RawPayload]    -- 来源原文；派生记录可无
    supersedes : [LogPosition];  -- 派生记录修正时所取代的旧位置

  -- 来源序号覆盖区间：该流本代内来源序号落在 [low, high) 的记录全部已在日志
  Range ::
    low  : nat   -- 起点（含）
    high : nat;  -- 终点（不含）

  -- 一组依据位置相对当前日志的有效性
  BasisValidity = <Fresh> | <Stale> | <Retracted> | <BeyondRetention>;

  -- 引用登记与保留推进的结局
  RefOutcome = <Applied> | <Rejected>;

  -- 追加的结局：记录的位置，或流尚未开代
  AppendOutcome = LogPosition | <NotOpen>;

  -- 观察日志：已开代的流及其是否携带来源连续序号、记录、来源序号覆盖、保留边界与活跃引用
state Journal of
  opened    : map StreamId to bool                 -- 已开代的流；值为该流记录是否携带来源连续序号
  entries   : map LogPosition to Entry             -- 全部在册记录
  coverage  : map StreamId to Range                -- 各流本代的来源序号覆盖
  retention : map StreamId to nat1                 -- 各流保留边界：低于它的位置已删
  refs      : map RefHolder to set of LogPosition  -- 活跃引用保护的位置
  inv mk_Journal(o, e, c, r, f) ==
        (forall p in set dom e & e(p).pos = p and p.stream in set dom o)
        and (forall p in set dom e & p.seqNo = 1 => e(p).kind = <SourceGap>)
        and (forall s in set dom c & o(s) and c(s).low <= c(s).high)
        and (forall p in set dom e &
               p.stream in set dom r => p.seqNo >= r(p.stream))
        and (forall h in set dom f &
               forall p in set f(h) &
                 p.stream in set dom r => p.seqNo >= r(p.stream))
  init j == j = mk_Journal({|->}, {|->}, {|->}, {|->}, {|->})
end

operations
  -- 为某来头的某流开新一代：代号比已有各代大 1，首条记录是来源连续性中断；记下该流是否携带来源连续序号
  OpenEpoch(origin: Origin, name: StreamName, raw: [RawPayload], sequenced: bool) s: StreamId
  ext wr opened wr entries
  post s.origin = origin and s.name = name
       and s not in set dom opened~
       and (forall t in set dom opened~ &
              (t.origin = origin and t.name = name) => t.epoch < s.epoch)
       and (s.epoch = 1
            or mk_StreamId(origin, name, s.epoch - 1) in set dom opened~)
       and opened = opened~ munion {s |-> sequenced}
       and entries = entries~ munion
             {mk_LogPosition(s, 1) |->
                mk_Entry(mk_LogPosition(s, 1), <SourceGap>, nil, raw, nil)};

  -- 追加一条记录：流未开代则不追加；解出字段的是数据记录，没解出的是解不出记录；位置号为该流已有最大位置号加 1；携带序号的流上解出的来源序号与已有覆盖衔接时覆盖前进、本代首次时建立覆盖、否则覆盖不变；取代位置必须在同一流上
  Append(stream: StreamId, decoded: [Decoded], raw: [RawPayload], supersedes: [LogPosition]) r: AppendOutcome
  ext wr entries wr coverage rd opened rd retention
  pre supersedes <> nil => supersedes.stream = stream
  post (stream not in set dom opened and r = <NotOpen>
        and entries = entries~ and coverage = coverage~)
       or (stream in set dom opened
           and r <> <NotOpen>
           and r.stream = stream
           and r not in set dom entries~
           and (forall p in set dom entries~ & p.stream = stream => p.seqNo < r.seqNo)
           and (r.seqNo = 1 or mk_LogPosition(stream, r.seqNo - 1) in set dom entries~
                or (stream in set dom retention and r.seqNo - 1 < retention(stream)))
           and entries = entries~ munion
                 {r |-> mk_Entry(r, if decoded = nil then <Unmapped> else <Data>,
                                 decoded, raw, supersedes)}
           and let seqd = opened(stream) and decoded <> nil and decoded.venueSeq <> nil in
                 (not seqd and coverage = coverage~)
                 or (seqd and stream not in set dom coverage~
                     and coverage = coverage~ munion
                           {stream |-> mk_Range(decoded.venueSeq, decoded.venueSeq + 1)})
                 or (seqd and stream in set dom coverage~
                     and decoded.venueSeq = coverage~(stream).high
                     and coverage = coverage~ ++
                           {stream |-> mk_Range(coverage~(stream).low, decoded.venueSeq + 1)})
                 or (seqd and stream in set dom coverage~
                     and decoded.venueSeq <> coverage~(stream).high
                     and coverage = coverage~));

  -- 查询一组依据位置的有效性：已被删、被取代、落后于流末超过允许距离或所在代已被新代接替、否则新鲜
  CheckBasis(basis: set of LogPosition, lag: nat) r: BasisValidity
  ext rd entries rd retention rd opened
  pre forall p in set basis &
        p in set dom entries
        or (p.stream in set dom retention and p.seqNo < retention(p.stream))
  post let beyond = exists p in set basis &
                      p.stream in set dom retention and p.seqNo < retention(p.stream),
           retracted = exists p in set basis &
                         exists e in set rng entries & e.supersedes = p,
           stale = exists p in set basis &
                     (exists q in set dom entries &
                        q.stream = p.stream and q.seqNo > p.seqNo + lag)
                     or (exists t in set dom opened &
                           t.origin = p.stream.origin and t.name = p.stream.name
                           and t.epoch > p.stream.epoch) in
         (beyond and r = <BeyondRetention>)
         or (not beyond and retracted and r = <Retracted>)
         or (not beyond and not retracted and stale and r = <Stale>)
         or (not beyond and not retracted and not stale and r = <Fresh>);

  -- 为持有者登记需要保护的位置集合，替换它先前的登记；任一位置已低于保留边界则拒绝
  AcquireRef(holder: RefHolder, positions: set of LogPosition) r: RefOutcome
  ext rd retention wr refs
  post let valid = forall p in set positions &
                     p.stream in set dom retention => p.seqNo >= retention(p.stream) in
         (valid and r = <Applied> and refs = refs~ ++ {holder |-> positions})
         or (not valid and r = <Rejected> and refs = refs~);

  -- 解除持有者的全部登记
  ReleaseRef(holder: RefHolder)
  ext wr refs
  post refs = {holder} <-: refs~;

  -- 单调推进某流的保留边界并删去边界之下的记录；边界倒退或越过活跃引用时拒绝
  AdvanceRetention(stream: StreamId, bound: nat1) r: RefOutcome
  ext rd refs wr retention wr entries
  post let forward = stream not in set dom retention~ or bound > retention~(stream),
           referencedOk = forall h in set dom refs &
                            forall p in set refs(h) &
                              p.stream = stream => p.seqNo >= bound in
         (forward and referencedOk and r = <Applied>
          and retention = retention~ ++ {stream |-> bound}
          and entries = {p | p in set dom entries~ & p.stream <> stream or p.seqNo >= bound} <: entries~)
         or (not (forward and referencedOk) and r = <Rejected>
             and retention = retention~ and entries = entries~);

  -- 按位置先后读出某流上位置号大于 after 的全部在册记录
  ReadFrom(stream: StreamId, after: nat) items: seq of Entry
  ext rd entries
  post elems items = {e | e in set rng entries & e.pos.stream = stream and e.pos.seqNo > after}
       and (forall i, j in set inds items &
              i < j => items(i).pos.seqNo < items(j).pos.seqNo);

  -- 某流当前保留边界；未推进过的流为 1
  RetentionOf(stream: StreamId) r: nat1
  ext rd retention
  post (stream in set dom retention and r = retention(stream))
       or (stream not in set dom retention and r = 1);

  -- 带有某调用方键回声的全部在册记录位置
  ByClientKey(key: ClientKey) r: set of LogPosition
  ext rd entries
  post r = {p | p in set dom entries &
              entries(p).decoded <> nil and entries(p).decoded.clientKey = key};
end Observation
```
