# 映射连接

```vdm
module Mapping
imports
  from Source
    types
      SourceId renamed SourceId;
      StreamName renamed StreamName;
      CapabilityKey renamed CapabilityKey;
      RawPayload renamed RawPayload;
      Verdict renamed Verdict;
      SessionVerdict renamed SessionVerdict;
      StreamDecl renamed StreamDecl
    operations
      SessionCapability renamed SessionCapability;
      LatestStreams renamed LatestStreams,
  from Observation
    types
      Decoded renamed Decoded;
      External renamed External;
      StreamId renamed StreamId;
      AppendOutcome renamed AppendOutcome
    operations
      OpenEpoch renamed OpenEpoch;
      Append renamed Append
exports
  types
    MappingId;
    LaneKey;
    IntentPayload;
    struct Envelope;
    struct EncodeRequest;
    struct Encoded;
    struct EncodeOutcome;
    struct Probe;
    struct Evidence;
    struct CapabilityStatus
  functions
    Combine: Verdict * SessionVerdict -> CapabilityStatus;
    Sequenced: StreamName * set of StreamDecl -> bool
  operations
    Install: SourceId * MappingId * map CapabilityKey to Verdict ==> ();
    Decode: Envelope ==> [Decoded];
    OpenStream: SourceId * StreamName * [RawPayload] ==> StreamId;
    Ingest: Envelope * nat1 ==> AppendOutcome;
    Encode: EncodeRequest ==> EncodeOutcome;
    ClassifyEvidence: Probe ==> Evidence;
    CurrentCapability: SourceId * CapabilityKey ==> CapabilityStatus
definitions
types
  -- 一份映射定义的身份；核心装载，使用方不读结构
  MappingId = token;

  -- 写通道键：后续写的语义依赖前一笔结果的范围；由映射从意图按品种导出
  LaneKey = token;

  -- 核心内表达的一次写意图载荷；由业务构造，映射负责编码
  IntentPayload = token;

  -- 集成交给核心的信封：固定头加来源原文
  Envelope ::
    source : SourceId    -- 来自哪个来源
    stream : StreamName  -- 属于来源的哪条流
    raw    : RawPayload; -- 来源原文，原样保存

  -- 一次编码请求：发往哪个来源的哪个意图
  EncodeRequest ::
    source : SourceId        -- 发往哪个来源
    intent : IntentPayload;  -- 要编码的意图

  -- 映射编码的结果：写通道与交给来源的请求原文
  Encoded ::
    lane    : LaneKey     -- 本次写所在通道
    request : RawPayload; -- 交给来源的请求

  -- 编码结局：编出，或当前映射编不出
  EncodeOutcome = Encoded | <Unmapped>;

  -- 一次取证分类请求：哪个来源回读到的原文
  Probe ::
    source : SourceId    -- 哪个来源
    raw    : RawPayload; -- 回读到的原文

  -- 取证回读的分类结论：映射匹配不到的一律为未查清
  Evidence = <Found> | <Absent> | <Inconclusive>;

  -- 当前能力的结局：静态声明与会话有效声明合成，离线时不可用
  CapabilityStatus = Verdict | <Unavailable>;

  -- 映射表：每个来源恰有一份当前映射及其静态能力声明
state Mappings of
  current : map SourceId to MappingId                     -- 每个来源的当前映射
  static  : map SourceId to map CapabilityKey to Verdict  -- 当前映射声明的静态能力
  inv mk_Mappings(c, s) == dom c = dom s
  init m == m = mk_Mappings({|->}, {|->})
end

functions
  -- 合成当前能力：离线不可用；任一方不支持即不支持；双方都支持才支持；其余未确认
  Combine: Verdict * SessionVerdict -> CapabilityStatus
  Combine(fixed, session) ==
    if session = <Unavailable> then <Unavailable>
    elseif fixed = <Unsupported> or session = <Unsupported> then <Unsupported>
    elseif fixed = <Supported> and session = <Supported> then <Supported>
    else <Unknown>;

  -- 判定：来源是否声明某条流携带可衔接的连续序号
  Sequenced: StreamName * set of StreamDecl -> bool
  Sequenced(name, decls) ==
    exists s in set decls & s.name = name and s.venueSeq;

operations
  -- 装载来源的当前映射：替换先前的映射与静态能力声明，不保留旧版本
  Install(source: SourceId, mapping: MappingId, caps: map CapabilityKey to Verdict)
  ext wr current wr static
  post current = current~ ++ {source |-> mapping}
       and static = static~ ++ {source |-> caps};

  -- 按来源当前映射解码信封，解不出为空；没有映射的来源一律解不出
  Decode(env: Envelope) r: [Decoded]
  ext rd current
  post env.source not in set dom current => r = nil;

  -- 场景：为某来源的某条流开新一代，按来源最近声明记下该流是否携带连续序号
  OpenStream: SourceId * StreamName * [RawPayload] ==> StreamId
  OpenStream(source, name, raw) ==
    cases Sequenced(name, LatestStreams(source)):
      true -> return OpenEpoch(mk_External(source), name, raw, true),
      others -> return OpenEpoch(mk_External(source), name, raw, false)
    end;

  -- 场景：集成交来一封信封时，先解码再追加到该来源该流的指定一代上；解不出的也追加，原文保留
  Ingest: Envelope * nat1 ==> AppendOutcome
  Ingest(env, epoch) ==
    return Append(mk_StreamId(mk_External(env.source), env.stream, epoch),
                  Decode(env), env.raw, nil);

  -- 按来源当前映射编码意图并导出通道；没有映射的来源一律编不出
  Encode(req: EncodeRequest) r: EncodeOutcome
  ext rd current
  post req.source not in set dom current => r = <Unmapped>;

  -- 按来源当前映射分类一次回读的原文；没有映射的来源一律未查清
  ClassifyEvidence(probe: Probe) r: Evidence
  ext rd current
  post probe.source not in set dom current => r = <Inconclusive>;

  -- 场景：合成来源此刻对某能力的结论；映射未声明的能力按不支持计
  CurrentCapability: SourceId * CapabilityKey ==> CapabilityStatus
  CurrentCapability(source, cap) ==
    let session = SessionCapability(source, cap) in
      if source in set dom static and cap in set dom static(source)
      then return Combine(static(source)(cap), session)
      else return Combine(<Unsupported>, session);
end Mapping
```
