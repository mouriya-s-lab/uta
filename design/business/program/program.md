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
      RequestRef renamed RequestRef;
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
    Store: ProgramId * Checkpoint ==> ();
    Load: ProgramId * Principal * set of StreamId * StreamName * State ==> ();
    Unload: ProgramId ==> ();
    Poll: ProgramId * StreamId ==> seq of Item;
    SaveCheckpoint: ProgramId * Checkpoint ==> RefOutcome;
    Advance: ProgramId * StreamId * Step ==> [RequestRef]
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

  -- 替换程序的最近检查点
  Store(prog: ProgramId, cp: Checkpoint)
  ext wr loaded
  pre prog in set dom loaded and dom cp.cursors = loaded(prog).inputs
  post loaded = loaded~ ++ {prog |-> mu(loaded~(prog), checkpoint |-> cp)};

  -- 场景：装载程序。建立输入订阅、开派生输出流，检查点从各输入流起点开始
  Load: ProgramId * Principal * set of StreamId * StreamName * State ==> ()
  Load(prog, who, inputs, outName, initial) ==
    let sub = Subscribe(inputs),
        out = OpenEpoch(mk_Derived(mk_token(prog)), outName, nil) in
      Register(prog, mk_Loaded(who, sub, inputs, out,
                               mk_Checkpoint({s |-> 0 | s in set inputs}, initial)))
  pre prog not in set dom loaded;

  -- 场景：卸载程序。删除订阅、释放检查点持有的保留引用、移除登记
  Unload: ProgramId ==> ()
  Unload(prog) ==
    (Unsubscribe(loaded(prog).subscription);
     ReleaseRef(mk_token(prog));
     Forget(prog))
  pre prog in set dom loaded;

  -- 场景：取出某输入流上游标之后的投递项
  Poll: ProgramId * StreamId ==> seq of Item
  Poll(prog, input) ==
    return Deliver(loaded(prog).subscription, input)
  pre prog in set dom loaded and input in set loaded(prog).inputs;

  -- 场景：保存检查点。先以程序身份登记各已消费位置的保留引用，登记被拒则不保存
  SaveCheckpoint: ProgramId * Checkpoint ==> RefOutcome
  SaveCheckpoint(prog, cp) ==
    let positions = {mk_LogPosition(s, cp.cursors(s)) | s in set dom cp.cursors & cp.cursors(s) > 0},
        granted = AcquireRef(mk_token(prog), positions) in
      if granted = <Rejected> then return <Rejected>
      else (Store(prog, cp); return <Applied>)
  pre prog in set dom loaded and dom cp.cursors = loaded(prog).inputs;

  -- 场景：推进一步。检查点保存成功后确认该输入流游标、追加派生记录、以装载人身份发出请求；保存被拒则本步无效
  Advance: ProgramId * StreamId * Step ==> [RequestRef]
  Advance(prog, input, step) ==
    let l = loaded(prog), saved = SaveCheckpoint(prog, step.next) in
      if saved = <Rejected> then return nil
      else
        (Ack(l.subscription, input, step.next.cursors(input));
         if step.derived <> nil
         then let - = Append(l.output, <Data>, mk_Decoded(nil, nil, nil, step.derived), nil, step.supersedes) in skip;
         if step.request <> nil
         then return Emit(mu(step.request, member |-> l.loadedBy))
         else return nil)
  pre prog in set dom loaded
      and input in set loaded(prog).inputs
      and dom step.next.cursors = loaded(prog).inputs
      and step.next.cursors(input) >= 1
      and step.next.cursors(input) >= loaded(prog).checkpoint.cursors(input)
      and (step.supersedes <> nil => step.supersedes.stream = loaded(prog).output);
end Program
```
