# 程序运行接入

```vdm
module Program
imports
  from Observation
    types
      StreamId renamed StreamId;
      RefHolder renamed RefHolder;
      ObsPayload renamed ObsPayload;
      Seq renamed Seq;
      LogPosition renamed LogPosition
    operations
      Append renamed Append;
      AcquireRef renamed AcquireRef,
  from Ticket
    types
      TicketRecord renamed TicketRecord
    operations
      Draft renamed Draft
exports
  types
    ProgramId;
    struct Checkpoint
  operations
    SaveCheckpoint: ProgramId * Checkpoint ==> ();
    Advance: ProgramId * Checkpoint * StreamId * Seq * ObsPayload * TicketRecord ==> LogPosition
definitions
types
  -- 一个已装载程序的身份
  ProgramId = token;

  -- 程序的持久检查点：记录已确认消费的输入流位置与对应的引用持有者
  Checkpoint ::
    holder    : RefHolder      -- 在观察日志中登记游标引用的持有者身份
    inStream  : StreamId       -- 所消费的输入观察流
    cursorSeq : Seq;           -- 已消费到的输入流序号

  -- 活动程序的检查点表
state Programs of
  checkpoints : map ProgramId to Checkpoint  -- 程序身份到最近已提交检查点的映射
  init p == p = mk_Programs({|->})
end

operations
  -- 更新程序的持久检查点记录
  SaveCheckpoint(prog: ProgramId, cp: Checkpoint)
  ext wr checkpoints
  post checkpoints = checkpoints~ ++ {prog |-> cp};

  -- 场景：程序完成一轮确定性推进——登记新输入位置保护引用、保存检查点、提交待审单据并追加派生观察
  Advance: ProgramId * Checkpoint * StreamId * Seq * ObsPayload * TicketRecord ==> LogPosition
  Advance(prog, cp, outStream, outSeq, derivedPayload, intentDraft) ==
    let cursorPos = mk_LogPosition(cp.inStream, cp.cursorSeq) in
      (let - = AcquireRef(cp.holder, {cursorPos}) in skip;
       SaveCheckpoint(prog, cp);
       Draft(intentDraft);
       return Append(outStream, outSeq, <Data>, derivedPayload, nil));
end Program
```
