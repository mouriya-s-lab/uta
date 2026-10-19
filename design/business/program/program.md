# 程序运行接入

```vdm
module Program
imports
  from Observation
    types
      StreamId renamed StreamId;
      ConsumerId renamed ConsumerId;
      RefHolder renamed RefHolder;
      ObsPayload renamed ObsPayload;
      Seq renamed Seq;
      LogPosition renamed LogPosition
    operations
      Append renamed Append;
      AcquireRef renamed AcquireRef;
      Acknowledge renamed Acknowledge,
  from Ticket
    types
      TicketId renamed TicketId;
      Principal renamed Principal
    operations
      Draft renamed Draft,
  from Effect
    types
      LaneKey renamed LaneKey;
      IntentPayload renamed IntentPayload
exports
  types
    ProgramId;
    struct Checkpoint
  operations
    Advance: ProgramId * ConsumerId * Principal * StreamId * ObsPayload * map StreamId to Seq * LaneKey * IntentPayload * set of LogPosition ==> TicketId
definitions
types
  -- 一个已装载程序的身份
  ProgramId = token;

  -- 程序的持久检查点：与已消费的输入流游标绑定
  Checkpoint ::
    holder  : RefHolder            -- 在观察日志中登记游标引用的持有者身份
    cursors : map StreamId to Seq; -- 各输入流已推进到的序号

  -- 活动程序的检查点表
state Programs of
  checkpoints : map ProgramId to Checkpoint  -- 程序身份到最近已提交检查点的映射
  init p == p = mk_Programs({|->})
end

operations
  -- 场景：程序完成一轮确定性推进——更新检查点与输入游标引用、追加派生观察、并以装载人身份起单
  Advance: ProgramId * ConsumerId * Principal * StreamId * ObsPayload * map StreamId to Seq * LaneKey * IntentPayload * set of LogPosition ==> TicketId
  Advance(prog, consumer, owner, outputStream, derivedPayload, newCursors, lane, intent, basis) ==
    is not yet specified;
end Program
```
