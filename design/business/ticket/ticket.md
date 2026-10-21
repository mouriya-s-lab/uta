# 单据连接

```vdm
module Ticket
imports
  from Session
    types
      Principal renamed Principal,
  from Source
    types
      SourceId renamed SourceId;
      CapabilityKey renamed CapabilityKey,
  from Mapping
    types
      LaneKey renamed LaneKey;
      IntentPayload renamed IntentPayload,
  from Observation
    types
      LogPosition renamed LogPosition;
      BasisValidity renamed BasisValidity
    operations
      CheckBasis renamed CheckBasis,
  from Request
    types
      EffectKind renamed EffectKind,
  from Write
    types
      AttemptRef renamed AttemptRef;
      Time renamed Time;
      Intent renamed Intent;
      Admission renamed Admission;
      PrepareOutcome renamed PrepareOutcome
    operations
      Prepare renamed Prepare;
      LaneClear renamed LaneClear
exports
  types
    TicketId;
    CheckKey;
    struct Verdict;
    struct Decision;
    struct Awaiting;
    struct Prepared;
    struct ClosedReason;
    struct Closed;
    struct Status;
    struct TicketRecord;
    struct Alignment;
    struct DecideOutcome;
    struct ReleaseOutcome
  operations
    Draft: TicketRecord ==> ();
    Revise: TicketId * Principal * IntentPayload ==> bool;
    SubmitForDecision: TicketId * Principal ==> bool;
    SendBack: TicketId * Principal ==> bool;
    Transfer: TicketId * Principal * Principal ==> bool;
    Decide: TicketId * nat1 * Principal * Verdict ==> DecideOutcome;
    Withdraw: TicketId * Principal ==> bool;
    Close: TicketId * ClosedReason ==> ();
    Release: TicketId * Alignment * Time * nat ==> ReleaseOutcome
  functions
    GateOpen: TicketRecord * BasisValidity * Alignment * Time -> bool
definitions
types
  -- 单据身份；业务造出，使用方不读结构
  TicketId = token;

  -- 放行前一项对账检查的身份；由业务选定必要项
  CheckKey = token;

  -- 审批裁决：批准或否决
  Verdict = <Approved> | <Rejected>;

  -- 一条绑定版本的裁决记录
  Decision ::
    decider : Principal  -- 谁作出
    verdict : Verdict;   -- 结论

  -- 送审中：冻结在某个版本
  Awaiting :: version : nat1;

  -- 已放行：对应的写尝试
  Prepared :: attempt : AttemptRef;

  -- 关闭原因
  ClosedReason = Prepared | <Withdrawn> | <DecisionRejected> | <Expired>;

  -- 已关闭
  Closed :: reason : ClosedReason;

  -- 单据状态
  Status = <Drafting> | Awaiting | Closed;

  -- 一张单据：一把锁、一条线性版本链、绑定版本的裁决
  TicketRecord ::
    id          : TicketId                -- 身份
    responsible : Principal               -- 当前持有编辑权的负责人
    source      : SourceId                -- 目标来源
    cap         : CapabilityKey           -- 放行前核对的能力
    kind        : EffectKind              -- 效应种类
    lane        : LaneKey                 -- 目标写通道
    versions    : seq1 of IntentPayload   -- 版本链，下标即版本号
    decisions   : map nat1 to Decision    -- 各版本的裁决
    basis       : set of LogPosition      -- 决策依据的观察位置
    deadline    : Time                    -- 过期时刻
    status      : Status;                 -- 当前状态

  -- 放行前的对账结果：各必要项是否与当前观察对齐
  Alignment = map CheckKey to (<Aligned> | <Diverged> | <Undecidable>);

  -- 裁决结局：已记录，或期望版本不符与同版本已决时的冲突
  DecideOutcome = <Recorded> | <Conflict>;

  -- 放行结局：已放行、门未开而等待、或已关闭
  ReleaseOutcome = AttemptRef | <Waiting> | <Closed>;

  -- 单据表
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
  -- 放行门：当前版本已批准、依据新鲜、必要项全部对齐、未过期
  GateOpen: TicketRecord * BasisValidity * Alignment * Time -> bool
  GateOpen(rec, validity, align, now) == is not yet specified;

operations
  Draft: TicketRecord ==> ()
  Draft(rec) == is not yet specified;

  Revise: TicketId * Principal * IntentPayload ==> bool
  Revise(id, who, next) == is not yet specified;

  SubmitForDecision: TicketId * Principal ==> bool
  SubmitForDecision(id, who) == is not yet specified;

  SendBack: TicketId * Principal ==> bool
  SendBack(id, who) == is not yet specified;

  Transfer: TicketId * Principal * Principal ==> bool
  Transfer(id, src, dst) == is not yet specified;

  Decide: TicketId * nat1 * Principal * Verdict ==> DecideOutcome
  Decide(id, version, who, verdict) == is not yet specified;

  Withdraw: TicketId * Principal ==> bool
  Withdraw(id, who) == is not yet specified;

  Close: TicketId * ClosedReason ==> ()
  Close(id, reason) == is not yet specified;

  Release: TicketId * Alignment * Time * nat ==> ReleaseOutcome
  Release(id, align, now, lag) == is not yet specified;
end Ticket
```
