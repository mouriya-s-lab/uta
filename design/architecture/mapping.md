# 映射连接

```vdm
module Mapping
imports
  from Source
    types
      SourceId renamed SourceId;
      StreamName renamed StreamName;
      CapabilityKey renamed CapabilityKey;
      InstrumentId renamed InstrumentId;
      ClientKey renamed ClientKey;
      RawPayload renamed RawPayload;
      Verdict renamed Verdict;
      SessionVerdict renamed SessionVerdict
    operations
      SessionCapability renamed SessionCapability
exports
  types
    MappingId;
    LaneKey;
    IntentPayload;
    Fields;
    struct Envelope;
    struct Decoded;
    struct DecodeOutcome;
    struct Encoded;
    struct EncodeOutcome;
    struct Evidence;
    struct CapabilityStatus
  operations
    Install: SourceId * MappingId * map CapabilityKey to Verdict ==> ();
    Decode: Envelope ==> DecodeOutcome;
    Encode: SourceId * IntentPayload ==> EncodeOutcome;
    ClassifyEvidence: SourceId * RawPayload ==> Evidence;
    CurrentCapability: SourceId * CapabilityKey ==> CapabilityStatus
definitions
types
  -- 一份映射定义的身份；核心装载，使用方不读结构
  MappingId = token;

  -- 写通道键：后续写的语义依赖前一笔结果的范围；由映射从意图导出
  LaneKey = token;

  -- 核心内表达的一次写意图载荷；由业务构造，映射负责编码
  IntentPayload = token;

  -- 映射从来源原文抽出的、供核心与业务使用的字段集
  Fields = token;

  -- 集成交给核心的信封：固定头加来源原文
  Envelope ::
    source : SourceId    -- 来自哪个来源
    stream : StreamName  -- 属于来源的哪条流
    raw    : RawPayload; -- 来源原文，原样保存

  -- 映射解码的结果：核心需要的字段，其余留在原文
  Decoded ::
    venueSeq   : [nat]           -- 来源连续序号，流声明带序号时必有
    instrument : [InstrumentId]  -- 记录所属品种
    clientKey  : [ClientKey]     -- 记录回声的调用方键，用于归因到本地写尝试
    fields     : Fields;         -- 其余抽出的字段

  -- 解码结局：解出，或当前映射解不出
  DecodeOutcome = Decoded | <Unmapped>;

  -- 映射编码的结果：写通道与交给来源的请求原文
  Encoded ::
    lane    : LaneKey     -- 本次写所在通道
    request : RawPayload; -- 交给来源的请求

  -- 编码结局：编出，或当前映射编不出
  EncodeOutcome = Encoded | <Unmapped>;

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

operations
  Install: SourceId * MappingId * map CapabilityKey to Verdict ==> ()
  Install(source, mapping, caps) == is not yet specified;

  Decode: Envelope ==> DecodeOutcome
  Decode(env) == is not yet specified;

  Encode: SourceId * IntentPayload ==> EncodeOutcome
  Encode(source, intent) == is not yet specified;

  ClassifyEvidence: SourceId * RawPayload ==> Evidence
  ClassifyEvidence(source, raw) == is not yet specified;

  CurrentCapability: SourceId * CapabilityKey ==> CapabilityStatus
  CurrentCapability(source, cap) == is not yet specified;
end Mapping
```
