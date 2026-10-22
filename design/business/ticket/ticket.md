# 单据连接

```vdm
module Ticket
imports
  from Source
    types
      SourceId renamed SourceId;
      CapabilityKey renamed CapabilityKey;
      SessionVerdict renamed SessionVerdict
    operations
      SessionCapability renamed SessionCapability,
  from Mapping
    types
      IntentPayload renamed IntentPayload,
  from Observation
    types
      LogPosition renamed LogPosition
    operations
      CheckBasis renamed CheckBasis,
  from Request
    types
      RequestRef renamed RequestRef;
      EffectKind renamed EffectKind;
      Principal renamed Principal;
      Note renamed Note
    operations
      RequestOf renamed RequestOf;
      StatusOf renamed StatusOf;
      Respond renamed Respond;
      Record renamed Record,
  from Write
    types
      AttemptRef renamed AttemptRef;
      Time renamed Time;
      Admission renamed Admission;
      Intent renamed Intent
    operations
      Prepare renamed Prepare
exports
  types
    TicketId;
    CheckKey;
    struct Ruling;
    struct Decision;
    struct Awaiting;
    struct Approved;
    struct Prepared;
    struct ClosedReason;
    struct Closed;
    struct TicketStatus;
    struct Version;
    struct Proposal;
    struct TicketRecord;
    struct Alignment;
    struct Lapse;
    struct GateVerdict;
    struct DecideOutcome;
    struct ReleaseOutcome
  functions
    Current: TicketRecord -> Version;
    Gate: TicketRecord * bool * Alignment * Time * SessionVerdict -> GateVerdict;
    ToIntent: TicketRecord -> Intent;
    Progress: TicketRecord -> Note
  operations
    Draft: Proposal ==> ();
    Revise: TicketId * Principal * Version ==> ();
    SubmitForDecision: TicketId * Principal ==> ();
    SendBack: TicketId ==> ();
    Transfer: TicketId * Principal * Principal ==> ();
    Decide: TicketId * nat1 * Principal * Ruling ==> DecideOutcome;
    Withdraw: TicketId * Principal ==> ();
    Release: TicketId * Alignment * Time ==> ReleaseOutcome;
    AdoptRequest: RequestRef * Proposal ==> bool;
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

  -- 送审中：冻结在某个版本，等待裁决
  Awaiting ::
    version : nat1;  -- 冻结的版本号

  -- 已批准：某个版本获批，等待放行
  Approved ::
    version : nat1;  -- 获批的版本号

  -- 已放行：对应的写尝试
  Prepared ::
    attempt : AttemptRef;  -- 放行产生的尝试

  -- 放行门关单的三种事实，后续处置各不相同：到期可重起同一意图；来源不支持要换来源或等能力变化；依据失效要重取依据再起
  Lapse = <Expired> | <Unsupported> | <BasisLost>;

  -- 关闭原因：已放行、负责人撤回、裁决否决、或放行门确认了一种不可逆事实
  ClosedReason = Prepared | <Withdrawn> | <DecisionRejected> | Lapse;

  -- 已关闭
  Closed ::
    reason : ClosedReason;  -- 为何关闭

  -- 单据状态：草稿中、送审中、已批准、已关闭
  TicketStatus = <Drafting> | Awaiting | Approved | Closed;

  -- 单据的一个版本：裁决所绑定的全部内容
  Version ::
    intent    : IntentPayload       -- 意图
    basis     : set of LogPosition  -- 决策依据的观察位置
    deadline  : Time                -- 过期时刻
    admission : Admission;          -- 准入方式

  -- 起草一张单据所需的提议：负责人、目标与首个版本
  Proposal ::
    id          : TicketId        -- 身份
    responsible : Principal       -- 负责人
    source      : SourceId        -- 目标来源
    cap         : CapabilityKey   -- 放行前核对的能力
    kind        : EffectKind      -- 效应种类
    first       : Version;        -- 首个版本

  -- 一张单据：一把锁、一条线性版本链、绑定版本的裁决、它接手的请求（若有）
  TicketRecord ::
    id          : TicketId              -- 身份
    responsible : Principal             -- 当前持有编辑权的负责人
    source      : SourceId              -- 目标来源
    cap         : CapabilityKey         -- 放行前核对的能力
    kind        : EffectKind            -- 效应种类
    versions    : seq1 of Version       -- 版本链，下标即版本号
    decisions   : map nat1 to Decision  -- 各版本的裁决
    request     : [RequestRef]          -- 接手的请求
    status      : TicketStatus;         -- 当前状态

  -- 放行前的对账结果：未对齐的必要项；空集即全部对齐
  Alignment = set of CheckKey;

  -- 放行门的判定：已关闭、尚未批准、确认了一种不可逆事实、门未开、门已开
  GateVerdict = <AlreadyClosed> | <NotApproved> | Lapse | <Hold> | <Open>;

  -- 裁决结局：已记录，或期望版本不符与同版本已决时的冲突
  DecideOutcome = <Recorded> | <Conflict>;

  -- 放行结局：已放行的尝试、门未开而等待、或单据已关闭
  ReleaseOutcome = AttemptRef | <Waiting> | <Closed>;

  -- 单据表：裁决只落在已有版本上，送审与获批冻结的是最新版本，获批的版本确有批准裁决
state Tickets of
  byId : map TicketId to TicketRecord  -- 全部单据
  inv mk_Tickets(b) ==
        forall id in set dom b &
          b(id).id = id
          and dom b(id).decisions subset inds b(id).versions
          and (is_Awaiting(b(id).status) => b(id).status.version = len b(id).versions)
          and (is_Approved(b(id).status) =>
                 b(id).status.version = len b(id).versions
                 and b(id).status.version in set dom b(id).decisions
                 and b(id).decisions(b(id).status.version).ruling = <Approved>)
  init t == t = mk_Tickets({|->})
end

functions
  -- 判定：单据的当前版本
  Current: TicketRecord -> Version
  Current(rec) == rec.versions(len rec.versions);

  -- 判定：一张单据此刻的状态记成一条记事；两个实现必须写出同一条
  Progress: TicketRecord -> Note
  Progress(rec) == mk_token(mk_(rec.id, rec.status));

  -- 判定：放行门。只对已批准的单据开门或关单；到期、来源明确不支持、依据不可逆失效各自关单；对账项未对齐或能力未确认时等待
  Gate: TicketRecord * bool * Alignment * Time * SessionVerdict -> GateVerdict
  Gate(rec, fresh, align, now, cap) ==
    if is_Closed(rec.status) then <AlreadyClosed>
    elseif not is_Approved(rec.status) then <NotApproved>
    elseif now >= Current(rec).deadline then <Expired>
    elseif cap = <Unsupported> then <Unsupported>
    elseif not fresh then <BasisLost>
    elseif align = {} and cap = <Supported> then <Open>
    else <Hold>;

  -- 判定：单据当前版本对应的写意图，带它接手的请求
  ToIntent: TicketRecord -> Intent
  ToIntent(rec) ==
    let v = Current(rec) in
      mk_Intent(rec.source, rec.cap, v.intent, v.deadline, v.admission, rec.request);

operations
  -- 起草新单据：负责人取锁，只有提议里的第一个版本，尚无裁决
  Draft(p: Proposal)
  ext wr byId
  pre p.id not in set dom byId
  post byId = byId~ munion
         {p.id |-> mk_TicketRecord(p.id, p.responsible, p.source, p.cap, p.kind,
                                   [p.first], {|->}, nil, <Drafting>)};

  -- 接手一条请求起草单据：与起草相同，另记下所接手的请求
  DraftFor(p: Proposal, req: RequestRef)
  ext wr byId
  pre p.id not in set dom byId
  post byId = byId~ munion
         {p.id |-> mk_TicketRecord(p.id, p.responsible, p.source, p.cap, p.kind,
                                   [p.first], {|->}, req, <Drafting>)};

  -- 负责人在草稿中追加新版本
  Revise(id: TicketId, who: Principal, next: Version)
  ext wr byId
  pre id in set dom byId and byId(id).responsible = who and byId(id).status = <Drafting>
  post byId = byId~ ++ {id |-> mu(byId~(id), versions |-> byId~(id).versions ^ [next])};

  -- 负责人把草稿送审：冻结在最新版本；已有裁决的版本必须先修订才能再送
  SubmitForDecision(id: TicketId, who: Principal)
  ext wr byId
  pre id in set dom byId and byId(id).responsible = who and byId(id).status = <Drafting>
      and len byId(id).versions not in set dom byId(id).decisions
  post byId = byId~ ++ {id |-> mu(byId~(id), status |-> mk_Awaiting(len byId~(id).versions))};

  -- 裁决者把送审中的单据退回草稿；已批准的单据不能退回；谁有权退回由业务决定
  SendBack(id: TicketId)
  ext wr byId
  pre id in set dom byId and is_Awaiting(byId(id).status)
  post byId = byId~ ++ {id |-> mu(byId~(id), status |-> <Drafting>)};

  -- 负责人把未关闭的单据移交给另一个主体
  Transfer(id: TicketId, src: Principal, dst: Principal)
  ext wr byId
  pre id in set dom byId and byId(id).responsible = src and not is_Closed(byId(id).status)
  post byId = byId~ ++ {id |-> mu(byId~(id), responsible |-> dst)};

  -- 对送审中的指定版本作出裁决；期望版本不符或该版本已决时返回冲突且不改状态；批准进入已批准，否决即关闭
  Decide(id: TicketId, version: nat1, who: Principal, ruling: Ruling) r: DecideOutcome
  ext wr byId
  pre id in set dom byId and is_Awaiting(byId(id).status)
  post let ok = byId~(id).status.version = version
                and version not in set dom byId~(id).decisions in
         (ok and r = <Recorded>
          and byId = byId~ ++
                {id |-> mu(byId~(id),
                           decisions |-> byId~(id).decisions munion {version |-> mk_Decision(who, ruling)},
                           status |-> if ruling = <Rejected>
                                      then mk_Closed(<DecisionRejected>)
                                      else mk_Approved(version))})
         or (not ok and r = <Conflict> and byId = byId~);

  -- 负责人撤回未关闭的单据
  Withdraw(id: TicketId, who: Principal)
  ext wr byId
  pre id in set dom byId and byId(id).responsible = who and not is_Closed(byId(id).status)
  post byId = byId~ ++ {id |-> mu(byId~(id), status |-> mk_Closed(<Withdrawn>))};

  -- 以给定原因关闭未关闭的单据
  Close(id: TicketId, reason: ClosedReason)
  ext wr byId
  pre id in set dom byId and not is_Closed(byId(id).status)
  post byId = byId~ ++ {id |-> mu(byId~(id), status |-> mk_Closed(reason))};

  -- 场景：以某原因关单并记事
  CloseAs: TicketId * ClosedReason ==> ()
  CloseAs(id, reason) ==
    (Close(id, reason);
     Record(byId(id).request, Progress(byId(id))))
  pre id in set dom byId and not is_Closed(byId(id).status);

  -- 场景：放行门。已批准的单据到期、来源明确不支持或依据失效即关单；门未开或通道不可准入即等待；准入成功则以准备记录关单
  Release: TicketId * Alignment * Time ==> ReleaseOutcome
  Release(id, align, now) ==
    cases Gate(byId(id), CheckBasis(Current(byId(id)).basis), align, now,
               SessionCapability(byId(id).source, byId(id).cap)):
      <AlreadyClosed> -> return <Closed>,
      <NotApproved> -> return <Waiting>,
      <Expired> -> (CloseAs(id, <Expired>); return <Closed>),
      <Unsupported> -> (CloseAs(id, <Unsupported>); return <Closed>),
      <BasisLost> -> (CloseAs(id, <BasisLost>); return <Closed>),
      <Hold> -> return <Waiting>,
      others ->
        cases Prepare(ToIntent(byId(id))):
          <Unmapped> -> return <Waiting>,
          <LaneBlocked> -> return <Waiting>,
          ref -> (CloseAs(id, mk_Prepared(ref)); return ref)
        end
    end
  pre id in set dom byId;

  -- 场景：写处理器接手一条等待中的请求。以请求发出者为负责人、请求依据为依据起草并送审，答复以单据身份回填；请求不存在、不在等待中或种类不符则不接手
  AdoptRequest: RequestRef * Proposal ==> bool
  AdoptRequest(ref, p) ==
    let req = RequestOf(ref) in
      if req = nil or StatusOf(ref) <> <Pending> or req.kind <> p.kind
      then return false
      else (DraftFor(mu(p, responsible |-> req.member,
                          first |-> mu(p.first, basis |-> req.basis)), ref);
            SubmitForDecision(p.id, req.member);
            Respond(ref, mk_token(p.id));
            return true)
  pre p.id not in set dom byId;

  -- 查询单据；不存在为空
  RecordOf(id: TicketId) r: [TicketRecord]
  ext rd byId
  post (id in set dom byId and r = byId(id))
       or (id not in set dom byId and r = nil);
end Ticket
```
