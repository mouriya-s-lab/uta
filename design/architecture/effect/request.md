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
    struct EffectKind;
    Principal;
    Payload;
    struct Answer;
    struct Note;
    struct EffectRequest;
    struct Answered;
    struct RequestStatus;
    struct Event
  operations
    Register: EffectKind ==> ();
    Emit: EffectRequest ==> RequestRef;
    Respond: RequestRef * Answer ==> ();
    Record: [RequestRef] * Note ==> ();
    ReadEvents: nat ==> seq of Event;
    StatusOf: RequestRef ==> [RequestStatus];
    RequestOf: RequestRef ==> [EffectRequest];
    Pending: EffectKind ==> set of RequestRef
definitions
types
  -- 一条效应请求在请求表里的身份；核心造出，使用方不读结构
  RequestRef = token;

  -- 效应种类；由处理器注册，核心不穷举
  EffectKind = token;

  -- 经解释层进入核心的行动主体身份；核心只作相等比较，不认证
  Principal = token;

  -- 请求载荷；核心不解释，交给注册了该种类的处理器
  Payload = token;

  -- 处理器给出的答复；核心不解释，作为事件交回请求方
  Answer = token;

  -- 效应侧的一条进展记事；由追加方写入并自行解释，本模块只保证它的顺序与不丢失
  Note = token;

  -- 一条效应请求：请求是值，不是调用
  EffectRequest ::
    kind    : EffectKind          -- 种类
    payload : Payload             -- 载荷
    basis   : set of LogPosition  -- 请求所依据的观察位置
    member  : Principal;          -- 以谁的身份发出

  -- 处理器已作答；答复作为事件回到请求方，请求从此不再等待
  Answered ::
    answer : Answer;  -- 处理器的答复

  -- 请求状态：等待处理器、无处理器、已答复
  RequestStatus = <Pending> | <Unhandled> | Answered;

  -- 效应事件：答复或进展记事，带它所关联的请求（若有）
  Event ::
    about : [RequestRef]  -- 关联的请求；进展记事可不关联
    note  : Note;         -- 内容

  -- 一条请求表记录
  RequestEntry ::
    request : EffectRequest  -- 请求本身
    status  : RequestStatus; -- 当前状态

  -- 请求表与效应事件序列：事件只追加、不压缩、无分代
state Requests of
  handlers : set of EffectKind                -- 已注册处理器的种类
  log      : map RequestRef to RequestEntry   -- 请求表
  events   : seq of Event                     -- 效应事件序列，下标即位置
  inv mk_Requests(h, l, -) ==
        forall r in set dom l &
          l(r).status = <Pending> => l(r).request.kind in set h
  init r == r = mk_Requests({}, {|->}, [])
end

operations
  -- 注册某种类的处理器
  Register(kind: EffectKind)
  ext wr handlers
  post handlers = handlers~ union {kind};

  -- 追加一条请求：有处理器则等待处理，否则停在无处理器；两者都不是错误
  Emit(req: EffectRequest) ref: RequestRef
  ext rd handlers wr log
  post ref not in set dom log~
       and log = log~ munion
             {ref |-> mk_RequestEntry(req, if req.kind in set handlers
                                           then <Pending> else <Unhandled>)};

  -- 记录处理器对一条等待中请求的答复，并把答复追加为事件
  Respond(ref: RequestRef, answer: Answer)
  ext wr log wr events
  pre ref in set dom log and log(ref).status = <Pending>
  post log = log~ ++ {ref |-> mu(log~(ref), status |-> mk_Answered(answer))}
       and events = events~ ^ [mk_Event(ref, answer)];

  -- 追加一条进展记事
  Record(about: [RequestRef], note: Note)
  ext rd log wr events
  pre about <> nil => about in set dom log
  post events = events~ ^ [mk_Event(about, note)];

  -- 读出位置晚于 after 的全部事件，按追加先后
  ReadEvents(after: nat) r: seq of Event
  ext rd events
  post r = (if after >= len events then [] else events(after + 1, ..., len events));

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
