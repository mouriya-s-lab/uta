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
      Cursor renamed Cursor;
      LogPosition renamed LogPosition
    functions
      Earlier renamed Earlier
    operations
      OpenEpoch renamed OpenEpoch;
      Append renamed Append,
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
      EffectRequest renamed EffectRequest;
      Event renamed Event
    operations
      Emit renamed Emit;
      ReadEvents renamed ReadEvents
exports
  types
    ProgramId;
    State;
    struct Checkpoint;
    struct Step
  operations
    Load: ProgramId * Principal * set of StreamId * StreamName * State ==> ();
    Unload: ProgramId ==> ();
    CheckpointOf: ProgramId ==> [Checkpoint];
    Poll: ProgramId * StreamId ==> seq of Item;
    Events: ProgramId ==> seq of Event;
    Advance: ProgramId * [StreamId] * Step ==> [RequestRef]
definitions
types
  -- 程序身份；由装载清单给出
  ProgramId = token;

  -- 程序自己的状态；核心不解释
  State = token;

  -- 检查点：程序状态、各输入流已消费的游标、效应事件已消费的位置
  Checkpoint ::
    cursors   : map StreamId to Cursor  -- 各输入流已消费的游标
    reqCursor : nat                     -- 效应事件已消费的位置
    snapshot  : State;                  -- 程序状态

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

  -- 把一个检查点记为程序的最近检查点，取代先前的
  Keep(prog: ProgramId, cp: Checkpoint)
  ext wr loaded
  pre prog in set dom loaded and dom cp.cursors = loaded(prog).inputs
  post loaded = loaded~ ++ {prog |-> mu(loaded~(prog), checkpoint |-> cp)};

  -- 场景：装载程序。建立输入订阅、开派生输出流，检查点从各输入流起点与效应事件起点开始
  Load: ProgramId * Principal * set of StreamId * StreamName * State ==> ()
  Load(prog, who, inputs, outName, initial) ==
    let sub = Subscribe(inputs),
        out = mk_StreamId(mk_Derived(mk_token(prog)), outName) in
      (OpenEpoch(out, nil);
       Register(prog, mk_Loaded(who, sub, inputs, out,
                                mk_Checkpoint({s |-> mk_Cursor(1, 0) | s in set inputs}, 0, initial))))
  pre prog not in set dom loaded;

  -- 场景：卸载程序。删除订阅、移除登记
  Unload: ProgramId ==> ()
  Unload(prog) ==
    (Unsubscribe(loaded(prog).subscription);
     Forget(prog))
  pre prog in set dom loaded;

  -- 查询程序最近的检查点；未装载的程序为空
  CheckpointOf(prog: ProgramId) r: [Checkpoint]
  ext rd loaded
  post (prog in set dom loaded and r = loaded(prog).checkpoint)
       or (prog not in set dom loaded and r = nil);

  -- 场景：取出某输入流上游标之后的投递项；被压缩越过的部分以缺口交出，程序自己决定怎样对待
  Poll: ProgramId * StreamId ==> seq of Item
  Poll(prog, input) ==
    return Deliver(loaded(prog).subscription, input)
  pre prog in set dom loaded and input in set loaded(prog).inputs;

  -- 场景：取出效应事件位置之后的全部事件
  Events: ProgramId ==> seq of Event
  Events(prog) ==
    return ReadEvents(loaded(prog).checkpoint.reqCursor)
  pre prog in set dom loaded;

  -- 场景：推进一步——记下新检查点；本步消费了某条输入流就确认它；有派生值就追加到输出流；有请求就以装载人的身份发出并交回请求身份
  Advance: ProgramId * [StreamId] * Step ==> [RequestRef]
  Advance(prog, input, step) ==
    (Keep(prog, step.next);
     if input <> nil then Ack(loaded(prog).subscription, input, step.next.cursors(input));
     if step.derived <> nil
     then let - = Append(loaded(prog).output, mk_Decoded(nil, step.derived), nil, step.supersedes) in skip;
     if step.request <> nil
     then return Emit(mu(step.request, member |-> loaded(prog).loadedBy))
     else return nil)
  pre prog in set dom loaded
      and dom step.next.cursors = loaded(prog).inputs
      and (forall s in set loaded(prog).inputs &
             s <> input => step.next.cursors(s) = loaded(prog).checkpoint.cursors(s))
      and (input <> nil =>
             input in set loaded(prog).inputs
             and not Earlier(step.next.cursors(input), loaded(prog).checkpoint.cursors(input)))
      and step.next.reqCursor >= loaded(prog).checkpoint.reqCursor
      and (step.supersedes <> nil => step.supersedes.stream = loaded(prog).output);
end Program
```
