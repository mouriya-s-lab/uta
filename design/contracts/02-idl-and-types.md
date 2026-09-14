# 契约 02：proto3 IDL 与 Rust 类型草图

本文件只编码 `contracts/01-operations.md` 的 A-01–A-11、B-01–B-14。每个 RPC 名称带操作编号；`oneof` 是对应操作结果 ADT，所有分支都在 01 中列出，没有通用结果分支。认证身份来自传输上下文，不能由请求体字段覆盖。

## 0. 编码边界

- `AccountId`、`aliceId`、`client key`、`broker 原生 id` 是不透明标识；这里使用 `string value` 作为跨语言承载，不改变其比较语义。
- `Generation` 与 `Seq` 是流内位置；不从它们推导跨流总序。
- v2 未定义的载荷消息保留为命名消息；当前契约已为 §2.12 裁决的载荷、键、游标、通知和健康字段给出精确结构。没有用通用 `status`、通用 `error` 或未列举分支掩盖缺口。
- 传输断开、没有收到调用许可确认、进程启动拒绝不是业务 `oneof` 的隐含分支；其可观察边界在 01 的对应操作中说明。
- 下列 IDL 与 01 的操作编号、方向和结果 ADT 一一对应；传输认证载荷与进程退出码不在 IDL 内选择。

## 1. proto3 IDL

```proto
syntax = "proto3";

package uta.v2.contracts;

import "google/protobuf/timestamp.proto";

// ---------- 基本标识与流 ----------

message Empty {}

message AccountId { string value = 1; }
message ClientKey { string value = 1; }
message AliceId { string value = 1; }
// A broker-native identifier is mandatory wherever the broker has assigned one.
message BrokerNativeId { string value = 1; }
message IntegrationSessionNumber { string value = 1; }
message InstructionId {
  AccountId account_id = 1;
  ClientKey client_key = 2;
}
message TryNumber { uint64 value = 1; }
message Generation { uint64 value = 1; }
message Seq { uint64 value = 1; }
message IntegrationCursor { bytes opaque = 1; }
message RequestSummary { bytes sha256 = 1; }
message DataTime { google.protobuf.Timestamp value = 1; }

message SourceId {
  oneof source {
    AccountId account = 1;
    PublicFeed public_feed = 2;
  }
  FlowKind flow_kind = 3;
}
message PublicFeed { string broker = 1; }
message FlowRef {
  SourceId source = 1;
  Generation generation = 2;
}
// Client cursor: exactly (flow, generation, seq). It is not an integration cursor.
message FlowCursor {
  FlowRef flow = 1;
  Seq seq = 2;
}

message FlowKind {
  oneof value {
    MarketFlow market = 1;
    AccountChangeFlow account_change = 2;
  }
}
message MarketFlow {}
message AccountChangeFlow {}

message GapReason {
  oneof value {
    DisconnectWithoutCursor disconnect_without_cursor = 1;
    QuotaRevoked quota_revoked = 2;
    InputOverflow input_overflow = 3;
  }
}
message DisconnectWithoutCursor {}
message QuotaRevoked {}
message InputOverflow {}

// ---------- 指令载荷：以 broker.ts 的 Contract / Order 为起点 ----------

message ExtensionScalar {
  oneof value {
    string string_value = 1;
    bool bool_value = 2;
    sint64 integer_value = 3;
    string decimal_value = 4;
  }
}
message TypedExtensions { map<string, ExtensionScalar> fields = 1; }
message RawEvidenceSummary {
  bytes sha256 = 1;
  optional string media_type = 2;
  optional uint64 byte_length = 3;
}

message ContractLeg {
  optional BrokerNativeId broker_native_id = 1;
  optional string numeric_con_id = 2; // normalized integer string
  optional string ratio = 3; // decimal string
  optional string action = 4;
  optional string exchange = 5;
  optional string open_close = 6;
  optional string short_sales_slot = 7;
  optional string designated_location = 8;
  optional string exempt_code = 9;
  optional string per_leg_price = 10; // decimal string
  TypedExtensions extensions = 11;
  RawEvidenceSummary raw_evidence_summary = 12;
}
message Contract {
  // Optional on transport; required once the broker has assigned an opaque id.
  optional BrokerNativeId broker_native_id = 1;
  optional AliceId alice_id = 2;
  optional string numeric_con_id = 3; // normalized integer string
  optional string symbol = 4;
  optional string sec_type = 5;
  optional string last_trade_date_or_contract_month = 6;
  optional string strike = 7;       // decimal string
  optional string right = 8;
  optional string multiplier = 9;   // decimal string
  optional string exchange = 10;
  optional string primary_exchange = 11;
  optional string currency = 12;
  optional string local_symbol = 13;
  optional string trading_class = 14;
  optional string sec_id_type = 15;
  optional string sec_id = 16;
  optional string description = 17;
  optional string issuer_id = 18;
  optional string combo_legs_description = 19;
  optional string last_trade_date = 20;
  repeated ContractLeg combo_legs = 21;
  TypedExtensions extensions = 22;
  RawEvidenceSummary raw_evidence_summary = 23;
}
message IneligibilityReason {
  optional string id = 1;
  optional string description = 2;
}
message ContractDescription {
  Contract contract = 1;
  repeated string derivative_sec_types = 2;
  TypedExtensions extensions = 3;
  RawEvidenceSummary raw_evidence_summary = 4;
}
message ContractDetails {
  optional string market_name = 1;
  optional string min_tick = 2; // decimal string
  optional string order_types = 3;
  optional string valid_exchanges = 4;
  optional string price_magnifier = 5; // decimal string
  optional string numeric_under_con_id = 6;
  optional string long_name = 7;
  optional string contract_month = 8;
  optional string industry = 9;
  optional string category = 10;
  optional string subcategory = 11;
  optional string time_zone_id = 12;
  optional string trading_hours = 13;
  optional string liquid_hours = 14;
  optional string ev_rule = 15;
  optional string ev_multiplier = 16; // decimal string
  map<string, string> sec_id_list = 17;
  optional string agg_group = 18;
  optional string under_symbol = 19;
  optional string under_sec_type = 20;
  optional string market_rule_ids = 21;
  optional string real_expiration_date = 22;
  optional string stock_type = 23;
  optional string min_size = 24; // decimal string
  optional string size_increment = 25; // decimal string
  optional string suggested_size_increment = 26; // decimal string
  optional string fund_name = 27;
  optional string fund_family = 28;
  optional string fund_type = 29;
  optional string fund_front_load = 30; // decimal string
  optional string fund_back_load = 31; // decimal string
  optional string fund_back_load_time_interval = 32;
  optional string fund_management_fee = 33; // decimal string
  optional bool fund_closed = 34;
  optional bool fund_closed_for_new_investors = 35;
  optional bool fund_closed_for_new_money = 36;
  optional string fund_notify_amount = 37; // decimal string
  optional string fund_minimum_initial_purchase = 38; // decimal string
  optional string fund_minimum_subsequent_purchase = 39; // decimal string
  optional string fund_blue_sky_states = 40;
  optional string fund_blue_sky_territories = 41;
  optional string fund_distribution_policy_indicator = 42;
  optional string fund_asset_type = 43;
  optional string cusip = 44;
  optional string issue_date = 45;
  optional string ratings = 46;
  optional string bond_type = 47;
  optional string coupon = 48; // decimal string
  optional string coupon_type = 49;
  optional bool convertible = 50;
  optional bool callable = 51;
  optional bool puttable = 52;
  optional string desc_append = 53;
  optional string next_option_date = 54;
  optional string next_option_type = 55;
  optional bool next_option_partial = 56;
  optional string bond_notes = 57;
  repeated IneligibilityReason ineligibility_reasons = 58;
  optional string event_contract_1 = 59;
  optional string event_contract_description_1 = 60;
  optional string event_contract_description_2 = 61;
  optional string min_algo_size = 62; // decimal string
  optional string last_price_precision = 63; // decimal string
  optional string last_size_precision = 64; // decimal string
  TypedExtensions extensions = 65;
  RawEvidenceSummary raw_evidence_summary = 66;
}
message OrderRequest {
  Contract contract = 1;
  optional string action = 2;
  optional string total_quantity = 3; // decimal string
  optional string display_size = 4;   // decimal string
  optional string order_type = 5;
  optional string limit_price = 6;    // decimal string
  optional string aux_price = 7;      // decimal string
  optional string tif = 8;
  optional string account = 9;
  optional string order_ref = 10;
  optional string good_after_time = 11;
  optional string good_till_date = 12;
  optional string trail_stop_price = 13; // decimal string
  optional string cash_quantity = 14;    // decimal string
  optional bool outside_rth = 15;
  optional bool hidden = 16;
  optional bool all_or_none = 17;
  optional bool post_only = 18;
  optional string trailing_percent = 19; // decimal string
  optional string parent_id = 20; // normalized integer string
  optional string oca_group = 21;
  optional string numeric_client_id = 22;
  optional string numeric_order_id = 23;
  optional string numeric_perm_id = 24;
  optional string numeric_parent_id = 25;
  optional string filled_quantity = 26; // decimal string
  TypedExtensions extensions = 27;
  RawEvidenceSummary raw_evidence_summary = 28;
}
message OrderChanges {
  optional string action = 1;
  optional string total_quantity = 2; // decimal string
  optional string order_type = 3;
  optional string limit_price = 4; // decimal string
  optional string aux_price = 5; // decimal string
  optional string tif = 6;
  optional string good_after_time = 7;
  optional string good_till_date = 8;
  optional string trail_stop_price = 9; // decimal string
  optional string cash_quantity = 10; // decimal string
  optional bool outside_rth = 11;
  optional bool hidden = 12;
  optional bool post_only = 13;
  optional string trailing_percent = 14; // decimal string
  optional string display_size = 15; // decimal string
  optional bool all_or_none = 16;
  optional string parent_id = 17; // normalized integer string
  optional string oca_group = 18;
  TypedExtensions extensions = 19;
  RawEvidenceSummary raw_evidence_summary = 20;
}
message OrderCancel {
  optional string manual_order_cancel_time = 1;
  optional string ext_operator = 2;
  optional string manual_order_indicator = 3; // normalized integer string
}
message PositionRisk {
  optional string leverage = 1;          // decimal string
  optional string liquidation_price = 2; // decimal string
  optional string margin_mode = 3;
  TypedExtensions extensions = 4;
  RawEvidenceSummary raw_evidence_summary = 5;
}
message Position {
  Contract contract = 1;
  string currency = 2;
  string side = 3;
  string quantity = 4;       // decimal string
  string avg_cost = 5;       // decimal string
  string market_price = 6;   // decimal string
  string market_value = 7;   // decimal string
  string unrealized_pnl = 8;  // decimal string
  string realized_pnl = 9;    // decimal string
  string multiplier = 10;     // decimal string
  optional string avg_cost_source = 11;
  PositionRisk risk = 12;
  TypedExtensions extensions = 13;
  RawEvidenceSummary raw_evidence_summary = 14;
}
message OrderAllocation {
  optional string account = 1;
  optional string position = 2; // decimal string
  optional string position_desired = 3; // decimal string
  optional string position_after = 4; // decimal string
  optional string desired_alloc_qty = 5; // decimal string
  optional string allowed_alloc_qty = 6; // decimal string
  optional bool is_monetary = 7;
}
message OrderState {
  optional string status = 1;
  optional string init_margin_before = 2; // decimal string
  optional string maint_margin_before = 3; // decimal string
  optional string equity_with_loan_before = 4; // decimal string
  optional string init_margin_change = 5; // decimal string
  optional string maint_margin_change = 6; // decimal string
  optional string equity_with_loan_change = 7; // decimal string
  optional string init_margin_after = 8; // decimal string
  optional string maint_margin_after = 9; // decimal string
  optional string equity_with_loan_after = 10; // decimal string
  optional string commission_and_fees = 11; // decimal string
  optional string min_commission_and_fees = 12; // decimal string
  optional string max_commission_and_fees = 13; // decimal string
  optional string commission_and_fees_currency = 14;
  optional string margin_currency = 15;
  optional string init_margin_before_outside_rth = 16; // decimal string
  optional string maint_margin_before_outside_rth = 17; // decimal string
  optional string equity_with_loan_before_outside_rth = 18; // decimal string
  optional string init_margin_change_outside_rth = 19; // decimal string
  optional string maint_margin_change_outside_rth = 20; // decimal string
  optional string equity_with_loan_change_outside_rth = 21; // decimal string
  optional string init_margin_after_outside_rth = 22; // decimal string
  optional string maint_margin_after_outside_rth = 23; // decimal string
  optional string equity_with_loan_after_outside_rth = 24; // decimal string
  optional string suggested_size = 25; // decimal string
  optional string reject_reason = 26;
  repeated OrderAllocation order_allocations = 27;
  optional string warning_text = 28;
  optional string completed_time = 29;
  optional string completed_status = 30;
  TypedExtensions extensions = 31;
  RawEvidenceSummary raw_evidence_summary = 32;
}
message TakeProfitStopLoss {
  optional string take_profit_price = 1; // decimal string
  optional string stop_loss_price = 2; // decimal string
  optional string stop_loss_limit_price = 3; // decimal string
}
message PlaceOrderLeg {
  optional BrokerNativeId broker_native_id = 1;
  optional string kind = 2;
  TypedExtensions extensions = 3;
  RawEvidenceSummary raw_evidence_summary = 4;
}
message BrokerPlacementResult { // broker.ts PlaceOrderResult
  bool success = 1;
  optional BrokerNativeId broker_native_id = 2;
  optional string error = 3;
  optional string message = 4;
  ExecutionReceipt execution = 5;
  OrderState order_state = 6;
  repeated PlaceOrderLeg legs = 7;
  optional string filled_quantity = 8; // decimal string；状态回票累计成交量
  optional string filled_price = 9; // decimal string；状态回票平均成交价
  TypedExtensions extensions = 10;
  RawEvidenceSummary raw_evidence_summary = 11;
  optional string action = 12;
  optional string symbol = 13;
}
message OpenOrder {
  optional BrokerNativeId broker_native_id = 1;
  Contract contract = 2;
  OrderRequest order = 3;
  OrderState order_state = 4;
  optional string avg_fill_price = 5; // decimal string
  TakeProfitStopLoss tpsl = 6;
  TypedExtensions extensions = 9;
  RawEvidenceSummary raw_evidence_summary = 10;
}

message InstructionKind {
  oneof value {
    EntrustKind entrust = 1;
    ModifyKind modify = 2;
    CancelKind cancel = 3;
    CloseKind close = 4;
  }
}
message EntrustKind {}
message ModifyKind {}
message CancelKind {}
message CloseKind {}

message InstructionPayload {
  oneof value {
    EntrustPayload entrust = 1;
    ModifyPayload modify = 2;
    CancelPayload cancel = 3;
    ClosePayload close = 4;
  }
}
message EntrustPayload {
  OrderRequest order = 1;
  TakeProfitStopLoss tpsl = 2;
}
message ModifyPayload {
  oneof target {
    InstructionId instruction_id = 1;
    BrokerNativeId broker_native_id = 2;
  }
  OrderChanges changes = 3;
}
message CancelPayload {
  oneof target {
    InstructionId instruction_id = 1;
    BrokerNativeId broker_native_id = 2;
  }
  OrderCancel order_cancel = 3;
}
message ClosePayload {
  Contract contract = 1;
  optional string quantity = 2; // decimal string; absent means full position
  optional string sub_account_id = 3;
}
message Instruction {
  InstructionId id = 1;
  AccountId account_id = 2;
  ClientKey client_key = 3;
  InstructionPayload payload = 4;
  optional string sub_account_id = 5;
}
message AuthorizedInstruction {
  Instruction instruction = 1;
  TryNumber try_number = 2;
}

// ---------- 能力声明 ----------

message Supported {}
message Unsupported {}
message Support {
  oneof value {
    Supported supported = 1;
    Unsupported unsupported = 2;
  }
}

message ClientKeySemantics {
  oneof value {
    DocumentedClientKey documented = 1;
    NoDocumentedClientKey no_documented = 2;
  }
}
message DocumentedClientKey {}
message NoDocumentedClientKey {}

message QueryChannel {
  oneof value {
    ClientKeyReadback client_key_readback = 1;
    UnfilledList unfilled_list = 2;
    TradePositionReconciliation trade_position_reconciliation = 3;
  }
}
message ClientKeyReadback {}
message UnfilledList {}
message TradePositionReconciliation {}
message TrustedConclusion {
  oneof value {
    Trusted trusted = 1;
    NotTrusted not_trusted = 2;
  }
}
message Trusted {}
message NotTrusted {}
message QueryChannelCapability {
  QueryChannel channel = 1;
  TrustedConclusion conclusion = 2;
}
message FieldCapability {
  string name = 1;
  Support support = 2;
}
message ExecutionReceiptCapability {
  // 逐笔成交：有 / 无；无表示逐笔成交去重能力不可用，不得提交成交回票。
  oneof value {
    TradeByTradeAvailable available = 1;
    NoTradeByTrade no_trade_by_trade = 2;
  }
}
message TradeByTradeAvailable {}
message NoTradeByTrade {}
message InstructionCapability {
  InstructionKind kind = 1;
  Support support = 2;
  ClientKeySemantics client_key_semantics = 3;
  repeated QueryChannelCapability query_channels = 4;
  repeated FieldCapability result_fields = 5;
  ExecutionReceiptCapability execution_receipts = 6;
}

message ReadKind {
  oneof value {
    AccountRead account = 1;
    PositionsRead positions = 2;
    OrdersRead orders = 3;
    QuotesRead quotes = 4;
    DerivativesDiscoveryRead derivatives_discovery = 5;
    DepthRead depth = 6;
    MarketClockRead market_clock = 7;
    HistoricalBarsRead historical_bars = 8;
    ContractSearchRead contract_search = 9;
    ContractDetailRead contract_detail = 10;
    WalletRead wallet = 11;
    HistoricalRecordsRead historical_records = 12;
  }
}
message AccountRead {}
message PositionsRead {}
message OrdersRead {}
message QuotesRead {}
message DerivativesDiscoveryRead {}
message DepthRead {}
message MarketClockRead {}
message HistoricalBarsRead {}
message ContractSearchRead {}
message ContractDetailRead {}
message WalletRead {}
message HistoricalRecordsRead {}
message ReturnFields { repeated string name = 1; }
message ReadCapability {
  ReadKind kind = 1;
  Support support = 2;
  ReturnFields return_fields = 3;
  repeated FieldCapability fields = 4;
}

message StreamKind {
  oneof value {
    MarketStream market = 1;
    AccountChangeStream account_change = 2;
  }
}
message MarketStream {}
message AccountChangeStream {}
message ContinuationCursor {
  oneof value {
    TrustedCursor trusted_cursor = 1;
    NoTrustedCursor no_trusted_cursor = 2;
  }
}
message TrustedCursor {}
message NoTrustedCursor {}
message Quota {}
message PauseSupport {
  oneof value {
    Pausable pausable = 1;
    NotPausable not_pausable = 2;
  }
}
message Pausable {}
message NotPausable {}
message StreamCapability {
  StreamKind kind = 1;
  Support support = 2;
  ContinuationCursor continuation = 3;
  Quota quota = 4;
  PauseSupport pause = 5;
}
message Pacing {}
message HistoricalBarCapability {
  Support support = 1;
  Pacing pacing = 2;
}
message SubAccountEnumeration {
  oneof value {
    Enumerable enumerable = 1;
    NotEnumerable not_enumerable = 2;
  }
}
message Enumerable {}
message NotEnumerable {}
message StatusMappingVersion {
  // Value is the integration artifact version string.
  string value = 1;
}
message CapabilityDeclarationVersion { string value = 1; }
message CapabilityDeclaration {
  repeated InstructionCapability instruction_capabilities = 1;
  repeated ReadCapability read_capabilities = 2;
  repeated StreamCapability stream_capabilities = 3;
  HistoricalBarCapability historical_bars = 4;
  SubAccountEnumeration sub_accounts = 5;
  StatusMappingVersion status_mapping_version = 6;
  CapabilityDeclarationVersion declaration_version = 7;
}

// ---------- A-01 至 A-03 ----------

message IntegrationSessionRequest {
  IntegrationSessionNumber session_number = 1;
  CapabilityDeclaration capabilities = 2;
}
message IntegrationSessionResult {
  oneof value {
    IntegrationSessionAccepted accepted = 1;
    IntegrationSessionCapabilityIncomplete capability_incomplete = 2;
    IntegrationSessionUnauthorized unauthorized = 3;
  }
}
message IntegrationSessionAccepted { IntegrationSessionNumber session_number = 1; }
message IntegrationSessionCapabilityIncomplete {}
message IntegrationSessionUnauthorized {}
message InstructionReceivedResult {
  oneof value {
    InstructionReceived received = 1;
  }
}
message InstructionReceived {}
message CallPermitRequest {
  InstructionId instruction_id = 1;
  TryNumber try_number = 2;
  AccountId account_id = 3;
  IntegrationSessionNumber session_number = 4;
  RequestSummary request_summary = 5;
}
message CallPermitResult {
  oneof value {
    CallPermitConfirmed confirmed = 1;
    SummaryMismatchRejected summary_mismatch = 2;
    PermitAlreadyRecordedRejected already_recorded = 3;
  }
}
message CallPermitConfirmed { IntegrationSessionNumber session_number = 1; }
message SummaryMismatchRejected {}
message PermitAlreadyRecordedRejected {}

// ---------- A-04 至 A-06 ----------

message ReceiptState {
  oneof value {
    AcceptedReceipt accepted = 1;
    PartiallyFilledReceipt partially_filled = 2;
    FilledReceipt filled = 3;
    RejectedReceipt rejected = 4;
    CanceledReceipt canceled = 5;
    BrokerSpecificReceipt broker_specific = 6;
    UnmappedBrokerStatus unmapped = 7;
  }
}
message AcceptedReceipt {}
message PartiallyFilledReceipt {}
message FilledReceipt {}
message RejectedReceipt {}
message CanceledReceipt {}
message BrokerSpecificReceipt { string original_status = 1; }
message UnmappedBrokerStatus { string original_status = 1; }
message StateReceiptFingerprint {
  // 只含 broker 原生 id、状态原值、累计成交量，不含时间戳；在提交账户范围内比较。
  BrokerNativeId broker_native_id = 1;
  string original_status = 2;
  string cumulative_quantity = 3; // decimal string
}
message OrderStateReceipt {
  optional BrokerNativeId broker_native_id = 1;
  optional ClientKey client_key = 2;
  optional string original_status = 3;
  ReceiptState mapped_state = 4;
  optional string cumulative_quantity = 5; // decimal string
  optional string remaining = 6; // decimal string
  optional string average_fill_price = 7; // decimal string
  optional string numeric_order_id = 8; // decimal string
  optional string numeric_perm_id = 9; // decimal string
  optional string numeric_parent_id = 10; // decimal string
  optional string last_fill_price = 11; // decimal string
  optional string numeric_client_id = 12; // decimal string
  optional string why_held = 13;
  optional string market_cap_price = 14; // decimal string
  optional DataTime broker_time = 15;
  optional OrderState order_state = 16;
  oneof dedupe_key {
    string broker_event_id = 17;
    StateReceiptFingerprint event_fingerprint = 18;
  }
  optional bytes original_payload_summary = 19;
  TypedExtensions extensions = 20;
  RawEvidenceSummary raw_evidence_summary = 21;
}
message ExecutionReceipt {
  optional BrokerNativeId broker_native_id = 1;
  optional ClientKey client_key = 2;
  string broker_execution_id = 3; // 必须；缺失时 typed 拒绝
  optional string numeric_order_id = 4; // decimal string
  optional string broker_time = 5;
  optional string acct_number = 6;
  optional string exchange = 7;
  optional string side = 8;
  optional string single_quantity = 9; // decimal string; 单笔数量
  optional string single_price = 10; // decimal string; 单笔价格
  optional string numeric_perm_id = 11; // decimal string
  optional string numeric_client_id = 12; // decimal string
  optional bool is_liquidation = 13;
  optional string cumulative_quantity = 14; // decimal string
  optional string average_price = 15; // decimal string
  optional string order_ref = 16;
  optional string ev_rule = 17;
  optional string ev_multiplier = 18; // decimal string
  optional string model_code = 19;
  optional string last_liquidity = 20; // normalized integer string
  optional bool is_price_revision_pending = 21;
  optional string submitter = 22;
  optional string opt_exercise_or_lapse_type = 23; // normalized integer string
  optional string commission_and_fees = 24; // decimal string
  optional string commission_currency = 25;
  optional string realized_pnl = 26; // decimal string
  optional string bond_yield = 27; // decimal string
  optional string yield_redemption_date = 28;
  TypedExtensions extensions = 29;
  RawEvidenceSummary raw_evidence_summary = 30;
}
message Receipt {
  oneof value {
    OrderStateReceipt state = 1;
    ExecutionReceipt execution = 2;
  }
}
message CommissionSupplement {
  string broker_execution_id = 1;
  optional string commission_and_fees = 2; // decimal string
  optional string commission_currency = 3;
  optional string realized_pnl = 4; // decimal string
  optional string bond_yield = 5; // decimal string
  optional string yield_redemption_date = 6;
  TypedExtensions extensions = 7;
  RawEvidenceSummary raw_evidence_summary = 8;
}
message ExecutionConflict {
  string broker_execution_id = 1;
}
message ReceiptSubmission { Receipt receipt = 1; }
message LinkedReceipt { InstructionId instruction_id = 1; }
message ExternalChangeRecorded {}
message DuplicateStateReceiptIgnored {}
message DuplicateExecutionReceiptIgnored {}
message CommissionSupplementRecorded {}
message ExecutionConflictRecorded {}
message MissingStateReceiptDedupeKeyRejected {}
message MissingExecutionIdRejected {}
message ExecutionReceiptUnsupportedRejected {}
message ReceiptPayloadMissingRejected {}
message ReceiptSubmissionResult {
  oneof value {
    LinkedReceipt linked = 1;
    ExternalChangeRecorded external_change = 2;
    DuplicateStateReceiptIgnored duplicate_state = 3;
    DuplicateExecutionReceiptIgnored duplicate_execution = 4;
    CommissionSupplementRecorded commission_supplement = 5;
    ExecutionConflictRecorded execution_conflict = 6;
    MissingStateReceiptDedupeKeyRejected missing_state_dedupe_key = 7;
    MissingExecutionIdRejected missing_execution_id = 8;
    ExecutionReceiptUnsupportedRejected execution_unsupported = 9;
    ReceiptPayloadMissingRejected payload_missing = 10;
  }
}

message PositionsResponse { repeated Position positions = 1; }
message OrdersResponse { repeated OpenOrder orders = 1; }
message AccountResponse { AccountInfo account = 1; }
message ReadbackResponse { ReadPayload payload = 1; }
message ReconciliationKind {
  oneof value {
    PositionsResponse positions = 1;
    OrdersResponse orders = 2;
    AccountResponse account = 3;
    ReadbackResponse readback = 4;
  }
}
message ReconciliationPayload {
  oneof value {
    PositionsResponse positions = 1;
    OrdersResponse orders = 2;
    AccountResponse account = 3;
    ReadbackResponse readback = 4;
  }
}
message ReconciliationKey {
  AccountId account_id = 1;
  ReconciliationKind kind = 2;
  IntegrationSessionNumber session_number = 3;
  uint64 integration_seq = 4;
}
message ReconciliationResponse {
  ReconciliationKey key = 1;
  AccountId account_id = 2;
  DataTime data_time = 3;
  ReconciliationKind kind = 4;
  ReconciliationPayload payload = 5;
  optional InstructionId instruction_id = 6;
  optional TryNumber try_number = 7;
  optional BrokerNativeId broker_native_id = 8;
  optional ClientKey client_key = 9;
}
message ReconciliationSubmission { ReconciliationResponse response = 1; }
message ReconciliationRecorded {}
message DuplicateReconciliationIgnored {}
message ReconciliationSubmissionResult {
  oneof value {
    ReconciliationRecorded recorded = 1;
    DuplicateReconciliationIgnored duplicate = 2;
  }
}
message ExternalChange {
  AccountId account_id = 1;
  OrderStateReceipt state_receipt = 2;
  ExternalChangeReason reason = 3;
}
message ExternalChangeReason {
  oneof value {
    NoInstructionMatch no_instruction_match = 1;
    InstructionAlreadyTerminal instruction_already_terminal = 2;
    UndecidedInstructionPresent undecided_instruction_present = 3;
  }
}
message NoInstructionMatch {}
message InstructionAlreadyTerminal {}
message UndecidedInstructionPresent {}
message ExternalChangeSubmission { ExternalChange change = 1; }
message ExternalChangeAccepted {}
message MissingExternalChangeDedupeKeyRejected {}
message ExternalChangeSubmissionResult {
  oneof value {
    ExternalChangeAccepted recorded = 1;
    MissingExternalChangeDedupeKeyRejected missing_dedupe_key = 2;
  }
}

// ---------- A-07 至 A-08 ----------

message MarketRecord {
  AliceId alice_id = 1;
  bytes record_fields = 2;
  DataTime data_time = 3;
  QualityFreshness quality_freshness = 4;
}
message MarketRecordSubmission {
  SourceId source = 1;
  Generation generation = 2;
  Seq seq = 3;
  MarketRecord record = 4;
}
message MarketRecordAccepted {}
message MarketRecordSubmissionResult {
  oneof value {
    MarketRecordAccepted recorded = 1;
  }
}
message ReconnectReport {
  SourceId source = 1;
  oneof value {
    CanContinue can_continue = 2;
    CannotContinue cannot_continue = 3;
  }
}
message CanContinue { IntegrationCursor cursor = 1; }
message CannotContinue {
  Generation previous_generation = 1;
  Seq previous_last_seq = 2;
  GapReason reason = 3;
}
message ReconnectReportAccepted {}
message ReconnectReportResult {
  oneof value {
    ReconnectReportAccepted recorded = 1;
  }
}

// ---------- A-09 ----------

message InstructionFeatures {
  AliceId instrument = 1;
  string direction = 2;
  string quantity = 3;     // decimal string
  string price = 4;        // decimal string
  string order_type = 5;
  DataTime submitted_time = 6;
}
message UnknownQueryChannel {
  oneof value {
    ClientKeyQuery client_key = 1;
    UnfilledListQuery unfilled_list = 2;
    TradePositionQuery trade_position = 3;
  }
}
message ClientKeyQuery { ClientKey client_key = 1; }
message UnfilledListQuery {
  BrokerNativeId broker_native_id = 1;
  InstructionFeatures instruction_features = 2;
}
message TradePositionQuery { InstructionFeatures instruction_features = 1; }
message UnknownQueryRequest {
  InstructionId instruction_id = 1;
  TryNumber try_number = 2;
  AccountId account_id = 3;
  UnknownQueryChannel channel = 4;
}
message QueryFoundReceipt { Receipt receipt = 1; }
message QueryProvedNotDelivered {}
message QueryNoConclusion {}
message UnknownQueryResult {
  oneof value {
    QueryFoundReceipt found_receipt = 1;
    QueryProvedNotDelivered proved_not_delivered = 2;
    QueryNoConclusion no_conclusion = 3;
  }
}

// ---------- A-10 与 B-02 ----------

message BarQuery {
  string interval = 1;
  optional DataTime start = 2;
  optional DataTime end = 3;
  optional uint64 limit = 4;
  optional string what_to_show = 5;
}
message ReadQuery {
  optional AliceId alice_id = 1;
  optional AccountId account_id = 2;
  optional string sub_account_id = 3;
  optional Contract contract = 4;
  optional string pattern = 5;
  optional BarQuery historical_bars = 6;
}
message ReadTarget {
  oneof value {
    ExactSource exact_source = 1;
    ProviderPrefix provider_prefix = 2;
    AllAccounts all_accounts = 3;
    TradingOnly trading_only = 4;
  }
}
message ExactSource { string value = 1; }
message ProviderPrefix { string value = 1; }
message AllAccounts {}
message TradingOnly {}
message ReadForwardRequest {
  AccountId account_id = 1;
  ReadKind kind = 2;
  ReadQuery query = 3;
}
message AccountInfo {
  string base_currency = 1;
  string net_liquidation = 2; // decimal string
  string total_cash_value = 3; // decimal string
  string unrealized_pnl = 4; // decimal string
  optional string realized_pnl = 5; // decimal string
  optional string buying_power = 6; // decimal string
  optional string init_margin_req = 7; // decimal string
  optional string maint_margin_req = 8; // decimal string
  optional string day_trades_remaining = 9; // normalized integer string
  TypedExtensions extensions = 10;
  RawEvidenceSummary raw_evidence_summary = 11;
}
message SubAccountRef {
  string id = 1;
  string label = 2;
  string kind = 3;
  TypedExtensions extensions = 4;
  RawEvidenceSummary raw_evidence_summary = 5;
}
message Quote {
  Contract contract = 1;
  string last = 2;   // decimal string
  string bid = 3;    // decimal string
  string ask = 4;    // decimal string
  string volume = 5; // decimal string
  optional string high = 6; // decimal string
  optional string low = 7; // decimal string
  DataTime timestamp = 8;
  TypedExtensions extensions = 9;
  RawEvidenceSummary raw_evidence_summary = 10;
}
message MarketClock {
  bool is_open = 1;
  optional DataTime next_open = 2;
  optional DataTime next_close = 3;
  optional DataTime timestamp = 4;
  TypedExtensions extensions = 5;
  RawEvidenceSummary raw_evidence_summary = 6;
}
message Bar {
  DataTime timestamp = 1;
  string open = 2;   // decimal string
  string high = 3;   // decimal string
  string low = 4;    // decimal string
  string close = 5;  // decimal string
  string volume = 6; // decimal string
  TypedExtensions extensions = 7;
  RawEvidenceSummary raw_evidence_summary = 8;
}
message OrderBookLevel {
  string price = 1;  // decimal string
  string amount = 2; // decimal string
}
message OrderBook {
  Contract contract = 1;
  repeated OrderBookLevel bids = 2;
  repeated OrderBookLevel asks = 3;
  DataTime timestamp = 4;
  TypedExtensions extensions = 5;
  RawEvidenceSummary raw_evidence_summary = 6;
}
message HistoryContract {
  optional string alice_id = 1;
  optional string symbol = 2;
  optional string local_symbol = 3;
  optional string sec_type = 4;
  optional string currency = 5;
  optional string exchange = 6;
  optional string expiry = 7;
  optional string strike = 8; // decimal string
  optional string right = 9;
  optional string multiplier = 10; // decimal string
}
message OrderHistoryEntry {
  optional BrokerNativeId broker_native_id = 1;
  DataTime timestamp = 2;
  optional DataTime resolved_at = 3;
  HistoryContract contract = 4;
  string side = 5;
  optional string order_type = 6;
  optional string quantity = 7; // decimal string
  optional string limit_price = 8; // decimal string
  optional string stop_price = 9; // decimal string
  string status = 10;
  optional string filled_quantity = 11; // decimal string
  optional string average_fill_price = 12; // decimal string
  string source = 13;
  string commit_hash = 14;
  string message = 15;
  optional string error = 16;
  TypedExtensions extensions = 17;
  RawEvidenceSummary raw_evidence_summary = 18;
}
message TradeHistoryEntry {
  DataTime timestamp = 1;
  optional BrokerNativeId broker_native_id = 2;
  HistoryContract contract = 3;
  string side = 4;
  string quantity = 5; // decimal string
  string price = 6; // decimal string
  string value = 7; // decimal string
  string source = 8;
  string commit_hash = 9;
  TypedExtensions extensions = 10;
  RawEvidenceSummary raw_evidence_summary = 11;
}
message ReadPayload {
  oneof value {
    AccountData account = 1;
    PositionsData positions = 2;
    OrdersData orders = 3;
    QuotesData quotes = 4;
    DerivativesDiscoveryData derivatives_discovery = 5;
    DepthData depth = 6;
    MarketClockData market_clock = 7;
    HistoricalBarsData historical_bars = 8;
    ContractSearchData contract_search = 9;
    ContractDetailData contract_detail = 10;
    WalletData wallet = 11;
    HistoricalRecordsData historical_records = 12;
  }
}
message AccountData { AccountInfo account = 1; }
message PositionsData { repeated Position positions = 1; }
message OrdersData { repeated OpenOrder orders = 1; }
message QuotesData { repeated Quote quotes = 1; }
message DerivativesDiscoveryData { repeated Contract contracts = 1; }
message DepthData { repeated OrderBook books = 1; }
message MarketClockData { MarketClock clock = 1; }
message HistoricalBarsData { repeated Bar bars = 1; }
message ContractSearchData { repeated ContractDescription descriptions = 1; }
message ContractDetailData {
  Contract contract = 1;
  ContractDetails details = 2;
}
message WalletData { repeated SubAccountRef sub_accounts = 1; }
message HistoricalRecordsData {
  repeated OrderHistoryEntry order_history = 1;
  repeated TradeHistoryEntry trade_history = 2;
}
message ReadSuccess {
  AccountId account_id = 1;
  SourceId source = 2;
  DataTime data_time = 3;
  ReadPayload payload = 4;
  QualityFreshness quality_freshness = 5;
}
message ReadUnavailable {}
message ReadNotFound {}
message ReadUnsupported {}
message ReadTimeout {}
message ReadFailure {
  oneof value {
    ReadUnavailable unavailable = 1;
    ReadNotFound not_found = 2;
    ReadUnsupported unsupported = 3;
    ReadTimeout timeout = 4;
  }
}
message ReadForwardResult {
  oneof value {
    ReadSuccess success = 1;
    ReadFailure failure = 2;
  }
}
message AccountReadResult {
  oneof value {
    ReadSuccess success = 1;
    ReadFailure failure = 2;
  }
}
message ReadFanoutResult { repeated AccountReadResult accounts = 1; }

// ---------- A-11 ----------

message BackfillPageRequest {
  SourceId source = 1;
  Generation generation = 2;
  AliceId alice_id = 3;
  ReadQuery query = 4;
  DataTime window_start = 5;
  bytes page_cursor = 6;
}
message BackfillPage {
  repeated MarketRecord records = 1;
  bytes next_page_cursor = 2;
  bool last_page = 3;
}
message BackfillPageUnsupported {}
message BackfillPageResult {
  oneof value {
    BackfillPage page = 1;
    BackfillPageUnsupported unsupported = 2;
  }
}

// ---------- B-01 ----------

message ClientSessionRequest {}
message ClientSessionAccepted {}
message ClientSessionUnauthorized {}
message ClientSessionResult {
  oneof value {
    ClientSessionAccepted accepted = 1;
    ClientSessionUnauthorized unauthorized = 2;
  }
}

// ---------- B-03 至 B-07 ----------

message MarketSelector { AliceId alice_id = 1; }
message AccountSelector { AccountId account_id = 1; }
message SubscriptionSelector {
  oneof value {
    MarketSelector market = 1;
    AccountSelector account = 2;
  }
}
message BackfillRequest {
  ReadQuery query = 1;
  DataTime window_start = 2;
}
message SubscriptionRequest {
  SubscriptionSelector selector = 1;
  FlowKind flow_kind = 2;
  BackfillRequest backfill = 3;
  repeated FlowCursor cursors = 4;
}
message SubscriptionActive {}
message SubscriptionSuspendedState { string reason = 1; }
message SubscriptionStatus {
  oneof value {
    SubscriptionActive active = 1;
    SubscriptionSuspendedState suspended = 2;
  }
}
message SubscriptionAccepted { SubscriptionStatus status = 1; }
message SubscriptionRejectedUnsupported {}
message SubscriptionRejectedQuota {}
message SubscriptionResult {
  oneof value {
    SubscriptionAccepted accepted = 1;
    SubscriptionRejectedUnsupported unsupported = 2;
    SubscriptionRejectedQuota quota = 3;
  }
}
message UnsubscribeRequest { SubscriptionSelector selector = 1; }
message Unsubscribed {}
message UnsubscribeResult {
  oneof value {
    Unsubscribed accepted = 1;
  }
}
message DeliverFlowRequest {
  SubscriptionSelector selector = 1;
  repeated FlowCursor cursors = 2;
}
message DeliveryAccountTicket {
  oneof value {
    OrderStateReceipt state_receipt = 1;
    ExecutionReceipt execution_receipt = 2;
    CommissionSupplement commission_supplement = 3;
    ExecutionConflict execution_conflict = 4;
    ReconciliationResponse reconciliation = 5;
    ExternalChange external_change = 6;
    DeliveryProgress delivery_progress = 7;
  }
}
message DeliveryProgress {
  InstructionId instruction_id = 1;
  TryNumber try_number = 2;
  oneof value {
    DeliveredToIntegration delivered_to_integration = 3;
    CallPermitProgress call_permit = 4;
    UnknownProgress unknown = 5;
    NotDeliveredProgress not_delivered = 6;
  }
}
message DeliveredToIntegration {}
message CallPermitProgress {}
message UnknownProgress {}
message NotDeliveredProgress {}
message SourceGap {
  FlowRef flow = 1;
  Generation previous_generation = 2;
  Seq previous_last_seq = 3;
  GapReason reason = 4;
}
message DeliveryGap {
  FlowRef flow = 1;
  Seq from_seq = 2;
  Seq to_seq = 3;
  GapReason reason = 4;
  Generation previous_generation = 5;
  Seq previous_last_seq = 6;
}
message SubscriptionSuspendedNotice {
  FlowRef flow = 1;
  string reason = 2;
}
message SubscriptionResumedNotice { FlowRef flow = 1; }
message ReadyNotice { FlowRef flow = 1; }
message DeliveryEvent {
  FlowRef flow = 1;
  Seq seq = 2;
  oneof value {
    MarketRecord market_record = 3;
    DeliveryAccountTicket account_ticket = 4;
    DeliveryGap delivery_gap = 5;
    SubscriptionSuspendedNotice suspended = 6;
    SubscriptionResumedNotice resumed = 7;
    ReadyNotice ready = 8;
  }
}
message AckRequest { FlowCursor cursor = 1; }
message AckAccepted {}
message AckResult {
  oneof value {
    AckAccepted accepted = 1;
  }
}
message AckGapRequest {
  FlowRef flow = 1;
  Seq from_seq = 2;
  Seq to_seq = 3;
  GapReason reason = 4;
}
message AckGapAccepted {}
message AckGapResult {
  oneof value {
    AckGapAccepted accepted = 1;
  }
}

// ---------- B-08 至 B-10 ----------

message InstrumentWrongAccount {}
message QuantityZeroOrNaN {}
message ReadOnlyAccount {}
message PolicyDenied {}
message TargetResultUnknown {}
message CreationRejectionReason {
  oneof value {
    InstrumentWrongAccount instrument_wrong_account = 1;
    QuantityZeroOrNaN quantity_zero_or_nan = 2;
    ReadOnlyAccount read_only = 3;
    PolicyDenied policy_denied = 4;
    TargetResultUnknown target_result_unknown = 5;
  }
}
message InstructionSubmissionRequest {
  AccountId account_id = 1;
  ClientKey client_key = 2;
  InstructionPayload payload = 3;
}
message InstructionStored { InstructionId instruction_id = 1; }
message InstructionRejected {
  InstructionId instruction_id = 1;
  CreationRejectionReason reason = 2;
}
message InstructionSubmissionResult {
  oneof value {
    InstructionStored stored = 1;
    InstructionRejected rejected = 2;
  }
}
message Approve {}
message Deny {}
message DecisionToken { string value = 1; }
message DecisionRequest {
  InstructionId instruction_id = 1;
  uint64 expected_version = 2;
  DecisionToken decision_token = 3;
  oneof decision {
    Approve approve = 4;
    Deny deny = 5;
  }
}
message DecisionStored {
  InstructionId instruction_id = 1;
  DecisionToken decision_token = 2;
}
message DecisionConflict {}
message DecisionExpired {}
message DecisionTokenConflict {}
message DecisionPermissionDenied {}
message DecisionResult {
  oneof value {
    DecisionStored stored = 1;
    DecisionConflict conflict = 2;
    DecisionExpired expired = 3;
    DecisionTokenConflict token_conflict = 4;
    DecisionPermissionDenied permission_denied = 5;
  }
}
message DeliveredConclusion {
  BrokerNativeId broker_native_id = 1;
  string original_status = 2;
}
message HumanConclusion {
  oneof value {
    DeliveredConclusion delivered = 1;
    NotDeliveredConclusion not_delivered = 2;
  }
}
message NotDeliveredConclusion {}
message CredentialToken { string value = 1; }
message HumanCredentialRequest {
  InstructionId instruction_id = 1;
  TryNumber try_number = 2;
  CredentialToken credential_token = 3;
  HumanConclusion conclusion = 4;
  bytes basis = 5;
}
message HumanCredentialStored { CredentialToken credential_token = 1; }
message CredentialTokenConflict {}
message NotHuman {}
message HumanCredentialResult {
  oneof value {
    HumanCredentialStored stored = 1;
    CredentialTokenConflict token_conflict = 2;
    NotHuman not_human = 3;
  }
}

// ---------- B-11 至 B-14 ----------

message InstructionTicket { Instruction instruction = 1; }
message DecisionTicket { DecisionRequest decision = 1; }
message DeliveryProgressTicket { DeliveryProgress progress = 1; }
message StateReceiptTicket { OrderStateReceipt receipt = 1; }
message ExecutionReceiptTicket { ExecutionReceipt receipt = 1; }
message CommissionSupplementTicket { CommissionSupplement record = 1; }
message ExecutionConflictTicket { ExecutionConflict record = 1; }
message ReconciliationTicket { ReconciliationResponse response = 1; }
message ExternalChangeTicket { ExternalChange change = 1; }
message HumanCredentialTicket { HumanCredentialRequest credential = 1; }
message HumanReceiptInconsistency {
  InstructionId instruction_id = 1;
  CredentialToken credential_token = 2;
  BrokerNativeId broker_native_id = 3;
  string original_status = 4;
}
message HumanReceiptInconsistencyTicket { HumanReceiptInconsistency record = 1; }
message UnmappedStatusTicket { OrderStateReceipt receipt = 1; }
message GapTicket {
  oneof value {
    SourceGap source_gap = 1;
    DeliveryGap delivery_gap = 2;
  }
}
message Ticket {
  oneof value {
    InstructionTicket instruction = 1;
    DecisionTicket decision = 2;
    DeliveryProgressTicket delivery_progress = 3;
    StateReceiptTicket state_receipt = 4;
    ExecutionReceiptTicket execution_receipt = 5;
    CommissionSupplementTicket commission_supplement = 6;
    ExecutionConflictTicket execution_conflict = 7;
    ReconciliationTicket reconciliation = 8;
    ExternalChangeTicket external_change = 9;
    HumanCredentialTicket human_credential = 10;
    HumanReceiptInconsistencyTicket human_receipt_inconsistency = 11;
    UnmappedStatusTicket unmapped_status = 12;
    GapTicket gap = 13;
  }
}
message InstructionView {
  InstructionId instruction_id = 1;
  repeated Ticket tickets = 2;
}
message InstructionViewFound { InstructionView view = 1; }
message InstructionViewNotFound {}
message InstructionViewResult {
  oneof value {
    InstructionViewFound found = 1;
    InstructionViewNotFound not_found = 2;
  }
}
message TicketKind {
  oneof value {
    InstructionTicketKind instruction = 1;
    DecisionTicketKind decision = 2;
    DeliveryProgressTicketKind delivery_progress = 3;
    StateReceiptTicketKind state_receipt = 4;
    ExecutionReceiptTicketKind execution_receipt = 5;
    CommissionSupplementTicketKind commission_supplement = 6;
    ExecutionConflictTicketKind execution_conflict = 7;
    ReconciliationTicketKind reconciliation = 8;
    ExternalChangeTicketKind external_change = 9;
    HumanCredentialTicketKind human_credential = 10;
    HumanReceiptInconsistencyTicketKind human_receipt_inconsistency = 11;
    UnmappedStatusTicketKind unmapped_status = 12;
    GapTicketKind gap = 13;
  }
}
message InstructionTicketKind {}
message DecisionTicketKind {}
message DeliveryProgressTicketKind {}
message StateReceiptTicketKind {}
message ExecutionReceiptTicketKind {}
message CommissionSupplementTicketKind {}
message ExecutionConflictTicketKind {}
message ReconciliationTicketKind {}
message ExternalChangeTicketKind {}
message HumanCredentialTicketKind {}
message HumanReceiptInconsistencyTicketKind {}
message GapTicketKind {}
message UnmappedStatusTicketKind {}
message TicketRange {
  Generation from_generation = 1;
  Seq from_seq = 2;
  Generation to_generation = 3;
  Seq to_seq = 4;
}
message AccountTicketsRequest {
  AccountId account_id = 1;
  TicketKind kind = 2;
  TicketRange range = 3;
  Seq page_cursor = 4;
}
message AccountTickets {
  AccountId account_id = 1;
  repeated Ticket tickets = 2;
  Seq next_page_cursor = 3;
}
message AccountTicketsFound { AccountTickets tickets = 1; }
message AccountTicketsNotFound {}
message AccountTicketsResult {
  oneof value {
    AccountTicketsFound found = 1;
    AccountTicketsNotFound not_found = 2;
  }
}
message AccountHealth {
  AccountId account_id = 1;
  bool integration_session_established = 2;
  CapabilityDeclarationVersion capability_declaration_version = 3;
  DataTime last_ticket_time = 4;
  DataTime last_market_record_time = 5;
  uint64 suspended_subscription_count = 6;
  uint64 rejected_record_count = 7;
}
message HealthSnapshot { repeated AccountHealth accounts = 1; }
message HealthReadResult {
  oneof value {
    HealthSnapshot snapshot = 1;
  }
}
message CapabilityReadRequest { SourceId source = 1; }
message CapabilityReadResult {
  oneof value {
    CapabilityDeclaration capabilities = 1;
  }
}
message ReadRequest {
  ReadTarget target = 1;
  ReadKind kind = 2;
  ReadQuery query = 3;
}


// ---------- 服务 ----------

service IntegrationCoreContract {
  rpc A01OpenSession(IntegrationSessionRequest) returns (IntegrationSessionResult);
  rpc A02ReceiveAuthorizedInstruction(AuthorizedInstruction) returns (InstructionReceivedResult);
  rpc A03RequestCallPermit(CallPermitRequest) returns (CallPermitResult);
  rpc A04SubmitReceipt(ReceiptSubmission) returns (ReceiptSubmissionResult);
  rpc A05SubmitReconciliation(ReconciliationSubmission) returns (ReconciliationSubmissionResult);
  rpc A06SubmitExternalChange(ExternalChangeSubmission) returns (ExternalChangeSubmissionResult);
  rpc A07SubmitMarketRecord(MarketRecordSubmission) returns (MarketRecordSubmissionResult);
  rpc A08ReportReconnect(ReconnectReport) returns (ReconnectReportResult);
  rpc A09QueryUnknown(UnknownQueryRequest) returns (UnknownQueryResult);
  rpc A10ForwardRead(ReadForwardRequest) returns (ReadForwardResult);
  rpc A11BackfillPage(BackfillPageRequest) returns (BackfillPageResult);
}

service ClientCoreContract {
  rpc B01OpenSession(ClientSessionRequest) returns (ClientSessionResult);
  rpc B02ReadOnce(ReadRequest) returns (ReadFanoutResult);
  rpc B03Subscribe(SubscriptionRequest) returns (SubscriptionResult);
  rpc B04Unsubscribe(UnsubscribeRequest) returns (UnsubscribeResult);
  rpc B05DeliverFlow(DeliverFlowRequest) returns (stream DeliveryEvent);
  rpc B06Ack(AckRequest) returns (AckResult);
  rpc B07AckGap(AckGapRequest) returns (AckGapResult);
  rpc B08SubmitInstruction(InstructionSubmissionRequest) returns (InstructionSubmissionResult);
  rpc B09SubmitDecision(DecisionRequest) returns (DecisionResult);
  rpc B10SubmitHumanCredential(HumanCredentialRequest) returns (HumanCredentialResult);
  rpc B11ReadInstructionView(InstructionId) returns (InstructionViewResult);
  rpc B12ReadAccountTickets(AccountTicketsRequest) returns (AccountTicketsResult);
  rpc B13ReadHealth(Empty) returns (HealthReadResult);
  rpc B14ReadCapabilities(CapabilityReadRequest) returns (CapabilityReadResult);
}
```

### 1.1 IDL 与 01 的逐项对应

|01 操作|proto3 RPC|请求|结果 oneof|
|---|---|---|---|
|A-01|`A01OpenSession`|`IntegrationSessionRequest`|`IntegrationSessionResult`|
|A-02|`A02ReceiveAuthorizedInstruction`|`AuthorizedInstruction`|`InstructionReceivedResult`|
|A-03|`A03RequestCallPermit`|`CallPermitRequest`|`CallPermitResult`|
|A-04|`A04SubmitReceipt`|`ReceiptSubmission`|`ReceiptSubmissionResult`|
|A-05|`A05SubmitReconciliation`|`ReconciliationSubmission`|`ReconciliationSubmissionResult`|
|A-06|`A06SubmitExternalChange`|`ExternalChangeSubmission`|`ExternalChangeSubmissionResult`|
|A-07|`A07SubmitMarketRecord`|`MarketRecordSubmission`|`MarketRecordSubmissionResult`|
|A-08|`A08ReportReconnect`|`ReconnectReport`|`ReconnectReportResult`|
|A-09|`A09QueryUnknown`|`UnknownQueryRequest`|`UnknownQueryResult`|
|A-10|`A10ForwardRead`|`ReadForwardRequest`|`ReadForwardResult`|
|A-11|`A11BackfillPage`|`BackfillPageRequest`|`BackfillPageResult`|
|B-01|`B01OpenSession`|`ClientSessionRequest`|`ClientSessionResult`|
|B-02|`B02ReadOnce`|`ReadRequest`|`ReadFanoutResult`|
|B-03|`B03Subscribe`|`SubscriptionRequest`|`SubscriptionResult`|
|B-04|`B04Unsubscribe`|`UnsubscribeRequest`|`UnsubscribeResult`|
|B-05|`B05DeliverFlow`|`DeliverFlowRequest`|`stream DeliveryEvent`|
|B-06|`B06Ack`|`AckRequest`|`AckResult`|
|B-07|`B07AckGap`|`AckGapRequest`|`AckGapResult`|
|B-08|`B08SubmitInstruction`|`InstructionSubmissionRequest`|`InstructionSubmissionResult`|
|B-09|`B09SubmitDecision`|`DecisionRequest`|`DecisionResult`|
|B-10|`B10SubmitHumanCredential`|`HumanCredentialRequest`|`HumanCredentialResult`|
|B-11|`B11ReadInstructionView`|`InstructionId`|`InstructionViewResult`|
|B-12|`B12ReadAccountTickets`|`AccountTicketsRequest`|`AccountTicketsResult`|
|B-13|`B13ReadHealth`|`Empty`|`HealthReadResult`|
|B-14|`B14ReadCapabilities`|`CapabilityReadRequest`|`CapabilityReadResult`|


## 2. Rust 类型草图

下面的 Rust 类型与 proto3 的 `oneof` 一一对应。空结构体只表示 v2 命名但不携带字段的结果分支，不引入默认值或通用结果。

```rust
// ---------- 基本标识与流 ----------

pub struct AccountId(pub String);
pub struct ClientKey(pub String);
pub struct AliceId(pub String);
pub struct BrokerNativeId(pub String);
pub struct IntegrationSessionNumber(pub String);
pub struct InstructionId {
    pub account_id: AccountId,
    pub client_key: ClientKey,
}
pub struct TryNumber(pub u64);
pub struct Generation(pub u64);
pub struct Seq(pub u64);
pub struct IntegrationCursor(pub Vec<u8>);
pub struct RequestSummary {
    pub sha256: Vec<u8>,
}
pub struct DataTime(pub /* google.protobuf.Timestamp */ ());

pub enum SourceId {
    Account { account_id: AccountId, flow_kind: FlowKind },
    PublicFeed { broker: String, flow_kind: FlowKind },
}
pub enum FlowKind { Market, AccountChange }
pub struct FlowRef {
    pub source: SourceId,
    pub generation: Generation,
}
// 客户端游标固定为 (流, 代数, seq)，不承载集成 opaque 游标。
pub struct FlowCursor {
    pub flow: FlowRef,
    pub seq: Seq,
}
pub enum GapReason {
    DisconnectWithoutCursor,
    QuotaRevoked,
    InputOverflow,
}

// ---------- 指令载荷 ----------

pub enum ExtensionScalar {
    String(String),
    Bool(bool),
    Integer(i64),
    Decimal(String),
}
pub struct TypedExtensions {
    pub fields: std::collections::BTreeMap<String, ExtensionScalar>,
}
pub struct RawEvidenceSummary {
    pub sha256: Vec<u8>,
    pub media_type: Option<String>,
    pub byte_length: Option<u64>,
}
pub struct ContractLeg {
    pub broker_native_id: Option<BrokerNativeId>,
    pub numeric_con_id: Option<String>,
    pub ratio: Option<String>,
    pub action: Option<String>,
    pub exchange: Option<String>,
    pub open_close: Option<String>,
    pub short_sales_slot: Option<String>,
    pub designated_location: Option<String>,
    pub exempt_code: Option<String>,
    pub per_leg_price: Option<String>,
    pub extensions: Option<TypedExtensions>,
    pub raw_evidence_summary: Option<RawEvidenceSummary>,
}
pub struct Contract {
    pub broker_native_id: Option<BrokerNativeId>,
    pub alice_id: Option<AliceId>,
    pub numeric_con_id: Option<String>,
    pub symbol: Option<String>,
    pub sec_type: Option<String>,
    pub last_trade_date_or_contract_month: Option<String>,
    pub strike: Option<String>,
    pub right: Option<String>,
    pub multiplier: Option<String>,
    pub exchange: Option<String>,
    pub primary_exchange: Option<String>,
    pub currency: Option<String>,
    pub local_symbol: Option<String>,
    pub trading_class: Option<String>,
    pub sec_id_type: Option<String>,
    pub sec_id: Option<String>,
    pub description: Option<String>,
    pub issuer_id: Option<String>,
    pub combo_legs_description: Option<String>,
    pub last_trade_date: Option<String>,
    pub combo_legs: Vec<ContractLeg>,
    pub extensions: Option<TypedExtensions>,
    pub raw_evidence_summary: Option<RawEvidenceSummary>,
}
pub struct IneligibilityReason {
    pub id: Option<String>,
    pub description: Option<String>,
}
pub struct ContractDescription {
    pub contract: Option<Contract>,
    pub derivative_sec_types: Vec<String>,
    pub extensions: Option<TypedExtensions>,
    pub raw_evidence_summary: Option<RawEvidenceSummary>,
}
pub struct ContractDetails {
    pub fields: std::collections::BTreeMap<String, String>,
    pub ineligibility_reasons: Vec<IneligibilityReason>,
    pub extensions: Option<TypedExtensions>,
    pub raw_evidence_summary: Option<RawEvidenceSummary>,
}
pub struct OrderRequest {
    pub contract: Contract,
    pub action: Option<String>,
    pub total_quantity: Option<String>,
    pub display_size: Option<String>,
    pub order_type: Option<String>,
    pub limit_price: Option<String>,
    pub aux_price: Option<String>,
    pub tif: Option<String>,
    pub account: Option<String>,
    pub order_ref: Option<String>,
    pub good_after_time: Option<String>,
    pub good_till_date: Option<String>,
    pub trail_stop_price: Option<String>,
    pub cash_quantity: Option<String>,
    pub outside_rth: Option<bool>,
    pub hidden: Option<bool>,
    pub all_or_none: Option<bool>,
    pub post_only: Option<bool>,
    pub trailing_percent: Option<String>,
    pub parent_id: Option<String>,
    pub oca_group: Option<String>,
    pub numeric_client_id: Option<String>,
    pub numeric_order_id: Option<String>,
    pub numeric_perm_id: Option<String>,
    pub numeric_parent_id: Option<String>,
    pub filled_quantity: Option<String>,
    pub extensions: Option<TypedExtensions>,
    pub raw_evidence_summary: Option<RawEvidenceSummary>,
}
pub struct OrderChanges {
    pub action: Option<String>,
    pub total_quantity: Option<String>,
    pub order_type: Option<String>,
    pub limit_price: Option<String>,
    pub aux_price: Option<String>,
    pub tif: Option<String>,
    pub good_after_time: Option<String>,
    pub good_till_date: Option<String>,
    pub trail_stop_price: Option<String>,
    pub cash_quantity: Option<String>,
    pub outside_rth: Option<bool>,
    pub hidden: Option<bool>,
    pub post_only: Option<bool>,
    pub trailing_percent: Option<String>,
    pub display_size: Option<String>,
    pub all_or_none: Option<bool>,
    pub parent_id: Option<String>,
    pub oca_group: Option<String>,
    pub extensions: Option<TypedExtensions>,
    pub raw_evidence_summary: Option<RawEvidenceSummary>,
}
pub struct OrderCancel {
    pub manual_order_cancel_time: Option<String>,
    pub ext_operator: Option<String>,
    pub manual_order_indicator: Option<String>,
}
pub struct PositionRisk {
    pub leverage: Option<String>,
    pub liquidation_price: Option<String>,
    pub margin_mode: Option<String>,
    pub extensions: Option<TypedExtensions>,
    pub raw_evidence_summary: Option<RawEvidenceSummary>,
}
pub struct Position {
    pub contract: Contract,
    pub currency: String,
    pub side: String,
    pub quantity: String,
    pub avg_cost: String,
    pub market_price: String,
    pub market_value: String,
    pub unrealized_pnl: String,
    pub realized_pnl: String,
    pub multiplier: String,
    pub avg_cost_source: Option<String>,
    pub risk: Option<PositionRisk>,
    pub extensions: Option<TypedExtensions>,
    pub raw_evidence_summary: Option<RawEvidenceSummary>,
}
pub struct OrderAllocation {
    pub account: Option<String>,
    pub position: Option<String>,
    pub position_desired: Option<String>,
    pub position_after: Option<String>,
    pub desired_alloc_qty: Option<String>,
    pub allowed_alloc_qty: Option<String>,
    pub is_monetary: Option<bool>,
}
pub struct OrderState {
    pub status: Option<String>,
    pub init_margin_before: Option<String>,
    pub maint_margin_before: Option<String>,
    pub equity_with_loan_before: Option<String>,
    pub init_margin_change: Option<String>,
    pub maint_margin_change: Option<String>,
    pub equity_with_loan_change: Option<String>,
    pub init_margin_after: Option<String>,
    pub maint_margin_after: Option<String>,
    pub equity_with_loan_after: Option<String>,
    pub commission_and_fees: Option<String>,
    pub min_commission_and_fees: Option<String>,
    pub max_commission_and_fees: Option<String>,
    pub commission_and_fees_currency: Option<String>,
    pub margin_currency: Option<String>,
    pub init_margin_before_outside_rth: Option<String>,
    pub maint_margin_before_outside_rth: Option<String>,
    pub equity_with_loan_before_outside_rth: Option<String>,
    pub init_margin_change_outside_rth: Option<String>,
    pub maint_margin_change_outside_rth: Option<String>,
    pub equity_with_loan_change_outside_rth: Option<String>,
    pub init_margin_after_outside_rth: Option<String>,
    pub maint_margin_after_outside_rth: Option<String>,
    pub equity_with_loan_after_outside_rth: Option<String>,
    pub suggested_size: Option<String>,
    pub reject_reason: Option<String>,
    pub order_allocations: Vec<OrderAllocation>,
    pub warning_text: Option<String>,
    pub completed_time: Option<String>,
    pub completed_status: Option<String>,
    pub extensions: Option<TypedExtensions>,
    pub raw_evidence_summary: Option<RawEvidenceSummary>,
}
pub struct StateReceiptFingerprint {
    // 只含 broker 原生 id、状态原值、累计成交量，不含时间戳；在提交账户范围内比较。
    pub broker_native_id: BrokerNativeId,
    pub original_status: String,
    pub cumulative_quantity: String,
}
pub enum StateReceiptDedupeKey {
    BrokerEventId(String),
    EventFingerprint(StateReceiptFingerprint),
}
pub struct OrderStateReceipt {
    pub broker_native_id: Option<BrokerNativeId>,
    pub client_key: Option<ClientKey>,
    pub original_status: Option<String>,
    pub mapped_state: ReceiptState,
    pub cumulative_quantity: Option<String>,
    pub remaining: Option<String>,
    pub average_fill_price: Option<String>,
    pub numeric_order_id: Option<String>,
    pub numeric_perm_id: Option<String>,
    pub numeric_parent_id: Option<String>,
    pub last_fill_price: Option<String>,
    pub numeric_client_id: Option<String>,
    pub why_held: Option<String>,
    pub market_cap_price: Option<String>,
    pub broker_time: Option<DataTime>,
    pub order_state: Option<OrderState>,
    pub dedupe_key: StateReceiptDedupeKey,
    pub original_payload_summary: Option<Vec<u8>>,
    pub extensions: Option<TypedExtensions>,
    pub raw_evidence_summary: Option<RawEvidenceSummary>,
}
pub struct ExecutionReceipt {
    pub broker_native_id: Option<BrokerNativeId>,
    pub client_key: Option<ClientKey>,
    pub broker_execution_id: String,
    pub numeric_order_id: Option<String>,
    pub broker_time: Option<String>,
    pub acct_number: Option<String>,
    pub exchange: Option<String>,
    pub side: Option<String>,
    pub single_quantity: Option<String>,
    pub single_price: Option<String>,
    pub numeric_perm_id: Option<String>,
    pub numeric_client_id: Option<String>,
    pub is_liquidation: Option<bool>,
    pub cumulative_quantity: Option<String>,
    pub average_price: Option<String>,
    pub order_ref: Option<String>,
    pub ev_rule: Option<String>,
    pub ev_multiplier: Option<String>,
    pub model_code: Option<String>,
    pub last_liquidity: Option<String>,
    pub is_price_revision_pending: Option<bool>,
    pub submitter: Option<String>,
    pub opt_exercise_or_lapse_type: Option<String>,
    pub commission_and_fees: Option<String>,
    pub commission_currency: Option<String>,
    pub realized_pnl: Option<String>,
    pub bond_yield: Option<String>,
    pub yield_redemption_date: Option<String>,
    pub extensions: Option<TypedExtensions>,
    pub raw_evidence_summary: Option<RawEvidenceSummary>,
}
pub struct CommissionSupplement {
    pub broker_execution_id: String,
    pub commission_and_fees: Option<String>,
    pub commission_currency: Option<String>,
    pub realized_pnl: Option<String>,
    pub bond_yield: Option<String>,
    pub yield_redemption_date: Option<String>,
    pub extensions: Option<TypedExtensions>,
    pub raw_evidence_summary: Option<RawEvidenceSummary>,
}
pub struct ExecutionConflict {
    pub broker_execution_id: String,
}
pub struct TakeProfitStopLoss {
    pub take_profit_price: Option<String>,
    pub stop_loss_price: Option<String>,
    pub stop_loss_limit_price: Option<String>,
}
pub struct PlaceOrderLeg {
    pub broker_native_id: Option<BrokerNativeId>,
    pub kind: Option<String>,
    pub extensions: Option<TypedExtensions>,
    pub raw_evidence_summary: Option<RawEvidenceSummary>,
}
pub struct BrokerPlacementResult {
    pub success: bool,
    pub broker_native_id: Option<BrokerNativeId>,
    pub error: Option<String>,
    pub message: Option<String>,
    pub execution: Option<ExecutionReceipt>,
    pub order_state: Option<OrderState>,
    pub filled_quantity: Option<String>, // 状态回票累计成交量
    pub filled_price: Option<String>, // 状态回票平均成交价
    pub extensions: Option<TypedExtensions>,
    pub raw_evidence_summary: Option<RawEvidenceSummary>,
    pub action: Option<String>,
    pub symbol: Option<String>,
}
pub struct OpenOrder {
    pub broker_native_id: Option<BrokerNativeId>,
    pub contract: Contract,
    pub order: OrderRequest,
    pub order_state: Option<OrderState>,
    pub avg_fill_price: Option<String>,
    pub tpsl: Option<TakeProfitStopLoss>,
    pub extensions: Option<TypedExtensions>,
    pub raw_evidence_summary: Option<RawEvidenceSummary>,
}
pub enum InstructionKind { Entrust, Modify, Cancel, Close }
pub struct EntrustPayload {
    pub order: OrderRequest,
    pub tpsl: Option<TakeProfitStopLoss>,
}
pub struct ModifyPayload {
    pub target: InstructionTarget,
    pub changes: OrderChanges,
}
pub struct CancelPayload {
    pub target: InstructionTarget,
    pub order_cancel: Option<OrderCancel>,
}
pub struct ClosePayload {
    pub contract: Contract,
    pub quantity: Option<String>,
    pub sub_account_id: Option<String>,
}
pub enum InstructionTarget {
    Instruction(InstructionId),
    BrokerNative(BrokerNativeId),
}
pub enum InstructionPayload {
    Entrust(EntrustPayload),
    Modify(ModifyPayload),
    Cancel(CancelPayload),
    Close(ClosePayload),
}
pub struct Instruction {
    pub id: InstructionId,
    pub account_id: AccountId,
    pub client_key: ClientKey,
    pub payload: InstructionPayload,
    pub sub_account_id: Option<String>,
}
pub struct AuthorizedInstruction {
    pub instruction: Instruction,
    pub try_number: TryNumber,
}

// ---------- 能力声明 ----------

pub enum Support { Supported, Unsupported }
pub enum ClientKeySemantics { Documented, NotDocumented }
pub enum QueryChannel { ClientKeyReadback, UnfilledList, TradePositionReconciliation }
pub enum TrustedConclusion { Trusted, NotTrusted }
pub struct QueryChannelCapability {
    pub channel: QueryChannel,
    pub conclusion: TrustedConclusion,
}
pub struct FieldCapability {
    pub name: String,
    pub support: Support,
}
pub enum ExecutionReceiptCapability {
    // NoTradeByTrade 表示逐笔成交去重能力不可用，不得提交成交回票。
    Available,
    NoTradeByTrade,
}
pub struct InstructionCapability {
    pub kind: InstructionKind,
    pub support: Support,
    pub client_key_semantics: ClientKeySemantics,
    pub query_channels: Vec<QueryChannelCapability>,
    pub result_fields: Vec<FieldCapability>,
    pub execution_receipts: ExecutionReceiptCapability,
}
pub enum ReadKind {
    Account, Positions, Orders, Quotes, DerivativesDiscovery, Depth,
    MarketClock, HistoricalBars, ContractSearch, ContractDetail, Wallet,
    HistoricalRecords,
}
pub struct ReturnFields(pub Vec<String>);
pub struct ReadCapability {
    pub kind: ReadKind,
    pub support: Support,
    pub return_fields: ReturnFields,
    pub fields: Vec<FieldCapability>,
}
pub enum StreamKind { Market, AccountChange }
pub enum ContinuationCursor { Trusted, NotTrusted }
pub struct Quota;
pub enum PauseSupport { Pausable, NotPausable }
pub struct StreamCapability {
    pub kind: StreamKind,
    pub support: Support,
    pub continuation: ContinuationCursor,
    pub quota: Quota,
    pub pause: PauseSupport,
}
pub struct Pacing;
pub struct HistoricalBarCapability {
    pub support: Support,
    pub pacing: Pacing,
}
pub enum SubAccountEnumeration { Enumerable, NotEnumerable }
pub struct StatusMappingVersion(pub String); // 集成制品版本字符串
pub struct CapabilityDeclarationVersion(pub String);
pub struct CapabilityDeclaration {
    pub instruction_capabilities: Vec<InstructionCapability>,
    pub read_capabilities: Vec<ReadCapability>,
    pub stream_capabilities: Vec<StreamCapability>,
    pub historical_bars: HistoricalBarCapability,
    pub sub_accounts: SubAccountEnumeration,
    pub status_mapping_version: StatusMappingVersion,
    pub declaration_version: CapabilityDeclarationVersion,
}

// ---------- A-01 至 A-03 ----------

pub struct IntegrationSessionRequest {
    pub session_number: IntegrationSessionNumber,
    pub capabilities: CapabilityDeclaration,
}
pub enum IntegrationSessionResult {
    Accepted { session_number: IntegrationSessionNumber },
    CapabilityIncomplete,
    Unauthorized,
}
pub enum InstructionReceivedResult { Received }
pub struct CallPermitRequest {
    pub instruction_id: InstructionId,
    pub try_number: TryNumber,
    pub account_id: AccountId,
    pub session_number: IntegrationSessionNumber,
    pub request_summary: RequestSummary,
}
pub enum CallPermitResult { Confirmed { session_number: IntegrationSessionNumber }, SummaryMismatch, AlreadyRecorded }

// ---------- A-04 至 A-06 ----------

pub enum ReceiptState {
    Accepted,
    PartiallyFilled,
    Filled,
    Rejected,
    Canceled,
    BrokerSpecific { original_status: String },
    Unmapped { original_status: String },
}
pub enum Receipt {
    State(OrderStateReceipt),
    Execution(ExecutionReceipt),
}
pub struct ReceiptSubmission {
    pub receipt: Receipt,
}
pub enum ReceiptSubmissionResult {
    Linked { instruction_id: InstructionId },
    ExternalChange,
    DuplicateState,
    DuplicateExecution,
    CommissionSupplement,
    ExecutionConflict,
    MissingStateDedupeKey,
    MissingExecutionId,
    ExecutionUnsupported,
    PayloadMissing,
}
pub enum ReconciliationKind { Positions, Orders, Account, Readback }
pub struct ReconciliationKey {
    pub account_id: AccountId,
    pub kind: ReconciliationKind,
    pub session_number: IntegrationSessionNumber,
    pub integration_seq: u64,
}
pub enum ReconciliationPayload {
    Positions(Vec<Position>),
    Orders(Vec<OpenOrder>),
    Account(AccountInfo),
    Readback(ReadPayload),
}
pub struct ReconciliationResponse {
    pub key: ReconciliationKey,
    pub account_id: AccountId,
    pub data_time: DataTime,
    pub kind: ReconciliationKind,
    pub payload: ReconciliationPayload,
    pub instruction_id: Option<InstructionId>,
    pub try_number: Option<TryNumber>,
    pub broker_native_id: Option<BrokerNativeId>,
    pub client_key: Option<ClientKey>,
}
pub enum ReconciliationSubmissionResult { Recorded, Duplicate }
pub enum ExternalChangeReason {
    NoInstructionMatch,
    InstructionAlreadyTerminal,
    UndecidedInstructionPresent,
}
pub struct ExternalChange {
    pub account_id: AccountId,
    pub state_receipt: OrderStateReceipt,
    pub reason: ExternalChangeReason,
}
pub enum ExternalChangeSubmissionResult { Recorded, MissingDedupeKey }

// ---------- A-07 至 A-08 ----------

pub struct QualityFreshness {
    pub quality: String,
    pub freshness: String,
}
pub struct MarketRecord {
    pub alice_id: AliceId,
    pub record_fields: Vec<u8>,
    pub data_time: DataTime,
    pub quality_freshness: QualityFreshness,
}
pub struct MarketRecordSubmission {
    pub source: SourceId,
    pub generation: Generation,
    pub seq: Seq,
    pub record: MarketRecord,
}
pub enum MarketRecordSubmissionResult { Recorded }
pub enum ReconnectReport {
    CanContinue { source: SourceId, cursor: IntegrationCursor },
    CannotContinue {
        source: SourceId,
        previous_generation: Generation,
        previous_last_seq: Seq,
        reason: GapReason,
    },
}
pub enum ReconnectReportResult { Recorded }

// ---------- A-09 ----------

pub struct InstructionFeatures {
    pub instrument: AliceId,
    pub direction: String,
    pub quantity: String,
    pub price: String,
    pub order_type: String,
    pub submitted_time: DataTime,
}
pub enum UnknownQueryChannel {
    ClientKey { client_key: ClientKey },
    UnfilledList {
        broker_native_id: BrokerNativeId,
        instruction_features: InstructionFeatures,
    },
    TradePosition { instruction_features: InstructionFeatures },
}
pub struct UnknownQueryRequest {
    pub instruction_id: InstructionId,
    pub try_number: TryNumber,
    pub account_id: AccountId,
    pub channel: UnknownQueryChannel,
}
pub enum UnknownQueryResult {
    FoundReceipt(Receipt),
    ProvedNotDelivered,
    NoConclusion,
}

// ---------- A-10 与 B-02 ----------

pub enum ReadTarget { ExactSource(String), ProviderPrefix(String), AllAccounts, TradingOnly }
pub struct BarQuery {
    pub interval: String,
    pub start: Option<DataTime>,
    pub end: Option<DataTime>,
    pub limit: Option<u64>,
    pub what_to_show: Option<String>,
}
pub struct ReadQuery {
    pub alice_id: Option<AliceId>,
    pub account_id: Option<AccountId>,
    pub sub_account_id: Option<String>,
    pub contract: Option<Contract>,
    pub pattern: Option<String>,
    pub historical_bars: Option<BarQuery>,
}
pub struct AccountInfo {
    pub base_currency: String,
    pub net_liquidation: String,
    pub total_cash_value: String,
    pub unrealized_pnl: String,
    pub realized_pnl: Option<String>,
    pub buying_power: Option<String>,
    pub init_margin_req: Option<String>,
    pub maint_margin_req: Option<String>,
    pub day_trades_remaining: Option<String>,
    pub extensions: Option<TypedExtensions>,
    pub raw_evidence_summary: Option<RawEvidenceSummary>,
}
pub struct SubAccountRef {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub extensions: Option<TypedExtensions>,
    pub raw_evidence_summary: Option<RawEvidenceSummary>,
}
pub struct Quote {
    pub contract: Contract,
    pub last: String,
    pub bid: String,
    pub ask: String,
    pub volume: String,
    pub high: Option<String>,
    pub low: Option<String>,
    pub timestamp: DataTime,
    pub extensions: Option<TypedExtensions>,
    pub raw_evidence_summary: Option<RawEvidenceSummary>,
}
pub struct MarketClock {
    pub is_open: bool,
    pub next_open: Option<DataTime>,
    pub next_close: Option<DataTime>,
    pub timestamp: Option<DataTime>,
    pub extensions: Option<TypedExtensions>,
    pub raw_evidence_summary: Option<RawEvidenceSummary>,
}
pub struct Bar {
    pub timestamp: DataTime,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub volume: String,
    pub extensions: Option<TypedExtensions>,
    pub raw_evidence_summary: Option<RawEvidenceSummary>,
}
pub struct OrderBookLevel {
    pub price: String,
    pub amount: String,
}
pub struct OrderBook {
    pub contract: Contract,
    pub bids: Vec<OrderBookLevel>,
    pub asks: Vec<OrderBookLevel>,
    pub timestamp: DataTime,
    pub extensions: Option<TypedExtensions>,
    pub raw_evidence_summary: Option<RawEvidenceSummary>,
}
pub struct HistoryContract {
    pub alice_id: Option<String>,
    pub symbol: Option<String>,
    pub local_symbol: Option<String>,
    pub sec_type: Option<String>,
    pub currency: Option<String>,
    pub exchange: Option<String>,
    pub expiry: Option<String>,
    pub strike: Option<String>,
    pub right: Option<String>,
    pub multiplier: Option<String>,
}
pub struct OrderHistoryEntry {
    pub broker_native_id: Option<BrokerNativeId>,
    pub timestamp: DataTime,
    pub resolved_at: Option<DataTime>,
    pub contract: HistoryContract,
    pub side: String,
    pub order_type: Option<String>,
    pub quantity: Option<String>,
    pub limit_price: Option<String>,
    pub stop_price: Option<String>,
    pub status: String,
    pub filled_quantity: Option<String>,
    pub average_fill_price: Option<String>,
    pub source: String,
    pub commit_hash: String,
    pub message: String,
    pub error: Option<String>,
    pub extensions: Option<TypedExtensions>,
    pub raw_evidence_summary: Option<RawEvidenceSummary>,
}
pub struct TradeHistoryEntry {
    pub timestamp: DataTime,
    pub broker_native_id: Option<BrokerNativeId>,
    pub contract: HistoryContract,
    pub side: String,
    pub quantity: String,
    pub price: String,
    pub value: String,
    pub source: String,
    pub commit_hash: String,
    pub extensions: Option<TypedExtensions>,
    pub raw_evidence_summary: Option<RawEvidenceSummary>,
}
pub enum ReadPayload {
    Account(AccountInfo),
    Positions(Vec<Position>),
    Orders(Vec<OpenOrder>),
    Quotes(Vec<Quote>),
    DerivativesDiscovery(Vec<Contract>),
    Depth(Vec<OrderBook>),
    MarketClock(MarketClock),
    HistoricalBars(Vec<Bar>),
    ContractSearch(Vec<ContractDescription>),
    ContractDetail { contract: Contract, details: ContractDetails },
    Wallet(Vec<SubAccountRef>),
    HistoricalRecords {
        order_history: Vec<OrderHistoryEntry>,
        trade_history: Vec<TradeHistoryEntry>,
    },
}
pub struct ReadSuccess {
    pub account_id: AccountId,
    pub source: SourceId,
    pub data_time: DataTime,
    pub payload: ReadPayload,
    pub quality_freshness: Option<QualityFreshness>,
}
pub enum ReadFailure { Unavailable, NotFound, Unsupported, Timeout }
pub enum AccountReadResult { Success(ReadSuccess), Failure(ReadFailure) }
pub struct ReadFanoutResult { pub accounts: Vec<AccountReadResult> }

// ---------- A-11 ----------

pub struct BackfillPageRequest {
    pub source: SourceId,
    pub generation: Generation,
    pub alice_id: AliceId,
    pub query: ReadQuery,
    pub window_start: DataTime,
    pub page_cursor: Option<Vec<u8>>,
}
pub struct BackfillPage {
    pub records: Vec<MarketRecord>,
    pub next_page_cursor: Option<Vec<u8>>,
    pub last_page: bool,
}
pub enum BackfillPageResult { Page(BackfillPage), Unsupported }

// ---------- B-01 ----------

pub enum ClientSessionResult { Accepted, Unauthorized }

// ---------- B-03 至 B-07 ----------

pub enum SubscriptionSelector {
    Market { alice_id: AliceId },
    Account { account_id: AccountId },
}
pub struct BackfillRequest {
    pub query: ReadQuery,
    pub window_start: Option<DataTime>,
}
pub struct SubscriptionRequest {
    pub selector: SubscriptionSelector,
    pub flow_kind: FlowKind,
    pub backfill: Option<BackfillRequest>,
    pub cursors: Vec<FlowCursor>,
}
pub enum SubscriptionStatus {
    Active,
    Suspended { reason: String },
}
pub enum SubscriptionResult { Accepted { status: SubscriptionStatus }, Unsupported, Quota }
pub enum UnsubscribeResult { Accepted }
pub enum DeliveryAccountTicket {
    StateReceipt(OrderStateReceipt),
    ExecutionReceipt(ExecutionReceipt),
    CommissionSupplement(CommissionSupplement),
    ExecutionConflict(ExecutionConflict),
    Reconciliation(ReconciliationResponse),
    ExternalChange(ExternalChange),
    DeliveryProgress(DeliveryProgress),
}
pub enum DeliveryProgress {
    DeliveredToIntegration,
    CallPermit,
    Unknown,
    NotDelivered,
}
pub struct SourceGap {
    pub flow: FlowRef,
    pub previous_generation: Generation,
    pub previous_last_seq: Seq,
    pub reason: GapReason,
}
pub struct DeliveryGap {
    pub flow: FlowRef,
    pub from_seq: Seq,
    pub to_seq: Seq,
    pub reason: GapReason,
    pub previous_generation: Option<Generation>,
    pub previous_last_seq: Option<Seq>,
}
pub enum DeliveryEvent {
    MarketRecord { flow: FlowRef, seq: Seq, record: MarketRecord },
    AccountTicket { flow: FlowRef, seq: Seq, ticket: DeliveryAccountTicket },
    DeliveryGap(DeliveryGap),
    Suspended { flow: FlowRef, reason: String },
    Resumed { flow: FlowRef },
    Ready { flow: FlowRef },
}
pub enum AckResult { Accepted }
pub enum AckGapResult { Accepted }

// ---------- B-08 至 B-10 ----------

pub enum CreationRejectionReason {
    InstrumentWrongAccount,
    QuantityZeroOrNaN,
    ReadOnlyAccount,
    PolicyDenied,
    TargetResultUnknown,
}
pub enum InstructionSubmissionResult {
    Stored { instruction_id: InstructionId },
    Rejected { instruction_id: InstructionId, reason: CreationRejectionReason },
}
pub enum DecisionKind { Approve, Deny }
pub struct DecisionToken(pub String);
pub struct DecisionRequest {
    pub instruction_id: InstructionId,
    pub expected_version: u64,
    pub decision_token: DecisionToken,
    pub decision: DecisionKind,
}
pub enum DecisionResult {
    Stored { instruction_id: InstructionId, decision_token: DecisionToken },
    Conflict,
    Expired,
    TokenConflict,
    PermissionDenied,
}
pub enum HumanConclusion {
    Delivered { broker_native_id: BrokerNativeId, original_status: String },
    NotDelivered,
}
pub struct CredentialToken(pub String);
pub struct HumanCredentialRequest {
    pub instruction_id: InstructionId,
    pub try_number: TryNumber,
    pub credential_token: CredentialToken,
    pub conclusion: HumanConclusion,
    pub basis: Vec<u8>,
}
pub enum HumanCredentialResult { Stored { credential_token: CredentialToken }, TokenConflict, NotHuman }

// ---------- B-11 至 B-14 ----------

pub struct HumanReceiptInconsistency {
    pub instruction_id: InstructionId,
    pub credential_token: CredentialToken,
    pub broker_native_id: BrokerNativeId,
    pub original_status: String,
}
pub struct StateReceiptTicket {
    pub receipt: OrderStateReceipt,
}
pub struct ExecutionReceiptTicket {
    pub receipt: ExecutionReceipt,
}
pub struct CommissionSupplementTicket {
    pub record: CommissionSupplement,
}
pub struct ExecutionConflictTicket {
    pub record: ExecutionConflict,
}
pub struct UnmappedStatusTicket {
    pub receipt: OrderStateReceipt,
}
pub enum Ticket {
    Instruction(Instruction),
    Decision(DecisionRequest),
    DeliveryProgress(DeliveryProgress),
    StateReceipt(StateReceiptTicket),
    ExecutionReceipt(ExecutionReceiptTicket),
    CommissionSupplement(CommissionSupplementTicket),
    ExecutionConflict(ExecutionConflictTicket),
    Reconciliation(ReconciliationResponse),
    ExternalChange(ExternalChange),
    HumanCredential(HumanCredentialRequest),
    HumanReceiptInconsistency(HumanReceiptInconsistency),
    UnmappedStatus(UnmappedStatusTicket),
    Gap(GapTicket),
}
pub enum GapTicket { Source(SourceGap), Delivery(DeliveryGap) }
pub struct InstructionView {
    pub instruction_id: InstructionId,
    pub tickets: Vec<Ticket>,
}
pub enum InstructionViewResult { Found(InstructionView), NotFound }
pub enum TicketKind {
    Instruction,
    Decision,
    DeliveryProgress,
    StateReceipt,
    ExecutionReceipt,
    CommissionSupplement,
    ExecutionConflict,
    Reconciliation,
    ExternalChange,
    HumanCredential,
    HumanReceiptInconsistency,
    UnmappedStatus,
    Gap,
}
pub struct TicketRange {
    pub from_generation: Generation,
    pub from_seq: Seq,
    pub to_generation: Generation,
    pub to_seq: Seq,
}
pub struct AccountTicketsRequest {
    pub account_id: AccountId,
    pub kind: TicketKind,
    pub range: TicketRange,
    pub page_cursor: Option<Seq>,
}
pub struct AccountTickets {
    pub account_id: AccountId,
    pub tickets: Vec<Ticket>,
    pub next_page_cursor: Option<Seq>,
}
pub enum AccountTicketsResult { Found(AccountTickets), NotFound }
pub struct AccountHealth {
    pub account_id: AccountId,
    pub integration_session_established: bool,
    pub capability_declaration_version: CapabilityDeclarationVersion,
    pub last_ticket_time: Option<DataTime>,
    pub last_market_record_time: Option<DataTime>,
    pub suspended_subscription_count: u64,
    pub rejected_record_count: u64,
}
pub struct HealthSnapshot { pub accounts: Vec<AccountHealth> }
pub enum HealthReadResult { Snapshot(HealthSnapshot) }
pub struct CapabilityReadRequest { pub source: SourceId }
pub enum CapabilityReadResult { Capabilities(CapabilityDeclaration) }
```

## 3. 规则与类型的核对

|规则或约束|IDL / Rust 落点|不能从类型推出的部分|
|---|---|---|
|R1 先许可后调用|`CallPermitRequest`、`CallPermitResult`、`TryNumber`、`IntegrationSessionNumber`、`RequestSummary.sha256`；确认回显会话号|许可落盘先于确认、确认后只调用一次、SDK 关闭写请求自动重试由运行时按 `uta-design.md:202` 执行。|
|R2 一账户一队|`AccountId`、`AuthorizedInstruction`、`DeliveryProgress`|队列调度与跨账户互不等待不是 protobuf 字段。|
|R3 不明不重发|`UnknownQueryRequest`、`UnknownQueryResult`、`CallPermitResult`|结果不明后不再许可由核心持久记录保证。|
|R4 单调折叠与逐笔去重|`OrderStateReceipt.dedupe_key`、`StateReceiptFingerprint`、`ReceiptSubmissionResult::{DuplicateState,DuplicateExecution,CommissionSupplement,ExecutionConflict,MissingStateDedupeKey,MissingExecutionId,ExecutionUnsupported}`、`ExternalChangeSubmissionResult::MissingDedupeKey`|状态回票按 broker 状态偏序折叠；状态指纹只有原生 id、状态原值、累计成交量，不含时间戳。成交回票按 broker 成交 id 逐笔去重，佣金补录与成交冲突保留为独立帐票。|
|R5 保真|`OrderStateReceipt`、`ExecutionReceipt`、`CommissionSupplement`、`ExecutionConflict`、`RawEvidenceSummary`、`TypedExtensions`、`BrokerPlacementResult`、`BrokerNativeId`、`UnmappedStatusTicket`|状态映射版本来自集成制品字符串；表外状态帐票和告警由运行时追加。|
|R6 期望版本|`DecisionRequest.expected_version`、`DecisionToken`、`DecisionResult::{Conflict,TokenConflict}`|期望版本等于帐票数的计算由核心执行；token 内容比对由核心执行。|
|R7 过期|`DecisionResult::Expired`|批准前 deadline 的时钟与字段由核心执行。|
|R8 创建期拒绝|`InstructionRejected`、`CreationRejectionReason`、`Instruction.sub_account_id`|指令与否决一次落盘；能力未枚举指定子账户或子账户与合约不匹配时同样按 R8 拒绝，权限类安全事件由核心执行。|
|R9 目标结果不明|`TargetResultUnknown`、`CreationRejectionReason`|目标收敛后重新提交由客户端决定。|
|R10 凭证与回票冲突|`HumanReceiptInconsistency`|回票优先、不再查询、追加告警帐票由核心执行。|
|R11 回票与确认无序|`OrderStateReceipt.broker_native_id` / `ExecutionReceipt.broker_execution_id` / `Receipt`|状态回票与成交回票都可按各自类型保存；两条路径的先后与直接关联由核心执行。|
|R12 会话与重派|`IntegrationSessionNumber`、`CallPermitConfirmed`、`TryNumber`|确认带集成会话号；当前集成实例收到非本会话确认时丢弃、不调用；核心不承诺仍存活的旧实例不会晚读到自己会话的合法确认并调用 broker；安全只依赖许可落盘即该尝试“可能已调用”、核心永不向任何集成实例再许可同一尝试、账户队头阻塞（R2）；旧实例晚调用产生的回票按 R11 关联同一尝试。|
|2.6 流与缺口|`IntegrationCursor`、`FlowCursor`、`DeliveryGap`、`SubscriptionStatus`、`SubscriptionSuspendedNotice`、`SubscriptionResumedNotice`、`ReadyNotice`|游标保存、挂起/恢复条件和至少一次投递由核心执行。|
|2.7 一次性读|`ReadTarget`、`ReadKind`、`ReadQuery`、`ReadPayload`、`ReadFanoutResult`、`ReadFailure`；字段能力用 `ReadCapability.fields`|fan-out 调度、权限和 deadline 由核心执行。|
|2.8 能力声明|`CapabilityDeclaration`、`InstructionCapability.result_fields`、`InstructionCapability.execution_receipts`、`ExecutionReceiptCapability`、`ReadCapability.fields`、`IntegrationSessionRequest.session_number`、`StatusMappingVersion`、`CapabilityDeclarationVersion`|同会话重复声明整体替换、能力缩减影响订阅由核心执行。|
|2.9 会话与权限|`A01/B01` 会话结果；所有 RPC 的认证上下文|认证载荷与 Windows 传输见 `uta-design.md:243-245` 与 §6 实验，不在 IDL 选择。|
|R5 原生 id 条件|`optional BrokerNativeId`、`ExecutionReceipt.broker_execution_id` 与 `numeric_*_id`|broker 已分配原生 id 时字段必填；没有 broker 原生 id 时不能用数字观察值替代，成交回票没有 broker 成交 id 时不能提交，能力与事件 id 共同说明缺失。|

## 4. 卡点对照

已落地的字段保真、扩展边界、optional 与能力字段见 `contracts/03-field-audit.md`。仍有一个证据卡点：`IBroker` 公共接口（`packages/uta-protocol/src/types/broker.ts:502-597`）没有订单簿和历史记录方法；订单簿只在 `services/uta/src/domain/trading/brokers/ccxt/ccxt-types.ts:68-75`，历史结构只在 `packages/uta-protocol/src/types/history.ts:16-76`，SDK 入口为 `src/services/uta-client/UTAAccountSDK.ts:174-175,259-272`。在补充权威读接口与能力声明前，不得把这两个读面判为某 broker 的明确 unavailable，也不得新增未定义请求字段。
