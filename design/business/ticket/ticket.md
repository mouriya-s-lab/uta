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
      Principal renamed Principal,
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
    struct GateVerdict;
    struct DecideOutcome;
    struct ReleaseOutcome
  functions
    Current: TicketRecord -> Version;
    Gate: TicketRecord * bool * bool * Time * SessionVerdict -> GateVerdict;
    ToIntent: TicketRecord -> Intent
  operations
    Draft: Proposal ==> ();
    Revise: TicketId * Principal * Version ==> ();
    SubmitForDecision: TicketId * Principal ==> ();
    SendBack: TicketId ==> ();
    Decide: TicketId * nat1 * Principal * Ruling ==> DecideOutcome;
    Release: TicketId * bool * Time ==> ReleaseOutcome
definitions
types
  -- 单据身份；业务造出，使用方不读结构
  TicketId = token;

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

  -- 关闭原因：已放行、裁决否决、或放行时已不可能再放行（到期、来源明确不支持、依据失效）
  ClosedReason = Prepared | <Rejected> | <Lapsed>;

  -- 已关闭
  Closed ::
    reason : ClosedReason;  -- 为何关闭

  -- 单据状态：草稿中、送审中、已批准、已关闭
  TicketStatus = <Drafting> | Awaiting | Approved | Closed;

  -- 单据的一个版本：裁决所绑定的全部内容；业务的条件写在意图载荷里，随版本一起受审
  Version ::
    intent    : IntentPayload       -- 意图
    basis     : set of LogPosition  -- 决策依据的观察位置
    deadline  : Time                -- 过期时刻
    admission : Admission;          -- 准入方式

  -- 起草一张单据所需的提议：负责人、目标、所响应的请求与首个版本
  Proposal ::
    id          : TicketId        -- 身份
    responsible : Principal       -- 负责人
    source      : SourceId        -- 目标来源
    cap         : CapabilityKey   -- 放行前核对的能力
    about       : [RequestRef]    -- 所响应的请求，交给写边界关联进展
    first       : Version;        -- 首个版本

  -- 一张单据：一把锁、一条线性版本链、绑定版本的裁决
  TicketRecord ::
    id          : TicketId              -- 身份
    responsible : Principal             -- 持有编辑权的负责人
    source      : SourceId              -- 目标来源
    cap         : CapabilityKey         -- 放行前核对的能力
    about       : [RequestRef]          -- 所响应的请求
    versions    : seq1 of Version       -- 版本链，下标即版本号
    decisions   : map nat1 to Decision  -- 各版本的裁决
    status      : TicketStatus;         -- 当前状态

  -- 放行门的判定：门已开、暂不能开、已不可能再开
  GateVerdict = <Open> | <Hold> | <Lapsed>;

  -- 裁决结局：已记录，或期望版本不符与同版本已决时的冲突
  DecideOutcome = <Recorded> | <Conflict>;

  -- 放行结局：已放行的尝试、暂不能放行、或已不可能再放行而关单
  ReleaseOutcome = AttemptRef | <Waiting> | <Lapsed>;

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

  -- 判定：放行门。到期、来源明确不支持或依据失效时已不可能再放行；对齐且来源此刻支持时开门；其余暂不能开
  Gate: TicketRecord * bool * bool * Time * SessionVerdict -> GateVerdict
  Gate(rec, fresh, aligned, now, cap) ==
    if now >= Current(rec).deadline or cap = <Unsupported> or not fresh then <Lapsed>
    elseif aligned and cap = <Supported> then <Open>
    else <Hold>;

  -- 判定：单据当前版本对应的写意图
  ToIntent: TicketRecord -> Intent
  ToIntent(rec) ==
    let v = Current(rec) in
      mk_Intent(rec.source, rec.cap, v.intent, v.deadline, v.admission, rec.about);

operations
  -- 起草新单据：负责人取锁，只有提议里的第一个版本，尚无裁决
  Draft(p: Proposal)
  ext wr byId
  pre p.id not in set dom byId
  post byId = byId~ munion
         {p.id |-> mk_TicketRecord(p.id, p.responsible, p.source, p.cap, p.about,
                                   [p.first], {|->}, <Drafting>)};

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

  -- 把送审中的单据退回草稿；已批准的单据不能退回
  SendBack(id: TicketId)
  ext wr byId
  pre id in set dom byId and is_Awaiting(byId(id).status)
  post byId = byId~ ++ {id |-> mu(byId~(id), status |-> <Drafting>)};

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
                                      then mk_Closed(<Rejected>)
                                      else mk_Approved(version))})
         or (not ok and r = <Conflict> and byId = byId~);

  -- 以给定原因关闭已批准的单据
  Close(id: TicketId, reason: ClosedReason)
  ext wr byId
  pre id in set dom byId and is_Approved(byId(id).status)
  post byId = byId~ ++ {id |-> mu(byId~(id), status |-> mk_Closed(reason))};

  -- 场景：放行已批准的单据。门已不可能再开即关单；门暂不能开、或写边界编不出与通道阻塞时等待；写尝试进入写边界即以它关单
  Release: TicketId * bool * Time ==> ReleaseOutcome
  Release(id, aligned, now) ==
    cases Gate(byId(id), CheckBasis(Current(byId(id)).basis), aligned, now,
               SessionCapability(byId(id).source, byId(id).cap)):
      <Lapsed> -> (Close(id, <Lapsed>); return <Lapsed>),
      <Hold> -> return <Waiting>,
      others ->
        cases Prepare(ToIntent(byId(id))):
          <Unmapped> -> return <Waiting>,
          <LaneBlocked> -> return <Waiting>,
          ref -> (Close(id, mk_Prepared(ref)); return ref)
        end
    end
  pre id in set dom byId and is_Approved(byId(id).status);
end Ticket
```
