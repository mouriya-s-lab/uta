# 单据连接

```vdm
module Ticket
imports
  from Source
    types
      SourceId renamed SourceId;
      CapabilityKey renamed CapabilityKey;
      ClientKey renamed ClientKey,
  from Mapping
    types
      IntentPayload renamed IntentPayload,
  from Observation
    types
      LogPosition renamed LogPosition;
      BasisValidity renamed BasisValidity
    operations
      CheckBasis renamed CheckBasis,
  from Request
    types
      RequestRef renamed RequestRef;
      EffectKind renamed EffectKind;
      Principal renamed Principal
    operations
      RequestOf renamed RequestOf;
      StatusOf renamed StatusOf;
      Respond renamed Respond,
  from Write
    types
      AttemptRef renamed AttemptRef;
      Time renamed Time;
      Intent renamed Intent;
      Admission renamed Admission
    operations
      Prepare renamed Prepare
exports
  types
    TicketId;
    CheckKey;
    struct Ruling;
    struct Decision;
    struct Awaiting;
    struct Prepared;
    struct ClosedReason;
    struct Closed;
    struct TicketStatus;
    struct TicketRecord;
    struct Alignment;
    struct DecideOutcome;
    struct ReleaseOutcome
  functions
    GateOpen: TicketRecord * BasisValidity * Alignment * Time -> bool
  operations
    Draft: TicketRecord ==> ();
    Revise: TicketId * Principal * IntentPayload ==> bool;
    SubmitForDecision: TicketId * Principal ==> bool;
    SendBack: TicketId ==> bool;
    Transfer: TicketId * Principal * Principal ==> bool;
    Decide: TicketId * nat1 * Principal * Ruling ==> DecideOutcome;
    Withdraw: TicketId * Principal ==> bool;
    Close: TicketId * ClosedReason ==> ();
    Release: TicketId * Admission * Alignment * Time * nat ==> ReleaseOutcome;
    AdoptRequest: RequestRef * TicketRecord ==> bool;
    RecordOf: TicketId ==> [TicketRecord]
definitions
types
  -- 单据身份；业务造出，使用方不读结构
  TicketId = token;

  -- 放行前一项对账检查的身份；由业务选定必要项
  CheckKey = token;

  -- 审批裁决：批准或否决
  Ruling = <Approved> | <Rejected>;

  -- 一条绑定版本的裁决记录
  Decision ::
    decider : Principal  -- 谁作出
    ruling  : Ruling;    -- 结论

  -- 送审中：冻结在某个版本
  Awaiting ::
    version : nat1;  -- 冻结的版本号

  -- 已放行：对应的写尝试
  Prepared ::
    attempt : AttemptRef;  -- 放行产生的尝试

  -- 关闭原因
  ClosedReason = Prepared | <Withdrawn> | <DecisionRejected> | <Expired>;

  -- 已关闭
  Closed ::
    reason : ClosedReason;  -- 为何关闭

  -- 单据状态：草稿中、送审中、已关闭
  TicketStatus = <Drafting> | Awaiting | Closed;

  -- 一张单据：一把锁、一条线性版本链、绑定版本的裁决
  TicketRecord ::
    id          : TicketId                -- 身份
    responsible : Principal               -- 当前持有编辑权的负责人
    source      : SourceId                -- 目标来源
    cap         : CapabilityKey           -- 放行前核对的能力
    kind        : EffectKind              -- 效应种类
    versions    : seq1 of IntentPayload   -- 版本链，下标即版本号
    decisions   : map nat1 to Decision    -- 各版本的裁决
    basis       : set of LogPosition      -- 决策依据的观察位置
    deadline    : Time                    -- 过期时刻
    target      : [ClientKey]             -- 撤阻塞头时指向的阻塞头调用方键
    status      : TicketStatus;           -- 当前状态

  -- 放行前的对账结果：各必要项是否与当前观察对齐
  Alignment = map CheckKey to (<Aligned> | <Diverged> | <Undecidable>);

  -- 裁决结局：已记录，或期望版本不符与同版本已决时的冲突
  DecideOutcome = <Recorded> | <Conflict>;

  -- 放行结局：已放行的尝试、门未开而等待、或单据已关闭
  ReleaseOutcome = AttemptRef | <Waiting> | <Closed>;

  -- 单据表：裁决只落在已有版本上，送审冻结的是最新版本
state Tickets of
  byId : map TicketId to TicketRecord  -- 全部单据
  inv mk_Tickets(b) ==
        forall id in set dom b &
          b(id).id = id
          and dom b(id).decisions subset inds b(id).versions
          and (is_Awaiting(b(id).status) => b(id).status.version = len b(id).versions)
  init t == t = mk_Tickets({|->})
end

functions
  -- 放行门：送审中的当前版本已批准、依据新鲜、必要项全部对齐、未过期
  GateOpen: TicketRecord * BasisValidity * Alignment * Time -> bool
  GateOpen(rec, validity, align, now) ==
    is_Awaiting(rec.status)
    and rec.status.version in set dom rec.decisions
    and rec.decisions(rec.status.version).ruling = <Approved>
    and validity = <Fresh>
    and (forall k in set dom align & align(k) = <Aligned>)
    and now < rec.deadline;

operations
  -- 起草新单据：负责人取锁，只有第一个版本，尚无裁决
  Draft(rec: TicketRecord)
  ext wr byId
  pre rec.id not in set dom byId
      and rec.status = <Drafting>
      and len rec.versions = 1
      and rec.decisions = {|->}
  post byId = byId~ munion {rec.id |-> rec};

  -- 负责人在草稿中追加新版本；非负责人或非草稿状态时不生效
  Revise(id: TicketId, who: Principal, next: IntentPayload) r: bool
  ext wr byId
  post let ok = id in set dom byId~
                and byId~(id).responsible = who
                and byId~(id).status = <Drafting> in
         r = ok
         and (ok => byId = byId~ ++ {id |-> mu(byId~(id), versions |-> byId~(id).versions ^ [next])})
         and (not ok => byId = byId~);

  -- 负责人把草稿送审：冻结在最新版本
  SubmitForDecision(id: TicketId, who: Principal) r: bool
  ext wr byId
  post let ok = id in set dom byId~
                and byId~(id).responsible = who
                and byId~(id).status = <Drafting> in
         r = ok
         and (ok => byId = byId~ ++ {id |-> mu(byId~(id), status |-> mk_Awaiting(len byId~(id).versions))})
         and (not ok => byId = byId~);

  -- 把送审中的单据退回草稿；负责人不变，已有裁决保留在其版本上
  SendBack(id: TicketId) r: bool
  ext wr byId
  post let ok = id in set dom byId~ and is_Awaiting(byId~(id).status) in
         r = ok
         and (ok => byId = byId~ ++ {id |-> mu(byId~(id), status |-> <Drafting>)})
         and (not ok => byId = byId~);

  -- 负责人把未关闭的单据移交给另一个主体
  Transfer(id: TicketId, src: Principal, dst: Principal) r: bool
  ext wr byId
  post let ok = id in set dom byId~
                and byId~(id).responsible = src
                and not is_Closed(byId~(id).status) in
         r = ok
         and (ok => byId = byId~ ++ {id |-> mu(byId~(id), responsible |-> dst)})
         and (not ok => byId = byId~);

  -- 对送审中的指定版本作出裁决；期望版本不符或该版本已决时返回冲突且不改状态；否决即关闭
  Decide(id: TicketId, version: nat1, who: Principal, ruling: Ruling) r: DecideOutcome
  ext wr byId
  post let ok = id in set dom byId~
                and is_Awaiting(byId~(id).status)
                and byId~(id).status.version = version
                and version not in set dom byId~(id).decisions in
         (ok and r = <Recorded>
          and byId = byId~ ++
                {id |-> mu(byId~(id),
                           decisions |-> byId~(id).decisions munion {version |-> mk_Decision(who, ruling)},
                           status |-> if ruling = <Rejected>
                                      then mk_Closed(<DecisionRejected>)
                                      else byId~(id).status)})
         or (not ok and r = <Conflict> and byId = byId~);

  -- 负责人撤回未关闭的单据
  Withdraw(id: TicketId, who: Principal) r: bool
  ext wr byId
  post let ok = id in set dom byId~
                and byId~(id).responsible = who
                and not is_Closed(byId~(id).status) in
         r = ok
         and (ok => byId = byId~ ++ {id |-> mu(byId~(id), status |-> mk_Closed(<Withdrawn>))})
         and (not ok => byId = byId~);

  -- 以给定原因关闭未关闭的单据
  Close(id: TicketId, reason: ClosedReason)
  ext wr byId
  pre id in set dom byId and not is_Closed(byId(id).status)
  post byId = byId~ ++ {id |-> mu(byId~(id), status |-> mk_Closed(reason))};

  -- 场景：放行门。到期即关闭；门未开或通道不可准入即等待；准入成功则以准备记录关闭单据
  Release: TicketId * Admission * Alignment * Time * nat ==> ReleaseOutcome
  Release(id, how, align, now, lag) ==
    let rec = byId(id) in
      if is_Closed(rec.status) then return <Closed>
      elseif now >= rec.deadline then (Close(id, <Expired>); return <Closed>)
      elseif not GateOpen(rec, CheckBasis(rec.basis, lag), align, now) then return <Waiting>
      else cases Prepare(mk_Intent(rec.source, rec.cap, rec.kind,
                                   rec.versions(len rec.versions), rec.basis,
                                   rec.deadline, rec.target, rec.responsible), how):
             <Unmapped> -> return <Waiting>,
             <LaneBlocked> -> return <Waiting>,
             ref -> (Close(id, mk_Prepared(ref)); return ref)
           end
  pre id in set dom byId and (how = <CancelHead> => byId(id).target <> nil);

  -- 场景：写处理器接手一条等待中的请求。以请求发出者为负责人、请求依据为依据起草单据并送审，答复以单据身份回填请求；请求不存在或不在等待中则不接手
  AdoptRequest: RequestRef * TicketRecord ==> bool
  AdoptRequest(ref, shape) ==
    let req = RequestOf(ref), status = StatusOf(ref) in
      if req = nil or status <> <Pending> or req.kind <> shape.kind
      then return false
      else (Draft(mu(shape, responsible |-> req.member, basis |-> req.basis));
            let - = SubmitForDecision(shape.id, req.member) in skip;
            Respond(ref, mk_token(shape.id));
            return true)
  pre shape.id not in set dom byId
      and shape.status = <Drafting>
      and len shape.versions = 1
      and shape.decisions = {|->};

  -- 查询单据；不存在为空
  RecordOf(id: TicketId) r: [TicketRecord]
  ext rd byId
  post (id in set dom byId and r = byId(id))
       or (id not in set dom byId and r = nil);
end Ticket
```
