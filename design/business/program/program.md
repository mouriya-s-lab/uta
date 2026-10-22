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
  functions
    Attributed: EffectRequest * Principal -> EffectRequest
  operations
    Load: ProgramId * Principal * set of StreamId * StreamName * State ==> ();
    Unload: ProgramId ==> ();
    CheckpointOf: ProgramId ==> [Checkpoint];
    Poll: ProgramId * StreamId ==> seq of Item;
    Events: ProgramId ==> seq of Event;
    AdvanceOn: ProgramId * StreamId * Step ==> [RequestRef];
    AdvanceOnEvents: ProgramId * Step ==> [RequestRef]
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

functions
  -- 判定：程序发出请求时归属的主体——一律取装载该程序的主体，程序自身不是行动主体
  Attributed: EffectRequest * Principal -> EffectRequest
  Attributed(req, who) == mu(req, member |-> who);

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

  -- 场景：发布本步的派生值到程序的输出流；本步没有派生值则不追加
  Publish: ProgramId * Step ==> ()
  Publish(prog, step) ==
    cases step.derived:
      nil -> skip,
      fields -> let - = Append(loaded(prog).output, mk_Decoded(nil, fields), nil, step.supersedes) in skip
    end
  pre prog in set dom loaded
      and (step.supersedes <> nil => step.supersedes.stream = loaded(prog).output);

  -- 场景：以装载人身份发出本步的请求并交回请求身份；本步没有请求则不发出
  Forward: ProgramId * [EffectRequest] ==> [RequestRef]
  Forward(prog, req) ==
    cases req:
      nil -> return nil,
      r -> return Emit(Attributed(r, loaded(prog).loadedBy))
    end
  pre prog in set dom loaded;

  -- 场景：对某条输入流上的记录推进一步——新检查点经 Keep 生效，该流经 Ack 确认，派生值经 Publish 发布，请求经 Forward 发出并交回请求身份
  AdvanceOn: ProgramId * StreamId * Step ==> [RequestRef]
  AdvanceOn(prog, input, step) ==
    (Keep(prog, step.next);
     Ack(loaded(prog).subscription, input, step.next.cursors(input));
     Publish(prog, step);
     return Forward(prog, step.request))
  pre prog in set dom loaded and input in set loaded(prog).inputs
      and dom step.next.cursors = loaded(prog).inputs
      and not Earlier(step.next.cursors(input), loaded(prog).checkpoint.cursors(input))
      and (forall s in set loaded(prog).inputs &
             s <> input => step.next.cursors(s) = loaded(prog).checkpoint.cursors(s))
      and step.next.reqCursor >= loaded(prog).checkpoint.reqCursor
      and (step.supersedes <> nil => step.supersedes.stream = loaded(prog).output);

  -- 场景：只因效应事件推进一步——新检查点经 Keep 生效，没有流要确认，派生值经 Publish 发布，请求经 Forward 发出并交回请求身份
  AdvanceOnEvents: ProgramId * Step ==> [RequestRef]
  AdvanceOnEvents(prog, step) ==
    (Keep(prog, step.next);
     Publish(prog, step);
     return Forward(prog, step.request))
  pre prog in set dom loaded
      and step.next.cursors = loaded(prog).checkpoint.cursors
      and step.next.reqCursor >= loaded(prog).checkpoint.reqCursor
      and (step.supersedes <> nil => step.supersedes.stream = loaded(prog).output);
end Program
```
