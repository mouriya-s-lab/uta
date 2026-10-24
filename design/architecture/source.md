# 来源会话连接

```vdm
module Source
exports
  types
    SourceId;
    StreamName;
    CapabilityKey;
    RawPayload;
    struct Verdict;
    struct SessionVerdict;
    struct Declaration
  operations
    OpenSession: SourceId * Declaration ==> ();
    CloseSession: SourceId ==> ();
    SessionCapability: SourceId * CapabilityKey ==> SessionVerdict;
    LatestStreams: SourceId ==> set of StreamName
definitions
types
  -- 外部来源的身份；由接入登记给出，使用方不读结构
  SourceId = token;

  -- 来源内一条推送流的名字；由来源在握手时给出
  StreamName = token;

  -- 一项可声明能力的身份；由使用方指定，本模块不解释其含义
  CapabilityKey = token;

  -- 来源给出的原文；核心把它当证据原样保存并原样交给下游，从不读其中的字段
  RawPayload = token;

  -- 来源在一次已建立会话中对某项能力的声明：支持、不支持或未确认
  Verdict = <Supported> | <Unsupported> | <Unknown>;

  -- 查询会话有效能力的结局：会话在线时为声明三值，离线时为不可用
  SessionVerdict = Verdict | <Unavailable>;

  -- 一次握手交出的声明全集：提供的流与对各能力的声明
  Declaration ::
    streams      : set of StreamName               -- 本来源提供的推送流
    capabilities : map CapabilityKey to Verdict;   -- 本次会话对各能力的声明

  -- 来源会话表：在线会话的有效声明总等于该来源的最近声明；会话结束后最近声明保留
state Sources of
  live   : map SourceId to Declaration  -- 当前在线会话及其会话有效声明
  latest : map SourceId to Declaration  -- 每个来源最近一次握手的声明副本
  inv mk_Sources(l, t) == forall s in set dom l & s in set dom t and l(s) = t(s)
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
  LatestStreams(source: SourceId) r: set of StreamName
  ext rd latest
  post (source in set dom latest and r = latest(source).streams)
       or (source not in set dom latest and r = {});
end Source
```
