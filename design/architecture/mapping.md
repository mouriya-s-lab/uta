# 映射连接

```vdm
module Mapping
imports
  from Source
    types
      SourceId renamed SourceId;
      StreamName renamed StreamName;
      RawPayload renamed RawPayload
    operations
      LatestStreams renamed LatestStreams,
  from Observation
    types
      ClientKey renamed ClientKey;
      Decoded renamed Decoded;
      External renamed External;
      StreamId renamed StreamId;
      LogPosition renamed LogPosition
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
    struct Evidence
  functions
    StreamOf: Envelope -> StreamId
  operations
    Install: SourceId * MappingId ==> ();
    Decode: Envelope ==> [Decoded];
    Break: Envelope ==> bool;
    Ingest: Envelope ==> [LogPosition];
    Encode: EncodeRequest ==> EncodeOutcome;
    ClassifyEvidence: Probe ==> Evidence
definitions
types
  -- 一份映射定义的身份；核心装载，使用方不读结构
  MappingId = token;

  -- 写通道键：后续写的语义依赖前一笔结果的范围；由映射从意图导出
  LaneKey = token;

  -- 核心内表达的一次写意图载荷；由业务构造，映射负责编码
  IntentPayload = token;

  -- 集成交给核心的信封：固定头加来源原文
  Envelope ::
    source : SourceId    -- 来自哪个来源
    stream : StreamName  -- 属于来源的哪条流
    raw    : RawPayload; -- 来源原文，原样保存

  -- 一次编码请求：发往哪个来源的哪个意图，带哪把调用方键
  EncodeRequest ::
    source : SourceId       -- 发往哪个来源
    intent : IntentPayload  -- 要编码的意图
    key    : ClientKey;     -- 核心为本次尝试铸造的调用方键，必须编进请求

  -- 映射编码的结果：写通道与交给来源的请求原文
  Encoded ::
    lane    : LaneKey     -- 本次写所在通道
    request : RawPayload; -- 交给来源的请求，已带调用方键

  -- 编码结局：编出，或当前映射编不出
  EncodeOutcome = Encoded | <Unmapped>;

  -- 一次取证分类请求：哪个来源、查哪把调用方键、回读到的原文
  Probe ::
    source : SourceId    -- 哪个来源
    key    : ClientKey   -- 被查尝试的调用方键
    raw    : RawPayload; -- 回读到的原文

  -- 取证回读的分类结论：映射匹配不到的一律为未查清
  Evidence = <Found> | <Absent> | <Inconclusive>;

  -- 映射表：每个来源至多一份当前映射
state Mappings of
  current : map SourceId to MappingId  -- 每个来源的当前映射
  init m == m = mk_Mappings({|->})
end

functions
  -- 判定：一封信封落在哪条逻辑流上——该来源的外部来头加流名
  StreamOf: Envelope -> StreamId
  StreamOf(env) == mk_StreamId(mk_External(env.source), env.stream);

operations
  -- 装载来源的当前映射：替换先前的映射，不保留旧版本
  Install(source: SourceId, mapping: MappingId)
  ext wr current
  post current = current~ ++ {source |-> mapping};

  -- 按来源当前映射解码信封，解不出为空；没有映射的来源一律解不出
  Decode(env: Envelope) r: [Decoded]
  ext rd current
  post env.source not in set dom current => r = nil;

  -- 场景：入账——集成交来的信封经 Decode 后经 Append 进入它所属的流；解不出的也进入，原文保留
  Ingest: Envelope ==> [LogPosition]
  Ingest(env) ==
    return Append(StreamOf(env), Decode(env), env.raw, nil);

  -- 场景：来源连续性中断——信封所属流在 LatestStreams 中时，经 OpenEpoch 为该流开新一代；会话建立后每条流的首条信封之前也经此开第一代
  Break: Envelope ==> bool
  Break(env) ==
    if env.stream in set LatestStreams(env.source)
    then (OpenEpoch(StreamOf(env), env.raw); return true)
    else return false;

  -- 按来源当前映射把意图连同调用方键编成请求并导出通道；没有映射的来源一律编不出
  Encode(req: EncodeRequest) r: EncodeOutcome
  ext rd current
  post req.source not in set dom current => r = <Unmapped>;

  -- 按来源当前映射判断一次回读的原文是否证明该调用方键的写发生过；没有映射的来源一律未查清
  ClassifyEvidence(probe: Probe) r: Evidence
  ext rd current
  post probe.source not in set dom current => r = <Inconclusive>;
end Mapping
```
