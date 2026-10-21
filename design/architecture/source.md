# 来源会话连接

```vdm
module Source
exports
  types
    SourceId;
    StreamName;
    CapabilityKey;
    InstrumentId;
    struct ClientKey;
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

  -- 来源给出的原文；核心把它当证据原样保存并原样交给下游，从不读其中的字段
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
  -- 建立来源会话：会话有效声明与最近声明都换成本次握手交出的声明
  OpenSession(source: SourceId, decl: Declaration)
  ext wr live wr latest
  post live = live~ ++ {source |-> decl}
       and latest = latest~ ++ {source |-> decl};

  -- 结束来源会话：会话有效声明消失，最近声明保留供离线查阅
  CloseSession(source: SourceId)
  ext wr live
  post live = {source} <-: live~;

  -- 查询会话有效能力：离线为不可用；在线而本次握手未声明的能力为未确认
  SessionCapability(source: SourceId, cap: CapabilityKey) r: SessionVerdict
  ext rd live
  post (source not in set dom live and r = <Unavailable>)
       or (source in set dom live
           and cap in set dom live(source).capabilities
           and r = live(source).capabilities(cap))
       or (source in set dom live
           and cap not in set dom live(source).capabilities
           and r = <Unknown>);

  -- 最近声明的流集合；从未握手的来源为空集
  LatestStreams(source: SourceId) r: set of StreamDecl
  ext rd latest
  post (source in set dom latest and r = latest(source).streams)
       or (source not in set dom latest and r = {});
end Source
```
