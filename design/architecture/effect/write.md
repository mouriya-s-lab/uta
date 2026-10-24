# 写边界连接

```vdm
module Write
imports
  from Source
    types
      SourceId renamed SourceId;
      CapabilityKey renamed CapabilityKey;
      RawPayload renamed RawPayload
    operations
      SessionCapability renamed SessionCapability,
  from Observation
    types
      ClientKey renamed ClientKey;
      LogPosition renamed LogPosition
    operations
      ByClientKey renamed ByClientKey,
  from Mapping
    types
      LaneKey renamed LaneKey;
      IntentPayload renamed IntentPayload;
      EncodeRequest renamed EncodeRequest;
      Encoded renamed Encoded;
      Probe renamed Probe;
      Evidence renamed Evidence
    operations
      Encode renamed Encode;
      ClassifyEvidence renamed ClassifyEvidence,
  from Request
    types
      RequestRef renamed RequestRef;
      Note renamed Note
    operations
      Record renamed Record
exports
  types
    AttemptRef;
    struct Time;
    struct CancelHead;
    struct Admission;
    struct Intent;
    struct Waiting;
    struct Outcome;
    struct Attempt;
    struct PrepareOutcome;
    struct GateOutcome;
    struct Reply
  functions
    KeyOf: AttemptRef -> ClientKey;
    Progress: Attempt -> Note;
    BlockingHeads: set of Attempt * LaneKey -> set of Attempt
  operations
    Prepare: Intent ==> PrepareOutcome;
    Barrier: AttemptRef * Time ==> GateOutcome;
    Expire: AttemptRef ==> ();
    CompleteSend: AttemptRef * Reply ==> ();
    Resolve: AttemptRef * Evidence ==> ();
    ProbeAttempt: AttemptRef * RawPayload ==> Evidence;
    Attribute: AttemptRef * LogPosition ==> bool;
    Abandon: AttemptRef ==> ();
    AttemptOf: AttemptRef ==> [Attempt]
definitions
types
  -- 一次写尝试的身份；核心造出，使用方不读结构
  AttemptRef = token;

  -- 核心时钟的一个时刻；由调用方在过门时读入
  Time = nat;

  -- 撤阻塞头准入：指向已过屏障的阻塞头的调用方键
  CancelHead ::
    target : ClientKey;  -- 要撤的阻塞头

  -- 准入方式：通道空时的普通准入、针对结果未知的阻塞头的撤单
  Admission = <Normal> | CancelHead;

  -- 进入写边界的意图；谁获准它进入由业务决定
  Intent ::
    source    : SourceId        -- 发往哪个来源
    cap       : CapabilityKey   -- 发出前要核对的能力
    payload   : IntentPayload   -- 意图载荷
    deadline  : Time            -- 过期时刻：未过屏障而到达它即不再发出
    admission : Admission       -- 准入方式，随意图一起进入
    about     : [RequestRef];   -- 这次写所响应的请求；进展事件由此关联回请求

  -- 等待轴：核心还要不要为这次尝试等下去；源头是核心自己的记录
  Waiting = <Active> | <Finished> | <Expired> | <Abandoned>;

  -- 结果轴：这次写在来源发生没有；源头是来源经集成取回的副本
  Outcome = <Pending> | <Accepted> | <Rejected> | <NotSent> | <Unknown> | <Found> | <Absent>;

  -- 一次写尝试的记录
  Attempt ::
    ref       : AttemptRef   -- 身份
    intent    : Intent       -- 获准的意图
    lane      : LaneKey      -- 所在写通道
    request   : RawPayload   -- 映射编出的、已带调用方键的请求原文
    barriered : bool         -- 是否已落发送屏障
    waiting   : Waiting      -- 等待轴
    outcome   : Outcome;     -- 结果轴

  -- 准备结局：建立尝试、被通道阻塞、或映射编不出
  PrepareOutcome = AttemptRef | <LaneBlocked> | <Unmapped>;

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
                 card {h | h in set heads & h.intent.admission = <Normal>} <= 1
                 and card {h | h in set heads & is_CancelHead(h.intent.admission)} <= 1)
  init s == s = mk_Attempts({|->})
end

functions
  -- 判定：一次尝试的调用方键——尝试身份的单射，同一意图再次准入得到新键
  KeyOf: AttemptRef -> ClientKey
  KeyOf(ref) == mk_token(ref);

  -- 一次尝试此刻在两根轴上的位置记成的一条记事；记事的编码属实现，不展开
  Progress: Attempt -> Note
  Progress(a) == is not yet specified;

  -- 判定：某通道的阻塞头集合——等待仍为 Active 的尝试，不论是否已过屏障
  BlockingHeads: set of Attempt * LaneKey -> set of Attempt
  BlockingHeads(as, lane) ==
    {x | x in set as & x.lane = lane and x.waiting = <Active>};

operations
  -- 取一个尚未使用的尝试身份
  Mint() r: AttemptRef
  ext rd attempts
  post r not in set dom attempts;

  -- 把编码后的意图记为新尝试：普通准入要求通道无阻塞头，撤阻塞头准入要求目标是结果未知的阻塞头且通道上没有别的撤阻塞头
  Admit(ref: AttemptRef, intent: Intent, enc: Encoded) r: bool
  ext wr attempts
  pre ref not in set dom attempts
  post let heads = BlockingHeads(rng attempts~, enc.lane),
           admitted = cases intent.admission:
                        <Normal> -> heads = {},
                        mk_CancelHead(t) ->
                          (exists b in set heads & b.outcome = <Unknown> and KeyOf(b.ref) = t)
                          and not (exists c in set heads & is_CancelHead(c.intent.admission))
                      end in
         r = admitted
         and (admitted => attempts = attempts~ munion
                {ref |-> mk_Attempt(ref, intent, enc.lane, enc.request, false, <Active>, <Pending>)})
         and (not admitted => attempts = attempts~);

  -- 场景：准备一次写——先取身份与键，带键经 Encode 编码，经 Admit 准入后经 Announce 让请求方得知；编不出或通道阻塞时不建尝试
  Prepare: Intent ==> PrepareOutcome
  Prepare(intent) ==
    let ref = Mint() in
      cases Encode(mk_EncodeRequest(intent.source, intent.payload, KeyOf(ref))):
        <Unmapped> -> return <Unmapped>,
        enc -> if Admit(ref, intent, enc)
               then (Announce(ref); return ref)
               else return <LaneBlocked>
      end;

  -- 落发送屏障：此后该尝试视为可能已发出
  Seal(ref: AttemptRef)
  ext wr attempts
  pre ref in set dom attempts
      and not attempts(ref).barriered and attempts(ref).waiting = <Active>
  post attempts = attempts~ ++ {ref |-> mu(attempts~(ref), barriered |-> true)};

  -- 未过屏障的尝试结束等待：确知未发出，移出阻塞头
  MarkExpired(ref: AttemptRef)
  ext wr attempts
  pre ref in set dom attempts
      and not attempts(ref).barriered and attempts(ref).waiting = <Active>
  post attempts = attempts~ ++ {ref |-> mu(attempts~(ref), waiting |-> <Expired>)};

  -- 场景：把一次尝试此刻的位置经 Record 交给它所响应的请求方
  Announce: AttemptRef ==> ()
  Announce(ref) ==
    Record(attempts(ref).intent.about, Progress(attempts(ref)))
  pre ref in set dom attempts;

  -- 场景：未过屏障的尝试走到出口——经 MarkExpired 离开阻塞头，再经 Announce 让请求方得知；到期与人工丢弃同一出口
  Expire: AttemptRef ==> ()
  Expire(ref) ==
    (MarkExpired(ref);
     Announce(ref))
  pre ref in set dom attempts
      and not attempts(ref).barriered and attempts(ref).waiting = <Active>;

  -- 场景：发出前门——到期或来源已明确不支持即经 Expire 终止；来源此刻支持才经 Seal 落屏障并经 Announce 让请求方得知；其余保持未发出等待
  Barrier: AttemptRef * Time ==> GateOutcome
  Barrier(ref, now) ==
    let cap = SessionCapability(attempts(ref).intent.source, attempts(ref).intent.cap) in
      if now >= attempts(ref).intent.deadline or cap = <Unsupported>
      then (Expire(ref); return <Expired>)
      elseif cap = <Supported>
      then (Seal(ref);
            Announce(ref);
            return <Sent>)
      else return <Waiting>
  pre ref in set dom attempts
      and not attempts(ref).barriered and attempts(ref).waiting = <Active>;

  -- 记下集成的回复：有结论即结束等待；无响应进入结果未知并继续等待
  MarkReply(ref: AttemptRef, reply: Reply)
  ext wr attempts
  pre ref in set dom attempts
      and attempts(ref).barriered and attempts(ref).outcome = <Pending>
  post attempts = attempts~ ++
         {ref |-> mu(attempts~(ref),
                     waiting |-> if reply = <NoResponse> then <Active> else <Finished>,
                     outcome |-> if reply = <NoResponse> then <Unknown> else reply)};

  -- 场景：集成的回复到达——经 MarkReply 进入两根轴，再经 Announce 让请求方得知
  CompleteSend: AttemptRef * Reply ==> ()
  CompleteSend(ref, reply) ==
    (MarkReply(ref, reply);
     Announce(ref))
  pre ref in set dom attempts
      and attempts(ref).barriered and attempts(ref).outcome = <Pending>;

  -- 以取证结论收敛结果未知的尝试：查清即记下结果并结束仍在进行的等待；未查清不变；已放弃的等待不恢复
  MarkEvidence(ref: AttemptRef, evidence: Evidence)
  ext wr attempts
  pre ref in set dom attempts and attempts(ref).outcome = <Unknown>
  post (evidence = <Inconclusive> and attempts = attempts~)
       or (evidence <> <Inconclusive>
           and attempts = attempts~ ++
                 {ref |-> mu(attempts~(ref),
                             outcome |-> evidence,
                             waiting |-> if attempts~(ref).waiting = <Active>
                                         then <Finished> else attempts~(ref).waiting)});

  -- 场景：来源给出的结论进入尝试——先经 MarkEvidence 改两根轴，再经 Announce；未查清也公布，请求方由此知道查过而未定
  Resolve: AttemptRef * Evidence ==> ()
  Resolve(ref, evidence) ==
    (MarkEvidence(ref, evidence);
     Announce(ref))
  pre ref in set dom attempts and attempts(ref).outcome = <Unknown>;

  -- 场景：主动取证——回读原文连同本尝试的调用方键经 ClassifyEvidence 得出结论，再经 Resolve 收敛
  ProbeAttempt: AttemptRef * RawPayload ==> Evidence
  ProbeAttempt(ref, raw) ==
    let ev = ClassifyEvidence(mk_Probe(attempts(ref).intent.source, KeyOf(ref), raw)) in
      (Resolve(ref, ev); return ev)
  pre ref in set dom attempts and attempts(ref).outcome = <Unknown>;

  -- 场景：被动归因。带本尝试调用方键回声的观察记录到达时，作为查到的证据收敛
  Attribute: AttemptRef * LogPosition ==> bool
  Attribute(ref, pos) ==
    if pos in set ByClientKey(KeyOf(ref))
    then (Resolve(ref, <Found>); return true)
    else return false
  pre ref in set dom attempts and attempts(ref).outcome = <Unknown>;

  -- 放弃为结果未知的尝试继续等待：移出阻塞头，不断言来源是否执行
  MarkAbandoned(ref: AttemptRef)
  ext wr attempts
  pre ref in set dom attempts
      and attempts(ref).waiting = <Active> and attempts(ref).outcome = <Unknown>
  post attempts = attempts~ ++ {ref |-> mu(attempts~(ref), waiting |-> <Abandoned>)};

  -- 场景：放弃等待——经 MarkAbandoned 离开阻塞头，再经 Announce 让请求方得知
  Abandon: AttemptRef ==> ()
  Abandon(ref) ==
    (MarkAbandoned(ref);
     Announce(ref))
  pre ref in set dom attempts
      and attempts(ref).waiting = <Active> and attempts(ref).outcome = <Unknown>;


  -- 查询尝试记录；不存在为空
  AttemptOf(ref: AttemptRef) r: [Attempt]
  ext rd attempts
  post (ref in set dom attempts and r = attempts(ref))
       or (ref not in set dom attempts and r = nil);
end Write
```
