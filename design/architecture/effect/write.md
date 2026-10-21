# 写边界连接

```vdm
module Write
imports
  from Source
    types
      SourceId renamed SourceId;
      CapabilityKey renamed CapabilityKey;
      ClientKey renamed ClientKey;
      RawPayload renamed RawPayload,
  from Mapping
    types
      LaneKey renamed LaneKey;
      IntentPayload renamed IntentPayload;
      Encoded renamed Encoded;
      EncodeOutcome renamed EncodeOutcome;
      Evidence renamed Evidence;
      CapabilityStatus renamed CapabilityStatus
    operations
      Encode renamed Encode;
      CurrentCapability renamed CurrentCapability,
  from Observation
    types
      RefHolder renamed RefHolder;
      LogPosition renamed LogPosition;
      RefOutcome renamed RefOutcome
    operations
      AcquireRef renamed AcquireRef;
      ReleaseRef renamed ReleaseRef;
      ByClientKey renamed ByClientKey,
  from Request
    types
      EffectKind renamed EffectKind,
  from Session
    types
      Principal renamed Principal
exports
  types
    AttemptRef;
    Time;
    struct Intent;
    struct Admission;
    struct Waiting;
    struct Outcome;
    struct Attempt;
    struct AdmitOutcome;
    struct PrepareOutcome;
    struct GateOutcome;
    struct Reply
  operations
    Admit: Intent * Encoded * Admission ==> AdmitOutcome;
    Prepare: Intent * Admission ==> PrepareOutcome;
    Barrier: AttemptRef * Time ==> GateOutcome;
    CompleteSend: AttemptRef * Reply ==> ();
    Resolve: AttemptRef * Evidence ==> ();
    Abandon: AttemptRef ==> ();
    Attribute: AttemptRef * LogPosition ==> ();
    LaneClear: LaneKey ==> bool;
    AttemptOf: AttemptRef ==> [Attempt]
  functions
    BlockingHeads: set of Attempt * LaneKey -> set of Attempt
definitions
types
  -- 一次写尝试的身份：它的 Prepared 记录在执行事实日志中的位置；核心造出
  AttemptRef = token;

  -- 核心时钟的一个时刻；由调用方在过门时读入
  Time = nat;

  -- 已获准进入写边界的意图
  Intent ::
    source   : SourceId             -- 发往哪个来源
    cap      : CapabilityKey        -- 发出前要核对的能力
    kind     : EffectKind           -- 效应种类
    payload  : IntentPayload        -- 意图载荷
    basis    : set of LogPosition   -- 决策所依据的观察位置
    deadline : Time                 -- 过期时刻：未过屏障而超过它即不再发出
    target   : [ClientKey]          -- 撤阻塞头时指向的阻塞头调用方键
    issuer   : Principal;           -- 以谁的身份发出

  -- 准入方式：通道空时的普通准入、针对已过屏障阻塞头的撤单、显式绕过
  Admission = <Normal> | <CancelHead> | <Bypass>;

  -- 等待轴：核心还要不要为这次尝试等下去；源头是核心自己的记录
  Waiting = <Active> | <Finished> | <Expired> | <Abandoned>;

  -- 结果轴：这次写在来源发生没有；源头是来源经集成取回的副本
  Outcome = <Pending> | <Accepted> | <Rejected> | <NotSent> | <Unknown> | <Found> | <Absent>;

  -- 一次写尝试的记录
  Attempt ::
    ref       : AttemptRef   -- 身份
    intent    : Intent       -- 获准的意图
    lane      : LaneKey      -- 所在写通道
    request   : RawPayload   -- 映射编出的请求原文
    key       : ClientKey    -- 核心按 ref 铸造的调用方键
    admitted  : Admission    -- 准入方式
    barriered : bool         -- 是否已落发送屏障
    waiting   : Waiting      -- 等待轴
    outcome   : Outcome;     -- 结果轴

  -- 准入结局：建立尝试，或被通道阻塞
  AdmitOutcome = AttemptRef | <LaneBlocked>;

  -- 准备结局：准入结局，或映射编不出
  PrepareOutcome = AdmitOutcome | <Unmapped>;

  -- 过门结局：已发出、已过期、或条件不满足而等待
  GateOutcome = <Sent> | <Expired> | <Waiting>;

  -- 集成对一次写调用的回复：受理、拒绝、证明未交出、无响应
  Reply = <Accepted> | <Rejected> | <NotSent> | <NoResponse>;

  -- 尝试表
state Attempts of
  attempts : map AttemptRef to Attempt  -- 全部写尝试
  inv mk_Attempts(a) ==
        (forall r in set dom a & a(r).ref = r)
        and (forall x in set rng a &
               (not x.barriered => x.outcome = <Pending>)
               and (x.waiting = <Active> => x.outcome in set {<Pending>, <Unknown>})
               and (x.waiting = <Finished> =>
                      x.outcome in set {<Accepted>, <Rejected>, <NotSent>, <Found>, <Absent>})
               and (x.waiting = <Expired> => not x.barriered)
               and (x.waiting = <Abandoned> => x.outcome in set {<Unknown>, <Found>, <Absent>}))
        and (forall x in set rng a &
               let heads = BlockingHeads(rng a, x.lane) in
                 card {h | h in set heads & h.admitted = <Normal>} <= 1
                 and forall h in set heads & h.admitted = <CancelHead> =>
                       exists b in set heads & b.barriered and b.key = h.intent.target)
  init s == s = mk_Attempts({|->})
end

functions
  -- 某通道的阻塞头集合：等待仍为 Active 的尝试
  BlockingHeads: set of Attempt * LaneKey -> set of Attempt
  BlockingHeads(as, lane) ==
    {x | x in set as & x.lane = lane and x.waiting = <Active>};

operations
  Admit: Intent * Encoded * Admission ==> AdmitOutcome
  Admit(intent, enc, how) == is not yet specified;

  Prepare: Intent * Admission ==> PrepareOutcome
  Prepare(intent, how) == is not yet specified;

  Barrier: AttemptRef * Time ==> GateOutcome
  Barrier(ref, now) == is not yet specified;

  CompleteSend: AttemptRef * Reply ==> ()
  CompleteSend(ref, reply) == is not yet specified;

  Resolve: AttemptRef * Evidence ==> ()
  Resolve(ref, evidence) == is not yet specified;

  Abandon: AttemptRef ==> ()
  Abandon(ref) == is not yet specified;

  Attribute: AttemptRef * LogPosition ==> ()
  Attribute(ref, pos) == is not yet specified;

  LaneClear: LaneKey ==> bool
  LaneClear(lane) == is not yet specified;

  AttemptOf: AttemptRef ==> [Attempt]
  AttemptOf(ref) == is not yet specified;
end Write
```
