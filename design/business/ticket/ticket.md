# 单据与审批接入

```vdm
module Ticket
imports
  from Source
    types
      SourceId renamed SourceId;
      CapabilityKey renamed CapabilityKey
    operations
      CurrentCapability renamed CurrentCapability,
  from Observation
    types
      LogPosition renamed LogPosition
    operations
      AcquireRef renamed AcquireRef;
      ReleaseRef renamed ReleaseRef;
      CheckBasis renamed CheckBasis,
  from Effect
    types
      LaneKey renamed LaneKey;
      AttemptId renamed AttemptId;
      IntentPayload renamed IntentPayload;
      Attempt renamed Attempt
    operations
      Prepare renamed Prepare
exports
  types
    TicketId;
    VersionId;
    Principal;
    struct DecisionVerdict;
    struct TicketStatus;
    struct DecisionOutcome;
    struct ReleaseOutcome;
    struct TicketRecord
  operations
    Draft: TicketRecord ==> ();
    Revise: TicketRecord ==> <Applied> | <Rejected>;
    Decide: TicketId * VersionId * DecisionVerdict ==> DecisionOutcome;
    CloseTicket: TicketId ==> ();
    Release: TicketId * Attempt * SourceId * CapabilityKey ==> ReleaseOutcome
definitions
types
  -- 一张单据的身份
  TicketId = token;

  -- 单据线性修订链上的版本身份
  VersionId = token;

  -- 发起或审批单据的责任主体身份
  Principal = token;

  -- 审批裁决的结论：批准或否决
  DecisionVerdict = <Approved> | <Rejected>;

  -- 单据的存续状态：待决可编辑或已关闭不可再改
  TicketStatus = <Open> | <Closed>;

  -- 提交审批决定的结局：生效，或因期望版本不匹配/同版本已决而冲突
  DecisionOutcome = <Applied> | <Conflict>;

  -- 单据放行进入写边界的结局：成功交出得到 AttemptId、等待能力或通道、或因依据失效/未获批而拒绝
  ReleaseOutcome = AttemptId | <Waiting> | <Rejected>;

  -- 单据的当前状态记录：持有负责人、当前版本、观察依据与绑定在该版本上的裁决
  TicketRecord ::
    id          : TicketId                   -- 单据身份
    responsible : Principal                  -- 当前持有编辑权的负责人
    lane        : LaneKey                    -- 目标写通道
    version     : VersionId                  -- 当前最新版本身份
    intent      : IntentPayload              -- 当前版本的意图载荷
    basis       : set of LogPosition         -- 当前版本引用的观察位置集合
    decision    : [DecisionVerdict]          -- 绑定在当前版本上的审批裁决
    status      : TicketStatus;              -- 单据是否仍处于开放状态

  -- 待决与已关闭的单据表
state Tickets of
  byId : map TicketId to TicketRecord  -- 单据身份到单据记录的映射
  init t == t = mk_Tickets({|->})
end

operations
  -- 创建单据草稿并由发起人持有责任锁；若单据身份已存在则保持原记录不变
  Draft(draft: TicketRecord)
  ext wr byId
  post (draft.id not in set dom byId~
        and draft.decision = nil
        and draft.status = <Open>
        and byId = byId~ ++ {draft.id |-> draft})
       or ((draft.id in set dom byId~ or draft.decision <> nil or draft.status <> <Open>)
           and byId = byId~);

  -- 负责人修订草稿：推进版本并清空旧版本上的审批裁决；非负责人或已关闭单据拒绝修订
  Revise(nextRec: TicketRecord) r: <Applied> | <Rejected>
  ext wr byId
  post (exists prev in set rng byId~ &
          prev.id = nextRec.id
          and prev.status = <Open>
          and prev.responsible = nextRec.responsible
          and prev.lane = nextRec.lane
          and nextRec.version <> prev.version
          and nextRec.decision = nil
          and nextRec.status = <Open>
          and r = <Applied>
          and byId = byId~ ++ {nextRec.id |-> nextRec})
       or ((not exists prev in set rng byId~ &
              prev.id = nextRec.id
              and prev.status = <Open>
              and prev.responsible = nextRec.responsible
              and prev.lane = nextRec.lane
              and nextRec.version <> prev.version
              and nextRec.decision = nil
              and nextRec.status = <Open>)
           and r = <Rejected>
           and byId = byId~);

  -- 对单据的指定版本作出批准或否决；期望版本不符或该版本已有决定时返回冲突且不改状态
  Decide(id: TicketId, expectedVersion: VersionId, verdict: DecisionVerdict) r: DecisionOutcome
  ext wr byId
  post (exists rec in set rng byId~ &
          rec.id = id
          and rec.status = <Open>
          and rec.version = expectedVersion
          and rec.decision = nil
          and r = <Applied>
          and byId = byId~ ++ {id |-> mu(rec,
                decision |-> verdict,
                status |-> if verdict = <Rejected> then <Closed> else <Open>)})
       or ((not exists rec in set rng byId~ &
              rec.id = id
              and rec.status = <Open>
              and rec.version = expectedVersion
              and rec.decision = nil)
           and r = <Conflict>
           and byId = byId~);

  -- 查询一张已获批准且仍处于开放状态的单据记录
  ApprovedOpenTicket(id: TicketId) rec: [TicketRecord]
  ext rd byId
  post (exists found in set rng byId &
          found.id = id
          and found.status = <Open>
          and found.decision = <Approved>
          and rec = found)
       or ((not exists found in set rng byId &
              found.id = id and found.status = <Open> and found.decision = <Approved>)
           and rec = nil);

  -- 将已放行或因依据失效而终止的单据标记为关闭，关闭后不可再次修订或放行
  CloseTicket(id: TicketId)
  ext wr byId
  post (exists rec in set rng byId~ &
          rec.id = id
          and byId = byId~ ++ {id |-> mu(rec, status |-> <Closed>)})
       or ((not exists rec in set rng byId~ & rec.id = id)
           and byId = byId~);

  -- 场景：在单据当前版本已获批准、观察依据新鲜且来源能力支持时，登记观察引用、交入写边界并关闭单据
  Release: TicketId * Attempt * SourceId * CapabilityKey ==> ReleaseOutcome
  Release(id, attemptEntry, source, cap) ==
    let rec = ApprovedOpenTicket(id) in
      if rec = nil
      then return <Rejected>
      else cases CheckBasis(rec.basis):
             <Fresh> ->
               cases CurrentCapability(source, cap):
                 <Unavailable>, <Unknown> -> return <Waiting>,
                 <Supported> ->
                   cases AcquireRef(attemptEntry.holder, rec.basis):
                     <Applied> ->
                       cases Prepare(attemptEntry, nil):
                         <Blocked> ->
                           (ReleaseRef(attemptEntry.holder);
                            return <Waiting>),
                         others ->
                           (CloseTicket(id);
                            return attemptEntry.id)
                       end,
                     others ->
                       (CloseTicket(id);
                        return <Rejected>)
                   end,
                 others ->
                   (CloseTicket(id);
                    return <Rejected>)
               end,
             others ->
               (CloseTicket(id);
                return <Rejected>)
           end;
end Ticket
```
