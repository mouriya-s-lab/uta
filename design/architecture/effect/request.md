# 效应请求连接

```vdm
module Request
imports
  from Observation
    types
      LogPosition renamed LogPosition
exports
  types
    RequestRef;
    EffectKind;
    Principal;
    Payload;
    struct Answer;
    struct HandlerClass;
    struct EffectRequest;
    struct Answered;
    struct RequestStatus
  operations
    Register: EffectKind * HandlerClass ==> ();
    Emit: EffectRequest ==> RequestRef;
    Respond: RequestRef * Answer ==> ();
    StatusOf: RequestRef ==> [RequestStatus];
    RequestOf: RequestRef ==> [EffectRequest];
    Pending: EffectKind ==> set of RequestRef
definitions
types
  -- 一条效应请求在请求流上的身份；核心造出，使用方不读结构
  RequestRef = token;

  -- 效应种类；由处理器注册，核心不穷举
  EffectKind = token;

  -- 经解释层进入核心的行动主体身份；核心只作相等比较，不认证
  Principal = token;

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

  -- 处理器已作答；答复作为流事件回到请求方，请求从此不再等待
  Answered ::
    answer : Answer;  -- 处理器的答复

  -- 请求状态：等待处理器、无处理器、已答复
  RequestStatus = <Pending> | <Unhandled> | Answered;

  -- 一条请求流记录
  Record ::
    request : EffectRequest  -- 请求本身
    status  : RequestStatus; -- 当前状态

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
  -- 注册某种类的处理器；同一种类再次注册以后者为准
  Register(kind: EffectKind, class: HandlerClass)
  ext wr handlers
  post handlers = handlers~ ++ {kind |-> class};

  -- 追加一条请求：有处理器则等待处理，否则停在无处理器；两者都不是错误
  Emit(req: EffectRequest) ref: RequestRef
  ext rd handlers wr log
  post ref not in set dom log~
       and log = log~ munion
             {ref |-> mk_Record(req, if req.kind in set dom handlers
                                     then <Pending> else <Unhandled>)};

  -- 记录处理器对一条等待中请求的答复
  Respond(ref: RequestRef, answer: Answer)
  ext wr log
  pre ref in set dom log and log(ref).status = <Pending>
  post log = log~ ++ {ref |-> mu(log~(ref), status |-> mk_Answered(answer))};

  -- 查询请求状态；不存在的请求为空
  StatusOf(ref: RequestRef) r: [RequestStatus]
  ext rd log
  post (ref in set dom log and r = log(ref).status)
       or (ref not in set dom log and r = nil);

  -- 查询请求本身；不存在的请求为空
  RequestOf(ref: RequestRef) r: [EffectRequest]
  ext rd log
  post (ref in set dom log and r = log(ref).request)
       or (ref not in set dom log and r = nil);

  -- 某种类下全部等待处理的请求
  Pending(kind: EffectKind) r: set of RequestRef
  ext rd log
  post r = {ref | ref in set dom log &
              log(ref).request.kind = kind and log(ref).status = <Pending>};
end Request
```
