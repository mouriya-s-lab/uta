# 观察与进度连接

```vdm
module Observation
imports
  from Source
    types SourceId renamed SourceId
exports
  types
    StreamId;
    ConsumerId;
    RefHolder;
    ObsPayload;
    struct Seq;
    struct LogPosition;
    struct Entry;
    struct DeliveryItem;
    struct BasisStatus;
    struct ReadOutcome
  functions
    BasisValid: set of LogPosition * map StreamId to seq of Entry * map StreamId to Seq -> BasisStatus
  operations
    Append: StreamId * (<Data> | <SourceGap>) * ObsPayload * [LogPosition] ==> LogPosition;
    AcquireRef: RefHolder * set of LogPosition ==> ();
    ReleaseRef: RefHolder ==> ();
    AdvanceRetention: StreamId * Seq ==> <Applied> | <Rejected>;
    Acknowledge: ConsumerId * StreamId * Seq ==> ();
    Poll: ConsumerId * StreamId ==> seq of DeliveryItem;
    ReadOnce: SourceId * StreamId ==> ReadOutcome
definitions
types
  -- 观察流的身份；提供方创建，使用方原样传递
  StreamId = token;

  -- 消费观察流的持有者身份
  ConsumerId = token;

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

  -- 观察流上的一条不可变记录：数据记录或来源断代缺口，可选指名它修正取代的旧位置
  Entry ::
    pos        : LogPosition            -- 本条记录的位置
    kind       : <Data> | <SourceGap>   -- 正常数据或来源连续性中断
    payload    : ObsPayload             -- 观察记录携带的不透明载荷
    supersedes : [LogPosition];         -- 修正派生值时所取代的旧记录位置

  -- 向消费者投递的项：流上的记录，或该消费者因慢消费/历史压缩产生的投递缺口
  DeliveryItem = Entry | <DeliveryGap>;

  -- 依据位置集合的有效性判定结果：新鲜可用、已被后续记录修正撤回、或已落入保留边界之下
  BasisStatus = <Fresh> | <Retracted> | <BeyondRetention>;

  -- 一次性主动读取的结局：返回追加的观察位置序列，或区分渠道不可用与能力不支持
  ReadOutcome = seq of LogPosition | <Unavailable> | <Unsupported>;

  -- 观察日志、三种进度（消费游标、完备区间、保留边界）与活跃引用登记
state Journal of
  entries      : map StreamId to seq of Entry            -- 每条流已追加的记录序列
  retention    : map StreamId to Seq                     -- 每条流仍可精确重建的最早序号
  completeness : map StreamId to set of (Seq * Seq)      -- 来源证据证明已完整覆盖的序号区间
  cursors      : map (ConsumerId * StreamId) to Seq      -- 消费者已确认处理到的序号
  activeRefs   : map RefHolder to set of LogPosition     -- 活跃对象登记保护的观察位置集合
  inv mk_Journal(es, ret, covered, cur, refs) ==
        dom ret subset dom es
        and dom covered subset dom es
        and (forall mk_(-, s) in set dom cur & s in set dom es)
        and forall p in set dunion rng refs &
              p.stream in set dom ret => p.seqNo >= ret(p.stream)
  init j == j = mk_Journal({|->}, {|->}, {|->}, {|->}, {|->})
end

functions
  -- 判定一组依据位置是否仍在保留边界之上且未被后续记录修正撤回
  BasisValid: set of LogPosition * map StreamId to seq of Entry * map StreamId to Seq -> BasisStatus
  BasisValid(basis, entries, retention) ==
    is not yet specified;

operations
  -- 在指定观察流末尾追加一条记录并分配下一个位置
  Append: StreamId * (<Data> | <SourceGap>) * ObsPayload * [LogPosition] ==> LogPosition
  Append(stream, kind, payload, supersedes) ==
    is not yet specified;

  -- 为活跃对象登记需要保护的观察位置集合，阻止保留边界越过它们
  AcquireRef: RefHolder * set of LogPosition ==> ()
  AcquireRef(holder, positions) ==
    is not yet specified;

  -- 释放对象持有的全部观察位置引用
  ReleaseRef: RefHolder ==> ()
  ReleaseRef(holder) ==
    is not yet specified;

  -- 推进指定流的保留边界；若越过仍被活跃引用的位置则拒绝推进
  AdvanceRetention: StreamId * Seq ==> <Applied> | <Rejected>
  AdvanceRetention(stream, bound) ==
    is not yet specified;

  -- 确认消费者在指定流上已处理到的位置
  Acknowledge: ConsumerId * StreamId * Seq ==> ()
  Acknowledge(consumer, stream, seqNo) ==
    is not yet specified;

  -- 按消费者当前游标取出待投递的记录或投递缺口
  Poll: ConsumerId * StreamId ==> seq of DeliveryItem
  Poll(consumer, stream) ==
    is not yet specified;

  -- 向指定来源发起一次性读取并将结果追加进观察流
  ReadOnce: SourceId * StreamId ==> ReadOutcome
  ReadOnce(source, stream) ==
    is not yet specified;
end Observation
```
