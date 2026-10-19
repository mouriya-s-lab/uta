# 受控写边界连接

```vdm
module Effect
imports
  from Source
    types
      SourceId renamed SourceId;
      CapabilityKey renamed CapabilityKey
    operations
      CurrentCapability renamed CurrentCapability,
  from Observation
    types
      RefHolder renamed RefHolder;
      LogPosition renamed LogPosition
    operations
      AcquireRef renamed AcquireRef;
      ReleaseRef renamed ReleaseRef
exports
  types
    LaneKey;
    struct AttemptId;
    IntentPayload;
    struct TerminalOutcome;
    struct Attention;
    struct InDoubt;
    struct Resolved;
    struct AttemptStage;
    struct Attempt;
    struct Evidence;
    struct PrepareOutcome;
    struct BarrierOutcome
  functions
    Blockers: LaneKey * map AttemptId to Attempt -> set of AttemptId
  operations
    LaneBlockers: LaneKey ==> set of AttemptId;
    Prepare: AttemptId * RefHolder * LaneKey * IntentPayload * set of LogPosition * [set of AttemptId] ==> PrepareOutcome;
    Barrier: AttemptId * SourceId * CapabilityKey ==> BarrierOutcome;
    CompleteSend: AttemptId * (TerminalOutcome | <NoResponse>) ==> ();
    Resolve: AttemptId * Evidence ==> ();
    Abandon: AttemptId ==> ();
    RecoverInDoubt: AttemptId * RefHolder * LaneKey * IntentPayload * set of LogPosition * SourceId * CapabilityKey * Evidence ==> ()
definitions
types
  -- 串行化外部写的通道键；结构对架构不透明，由业务单元映射
  LaneKey = token;

  -- 一次外部写尝试的身份
  AttemptId = token;

  -- 写意图的载荷；对架构不透明
  IntentPayload = token;

  -- 已由权威证据确立的写尝试终态：上游受理、上游拒绝或确知未发出
  TerminalOutcome = <Accepted> | <Rejected> | <NotSent>;

  -- 结果未知时的本地等待状态：仍在阻塞等待，或已由操作员放弃本地等待
  Attention = <Waiting> | <Abandoned>;

  -- 写已越过发送屏障但上游结果未知；放弃等待只改变 attention，不产生终态结论
  InDoubt ::
    attention : Attention;  -- 本地是否仍在阻塞同通道后续普通写

  -- 写尝试的结果已由来源证据最终确立
  Resolved ::
    outcome : TerminalOutcome;  -- 最终确立的结论

  -- 一次写尝试的阶段：已准备未发、已落发送屏障、结果未知或已终结
  AttemptStage = <Prepared> | <Barriered> | InDoubt | Resolved;

  -- 一条不可抹除的外部写尝试记录
  Attempt ::
    id     : AttemptId           -- 本次尝试的身份
    holder : RefHolder           -- 对应在观察日志中登记 basis 引用的持有者身份
    lane   : LaneKey             -- 所属写通道
    intent : IntentPayload       -- 所执行的意图载荷
    basis  : set of LogPosition  -- 决策所依据的观察位置集合
    stage  : AttemptStage;       -- 当前阶段与结局

  -- 取证渠道返回的三种封闭结论：未查清（Inconclusive）不得当作未发生（Absent）
  Evidence = <Found> | <Absent> | <Inconclusive>;

  -- Prepare 的结局：成功建立尝试，或因通道阻塞/引用越界而被拦下
  PrepareOutcome = AttemptId | <Blocked> | <Rejected>;

  -- 发送前过门与落屏障的结局：已落屏障可发送、因离线/能力未明而等待、或能力明确不支持而未发出
  BarrierOutcome = <Ready> | <Waiting> | <NotSent>;

  -- 写边界的尝试表：已记录的尝试永不删除
state EffectLog of
  attempts : map AttemptId to Attempt  -- 尝试身份到尝试记录的映射
  init e == e = mk_EffectLog({|->})
end

functions
  -- 判定指定通道上当前阻塞后续普通写的尝试集合：处于 Prepared、Barriered 或 InDoubt(Waiting) 的尝试
  Blockers: LaneKey * map AttemptId to Attempt -> set of AttemptId
  Blockers(lane, attempts) ==
    {a.id | a in set rng attempts &
       a.lane = lane
       and (a.stage = <Prepared>
            or a.stage = <Barriered>
            or a.stage = mk_InDoubt(<Waiting>))};

operations
  -- 查询指定通道上当前阻塞后续普通写的尝试集合
  LaneBlockers(lane: LaneKey) r: set of AttemptId
  ext rd attempts
  post r = Blockers(lane, attempts);

  -- 在尝试表中写入一条处于 Prepared 阶段的新尝试记录；已存在或通道未准入时返回 Blocked
  RecordPrepared(entry: Attempt, overrideBlockers: [set of AttemptId]) r: <Admitted> | <Blocked>
  ext wr attempts
  post let currentBlockers = Blockers(entry.lane, attempts~),
           admitted = (currentBlockers = {}) or (overrideBlockers = currentBlockers) in
         (entry.id not in set dom attempts~
          and entry.stage = <Prepared>
          and admitted
          and r = <Admitted>
          and attempts = attempts~ ++ {entry.id |-> entry})
         or ((entry.id in set dom attempts~ or entry.stage <> <Prepared> or not admitted)
             and r = <Blocked>
             and attempts = attempts~);

  -- 将获准意图交入写边界并锁住 basis 引用；普通放行要求通道无阻塞头，显式越顶要求精确指明当前阻塞头集合
  Prepare: AttemptId * RefHolder * LaneKey * IntentPayload * set of LogPosition * [set of AttemptId] ==> PrepareOutcome
  Prepare(id, holder, lane, intent, basis, overrideBlockers) ==
    cases AcquireRef(holder, basis):
      <Applied> ->
        let entry = mk_Attempt(id, holder, lane, intent, basis, <Prepared>) in
          cases RecordPrepared(entry, overrideBlockers):
            <Admitted> -> return id,
            others ->
              (ReleaseRef(holder);
               return <Blocked>)
          end,
      others -> return <Rejected>
    end;

  -- 将处于 Prepared 阶段的尝试推进为已落盘发送屏障的 Barriered 阶段
  MarkBarriered(id: AttemptId)
  ext wr attempts
  post (exists a in set rng attempts~ &
          a.id = id
          and a.stage = <Prepared>
          and attempts = attempts~ ++ {id |-> mu(a, stage |-> <Barriered>)})
       or ((not exists a in set rng attempts~ & a.id = id and a.stage = <Prepared>)
           and attempts = attempts~);

  -- 将未终结的尝试推进为终态 Resolved 并交回其引用持有者身份；已确立的终态不可逆转
  FinishAttempt(id: AttemptId, outcome: TerminalOutcome) holder: [RefHolder]
  ext wr attempts
  post (exists a in set rng attempts~ &
          a.id = id
          and (a.stage = <Prepared> or a.stage = <Barriered> or is_InDoubt(a.stage))
          and holder = a.holder
          and attempts = attempts~ ++ {id |-> mu(a, stage |-> mk_Resolved(outcome))})
       or ((not exists a in set rng attempts~ &
              a.id = id and (a.stage = <Prepared> or a.stage = <Barriered> or is_InDoubt(a.stage)))
           and holder = nil
           and attempts = attempts~);

  -- 将已落屏障或处于结果未知等待中的尝试更新为指定的 InDoubt 状态，并交回其引用持有者身份
  MarkInDoubt(id: AttemptId, attention: Attention) holder: [RefHolder]
  ext wr attempts
  post (exists a in set rng attempts~ &
          a.id = id
          and (a.stage = <Barriered> or a.stage = mk_InDoubt(<Waiting>))
          and holder = a.holder
          and attempts = attempts~ ++ {id |-> mu(a, stage |-> mk_InDoubt(attention))})
       or ((not exists a in set rng attempts~ &
              a.id = id and (a.stage = <Barriered> or a.stage = mk_InDoubt(<Waiting>)))
           and holder = nil
           and attempts = attempts~);

  -- 在真正调用外部写之前核对来源当前会话能力并落盘发送屏障；已越过屏障的尝试永不重入
  Barrier: AttemptId * SourceId * CapabilityKey ==> BarrierOutcome
  Barrier(id, source, cap) ==
    cases CurrentCapability(source, cap):
      <Supported> ->
        (MarkBarriered(id);
         return <Ready>),
      <Unavailable>, <Unknown> -> return <Waiting>,
      others ->
        let h = FinishAttempt(id, <NotSent>) in
          (if h <> nil then ReleaseRef(h) else skip;
           return <NotSent>)
    end;

  -- 记录单次外部写调用的返回：得业务回执或确知未发即终结并释放引用，无响应则进入 InDoubt(Waiting)
  CompleteSend: AttemptId * (TerminalOutcome | <NoResponse>) ==> ()
  CompleteSend(id, reply) ==
    cases reply:
      <NoResponse> ->
        let - = MarkInDoubt(id, <Waiting>) in skip,
      others ->
        let h = FinishAttempt(id, reply) in
          if h <> nil then ReleaseRef(h) else skip
    end;

  -- 用上游取证或归因证据收敛结果未知的尝试；即使已 Abandoned 仍可补全 Resolved 结论
  Resolve: AttemptId * Evidence ==> ()
  Resolve(id, evidence) ==
    cases evidence:
      <Found> ->
        let h = FinishAttempt(id, <Accepted>) in
          if h <> nil then ReleaseRef(h) else skip,
      <Absent> ->
        let h = FinishAttempt(id, <NotSent>) in
          if h <> nil then ReleaseRef(h) else skip,
      <Inconclusive> -> skip
    end;

  -- 放弃对结果未知尝试的本地等待以解除通道阻塞，并释放观察引用，但不改写外部结果结论
  Abandon: AttemptId ==> ()
  Abandon(id) ==
    let h = MarkInDoubt(id, <Abandoned>) in
      if h <> nil then ReleaseRef(h) else skip;

  -- 场景：写尝试越过屏障后无响应进入结果未知，先放弃本地等待解除阻塞，随后迟到的来源证据仍收敛其终态
  RecoverInDoubt: AttemptId * RefHolder * LaneKey * IntentPayload * set of LogPosition * SourceId * CapabilityKey * Evidence ==> ()
  RecoverInDoubt(id, holder, lane, intent, basis, source, cap, lateEvidence) ==
    cases Prepare(id, holder, lane, intent, basis, nil):
      <Blocked>, <Rejected> -> skip,
      others ->
        cases Barrier(id, source, cap):
          <Ready> ->
            (CompleteSend(id, <NoResponse>);
             Abandon(id);
             Resolve(id, lateEvidence)),
          others -> skip
        end
    end;
end Effect
```
