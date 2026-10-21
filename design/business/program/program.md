# 程序连接

```vdm
module Program
imports
  from Source
    types
      StreamName renamed StreamName,
  from Observation
    types
      Fields renamed Fields;
      Decoded renamed Decoded;
      RefHolder renamed RefHolder;
      Derived renamed Derived;
      StreamId renamed StreamId;
      LogPosition renamed LogPosition;
      RefOutcome renamed RefOutcome
    operations
      OpenEpoch renamed OpenEpoch;
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
      Principal renamed Principal;
      EffectRequest renamed EffectRequest
    operations
      Emit renamed Emit
exports
  types
    ProgramId;
    State;
    struct Checkpoint;
    struct Step;
    struct Loaded
  operations
    Register: ProgramId * Loaded ==> ();
    Forget: ProgramId ==> ();
    Keep: ProgramId * Checkpoint ==> ();
    Protect: ProgramId * Checkpoint ==> RefOutcome;
    Load: ProgramId * Principal * set of StreamId * StreamName * State ==> ();
    Unload: ProgramId ==> ();
    Poll: ProgramId * StreamId ==> seq of Item;
    Confirm: ProgramId * StreamId * Checkpoint ==> ();
    Publish: ProgramId * Step ==> ();
    Forward: ProgramId * [EffectRequest] ==> ();
    Advance: ProgramId * StreamId * Step ==> ()
  functions
    Protected: Checkpoint -> set of LogPosition
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

  -- 一步推进的产出：新的检查点、至多一条派生记录、至多一条效应请求
  Step ::
    next       : Checkpoint        -- 推进后的检查点
    derived    : [Fields]          -- 本步算出的派生值
    supersedes : [LogPosition]     -- 派生值所修正的旧位置
    request    : [EffectRequest];  -- 本步发出的请求

  -- 已装载的程序
  Loaded ::
    loadedBy     : Principal        -- 装载人：程序以此身份发出请求
    holder       : RefHolder        -- 程序在观察日志登记引用所用的持有者身份
    subscription : SubscriptionId   -- 程序的输入订阅
    inputs       : set of StreamId  -- 输入流
    output       : StreamId         -- 派生输出流
    checkpoint   : Checkpoint;      -- 最近检查点

  -- 程序表：检查点覆盖且只覆盖程序的输入流
state Programs of
  loaded : map ProgramId to Loaded  -- 已装载的程序
  inv mk_Programs(l) ==
        forall p in set dom l & dom l(p).checkpoint.cursors = l(p).inputs
  init s == s = mk_Programs({|->})
end

functions
  -- 判定：检查点需要保留边界保护的位置——各输入流上已消费到的位置；尚未消费的流没有位置
  Protected: Checkpoint -> set of LogPosition
  Protected(cp) ==
    {mk_LogPosition(s, cp.cursors(s)) | s in set dom cp.cursors & cp.cursors(s) > 0};

operations
  -- 登记已装载的程序
  Register(prog: ProgramId, entry: Loaded)
  ext wr loaded
  pre prog not in set dom loaded and dom entry.checkpoint.cursors = entry.inputs
  post loaded = loaded~ munion {prog |-> entry};

  -- 移除程序登记
  Forget(prog: ProgramId)
  ext wr loaded
  post loaded = {prog} <-: loaded~;

  -- 把一个检查点记为程序的最近检查点，取代先前的
  Keep(prog: ProgramId, cp: Checkpoint)
  ext wr loaded
  pre prog in set dom loaded and dom cp.cursors = loaded(prog).inputs
  post loaded = loaded~ ++ {prog |-> mu(loaded~(prog), checkpoint |-> cp)};

  -- 场景：装载程序。建立输入订阅、开派生输出流，检查点从各输入流起点开始
  Load: ProgramId * Principal * set of StreamId * StreamName * State ==> ()
  Load(prog, who, inputs, outName, initial) ==
    let sub = Subscribe(inputs),
        out = OpenEpoch(mk_Derived(mk_token(prog)), outName, nil, false) in
      Register(prog, mk_Loaded(who, mk_token(prog), sub, inputs, out,
                               mk_Checkpoint({s |-> 0 | s in set inputs}, initial)))
  pre prog not in set dom loaded;

  -- 场景：卸载程序。删除订阅、释放检查点持有的保留引用、移除登记
  Unload: ProgramId ==> ()
  Unload(prog) ==
    (Unsubscribe(loaded(prog).subscription);
     ReleaseRef(loaded(prog).holder);
     Forget(prog))
  pre prog in set dom loaded;

  -- 场景：取出某输入流上游标之后的投递项
  Poll: ProgramId * StreamId ==> seq of Item
  Poll(prog, input) ==
    return Deliver(loaded(prog).subscription, input)
  pre prog in set dom loaded and input in set loaded(prog).inputs;

  -- 场景：向程序的订阅确认某输入流已消费到检查点所记的位置
  Confirm: ProgramId * StreamId * Checkpoint ==> ()
  Confirm(prog, input, cp) ==
    Ack(loaded(prog).subscription, input, cp.cursors(input))
  pre prog in set dom loaded and input in set loaded(prog).inputs
      and input in set dom cp.cursors and cp.cursors(input) >= 1;

  -- 场景：把本步的派生值追加到程序的输出流；本步没有派生值则不追加
  Publish: ProgramId * Step ==> ()
  Publish(prog, step) ==
    cases step.derived:
      nil -> skip,
      fields -> let - = Append(loaded(prog).output,
                                mk_Decoded(nil, nil, nil, fields), nil, step.supersedes) in skip
    end
  pre prog in set dom loaded
      and (step.supersedes <> nil => step.supersedes.stream = loaded(prog).output);

  -- 场景：以装载人身份发出本步的请求；本步没有请求则不发出
  Forward: ProgramId * [EffectRequest] ==> ()
  Forward(prog, req) ==
    cases req:
      nil -> skip,
      r -> let - = Emit(mu(r, member |-> loaded(prog).loadedBy)) in skip
    end
  pre prog in set dom loaded;

  -- 场景：以程序身份登记新检查点需要保护的位置；登记成功才把它记为最近检查点
  Protect: ProgramId * Checkpoint ==> RefOutcome
  Protect(prog, cp) ==
    cases AcquireRef(loaded(prog).holder, Protected(cp)):
      <Rejected> -> return <Rejected>,
      others -> (Keep(prog, cp); return <Applied>)
    end
  pre prog in set dom loaded and dom cp.cursors = loaded(prog).inputs;

  -- 场景：推进一步。新检查点受保护后，确认该输入流游标、发布派生值、发出请求；未受保护则本步无效
  Advance: ProgramId * StreamId * Step ==> ()
  Advance(prog, input, step) ==
    cases Protect(prog, step.next):
      <Rejected> -> skip,
      others -> (Confirm(prog, input, step.next);
                 Publish(prog, step);
                 Forward(prog, step.request))
    end
  pre prog in set dom loaded and input in set loaded(prog).inputs
      and dom step.next.cursors = loaded(prog).inputs;
end Program
```
