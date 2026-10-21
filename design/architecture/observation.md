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
      RawPayload renamed RawPayload,
  from Mapping
    types
      Fields renamed Fields;
      Envelope renamed Envelope;
      Decoded renamed Decoded;
      DecodeOutcome renamed DecodeOutcome
    operations
      Decode renamed Decode
exports
  types
    DerivedId;
    RefHolder;
    struct Origin;
    struct StreamId;
    struct LogPosition;
    struct EntryKind;
    struct Entry;
    struct Range;
    struct BasisValidity;
    struct RefOutcome
  operations
    OpenEpoch: Origin * StreamName * [RawPayload] ==> StreamId;
    Append: StreamId * EntryKind * [Decoded] * [RawPayload] * [LogPosition] ==> LogPosition;
    Ingest: Envelope * StreamId ==> LogPosition;
    ExtendCoverage: StreamId * nat ==> ();
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

  -- 流的来头：外部来源，或核心内的派生产出者
  External :: source : SourceId;
  Derived  :: producer : DerivedId;
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

  -- 观察日志：记录、来源序号覆盖、保留边界与活跃引用
state Journal of
  entries   : map LogPosition to Entry        -- 全部在册记录
  coverage  : map StreamId to Range           -- 各流本代的来源序号覆盖
  retention : map StreamId to nat1            -- 各流保留边界：低于它的位置已删
  refs      : map RefHolder to set of LogPosition  -- 活跃引用保护的位置
  inv mk_Journal(e, -, r, f) ==
        (forall p in set dom e & e(p).pos = p)
        and (forall p in set dom e & p.seqNo = 1 => e(p).kind = <SourceGap>)
        and (forall s in set dom r &
               forall p in set dom e & p.stream = s => p.seqNo >= r(s))
        and (forall h in set dom f &
               forall p in set f(h) &
                 p.stream in set dom r => p.seqNo >= r(p.stream))
  init j == j = mk_Journal({|->}, {|->}, {|->}, {|->})
end

operations
  OpenEpoch: Origin * StreamName * [RawPayload] ==> StreamId
  OpenEpoch(origin, name, raw) == is not yet specified;

  Append: StreamId * EntryKind * [Decoded] * [RawPayload] * [LogPosition] ==> LogPosition
  Append(stream, kind, decoded, raw, supersedes) == is not yet specified;

  Ingest: Envelope * StreamId ==> LogPosition
  Ingest(env, stream) == is not yet specified;

  ExtendCoverage: StreamId * nat ==> ()
  ExtendCoverage(stream, through) == is not yet specified;

  CheckBasis: set of LogPosition * nat ==> BasisValidity
  CheckBasis(basis, lag) == is not yet specified;

  AcquireRef: RefHolder * set of LogPosition ==> RefOutcome
  AcquireRef(holder, positions) == is not yet specified;

  ReleaseRef: RefHolder ==> ()
  ReleaseRef(holder) == is not yet specified;

  AdvanceRetention: StreamId * nat1 ==> RefOutcome
  AdvanceRetention(stream, bound) == is not yet specified;

  ReadFrom: StreamId * nat ==> seq of Entry
  ReadFrom(stream, after) == is not yet specified;

  RetentionOf: StreamId ==> nat1
  RetentionOf(stream) == is not yet specified;

  ByClientKey: ClientKey ==> set of LogPosition
  ByClientKey(key) == is not yet specified;
end Observation
```
