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
  operations
    LaneBlockers: LaneKey ==> set of AttemptId;
    Prepare: Attempt * [set of AttemptId] ==> PrepareOutcome;
    Barrier: AttemptId ==> BarrierOutcome;
    CompleteSend: AttemptId * (TerminalOutcome | <NoResponse>) ==> [RefHolder];
    Resolve: AttemptId * Evidence ==> [RefHolder];
    Abandon: AttemptId ==> [RefHolder];
    RecoverInDoubt: Attempt * SourceId * CapabilityKey * Evidence ==> ()
definitions
types
  -- 并发互斥通道的身份；供单据、交易与程序等所有发起外部作用的单元共享串行化边界
  LaneKey = token;

  -- 一次外部作用尝试的身份；供所有发起或追踪外部作用的业务单元共享
  AttemptId = token;

  -- 待执行的外部作用内容；供单据、交易与程序等单元交入写边界并原样传递
  IntentPayload = token;

  -- 外部作用尝试由来源确认的终态结论：已被来源接受、被来源拒绝或确知未发出
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

  -- Prepare 的结局：成功建立尝试，或因通道阻塞而被拦下
  PrepareOutcome = AttemptId | <Blocked>;

  -- 发送前落盘屏障的结局：已落屏障可发送，或因状态不符而未发出
  BarrierOutcome = <Ready> | <NotSent>;

  -- 写边界的尝试表：已记录的尝试永不删除
state EffectLog of
  attempts : map AttemptId to Attempt  -- 尝试身份到尝试记录的映射
  init e == e = mk_EffectLog({|->})
end

operations
  -- 查询指定通道上当前阻塞后续普通写的尝试集合
  LaneBlockers(lane: LaneKey) r: set of AttemptId
  ext rd attempts
  post r = {a.id | a in set rng attempts &
              a.lane = lane
              and (a.stage = <Prepared>
                   or a.stage = <Barriered>
                   or a.stage = mk_InDoubt(<Waiting>))};

  -- 将获准尝试记入写边界；普通准入要求通道无未决尝试，越顶准入要求显式指明当前全部未决尝试
  Prepare(entry: Attempt, overrideBlockers: [set of AttemptId]) r: PrepareOutcome
  ext wr attempts
  post let currentBlockers = {a.id | a in set rng attempts~ &
                                a.lane = entry.lane
                                and (a.stage = <Prepared>
                                     or a.stage = <Barriered>
                                     or a.stage = mk_InDoubt(<Waiting>))},
           admitted = (currentBlockers = {}) or (overrideBlockers = currentBlockers) in
         (entry.id not in set dom attempts~
          and entry.stage = <Prepared>
          and admitted
          and r = entry.id
          and attempts = attempts~ ++ {entry.id |-> entry})
         or ((entry.id in set dom attempts~ or entry.stage <> <Prepared> or not admitted)
             and r = <Blocked>
             and attempts = attempts~);

  -- 在真正调用外部写之前将 Prepared 尝试推进为已落盘发送屏障的 Barriered 阶段；已越过屏障的尝试永不重入
  Barrier(id: AttemptId) r: BarrierOutcome
  ext wr attempts
  post (exists a in set rng attempts~ &
          a.id = id
          and a.stage = <Prepared>
          and r = <Ready>
          and attempts = attempts~ ++ {id |-> mu(a, stage |-> <Barriered>)})
       or ((not exists a in set rng attempts~ & a.id = id and a.stage = <Prepared>)
           and r = <NotSent>
           and attempts = attempts~);

  -- 记录单次外部写调用的返回：得回执即终结并交回引用持有者，无响应则进入 InDoubt(Waiting)
  CompleteSend(id: AttemptId, reply: TerminalOutcome | <NoResponse>) releasedHolder: [RefHolder]
  ext wr attempts
  post (reply = <NoResponse>
        and exists a in set rng attempts~ &
              a.id = id
              and a.stage = <Barriered>
              and releasedHolder = nil
              and attempts = attempts~ ++ {id |-> mu(a, stage |-> mk_InDoubt(<Waiting>))})
       or (reply <> <NoResponse>
           and exists a in set rng attempts~ &
                 a.id = id
                 and a.stage = <Barriered>
                 and releasedHolder = a.holder
                 and attempts = attempts~ ++ {id |-> mu(a, stage |-> mk_Resolved(reply))})
       or ((not exists a in set rng attempts~ & a.id = id and a.stage = <Barriered>)
           and releasedHolder = nil
           and attempts = attempts~);

  -- 用上游取证或归因证据收敛结果未知的尝试；即使已 Abandoned 仍可补全 Resolved 结论
  Resolve(id: AttemptId, evidence: Evidence) releasedHolder: [RefHolder]
  ext wr attempts
  post (evidence = <Found>
        and exists a in set rng attempts~ &
              a.id = id
              and is_InDoubt(a.stage)
              and releasedHolder = a.holder
              and attempts = attempts~ ++ {id |-> mu(a, stage |-> mk_Resolved(<Accepted>))})
       or (evidence = <Absent>
           and exists a in set rng attempts~ &
                 a.id = id
                 and is_InDoubt(a.stage)
                 and releasedHolder = a.holder
                 and attempts = attempts~ ++ {id |-> mu(a, stage |-> mk_Resolved(<NotSent>))})
       or ((evidence = <Inconclusive>
            or not exists a in set rng attempts~ & a.id = id and is_InDoubt(a.stage))
           and releasedHolder = nil
           and attempts = attempts~);

  -- 放弃对结果未知尝试的本地等待以解除通道阻塞，并交回其观察引用持有者，但不改写外部结果结论
  Abandon(id: AttemptId) releasedHolder: [RefHolder]
  ext wr attempts
  post (exists a in set rng attempts~ &
          a.id = id
          and a.stage = mk_InDoubt(<Waiting>)
          and releasedHolder = a.holder
          and attempts = attempts~ ++ {id |-> mu(a, stage |-> mk_InDoubt(<Abandoned>))})
       or ((not exists a in set rng attempts~ & a.id = id and a.stage = mk_InDoubt(<Waiting>))
           and releasedHolder = nil
           and attempts = attempts~);

  -- 场景：登记观察引用并准入尝试，核对来源能力后落屏障发出，无响应进入结果未知，放弃等待后由迟到证据收敛
  RecoverInDoubt: Attempt * SourceId * CapabilityKey * Evidence ==> ()
  RecoverInDoubt(entry, source, cap, lateEvidence) ==
    cases AcquireRef(entry.holder, entry.basis):
      <Applied> ->
        cases Prepare(entry, nil):
          <Blocked> -> ReleaseRef(entry.holder),
          others ->
            cases CurrentCapability(source, cap):
              <Supported> ->
                cases Barrier(entry.id):
                  <Ready> ->
                    (let - = CompleteSend(entry.id, <NoResponse>) in skip;
                     let h1 = Abandon(entry.id) in
                       if h1 <> nil then ReleaseRef(h1) else skip;
                     let h2 = Resolve(entry.id, lateEvidence) in
                       if h2 <> nil then ReleaseRef(h2) else skip),
                  others -> ReleaseRef(entry.holder)
                end,
              others -> ReleaseRef(entry.holder)
            end
        end,
      others -> skip
    end;
end Effect
```
