# 来源与能力连接

```vdm
module Source
exports
  types
    SourceId;
    CapabilityKey;
    struct Verdict;
    struct CapabilityStatus;
    struct CapabilityDecl
  operations
    OpenSession: SourceId * set of CapabilityDecl ==> ();
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

  -- 来源对单项能力的声明条目
  CapabilityDecl ::
    source  : SourceId       -- 声明所属的外部来源
    cap     : CapabilityKey  -- 被声明的能力键
    verdict : Verdict;       -- 来源给出的支持结论

  -- 来源的在线会话集合与最近一次握手留下的能力声明副本
state Sources of
  live     : set of SourceId        -- 当前会话在线的来源集合
  declared : set of CapabilityDecl  -- 最近握手留下的能力声明条目集合
  inv mk_Sources(l, d) ==
        forall s in set l & exists c in set d & c.source = s
  init s == s = mk_Sources({}, {})
end

operations
  -- 建立来源会话并更新该来源的能力声明副本
  OpenSession(source: SourceId, caps: set of CapabilityDecl)
  ext wr live wr declared
  post (exists c in set caps & c.source = source)
       and live = live~ union {source}
       and declared = {d | d in set declared~ & d.source <> source} union caps;

  -- 结束来源会话；最近声明副本保留供离线查阅，但该来源不再属于在线集合
  CloseSession(source: SourceId)
  ext wr live
  post live = live~ \ {source};

  -- 查询某来源此刻对某项能力的有效结论；离线时返回不可用，不以历史声明充当当前能力
  CurrentCapability(source: SourceId, cap: CapabilityKey) r: CapabilityStatus
  ext rd live rd declared
  post (source not in set live and r = <Unavailable>)
       or (source in set live
           and exists c in set declared &
                 c.source = source and c.cap = cap and r = c.verdict)
       or (source in set live
           and (not exists c in set declared & c.source = source and c.cap = cap)
           and r = <Unsupported>);
end Source
```
