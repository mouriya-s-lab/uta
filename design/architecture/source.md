# 来源与能力连接

```vdm
module Source
exports
  types
    SourceId;
    CapabilityKey;
    struct Verdict;
    struct CapabilityStatus
  operations
    OpenSession: SourceId * map CapabilityKey to Verdict ==> ();
    CloseSession: SourceId ==> ();
    CurrentCapability: SourceId * CapabilityKey ==> CapabilityStatus
definitions
types
  -- 外部来源的身份；由接入配置给出，使用方不读结构
  SourceId = token;

  -- 一项可声明能力的身份；由使用方模块指定，本模块不解释具体操作含义
  CapabilityKey = token;

  -- 来源在一次已建立会话中对某项能力的声明：支持、不支持或未确认
  Verdict = <Supported> | <Unsupported> | <Unknown>;

  -- 查询来源当前能力的结局：只有会话在线时才返回声明三值，离线时为不可用
  CapabilityStatus = Verdict | <Unavailable>;

  -- 来源会话与最近一次握手声明的记录
state Sources of
  live : set of SourceId  -- 当前会话在线的来源集合
  declared : map SourceId to (map CapabilityKey to Verdict)  -- 每个来源最近一次握手留下的声明副本
  inv mk_Sources(l, d) == l subset dom d
  init s == s = mk_Sources({}, {|->})
end

operations
  -- 建立来源会话并记下本次握手的能力声明
  OpenSession(source: SourceId, caps: map CapabilityKey to Verdict)
  ext wr live wr declared
  post live = live~ union {source}
       and declared = declared~ ++ {source |-> caps};

  -- 结束来源会话；历史声明副本保留，但该来源不再属于在线集合
  CloseSession(source: SourceId)
  ext wr live
  post live = live~ \ {source};

  -- 查询某来源此刻对某项能力的有效结论；离线时不以历史声明充当当前能力
  CurrentCapability(source: SourceId, cap: CapabilityKey) r: CapabilityStatus
  ext rd live rd declared
  post (source not in set live and r = <Unavailable>)
       or (source in set live
           and cap in set dom declared(source)
           and r = declared(source)(cap))
       or (source in set live
           and cap not in set dom declared(source)
           and r = <Unsupported>);
end Source
```
