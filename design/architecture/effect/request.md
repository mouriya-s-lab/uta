# 效应请求连接

```vdm
module Request
imports
  from Observation
    types
      LogPosition renamed LogPosition,
  from Session
    types
      Principal renamed Principal
exports
  types
    RequestRef;
    EffectKind;
    Payload;
    Answer;
    struct HandlerClass;
    struct EffectRequest;
    struct Answered;
    struct Status
  operations
    Register: EffectKind * HandlerClass ==> ();
    Emit: EffectRequest ==> RequestRef;
    Respond: RequestRef * Answer ==> ();
    StatusOf: RequestRef ==> [Status];
    Pending: EffectKind ==> set of RequestRef
definitions
types
  -- 一条效应请求在请求流上的身份；核心造出，使用方不读结构
  RequestRef = token;

  -- 效应种类；由处理器注册，核心不穷举
  EffectKind = token;

  -- 请求载荷；核心不解释，交给注册了该种类的处理器
  Payload = token;

  -- 处理器给出的答复；核心不解释，作为流事件交回请求方
  Answer = token;

  -- 处理器类别：向来源只读、改变外部世界、或由核心自己的记录作答
  HandlerClass = <Read> | <Write> | <Core>;

  -- 一条效应请求：请求是值，不是调用
  EffectRequest ::
    kind    : EffectKind          -- 种类
    payload : Payload             -- 载荷
    basis   : set of LogPosition  -- 请求所依据的观察位置
    member  : Principal;          -- 以谁的身份发出

  -- 已答复
  Answered :: answer : Answer;

  -- 请求状态：等待处理器、无处理器、已答复
  Status = <Pending> | <Unhandled> | Answered;

  -- 一条请求流记录
  Record ::
    request : EffectRequest  -- 请求本身
    status  : Status;        -- 当前状态

  -- 请求表：处理器注册与请求流
state Requests of
  handlers : map EffectKind to HandlerClass  -- 已注册的处理器及其类别
  log      : map RequestRef to Record        -- 请求流
  inv mk_Requests(h, l) ==
        forall r in set dom l &
          l(r).status = <Pending> => l(r).request.kind in set dom h
  init r == r = mk_Requests({|->}, {|->})
end

operations
  Register: EffectKind * HandlerClass ==> ()
  Register(kind, class) == is not yet specified;

  Emit: EffectRequest ==> RequestRef
  Emit(req) == is not yet specified;

  Respond: RequestRef * Answer ==> ()
  Respond(ref, answer) == is not yet specified;

  StatusOf: RequestRef ==> [Status]
  StatusOf(ref) == is not yet specified;

  Pending: EffectKind ==> set of RequestRef
  Pending(kind) == is not yet specified;
end Request
```
