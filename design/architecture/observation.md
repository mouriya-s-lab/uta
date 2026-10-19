# 观察与进度连接

```vdm
module Observation
imports
  from Source
    types
      SourceId renamed SourceId;
      CapabilityKey renamed CapabilityKey
    operations
      CurrentCapability renamed CurrentCapability
exports
  types
    StreamId;
    RefHolder;
    ObsPayload;
    struct Seq;
    struct LogPosition;
    struct EntryKind;
    struct Entry;
    struct DeliveryItem;
    struct BasisStatus;
    struct ReadOutcome
  functions
    BasisValid: set of LogPosition * set of Entry * set of LogPosition -> BasisStatus
  operations
    Append: StreamId * Seq * EntryKind * ObsPayload * [LogPosition] ==> LogPosition;
    CheckBasis: set of LogPosition ==> BasisStatus;
    AcquireRef: RefHolder * set of LogPosition ==> <Applied> | <Rejected>;
    ReleaseRef: RefHolder ==> ();
    AdvanceRetention: StreamId * Seq ==> <Applied> | <Rejected>;
    PollFrom: StreamId * Seq ==> set of DeliveryItem;
    ReadOnce: SourceId * CapabilityKey * StreamId * Seq * ObsPayload ==> ReadOutcome
definitions
types
  -- 观察流的身份；提供方创建，使用方原样传递
  StreamId = token;

  -- 持有观察位置活跃引用的对象身份（如在途写尝试或程序检查点）
  RefHolder = token;

  -- 观察记录携带的载荷；对架构不透明，由业务单元解释
  ObsPayload = token;

  -- 单条流内的单调递增序号，从 1 开始
  Seq = nat
  inv n == n > 0;

  -- 一条观察记录在所属流上的唯一顺序位置
  LogPosition ::
    stream : StreamId  -- 所属观察流
    seqNo  : Seq;      -- 流内序号

  -- 观察流记录的类别：正常观察数据、来源连续性断代缺口、或来源完备覆盖证明
  EntryKind = <Data> | <SourceGap> | <Coverage>;

  -- 观察流上的一条不可变记录：可选指名它修正取代的旧位置
  Entry ::
    pos        : LogPosition   -- 本条记录的位置
    kind       : EntryKind     -- 正常数据、来源断代或完备覆盖证明
    payload    : ObsPayload    -- 观察记录携带的不透明载荷
    supersedes : [LogPosition];-- 修正派生值时所取代的旧记录位置

  -- 向消费者投递的项：流上的记录，或该消费者因慢消费/历史压缩产生的投递缺口
  DeliveryItem = Entry | <DeliveryGap>;

  -- 依据位置集合的有效性判定：新鲜可用、已被后续记录修正撤回、或已落入保留边界之下
  BasisStatus = <Fresh> | <Retracted> | <BeyondRetention>;

  -- 一次性主动读取的结局：返回追加的观察位置，或区分渠道不可用、能力不支持与能力未确认
  ReadOutcome = LogPosition | <Unavailable> | <Unsupported> | <Unknown>;

  -- 观察日志、保留边界与活跃引用登记：保留边界绝不越过任何仍被活跃引用保护的位置
state Journal of
  entries    : set of Entry                          -- 已追加的观察记录集合
  retention  : set of LogPosition                    -- 每条流当前的保留下界位置集合
  activeRefs : map RefHolder to set of LogPosition   -- 活跃对象登记保护的观察位置集合
  inv mk_Journal(-, ret, refs) ==
        forall b in set ret &
          forall p in set dunion rng refs &
            p.stream = b.stream => p.seqNo >= b.seqNo
  init j == j = mk_Journal({}, {}, {|->})
end

functions
  -- 判定一组依据位置是否仍在保留边界之上且未被后续记录修正撤回；两个实现必须得出同一结论
  BasisValid: set of LogPosition * set of Entry * set of LogPosition -> BasisStatus
  BasisValid(basis, entries, retention) ==
    if exists p in set basis &
         (not exists e in set entries & e.pos = p)
         or (exists b in set retention & b.stream = p.stream and p.seqNo < b.seqNo)
    then <BeyondRetention>
    elseif exists p in set basis &
             exists e in set entries & e.supersedes = p
    then <Retracted>
    else <Fresh>;

operations
  -- 在指定观察流追加一条记录并返回其位置
  Append(stream: StreamId, seqNo: Seq, kind: EntryKind, payload: ObsPayload, supersedes: [LogPosition]) pos: LogPosition
  ext wr entries
  post pos = mk_LogPosition(stream, seqNo)
       and entries = entries~ union {mk_Entry(pos, kind, payload, supersedes)};

  -- 查询一组依据位置在当前观察日志与保留边界下的有效性状态
  CheckBasis(basis: set of LogPosition) r: BasisStatus
  ext rd entries rd retention
  post r = BasisValid(basis, entries, retention);

  -- 为活跃对象登记需要保护的观察位置集合；若位置已低于保留边界则拒绝登记
  AcquireRef(holder: RefHolder, positions: set of LogPosition) r: <Applied> | <Rejected>
  ext rd retention wr activeRefs
  post let valid = forall b in set retention &
                     forall p in set positions &
                       p.stream = b.stream => p.seqNo >= b.seqNo in
         (valid
          and r = <Applied>
          and activeRefs = activeRefs~ ++ {holder |-> positions})
         or (not valid
             and r = <Rejected>
             and activeRefs = activeRefs~);

  -- 释放对象持有的全部观察位置引用，允许保留边界越过这些位置
  ReleaseRef(holder: RefHolder)
  ext wr activeRefs
  post activeRefs = {holder} <-: activeRefs~;

  -- 单调推进指定流的保留边界；边界倒退或越过活跃引用位置时拒绝推进
  AdvanceRetention(stream: StreamId, bound: Seq) r: <Applied> | <Rejected>
  ext rd activeRefs wr retention
  post let monotonic = forall b in set retention~ & b.stream = stream => bound >= b.seqNo,
           referencedOk = forall p in set dunion rng activeRefs &
                            p.stream = stream => p.seqNo >= bound in
         (monotonic and referencedOk
          and r = <Applied>
          and retention = {b | b in set retention~ & b.stream <> stream}
                          union {mk_LogPosition(stream, bound)})
         or ((not monotonic or not referencedOk)
             and r = <Rejected>
             and retention = retention~);

  -- 按消费者已确认游标序号取出后续记录；若游标已落入保留边界之下则一并交出投递缺口
  PollFrom(stream: StreamId, cursorSeq: Seq) items: set of DeliveryItem
  ext rd entries rd retention
  post let afterCursor = {e | e in set entries &
                            e.pos.stream = stream and e.pos.seqNo > cursorSeq},
           compacted = exists b in set retention &
                         b.stream = stream and cursorSeq + 1 < b.seqNo in
         (compacted and items = afterCursor union {<DeliveryGap>})
         or (not compacted and items = afterCursor);

  -- 场景：向来源发起一次性读取；先核对来源当前会话能力，支持时追加观察记录并返回其位置
  ReadOnce: SourceId * CapabilityKey * StreamId * Seq * ObsPayload ==> ReadOutcome
  ReadOnce(source, cap, stream, seqNo, payload) ==
    cases CurrentCapability(source, cap):
      <Supported>   -> return Append(stream, seqNo, <Data>, payload, nil),
      <Unavailable> -> return <Unavailable>,
      <Unsupported> -> return <Unsupported>,
      others        -> return <Unknown>
    end;
end Observation
```
