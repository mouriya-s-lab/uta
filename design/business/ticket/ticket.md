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
      LogPosition renamed LogPosition;
      BasisStatus renamed BasisStatus
    functions
      BasisValid renamed BasisValid,
  from Effect
    types
      LaneKey renamed LaneKey;
      AttemptId renamed AttemptId;
      IntentPayload renamed IntentPayload;
      PrepareOutcome renamed PrepareOutcome
    operations
      Prepare renamed Prepare
exports
  types
    TicketId;
    VersionId;
    Principal;
    struct DecisionVerdict;
    struct DecisionOutcome;
    struct ReleaseOutcome;
    struct TicketRecord
  operations
    Draft: Principal * LaneKey * IntentPayload * set of LogPosition ==> TicketId;
    Revise: TicketId * Principal * IntentPayload * set of LogPosition ==> <Applied> | <Rejected>;
    Decide: TicketId * VersionId * DecisionVerdict ==> DecisionOutcome;
    Release: TicketId * SourceId * CapabilityKey ==> ReleaseOutcome
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

  -- 提交审批决定的结局：生效，或因期望版本不匹配/同版本已决而冲突
  DecisionOutcome = <Applied> | <Conflict>;

  -- 单据放行进入写边界的结局：成功交出得到 AttemptId、等待能力/通道、或因依据失效/否决而关闭
  ReleaseOutcome = AttemptId | <Waiting> | <Rejected>;

  -- 单据的当前状态记录
  TicketRecord ::
    responsible : Principal                        -- 当前持有编辑权的负责人
    lane        : LaneKey                          -- 目标写通道
    version     : VersionId                        -- 当前最新版本身份
    intent      : IntentPayload                    -- 当前版本的意图载荷
    basis       : set of LogPosition               -- 当前版本引用的观察位置集合
    decision    : [DecisionVerdict]                -- 绑定在当前版本上的审批裁决
    closed      : bool;                            -- 是否已关闭（已交出或已否决）

  -- 待决与已关闭的单据表
state Tickets of
  byId : map TicketId to TicketRecord  -- 单据身份到单据记录的映射
  init t == t = mk_Tickets({|->})
end

operations
  -- 创建单据草稿并由发起人持有责任锁
  Draft: Principal * LaneKey * IntentPayload * set of LogPosition ==> TicketId
  Draft(actor, lane, intent, basis) ==
    is not yet specified;

  -- 负责人修订草稿：生成新版本并使此前版本上的审批决定自动失效
  Revise: TicketId * Principal * IntentPayload * set of LogPosition ==> <Applied> | <Rejected>
  Revise(id, actor, nextIntent, nextBasis) ==
    is not yet specified;

  -- 对单据的指定版本作出批准或否决；版本不符或同版本重复决定返回冲突
  Decide: TicketId * VersionId * DecisionVerdict ==> DecisionOutcome
  Decide(id, expectedVersion, verdict) ==
    is not yet specified;

  -- 在当前版本获批、观察依据新鲜且来源能力支持时，原子关闭单据并交入 Effect`Prepare
  Release: TicketId * SourceId * CapabilityKey ==> ReleaseOutcome
  Release(id, source, cap) ==
    is not yet specified;
end Ticket
```
