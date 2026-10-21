# 程序连接

```vdm
module Program
imports
  from Session
    types
      Principal renamed Principal,
  from Observation
    types
      DerivedId renamed DerivedId;
      RefHolder renamed RefHolder;
      StreamId renamed StreamId;
      LogPosition renamed LogPosition;
      RefOutcome renamed RefOutcome
    operations
      Append renamed Append;
      AcquireRef renamed AcquireRef;
      ReleaseRef renamed ReleaseRef,
  from Delivery
    types
      SubscriptionId renamed SubscriptionId;
      Item renamed Item
    operations
      Subscribe renamed Subscribe;
      Unsubscribe renamed Unsubscribe;
      Deliver renamed Deliver;
      Ack renamed Ack,
  from Request
    types
      RequestRef renamed RequestRef;
      EffectRequest renamed EffectRequest
    operations
      Emit renamed Emit
exports
  types
    ProgramId;
    State;
    struct Checkpoint;
    struct Step
  operations
    Load: ProgramId * Principal * set of StreamId * StreamId ==> ();
    Unload: ProgramId ==> ();
    SaveCheckpoint: ProgramId * Checkpoint ==> RefOutcome;
    Advance: ProgramId * StreamId * Step ==> seq of RequestRef
definitions
types
  -- 程序身份；由装载清单给出
  ProgramId = token;

  -- 程序自己的状态；核心不解释
  State = token;

  -- 检查点：程序状态与它消费到的各输入流位置
  Checkpoint ::
    cursors  : map StreamId to nat  -- 各输入流已消费的位置号
    snapshot : State;               -- 程序状态

  -- 一步推进的产出：新的检查点、派生记录载荷、效应请求
  Step ::
    next     : Checkpoint             -- 推进后的检查点
    requests : seq of EffectRequest;  -- 本步发出的请求

  -- 已装载的程序
  Loaded ::
    loadedBy     : Principal        -- 装载人：程序以此身份发出请求
    subscription : SubscriptionId   -- 程序的输入订阅
    inputs       : set of StreamId  -- 输入流
    output       : StreamId         -- 派生输出流
    checkpoint   : Checkpoint;      -- 最近检查点

  -- 程序表
state Programs of
  loaded : map ProgramId to Loaded  -- 已装载的程序
  inv mk_Programs(l) ==
        forall p in set dom l & dom l(p).checkpoint.cursors = l(p).inputs
  init s == s = mk_Programs({|->})
end

operations
  Load: ProgramId * Principal * set of StreamId * StreamId ==> ()
  Load(prog, who, inputs, output) == is not yet specified;

  Unload: ProgramId ==> ()
  Unload(prog) == is not yet specified;

  SaveCheckpoint: ProgramId * Checkpoint ==> RefOutcome
  SaveCheckpoint(prog, cp) == is not yet specified;

  Advance: ProgramId * StreamId * Step ==> seq of RequestRef
  Advance(prog, input, step) == is not yet specified;
end Program
```
