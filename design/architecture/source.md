# 来源会话连接

```vdm
module Source
exports
  types
    SourceId;
    StreamName;
    CapabilityKey;
    InstrumentId;
    ClientKey;
    RawPayload;
    struct Verdict;
    struct SessionVerdict;
    struct StreamDecl;
    struct Declaration
  operations
    OpenSession: SourceId * Declaration ==> ();
    CloseSession: SourceId ==> ();
    SessionCapability: SourceId * CapabilityKey ==> SessionVerdict;
    LatestStreams: SourceId ==> set of StreamDecl
definitions
types
  -- 外部来源的身份；由接入登记给出，使用方不读结构
  SourceId = token;

  -- 来源内一条推送流的名字；由来源在握手时给出
  StreamName = token;

  -- 一项可声明能力的身份；由使用方指定，本模块不解释其含义
  CapabilityKey = token;

  -- 来源作用域内的品种身份；由映射从来源原文归一得出，核心只作相等比较
  InstrumentId = token;

  -- 核心为一次外部写铸造、随请求交给来源并在来源回声中带回的调用方键
  ClientKey = token;

  -- 来源原文；核心原样保存与转交，不解释
  RawPayload = token;

  -- 来源在一次已建立会话中对某项能力的声明：支持、不支持或未确认
  Verdict = <Supported> | <Unsupported> | <Unknown>;

  -- 查询会话有效能力的结局：会话在线时为声明三值，离线时为不可用
  SessionVerdict = Verdict | <Unavailable>;

  -- 来源对一条推送流的声明：是否带可衔接的来源序号
  StreamDecl ::
    name     : StreamName  -- 流名
    venueSeq : bool;       -- 该流记录是否携带可用于判完备的来源连续序号

  -- 一次握手交出的声明全集
  Declaration ::
    streams      : set of StreamDecl                -- 本来源提供的推送流
    capabilities : map CapabilityKey to Verdict;    -- 本次会话对各能力的声明

  -- 来源会话表：在线会话的有效声明，以及每个来源最近一次握手留下的声明副本
state Sources of
  live   : map SourceId to Declaration  -- 当前在线会话及其会话有效声明
  latest : map SourceId to Declaration  -- 每个来源最近一次握手的声明副本
  inv mk_Sources(l, t) == dom l subset dom t
  init s == s = mk_Sources({|->}, {|->})
end

operations
  OpenSession: SourceId * Declaration ==> ()
  OpenSession(source, decl) == is not yet specified;

  CloseSession: SourceId ==> ()
  CloseSession(source) == is not yet specified;

  SessionCapability: SourceId * CapabilityKey ==> SessionVerdict
  SessionCapability(source, cap) == is not yet specified;

  LatestStreams: SourceId ==> set of StreamDecl
  LatestStreams(source) == is not yet specified;
end Source
```
