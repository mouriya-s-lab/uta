# 下游会话连接

```vdm
module Session
exports
  types
    Principal;
    SessionId;
    ControlAction;
    struct ControlOutcome
  operations
    Handshake: Principal ==> SessionId;
    Close: SessionId ==> ();
    PrincipalOf: SessionId ==> [Principal];
    Control: SessionId * ControlAction ==> ControlOutcome
definitions
types
  -- 经解释层进入核心的行动主体身份；核心只作相等比较
  Principal = token;

  -- 一次下游会话的身份；核心造出，使用方不读结构
  SessionId = token;

  -- 一个控制动作；由解释层给出，核心按注册的授权执行
  ControlAction = token;

  -- 控制动作的结局：已执行或被拒绝
  ControlOutcome = <Applied> | <Rejected>;

  -- 下游会话表：在线会话与它们的主体
state Sessions of
  live : map SessionId to Principal  -- 当前在线的下游会话及其主体
  init s == s = mk_Sessions({|->})
end

operations
  Handshake: Principal ==> SessionId
  Handshake(who) == is not yet specified;

  Close: SessionId ==> ()
  Close(session) == is not yet specified;

  PrincipalOf: SessionId ==> [Principal]
  PrincipalOf(session) == is not yet specified;

  Control: SessionId * ControlAction ==> ControlOutcome
  Control(session, action) == is not yet specified;
end Session
```
