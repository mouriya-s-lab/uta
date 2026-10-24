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
      Note renamed Note;
      EffectRequest renamed EffectRequest;
      RequestStatus renamed RequestStatus
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
    MatchesRequest: Proposal * [EffectRequest] * [RequestStatus] -> bool;
    Gate: TicketRecord * bool * Alignment * Time * SessionVerdict -> GateVerdict;
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

  -- 一张单据此刻的状态记成的一条记事；记事的编码属实现，不展开
  Progress: TicketRecord -> Note
  Progress(rec) == is not yet specified;

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

  -- 判定：一张单据提议是否如实承接了一条等待处理的请求——请求存在且等待处理，负责人、种类与依据同请求一致
  MatchesRequest: Proposal * [EffectRequest] * [RequestStatus] -> bool
  MatchesRequest(p, req, status) ==
    req <> nil
    and status = <Pending>
    and p.responsible = req.member
    and p.kind = req.kind
    and p.first.basis = req.basis;

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

  -- 对送审中的指定版本记下裁决；期望版本不符或该版本已决时返回冲突且不改状态；批准进入已批准，否决即关闭
  RecordDecision(id: TicketId, version: nat1, who: Principal, ruling: Ruling) r: DecideOutcome
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

  -- 以给定原因关闭未关闭的单据
  Close(id: TicketId, reason: ClosedReason)
  ext wr byId
  pre id in set dom byId and not is_Closed(byId(id).status)
  post byId = byId~ ++ {id |-> mu(byId~(id), status |-> mk_Closed(reason))};

  -- 场景：对送审中的指定版本作出裁决；裁决记下后让它所接手的请求方得知
  Decide: TicketId * nat1 * Principal * Ruling ==> DecideOutcome
  Decide(id, version, who, ruling) ==
    cases RecordDecision(id, version, who, ruling):
      <Conflict> -> return <Conflict>,
      others -> (Record(byId(id).request, Progress(byId(id))); return <Recorded>)
    end
  pre id in set dom byId and is_Awaiting(byId(id).status);

  -- 场景：负责人撤回未关闭的单据；关单后让它所接手的请求方得知
  Withdraw: TicketId * Principal ==> ()
  Withdraw(id, who) ==
    (Close(id, <Withdrawn>);
     Record(byId(id).request, Progress(byId(id))))
  pre id in set dom byId and byId(id).responsible = who and not is_Closed(byId(id).status);

  -- 场景：放行门。已批准的单据到期、来源明确不支持或依据失效即关单；门未开或通道不可准入即等待；准入成功则以准备记录关单；每次关单后让它所接手的请求方得知
  Release: TicketId * Alignment * Time ==> ReleaseOutcome
  Release(id, align, now) ==
    cases Gate(byId(id), CheckBasis(Current(byId(id)).basis), align, now,
               SessionCapability(byId(id).source, byId(id).cap)):
      <AlreadyClosed> -> return <Closed>,
      <NotApproved> -> return <Waiting>,
      <Expired> -> (Close(id, <Expired>); Record(byId(id).request, Progress(byId(id))); return <Closed>),
      <Unsupported> -> (Close(id, <Unsupported>); Record(byId(id).request, Progress(byId(id))); return <Closed>),
      <BasisLost> -> (Close(id, <BasisLost>); Record(byId(id).request, Progress(byId(id))); return <Closed>),
      <Hold> -> return <Waiting>,
      others ->
        cases Prepare(mk_Intent(byId(id).source, byId(id).cap, Current(byId(id)).intent,
                                Current(byId(id)).deadline, Current(byId(id)).admission,
                                byId(id).request)):
          <Unmapped> -> return <Waiting>,
          <LaneBlocked> -> return <Waiting>,
          ref -> (Close(id, mk_Prepared(ref)); Record(byId(id).request, Progress(byId(id))); return ref)
        end
    end
  pre id in set dom byId;

  -- 场景：写处理器接手一条等待中的请求——提议经 MatchesRequest 确认如实承接请求时，经 DraftFor 起草、经 SubmitForDecision 送审、再经 Respond 以单据身份答复请求
  AdoptRequest: RequestRef * Proposal ==> bool
  AdoptRequest(ref, p) ==
    if MatchesRequest(p, RequestOf(ref), StatusOf(ref))
    then (DraftFor(p, ref);
          SubmitForDecision(p.id, p.responsible);
          Respond(ref, mk_token(p.id));
          return true)
    else return false
  pre p.id not in set dom byId;

  -- 查询单据；不存在为空
  RecordOf(id: TicketId) r: [TicketRecord]
  ext rd byId
  post (id in set dom byId and r = byId(id))
       or (id not in set dom byId and r = nil);
end Ticket
```
