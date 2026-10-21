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
      EncodeRequest renamed EncodeRequest;
      Encoded renamed Encoded;
      Evidence renamed Evidence
    operations
      Encode renamed Encode;
      CurrentCapability renamed CurrentCapability,
  from Observation
    types
      LogPosition renamed LogPosition
    operations
      ByClientKey renamed ByClientKey,
  from Request
    types
      EffectKind renamed EffectKind;
      Principal renamed Principal
exports
  types
    AttemptRef;
    struct Time;
    struct Intent;
    struct Admission;
    struct Waiting;
    struct Outcome;
    struct Attempt;
    struct AdmitOutcome;
    struct PrepareOutcome;
    struct GateOutcome;
    struct Reply
  functions
    BlockingHeads: set of Attempt * LaneKey -> set of Attempt
  operations
    Admit: Intent * Encoded * Admission ==> AdmitOutcome;
    Prepare: Intent * Admission ==> PrepareOutcome;
    Seal: AttemptRef ==> ();
    Expire: AttemptRef ==> ();
    Barrier: AttemptRef * Time ==> GateOutcome;
    CompleteSend: AttemptRef * Reply ==> ();
    Resolve: AttemptRef * Evidence ==> ();
    Abandon: AttemptRef ==> ();
    Attribute: AttemptRef * LogPosition ==> bool;
    LaneClear: LaneKey ==> bool;
    AttemptOf: AttemptRef ==> [Attempt]
definitions
types
  -- 一次写尝试的身份：它的准备记录在执行事实日志中的位置；核心造出
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
    deadline : Time                 -- 过期时刻：未过屏障而到达它即不再发出
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

  -- 尝试表：每条通道的阻塞头中普通准入至多一个，撤阻塞头准入必指向已过屏障的阻塞头
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
  -- 把编码后的意图记为新尝试：普通准入要求通道无阻塞头，撤阻塞头准入要求目标是已过屏障的阻塞头，绕过准入不看通道
  Admit(intent: Intent, enc: Encoded, how: Admission) r: AdmitOutcome
  ext wr attempts
  pre how = <CancelHead> => intent.target <> nil
  post let heads = BlockingHeads(rng attempts~, enc.lane),
           admitted = cases how:
                        <Normal> -> heads = {},
                        <CancelHead> -> exists b in set heads &
                                          b.barriered and b.key = intent.target,
                        <Bypass> -> true
                      end in
         (admitted and r <> <LaneBlocked>
          and r not in set dom attempts~
          and attempts = attempts~ munion
                {r |-> mk_Attempt(r, intent, enc.lane, enc.request, mk_token(r),
                                  how, false, <Active>, <Pending>)})
         or (not admitted and r = <LaneBlocked> and attempts = attempts~);

  -- 场景：编码意图并准入；编不出时不建立尝试
  Prepare: Intent * Admission ==> PrepareOutcome
  Prepare(intent, how) ==
    cases Encode(mk_EncodeRequest(intent.source, intent.payload)):
      <Unmapped> -> return <Unmapped>,
      enc -> return Admit(intent, enc, how)
    end;

  -- 落发送屏障：此后该尝试视为可能已发出
  Seal(ref: AttemptRef)
  ext wr attempts
  pre ref in set dom attempts
      and not attempts(ref).barriered and attempts(ref).waiting = <Active>
  post attempts = attempts~ ++ {ref |-> mu(attempts~(ref), barriered |-> true)};

  -- 未过屏障的尝试到期：等待结束，确知未发出
  Expire(ref: AttemptRef)
  ext wr attempts
  pre ref in set dom attempts
      and not attempts(ref).barriered and attempts(ref).waiting = <Active>
  post attempts = attempts~ ++ {ref |-> mu(attempts~(ref), waiting |-> <Expired>)};

  -- 场景：发出前门。到期即过期；来源此刻能力为支持才落屏障并发出；否则保持未发出等待
  Barrier: AttemptRef * Time ==> GateOutcome
  Barrier(ref, now) ==
    let a = attempts(ref) in
      if now >= a.intent.deadline
      then (Expire(ref); return <Expired>)
      elseif CurrentCapability(a.intent.source, a.intent.cap) = <Supported>
      then (Seal(ref); return <Sent>)
      else return <Waiting>
  pre ref in set dom attempts
      and not attempts(ref).barriered and attempts(ref).waiting = <Active>;

  -- 记录集成对已过屏障尝试的回复：有结论即结束等待；无响应进入结果未知并继续等待
  CompleteSend(ref: AttemptRef, reply: Reply)
  ext wr attempts
  pre ref in set dom attempts
      and attempts(ref).barriered and attempts(ref).outcome = <Pending>
  post attempts = attempts~ ++
         {ref |-> mu(attempts~(ref),
                     waiting |-> if reply = <NoResponse> then <Active> else <Finished>,
                     outcome |-> if reply = <NoResponse> then <Unknown> else reply)};

  -- 以取证结论收敛结果未知的尝试：查清即记下结果并结束仍在进行的等待；未查清不变；已放弃的等待不恢复
  Resolve(ref: AttemptRef, evidence: Evidence)
  ext wr attempts
  pre ref in set dom attempts and attempts(ref).outcome = <Unknown>
  post (evidence = <Inconclusive> and attempts = attempts~)
       or (evidence <> <Inconclusive>
           and attempts = attempts~ ++
                 {ref |-> mu(attempts~(ref),
                             outcome |-> evidence,
                             waiting |-> if attempts~(ref).waiting = <Active>
                                         then <Finished> else attempts~(ref).waiting)});

  -- 放弃为结果未知的尝试继续等待：移出阻塞头，不断言来源是否执行
  Abandon(ref: AttemptRef)
  ext wr attempts
  pre ref in set dom attempts
      and attempts(ref).waiting = <Active> and attempts(ref).outcome = <Unknown>
  post attempts = attempts~ ++ {ref |-> mu(attempts~(ref), waiting |-> <Abandoned>)};

  -- 场景：被动归因。带本尝试调用方键回声的观察记录到达时，作为查到的证据收敛
  Attribute: AttemptRef * LogPosition ==> bool
  Attribute(ref, pos) ==
    if pos in set ByClientKey(attempts(ref).key)
    then (Resolve(ref, <Found>); return true)
    else return false
  pre ref in set dom attempts and attempts(ref).outcome = <Unknown>;

  -- 某通道此刻是否没有阻塞头
  LaneClear(lane: LaneKey) r: bool
  ext rd attempts
  post r = (BlockingHeads(rng attempts, lane) = {});

  -- 查询尝试记录；不存在为空
  AttemptOf(ref: AttemptRef) r: [Attempt]
  ext rd attempts
  post (ref in set dom attempts and r = attempts(ref))
       or (ref not in set dom attempts and r = nil);
end Write
```
