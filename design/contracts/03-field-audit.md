# 契约 03：字段保真审计

本表是 `contracts/02-idl-and-types.md` 的字段来源清单。审计范围是 `packages/uta-protocol/src/types/broker.ts`、`packages/uta-protocol/src/types/git.ts` 及其直接依赖的 IBKR 形状；N3 的订单簿与历史读面另以现有公开类型为证据。每个叶字段恰好选择一种处置：**保留**、**规范化**、**明确 unavailable**、**不在范围**。没有四选一证据的项目列为卡点，不补规则。

## 0. 统一口径

### 0.1 四种处置

|处置|含义|02 落点|
|---|---|---|
|保留|字段语义、存在性和原值可由 Alice 消费；只做 snake_case 命名。|同名 IDL 字段；源字段可选时使用 `optional`。|
|规范化|只改变跨语言表示，不改变值；Decimal、number、金额、数量、价格、倍率和整数标识写成十进制字符串，禁止浮点中间值；`Date` 写 `google.protobuf.Timestamp`；不规定 scale、舍入或单位换算。|带 `decimal string` 注释的 IDL 字段。|
|明确 unavailable|当前 broker 没有该字段或读面；IDL 字段为 `optional`，同一读/指令能力声明用 `FieldCapability` 标为 `unsupported`；不能用零、空串或空数组冒充。|字段保持可区分缺失的 optional；能力声明给出字段路径。|
|不在范围|字段属于 broker 私有扩展、IBKR 传输控制或当前没有 v2 语义的原始结构；不进入核心解释字段。若实际出现，必须进入 `TypedExtensions` 和 `RawEvidenceSummary`，不得静默丢弃。|扩展值只作保真证据，不参与状态折叠；重启后可读。|

`TypedExtensions.fields` 是字符串键到标量值（字符串、布尔、整数或十进制字符串）的映射；核心不解释键、不把扩展字段用于关联或状态折叠。`RawEvidenceSummary` 至少保存摘要哈希，并可保存媒体类型和字节长度；摘要与扩展随帐票、对账响应和读响应一同持久化，重启重读不依赖进程内缓存。`raw` 任意对象本身不进入跨语言核心类型，只能以该摘要和扩展承载。

### 0.2 来源与组合规则

- `Contract` 与 `Order` 的依赖字段以 `packages/ibkr/src/protobuf/Contract.ts:14-36`、`Order.ts:14-221` 为叶字段全集；`aliceId` 来自 `packages/uta-protocol/src/types/contract-ext.ts:27-30`。
- 取消载荷以 `OrderCancel.ts:12-16`；组合腿以 `ComboLeg.ts:12-22`；条件、分配和软美元对象分别以 `OrderCondition.ts:12-26`、`OrderAllocation.ts:12-20`、`SoftDollarTier.ts:12-16` 为全集。
- `Position`、`PlaceOrderResult`、`OpenOrder`、账户、行情、bar、时钟和子账户以 `broker.ts:99-180,226-320,355-364,472-475`；操作、结果、状态和 staging 字段以 `git.ts:22-77,82-89,178-192,256-306`。
- `OrderStatus`、`Execution`、`OrderState` 的叶字段分别以 `OrderStatus.ts:12-24`、`Execution.ts:12-34`、`OrderState.ts:13-44` 为全集。
- `ContractDetails`、描述、DeltaNeutral 与不可交易原因以 `ContractDetails.ts:13-85`、`ContractDescription.ts:13-16`、`DeltaNeutralContract.ts:12-16`、`IneligibilityReason.ts:12-15`；订单簿字段以 `services/uta/src/domain/trading/brokers/ccxt/ccxt-types.ts:68-75`；历史字段以 `packages/uta-protocol/src/types/history.ts:16-76`。

## 1. Alice 发出的指令载荷

### 1.1 操作外层（`git.ts`）

|来源字段|处置|02 字段或原因|
|---|---|---|
|`Operation.action`|保留|`InstructionPayload` 的 `entrust`、`modify`、`cancel`、`close` oneof；`syncOrders`、`observeExternalOrder`、`reconcileBalance` 不是客户端下发指令。|
|`Operation.contract`|保留|`Contract contract`；逐叶见 2.1。|
|`Operation.order`|保留|`OrderRequest order`；逐叶见 1.2。|
|`Operation.tpsl`|保留|`EntrustPayload.tpsl` → `TakeProfitStopLoss`。|
|`Operation.orderId`（改单）|保留|目标 oneof 的 `BrokerNativeId`；客户端只能引用已知原生字符串 id。|
|`Operation.changes`|保留|`OrderChanges changes`；字段逐叶与 1.2 的可改单字段一致，源缺失保持 optional。|
|`Operation.quantity`（平仓）|规范化|`ClosePayload.quantity`，`Decimal` 写十进制字符串；缺失与全量平仓由 optional 区分。|
|`Operation.orderCancel`|保留|`CancelPayload.order_cancel` → `OrderCancel`。|
|`Operation.aliceId`（余额对账）|不在范围|该操作不是 Alice 发出的委托；若作为帐票读出的历史扩展出现，按 `HistoryContract.alice_id` 保留。|
|`Operation.quantityDelta`|规范化|仅 `reconcileBalance` 历史记录的十进制字符串；不进入指令 oneof。|
|`Operation.markPrice`|规范化|仅 `reconcileBalance` 历史记录的十进制字符串；不进入指令 oneof。|
|`Operation.symbol`|保留|仅结果行归因提示，进入 `OperationResult.symbol` optional；不取代 `alice_id`。|

### 1.1a staging 参数（`git.ts:256-306`）

|来源字段|处置|02 字段或原因|
|---|---|---|
|`StagePlaceOrderParams.aliceId`|保留|`Contract.alice_id`；来源前缀不可移除。|
|`StagePlaceOrderParams.symbol`|保留|解析前的可选提示；解析后由 `Contract.symbol` 承载。|
|`StagePlaceOrderParams.subAccountId`|保留|`Instruction.sub_account_id`；请求目标子账户，不写入 broker `Order`，且能力未枚举子账户时创建期拒绝。|
|`StagePlaceOrderParams.action`|保留|`OrderRequest.action`。|
|`StagePlaceOrderParams.orderType`|保留|`OrderRequest.order_type`。|
|`StagePlaceOrderParams.totalQuantity`|规范化|`OrderRequest.total_quantity` 十进制字符串。|
|`StagePlaceOrderParams.cashQty`|规范化|`OrderRequest.cash_quantity` 十进制字符串。|
|`StagePlaceOrderParams.lmtPrice`|规范化|`OrderRequest.limit_price` 十进制字符串。|
|`StagePlaceOrderParams.auxPrice`|规范化|`OrderRequest.aux_price` 十进制字符串。|
|`StagePlaceOrderParams.trailStopPrice`|规范化|`OrderRequest.trail_stop_price` 十进制字符串。|
|`StagePlaceOrderParams.trailingPercent`|规范化|`OrderRequest.trailing_percent` 十进制字符串。|
|`StagePlaceOrderParams.tif`|保留|`OrderRequest.tif`。|
|`StagePlaceOrderParams.goodTillDate`|保留|`OrderRequest.good_till_date` 原字符串。|
|`StagePlaceOrderParams.outsideRth`|保留|`OrderRequest.outside_rth` optional。|
|`StagePlaceOrderParams.parentId`|规范化|`OrderRequest.parent_id` 十进制字符串。|
|`StagePlaceOrderParams.ocaGroup`|保留|`OrderRequest.oca_group`。|
|`StagePlaceOrderParams.takeProfit.price`|规范化|`EntrustPayload.tpsl.take_profit_price` 十进制字符串。|
|`StagePlaceOrderParams.stopLoss.price`|规范化|`EntrustPayload.tpsl.stop_loss_price` 十进制字符串。|
|`StagePlaceOrderParams.stopLoss.limitPrice`|规范化|`EntrustPayload.tpsl.stop_loss_limit_price` 十进制字符串。|
|`StageModifyOrderParams.orderId`|保留|改单目标 `BrokerNativeId`；数字形状只能另存 numeric 观察值。|
|`StageModifyOrderParams.totalQuantity`|规范化|`OrderChanges.total_quantity` 十进制字符串。|
|`StageModifyOrderParams.lmtPrice`|规范化|`OrderChanges.limit_price` 十进制字符串。|
|`StageModifyOrderParams.auxPrice`|规范化|`OrderChanges.aux_price` 十进制字符串。|
|`StageModifyOrderParams.trailStopPrice`|规范化|`OrderChanges.trail_stop_price` 十进制字符串。|
|`StageModifyOrderParams.trailingPercent`|规范化|`OrderChanges.trailing_percent` 十进制字符串。|
|`StageModifyOrderParams.orderType`|保留|`OrderChanges.order_type`。|
|`StageModifyOrderParams.tif`|保留|`OrderChanges.tif`。|
|`StageModifyOrderParams.goodTillDate`|保留|`OrderChanges.good_till_date` 原字符串。|
|`StageClosePositionParams.aliceId`|保留|`ClosePayload.contract.alice_id`。|
|`StageClosePositionParams.symbol`|保留|解析前的可选提示；解析后由 `ClosePayload.contract.symbol` 承载。|
|`StageClosePositionParams.qty`|规范化|`ClosePayload.quantity` optional 十进制字符串。|
|`StageClosePositionParams.subAccountId`|保留|`ClosePayload.sub_account_id`；请求目标子账户，且能力未枚举子账户时创建期拒绝。|

### 1.2 `Order` 逐叶字段

下表中的“不在范围”不是删除：实际值进入 `OrderRequest.extensions` 或响应对象的 `TypedExtensions`，并有 `RawEvidenceSummary`。`parentId`、`clientId`、`orderId`、`permId` 等数字标识只保留为十进制字符串观察值，不能当作 `BrokerNativeId`。

|来源字段|处置|02 字段或原因|
|---|---|---|
|`clientId`|规范化|`OrderRequest.numeric_client_id`；整数写十进制字符串。|
|`orderId`|规范化|`OrderRequest.numeric_order_id`；仅是数字观察值，原生关联必须使用 `BrokerNativeId`。|
|`permId`|规范化|`OrderRequest.numeric_perm_id`；整数写十进制字符串。|
|`parentId`|规范化|`OrderRequest.parent_id`；数字形状写十进制字符串，不解释层级。|
|`action`|保留|`OrderRequest.action`。|
|`totalQuantity`|规范化|`OrderRequest.total_quantity`，十进制字符串。|
|`displaySize`|规范化|`OrderRequest.display_size`，十进制字符串。|
|`orderType`|保留|`OrderRequest.order_type` 原样。|
|`lmtPrice`|规范化|`OrderRequest.limit_price`，十进制字符串。|
|`auxPrice`|规范化|`OrderRequest.aux_price`，十进制字符串。|
|`tif`|保留|`OrderRequest.tif` 原样。|
|`account`|保留|`OrderRequest.account`；账户身份仍来自传输上下文。|
|`settlingFirm`|不在范围|清算私有字段；进入扩展与摘要。|
|`clearingAccount`|不在范围|清算私有字段；进入扩展与摘要。|
|`clearingIntent`|不在范围|清算私有字段；进入扩展与摘要。|
|`allOrNone`|保留|`OrderRequest.all_or_none` optional。|
|`blockOrder`|不在范围|broker 私有执行控制；进入扩展与摘要。|
|`hidden`|保留|`OrderRequest.hidden` optional。|
|`outsideRth`|保留|`OrderRequest.outside_rth` optional。|
|`sweepToFill`|不在范围|执行路由控制；进入扩展与摘要。|
|`percentOffset`|规范化|`OrderRequest.extensions` 的 decimal 值；当前 02 不把它解释为统一订单语义。|
|`trailingPercent`|规范化|`OrderRequest.trailing_percent`，十进制字符串。|
|`trailStopPrice`|规范化|`OrderRequest.trail_stop_price`，十进制字符串。|
|`minQty`|规范化|扩展中的十进制字符串；没有 v2 最小成交量语义。|
|`goodAfterTime`|保留|`OrderRequest.good_after_time` 原始时间字符串。|
|`goodTillDate`|保留|`OrderRequest.good_till_date` 原始时间字符串。|
|`ocaGroup`|保留|`OrderRequest.oca_group`。|
|`orderRef`|保留|`OrderRequest.order_ref`。|
|`rule80A`|不在范围|IBKR 分类控制；进入扩展与摘要。|
|`ocaType`|规范化|扩展中的整数十进制字符串；不解释枚举。|
|`triggerMethod`|规范化|扩展中的整数十进制字符串；不解释枚举。|
|`activeStartTime`|不在范围|broker 私有活动窗口；进入扩展与摘要。|
|`activeStopTime`|不在范围|broker 私有活动窗口；进入扩展与摘要。|
|`faGroup`|不在范围|顾问分配字段；进入扩展与摘要。|
|`faMethod`|不在范围|顾问分配字段；进入扩展与摘要。|
|`faPercentage`|规范化|扩展中的十进制字符串；不解释分配规则。|
|`volatility`|规范化|扩展中的十进制字符串；当前没有统一波动率订单语义。|
|`volatilityType`|规范化|扩展中的整数十进制字符串；不解释枚举。|
|`continuousUpdate`|不在范围|波动率订单控制；进入扩展与摘要。|
|`referencePriceType`|不在范围|波动率订单控制；进入扩展与摘要。|
|`deltaNeutralOrderType`|不在范围|Delta-neutral 私有订单控制；进入扩展与摘要。|
|`deltaNeutralAuxPrice`|规范化|扩展中的十进制字符串。|
|`deltaNeutralConId`|规范化|扩展中的整数十进制字符串。|
|`deltaNeutralOpenClose`|不在范围|Delta-neutral 私有订单控制；进入扩展与摘要。|
|`deltaNeutralShortSale`|不在范围|Delta-neutral 私有订单控制；进入扩展与摘要。|
|`deltaNeutralShortSaleSlot`|规范化|扩展中的整数十进制字符串。|
|`deltaNeutralDesignatedLocation`|不在范围|Delta-neutral 私有订单控制；进入扩展与摘要。|
|`scaleInitLevelSize`|规范化|扩展中的十进制字符串。|
|`scaleSubsLevelSize`|规范化|扩展中的十进制字符串。|
|`scalePriceIncrement`|规范化|扩展中的十进制字符串。|
|`scalePriceAdjustValue`|规范化|扩展中的十进制字符串。|
|`scalePriceAdjustInterval`|规范化|扩展中的十进制字符串。|
|`scaleProfitOffset`|规范化|扩展中的十进制字符串。|
|`scaleAutoReset`|不在范围|Scale 私有控制；进入扩展与摘要。|
|`scaleInitPosition`|规范化|扩展中的十进制字符串。|
|`scaleInitFillQty`|规范化|扩展中的十进制字符串。|
|`scaleRandomPercent`|不在范围|Scale 私有控制；进入扩展与摘要。|
|`scaleTable`|不在范围|Scale 私有结构；进入扩展与摘要。|
|`hedgeType`|不在范围|Hedge 私有控制；进入扩展与摘要。|
|`hedgeParam`|不在范围|Hedge 私有控制；进入扩展与摘要。|
|`algoStrategy`|不在范围|算法私有控制；进入扩展与摘要。|
|`algoParams`|不在范围|字符串键值算法参数；进入 `TypedExtensions`，不由核心解释。|
|`algoId`|不在范围|算法私有标识；进入扩展与摘要。|
|`smartComboRoutingParams`|不在范围|组合路由私有参数；进入扩展与摘要。|
|`whatIf`|不在范围|试算传输控制；不属于 v2 实际委托。|
|`transmit`|不在范围|传输控制；不属于 v2 实际委托。|
|`overridePercentageConstraints`|不在范围|broker 校验控制；进入扩展与摘要。|
|`openClose`|不在范围|机构订单控制；进入扩展与摘要。|
|`origin`|规范化|扩展中的整数十进制字符串。|
|`shortSaleSlot`|规范化|扩展中的整数十进制字符串。|
|`designatedLocation`|不在范围|机构订单控制；进入扩展与摘要。|
|`exemptCode`|规范化|`OrderRequest.extensions` 中的整数十进制字符串；当前没有统一豁免代码语义。|
|`deltaNeutralSettlingFirm`|不在范围|清算私有字段；进入扩展与摘要。|
|`deltaNeutralClearingAccount`|不在范围|清算私有字段；进入扩展与摘要。|
|`deltaNeutralClearingIntent`|不在范围|清算私有字段；进入扩展与摘要。|
|`discretionaryAmt`|规范化|扩展中的十进制字符串。|
|`optOutSmartRouting`|不在范围|SMART 路由控制；进入扩展与摘要。|
|`startingPrice`|规范化|扩展中的十进制字符串。|
|`stockRefPrice`|规范化|扩展中的十进制字符串。|
|`delta`|规范化|扩展中的十进制字符串。|
|`stockRangeLower`|规范化|扩展中的十进制字符串。|
|`stockRangeUpper`|规范化|扩展中的十进制字符串。|
|`notHeld`|不在范围|执行控制；进入扩展与摘要。|
|`orderMiscOptions`|不在范围|字符串键值私有选项；进入 `TypedExtensions`。|
|`solicited`|不在范围|监管/执行控制；进入扩展与摘要。|
|`randomizeSize`|不在范围|执行控制；进入扩展与摘要。|
|`randomizePrice`|不在范围|执行控制；进入扩展与摘要。|
|`referenceContractId`|规范化|扩展中的整数十进制字符串。|
|`peggedChangeAmount`|规范化|扩展中的十进制字符串。|
|`isPeggedChangeAmountDecrease`|不在范围|Peg 控制；进入扩展与摘要。|
|`referenceChangeAmount`|规范化|扩展中的十进制字符串。|
|`referenceExchangeId`|不在范围|Peg 控制；进入扩展与摘要。|
|`adjustedOrderType`|不在范围|调整后订单控制；进入扩展与摘要。|
|`triggerPrice`|规范化|扩展中的十进制字符串。|
|`adjustedStopPrice`|规范化|扩展中的十进制字符串。|
|`adjustedStopLimitPrice`|规范化|扩展中的十进制字符串。|
|`adjustedTrailingAmount`|规范化|扩展中的十进制字符串。|
|`adjustableTrailingUnit`|规范化|扩展中的整数十进制字符串。|
|`lmtPriceOffset`|规范化|扩展中的十进制字符串。|
|`conditions`|不在范围|条件数组逐叶进入扩展与摘要；见 1.4。|
|`conditionsCancelOrder`|不在范围|条件控制；进入扩展与摘要。|
|`conditionsIgnoreRth`|不在范围|条件控制；进入扩展与摘要。|
|`modelCode`|不在范围|模型分配字段；进入扩展与摘要。|
|`extOperator`|不在范围|broker 私有操作字段；进入扩展与摘要。|
|`softDollarTier`|不在范围|软美元对象逐叶进入扩展与摘要；见 1.5。|
|`cashQty`|规范化|`OrderRequest.cash_quantity`，十进制字符串。|
|`mifid2DecisionMaker`|不在范围|监管字段；进入扩展与摘要。|
|`mifid2DecisionAlgo`|不在范围|监管字段；进入扩展与摘要。|
|`mifid2ExecutionTrader`|不在范围|监管字段；进入扩展与摘要。|
|`mifid2ExecutionAlgo`|不在范围|监管字段；进入扩展与摘要。|
|`dontUseAutoPriceForHedge`|不在范围|Hedge 控制；进入扩展与摘要。|
|`isOmsContainer`|不在范围|broker 容器控制；进入扩展与摘要。|
|`discretionaryUpToLimitPrice`|不在范围|执行控制；进入扩展与摘要。|
|`autoCancelDate`|不在范围|broker 生命周期控制；进入扩展与摘要。|
|`filledQuantity`|规范化|`OrderRequest.filled_quantity`，回读时为十进制字符串；指令提交时缺失。|
|`refFuturesConId`|规范化|扩展中的整数十进制字符串。|
|`autoCancelParent`|不在范围|broker 生命周期控制；进入扩展与摘要。|
|`shareholder`|不在范围|监管字段；进入扩展与摘要。|
|`imbalanceOnly`|不在范围|交易所执行控制；进入扩展与摘要。|
|`routeMarketableToBbo`|规范化|扩展中的整数十进制字符串。|
|`parentPermId`|规范化|扩展中的整数十进制字符串。|
|`usePriceMgmtAlgo`|规范化|扩展中的整数十进制字符串。|
|`duration`|规范化|扩展中的整数十进制字符串。|
|`postToAts`|规范化|扩展中的整数十进制字符串。|
|`advancedErrorOverride`|不在范围|错误处理控制；进入扩展与摘要。|
|`manualOrderTime`|不在范围|人工下单控制；进入扩展与摘要。|
|`minTradeQty`|规范化|扩展中的十进制字符串。|
|`minCompeteSize`|规范化|扩展中的十进制字符串。|
|`competeAgainstBestOffset`|规范化|扩展中的十进制字符串。|
|`midOffsetAtWhole`|规范化|扩展中的十进制字符串。|
|`midOffsetAtHalf`|规范化|扩展中的十进制字符串。|
|`customerAccount`|不在范围|机构字段；进入扩展与摘要。|
|`professionalCustomer`|不在范围|监管字段；进入扩展与摘要。|
|`bondAccruedInterest`|规范化|扩展中的十进制字符串。|
|`includeOvernight`|不在范围|交易时段控制；进入扩展与摘要。|
|`manualOrderIndicator`|规范化|扩展中的整数十进制字符串。|
|`submitter`|不在范围|来源字段；进入扩展与摘要。|
|`deactivate`|不在范围|生命周期控制；进入扩展与摘要。|
|`postOnly`|保留|`OrderRequest.post_only` 与 `OrderChanges.post_only` optional。|
|`allowPreOpen`|不在范围|交易时段控制；进入扩展与摘要。|
|`ignoreOpenAuction`|不在范围|交易时段控制；进入扩展与摘要。|
|`seekPriceImprovement`|规范化|扩展中的整数十进制字符串。|
|`whatIfType`|规范化|扩展中的整数十进制字符串。|

### 1.3 `OrderCancel`、TP-SL 与 legs

|来源字段|处置|02 字段或原因|
|---|---|---|
|`OrderCancel.manualOrderCancelTime`|保留|`OrderCancel.manual_order_cancel_time` optional 原字符串。|
|`OrderCancel.extOperator`|保留|`OrderCancel.ext_operator` optional。|
|`OrderCancel.manualOrderIndicator`|规范化|`OrderCancel.manual_order_indicator`，整数十进制字符串。|
|`TpSlParams.takeProfit.price`|规范化|`TakeProfitStopLoss.take_profit_price`，十进制字符串。|
|`TpSlParams.stopLoss.price`|规范化|`TakeProfitStopLoss.stop_loss_price`，十进制字符串。|
|`TpSlParams.stopLoss.limitPrice`|规范化|`TakeProfitStopLoss.stop_loss_limit_price`，十进制字符串。|
|`PlaceOrderLeg.orderId`|保留|`PlaceOrderLeg.broker_native_id`；这是已创建子委托的 opaque 原生字符串。|
|`PlaceOrderLeg.kind`|保留|`PlaceOrderLeg.kind`，只允许 `take_profit` / `stop_loss`。|

### 1.4 组合腿与条件

|来源字段|处置|02 字段或原因|
|---|---|---|
|`ComboLeg.conId`|规范化|`ContractLeg.numeric_con_id`，整数十进制字符串；有 broker 原生字符串时另填 `broker_native_id`。|
|`ComboLeg.ratio`|规范化|`ContractLeg.ratio`，十进制字符串。|
|`ComboLeg.action`|保留|`ContractLeg.action` optional。|
|`ComboLeg.exchange`|保留|`ContractLeg.exchange` optional。|
|`ComboLeg.openClose`|规范化|`ContractLeg.open_close`，整数十进制字符串，不解释枚举。|
|`ComboLeg.shortSalesSlot`|规范化|`ContractLeg.short_sales_slot`，整数十进制字符串。|
|`ComboLeg.designatedLocation`|保留|`ContractLeg.designated_location` optional。|
|`ComboLeg.exemptCode`|规范化|`ContractLeg.exempt_code`，整数十进制字符串。|
|`ComboLeg.perLegPrice`|规范化|`ContractLeg.per_leg_price`，十进制字符串。|
|`OrderCondition.type`|规范化|扩展中的整数十进制字符串；v2 不解释条件编码。|
|`OrderCondition.isConjunctionConnection`|不在范围|条件组合控制；进入扩展与摘要。|
|`OrderCondition.isMore`|不在范围|条件比较控制；进入扩展与摘要。|
|`OrderCondition.conId`|规范化|扩展中的整数十进制字符串。|
|`OrderCondition.exchange`|保留|扩展中的字符串。|
|`OrderCondition.symbol`|保留|扩展中的字符串。|
|`OrderCondition.secType`|保留|扩展中的字符串。|
|`OrderCondition.percent`|规范化|扩展中的十进制字符串。|
|`OrderCondition.changePercent`|规范化|扩展中的十进制字符串。|
|`OrderCondition.price`|规范化|扩展中的十进制字符串。|
|`OrderCondition.triggerMethod`|规范化|扩展中的整数十进制字符串。|
|`OrderCondition.time`|保留|扩展中的原始时间字符串。|
|`OrderCondition.volume`|规范化|扩展中的十进制字符串。|

### 1.5 订单扩展对象

|来源字段|处置|02 字段或原因|
|---|---|---|
|`OrderAllocation.account`|保留|扩展字符串。|
|`OrderAllocation.position`|规范化|扩展中的十进制字符串。|
|`OrderAllocation.positionDesired`|规范化|扩展中的十进制字符串。|
|`OrderAllocation.positionAfter`|规范化|扩展中的十进制字符串。|
|`OrderAllocation.desiredAllocQty`|规范化|扩展中的十进制字符串。|
|`OrderAllocation.allowedAllocQty`|规范化|扩展中的十进制字符串。|
|`OrderAllocation.isMonetary`|保留|扩展布尔值。|
|`SoftDollarTier.name`|保留|扩展字符串。|
|`SoftDollarTier.value`|保留|扩展字符串。|
|`SoftDollarTier.displayName`|保留|扩展字符串。|

## 2. broker 回票与账户对账

### 2.1 `PlaceOrderResult` / `OperationResult`

|来源字段|归属|02 字段或原因|
|---|---|---|
|`PlaceOrderResult.success`|结果元数据|`BrokerPlacementResult.success`。|
|`PlaceOrderResult.orderId`|结果元数据|映射为 optional `BrokerPlacementResult.broker_native_id`；broker 已分配 id 时必须有值，未分配时由能力声明说明。|
|`PlaceOrderResult.error`|结果元数据|`BrokerPlacementResult.error` optional。|
|`PlaceOrderResult.message`|结果元数据|`BrokerPlacementResult.message` optional。|
|`PlaceOrderResult.execution`|成交回票|`BrokerPlacementResult.execution` → `ExecutionReceipt`；逐笔字段见 2.4。|
|`PlaceOrderResult.orderState`|状态回票|`BrokerPlacementResult.order_state` → `OrderState`；状态回票中的完整状态快照见 2.3。|
|`PlaceOrderResult.legs`|结果元数据|`BrokerPlacementResult.legs[]`；每个 id 为 opaque 原生字符串。|
|`OperationResult.action`|结果元数据|`BrokerPlacementResult.action`，保存结果行的指令种类。|
|`OperationResult.success`|结果元数据|进入 `BrokerPlacementResult.success`，不替代状态回票。|
|`OperationResult.orderId`|结果元数据|进入 `BrokerPlacementResult.broker_native_id`，数字 id 不替代原生字符串 id。|
|`OperationResult.status`|状态回票|进入 `OrderStateReceipt.original_status` 与 `OrderStateReceipt.mapped_state`。|
|`OperationResult.execution`|成交回票|进入 `ExecutionReceipt`；不能放入状态回票。|
|`OperationResult.orderState`|状态回票|进入 `OrderStateReceipt.order_state`；不能放入成交回票。|
|`OperationResult.filledQty`|状态回票|进入 `OrderStateReceipt.cumulative_quantity`，十进制字符串；这是累计值，不是逐笔数量。|
|`OperationResult.filledPrice`|状态回票|进入 `OrderStateReceipt.average_fill_price`，十进制字符串；`UnifiedTradingAccount.ts:880-904` 以平均成交价记录同步值。|
|`OperationResult.error`|结果元数据|进入 `BrokerPlacementResult.error` optional。|
|`OperationResult.legs`|结果元数据|进入 `BrokerPlacementResult.legs[]`。|
|`OperationResult.symbol`|结果元数据|进入 `BrokerPlacementResult.symbol` optional，只作结果行归因提示。|
|`OperationResult.raw`|不在范围|任意对象不进入核心类型；摘要写 `RawEvidenceSummary`，可识别标量写 `TypedExtensions`。|

### 2.2 `OpenOrder`

|来源字段|归属|02 字段或原因|
|---|---|---|
|`OpenOrder.contract`|状态读面|`OpenOrder.contract`；逐叶见 3.1。|
|`OpenOrder.order`|状态读面|`OpenOrder.order`；逐叶见 1.2。|
|`OpenOrder.order.filledQuantity`|状态读面|嵌套 `OrderRequest.filled_quantity`，十进制字符串；`UnifiedTradingAccount.ts:880-904` 从此字段与 `OpenOrder.avgFillPrice` 记录同步成交；订单读面没有该值时按字段能力报告 unavailable。|
|`OpenOrder.orderState`|状态读面|`OpenOrder.order_state`；逐叶见 2.3。|
|`OpenOrder.orderId`|状态读面|映射为 optional `OpenOrder.broker_native_id`；broker 已分配即必填，能力可明确不可提供。|
|`OpenOrder.avgFillPrice`|状态读面|`OpenOrder.avg_fill_price` optional 十进制字符串。|
|`OpenOrder.tpsl`|状态读面|`OpenOrder.tpsl`；TP-SL 子委托 id 在 legs 或扩展中保留。|

### 2.3 `OrderState`

下表的每个叶字段均属于状态回票；`OrderStateReceipt.order_state` 保留完整 broker 状态快照，顶层的 `original_status`、`mapped_state`、`cumulative_quantity`、`remaining`、`average_fill_price` 供状态折叠直接读取。

|来源字段|归属|02 字段或原因|
|---|---|---|
|`status`|状态回票|`OrderStateReceipt.order_state.status` optional 原值；顶层同时保留 `original_status` 并写入 `mapped_state`。|
|`initMarginBefore`|状态回票|`OrderStateReceipt.order_state.init_margin_before` optional 十进制字符串。|
|`maintMarginBefore`|状态回票|`OrderStateReceipt.order_state.maint_margin_before` optional 十进制字符串。|
|`equityWithLoanBefore`|状态回票|`OrderStateReceipt.order_state.equity_with_loan_before` optional 十进制字符串。|
|`initMarginChange`|状态回票|`OrderStateReceipt.order_state.init_margin_change` optional 十进制字符串。|
|`maintMarginChange`|状态回票|`OrderStateReceipt.order_state.maint_margin_change` optional 十进制字符串。|
|`equityWithLoanChange`|状态回票|`OrderStateReceipt.order_state.equity_with_loan_change` optional 十进制字符串。|
|`initMarginAfter`|状态回票|`OrderStateReceipt.order_state.init_margin_after` optional 十进制字符串。|
|`maintMarginAfter`|状态回票|`OrderStateReceipt.order_state.maint_margin_after` optional 十进制字符串。|
|`equityWithLoanAfter`|状态回票|`OrderStateReceipt.order_state.equity_with_loan_after` optional 十进制字符串。|
|`commissionAndFees`|状态回票|`OrderStateReceipt.order_state.commission_and_fees` optional 十进制字符串；这是状态快照估算，不替代逐笔佣金。|
|`minCommissionAndFees`|状态回票|`OrderStateReceipt.order_state.min_commission_and_fees` optional 十进制字符串。|
|`maxCommissionAndFees`|状态回票|`OrderStateReceipt.order_state.max_commission_and_fees` optional 十进制字符串。|
|`commissionAndFeesCurrency`|状态回票|`OrderStateReceipt.order_state.commission_and_fees_currency` optional。|
|`marginCurrency`|状态回票|`OrderStateReceipt.order_state.margin_currency` optional。|
|`initMarginBeforeOutsideRTH`|状态回票|`OrderStateReceipt.order_state.init_margin_before_outside_rth` optional 十进制字符串。|
|`maintMarginBeforeOutsideRTH`|状态回票|`OrderStateReceipt.order_state.maint_margin_before_outside_rth` optional 十进制字符串。|
|`equityWithLoanBeforeOutsideRTH`|状态回票|`OrderStateReceipt.order_state.equity_with_loan_before_outside_rth` optional 十进制字符串。|
|`initMarginChangeOutsideRTH`|状态回票|`OrderStateReceipt.order_state.init_margin_change_outside_rth` optional 十进制字符串。|
|`maintMarginChangeOutsideRTH`|状态回票|`OrderStateReceipt.order_state.maint_margin_change_outside_rth` optional 十进制字符串。|
|`equityWithLoanChangeOutsideRTH`|状态回票|`OrderStateReceipt.order_state.equity_with_loan_change_outside_rth` optional 十进制字符串。|
|`initMarginAfterOutsideRTH`|状态回票|`OrderStateReceipt.order_state.init_margin_after_outside_rth` optional 十进制字符串。|
|`equityWithLoanAfterOutsideRTH`|状态回票|`OrderStateReceipt.order_state.equity_with_loan_after_outside_rth` optional 十进制字符串。|
|`suggestedSize`|状态回票|`OrderStateReceipt.order_state.suggested_size` optional 十进制字符串。|
|`rejectReason`|状态回票|`OrderStateReceipt.order_state.reject_reason` optional 原文。|
|`orderAllocations`|状态回票|`OrderStateReceipt.order_state.order_allocations[]`；叶字段按 1.5 保真。|
|`warningText`|状态回票|`OrderStateReceipt.order_state.warning_text` optional 原文。|
|`completedTime`|状态回票|`OrderStateReceipt.order_state.completed_time` optional 原始时间字符串，不擅自解析。|
|`completedStatus`|状态回票|`OrderStateReceipt.order_state.completed_status` optional 原值。|

### 2.4 `Execution` → `ExecutionReceipt`（成交回票）

以下叶字段全集依据 `packages/ibkr/src/protobuf/Execution.ts:12-34`。每一行均属于成交回票；`cumQty`、`avgPrice` 虽是来源中可能出现的累计或平均观察值，也不改变成交回票按 broker 成交 id 逐笔保存的语义。

|来源字段|归属|02 字段或原因|
|---|---|---|
|`orderId`|成交回票|`ExecutionReceipt.numeric_order_id` optional 十进制字符串；不替代 `ExecutionReceipt.broker_native_id`。|
|`execId`|成交回票|`ExecutionReceipt.broker_execution_id`；成交回票去重键，缺失时 typed 拒绝。|
|`time`|成交回票|`ExecutionReceipt.broker_time` optional 原始时间字符串。|
|`acctNumber`|成交回票|`ExecutionReceipt.acct_number` optional。|
|`exchange`|成交回票|`ExecutionReceipt.exchange` optional。|
|`side`|成交回票|`ExecutionReceipt.side` optional 原值。|
|`shares`|成交回票|`ExecutionReceipt.single_quantity` optional 十进制字符串；这是单笔数量。|
|`price`|成交回票|`ExecutionReceipt.single_price` optional 十进制字符串；这是单笔价格。|
|`permId`|成交回票|`ExecutionReceipt.numeric_perm_id` optional 十进制字符串。|
|`clientId`|成交回票|`ExecutionReceipt.numeric_client_id` optional 十进制字符串。|
|`isLiquidation`|成交回票|`ExecutionReceipt.is_liquidation` optional。|
|`cumQty`|成交回票|`ExecutionReceipt.cumulative_quantity` optional 十进制字符串；保留来源叶字段，不参与状态回票折叠。|
|`avgPrice`|成交回票|`ExecutionReceipt.average_price` optional 十进制字符串；保留来源叶字段，不替代单笔价格。|
|`orderRef`|成交回票|`ExecutionReceipt.order_ref` optional。|
|`evRule`|成交回票|`ExecutionReceipt.ev_rule` optional。|
|`evMultiplier`|成交回票|`ExecutionReceipt.ev_multiplier` optional 十进制字符串。|
|`modelCode`|成交回票|`ExecutionReceipt.model_code` optional。|
|`lastLiquidity`|成交回票|`ExecutionReceipt.last_liquidity` optional 整数十进制字符串。|
|`isPriceRevisionPending`|成交回票|`ExecutionReceipt.is_price_revision_pending` optional。|
|`submitter`|成交回票|`ExecutionReceipt.submitter` optional。|
|`optExerciseOrLapseType`|成交回票|`ExecutionReceipt.opt_exercise_or_lapse_type` optional 整数十进制字符串。|

### 2.5 `CommissionAndFeesReport` → `ExecutionReceipt` / `佣金补录`

`CommissionAndFeesReport` 的叶字段全集依据 `packages/ibkr/src/protobuf/CommissionAndFeesReport.ts:12-19`；这些叶字段已并入 `ExecutionReceipt`，不是第三种回票载荷。

|来源字段|归属|02 字段或原因|
|---|---|---|
|`execId`|成交回票|与 `ExecutionReceipt.broker_execution_id` 相同，用于关联逐笔成交。|
|`commissionAndFees`|成交回票|`ExecutionReceipt.commission_and_fees` optional 十进制字符串。|
|`currency`|成交回票|`ExecutionReceipt.commission_currency` optional。|
|`realizedPNL`|成交回票|`ExecutionReceipt.realized_pnl` optional 十进制字符串。|
|`bondYield`|成交回票|`ExecutionReceipt.bond_yield` optional 十进制字符串。|
|`yieldRedemptionDate`|成交回票|`ExecutionReceipt.yield_redemption_date` optional。|

同一 broker 成交 id 的成交回票佣金从无到有时，追加 `CommissionSupplement` 佣金补录并引用该 id，不修改原成交回票；没有该 id 不能建立这条关联。

### 2.6 `OrderStatus` → `OrderStateReceipt`（状态回票）

以下叶字段全集依据 `packages/ibkr/src/protobuf/OrderStatus.ts:12-24`；每一行均属于状态回票，不属于成交回票。

|来源字段|归属|02 字段或原因|
|---|---|---|
|`orderId`|状态回票|`OrderStateReceipt.numeric_order_id` optional 十进制字符串。|
|`status`|状态回票|`OrderStateReceipt.original_status` 原值，并映射为 `OrderStateReceipt.mapped_state`。|
|`filled`|状态回票|`OrderStateReceipt.cumulative_quantity` optional 十进制字符串；累计成交量不递增相加。|
|`remaining`|状态回票|`OrderStateReceipt.remaining` optional 十进制字符串。|
|`avgFillPrice`|状态回票|`OrderStateReceipt.average_fill_price` optional 十进制字符串。|
|`permId`|状态回票|`OrderStateReceipt.numeric_perm_id` optional 十进制字符串。|
|`parentId`|状态回票|`OrderStateReceipt.numeric_parent_id` optional 十进制字符串。|
|`lastFillPrice`|状态回票|`OrderStateReceipt.last_fill_price` optional 十进制字符串。|
|`clientId`|状态回票|`OrderStateReceipt.numeric_client_id` optional 十进制字符串。|
|`whyHeld`|状态回票|`OrderStateReceipt.why_held` optional 原值。|
|`mktCapPrice`|状态回票|`OrderStateReceipt.market_cap_price` optional 十进制字符串。|

### 2.7 六个集成逐笔成交 id 现状

下表只审计 `services/uta/src/domain/trading/brokers/` 当前适配器源码；“未见”表示所列路径中没有观察到可提交的逐笔成交 id，不推断 broker 本身的能力。

|集成|当前源码观察|逐笔成交（broker 成交）id 当前可用性|
|---|---|---|
|IBKR|`services/uta/src/domain/trading/brokers/ibkr/request-bridge.ts:750-766` 处理 `orderStatus` 的累计成交量、剩余和平均成交价；`services/uta/src/domain/trading/brokers/ibkr/IbkrBroker.ts:480-503` 返回原生订单 id 与状态。|未见|
|Alpaca|`services/uta/src/domain/trading/brokers/alpaca/alpaca-types.ts:33-52` 的订单字段含累计成交量与平均价；`services/uta/src/domain/trading/brokers/alpaca/alpaca-types.ts:70-81` 的成交活动含订单 id、数量和价格但未见逐笔成交 id；`services/uta/src/domain/trading/brokers/alpaca/AlpacaBroker.ts:758-783` 映射累计数量与平均价。|未见|
|Longbridge|`services/uta/src/domain/trading/brokers/longbridge/longbridge-types.ts:76-90` 的订单字段含订单 id、执行数量与执行价；`services/uta/src/domain/trading/brokers/longbridge/LongbridgeBroker.ts:696-712` 映射订单 id、状态与平均成交价。|未见|
|CCXT|`services/uta/src/domain/trading/brokers/ccxt/CcxtBroker.ts:1127-1183` 读取订单 id、累计成交量与平均价；`services/uta/src/domain/trading/brokers/ccxt/CcxtBroker.ts:633-637` 的委托结果返回订单 id 与状态。|未见|
|LeverUp|`services/uta/src/domain/trading/brokers/others/leverup/relayer-client.ts:42-52` 记录提交哈希与状态；`services/uta/src/domain/trading/brokers/others/leverup/reader-client.ts:31-57` 读取数量、价格、费用和时间；`services/uta/src/domain/trading/brokers/others/leverup/LeverupBroker.ts:460-491` 返回状态。提交哈希不当作逐笔成交 id。|未见|
|Mock|`services/uta/src/domain/trading/brokers/mock/MockBroker.ts:65-76` 的内部订单只有订单 id、成交价、成交数量与平均成交价；`services/uta/src/domain/trading/brokers/mock/MockBroker.ts:278-324`、`services/uta/src/domain/trading/brokers/mock/MockBroker.ts:464-476`、`services/uta/src/domain/trading/brokers/mock/MockBroker.ts:580-621` 仅更新或读取累计字段。|未见|

在所查六个 `services/uta` 集成适配器源码中均未见当前可提交的逐笔成交 id；因此没有 id 的集成只能提交状态回票，并在能力声明中写明 `逐笔成交：无`。这不是对 broker 能力的推断；若集成另有稳定 id，应按 `ExecutionReceipt.broker_execution_id` 提交并声明 `逐笔成交：有`。

## 3. N3 一次性读响应

### 3.1 `Contract`、搜索描述与详情

|来源字段|处置|02 字段或原因|
|---|---|---|
|`Contract.aliceId`|保留|`Contract.alice_id` optional；来源前缀不可移除。|
|`Contract.conId`|规范化|`Contract.numeric_con_id` optional 十进制字符串；`BrokerNativeId` 仍承载 broker 原生 opaque 字符串。|
|`Contract.symbol`|保留|`Contract.symbol` optional。|
|`Contract.secType`|保留|`Contract.sec_type` optional。|
|`Contract.lastTradeDateOrContractMonth`|保留|`Contract.last_trade_date_or_contract_month` optional 原值。|
|`Contract.strike`|规范化|`Contract.strike` optional 十进制字符串。|
|`Contract.right`|保留|`Contract.right` optional。|
|`Contract.multiplier`|规范化|`Contract.multiplier` optional 十进制字符串。|
|`Contract.exchange`|保留|`Contract.exchange` optional。|
|`Contract.primaryExch`|保留|`Contract.primary_exchange` optional。|
|`Contract.currency`|保留|`Contract.currency` optional。|
|`Contract.localSymbol`|保留|`Contract.local_symbol` optional。|
|`Contract.tradingClass`|保留|`Contract.trading_class` optional。|
|`Contract.secIdType`|保留|`Contract.sec_id_type` optional。|
|`Contract.secId`|保留|`Contract.sec_id` optional。|
|`Contract.description`|保留|`Contract.description` optional。|
|`Contract.issuerId`|保留|`Contract.issuer_id` optional。|
|`Contract.deltaNeutralContract`|不在范围|当前没有统一 Delta-neutral 合约语义；其叶字段进入扩展与摘要。|
|`Contract.includeExpired`|不在范围|查询控制字段，不是返回的合约身份；若 broker 回显进入扩展。|
|`Contract.comboLegsDescrip`|保留|`Contract.combo_legs_description` optional。|
|`Contract.comboLegs`|保留|`Contract.combo_legs[]`，叶字段见 1.4。|
|`Contract.lastTradeDate`|保留|`Contract.last_trade_date` optional 原值。|
|`ContractDescription.contract`|保留|`ContractDescription.contract` optional。|
|`ContractDescription.derivativeSecTypes`|保留|`ContractDescription.derivative_sec_types[]`，搜索不能丢失衍生品类别。|
|`ContractDetails.marketName`|保留|`ContractDetails.market_name` optional。|
|`ContractDetails.minTick`|规范化|`ContractDetails.min_tick` optional 十进制字符串。|
|`ContractDetails.orderTypes`|保留|`ContractDetails.order_types` optional。|
|`ContractDetails.validExchanges`|保留|`ContractDetails.valid_exchanges` optional。|
|`ContractDetails.priceMagnifier`|规范化|`ContractDetails.price_magnifier` optional 十进制字符串。|
|`ContractDetails.underConId`|规范化|`ContractDetails.numeric_under_con_id` optional 十进制字符串。|
|`ContractDetails.longName`|保留|`ContractDetails.long_name` optional。|
|`ContractDetails.contractMonth`|保留|`ContractDetails.contract_month` optional。|
|`ContractDetails.industry`|保留|`ContractDetails.industry` optional。|
|`ContractDetails.category`|保留|`ContractDetails.category` optional。|
|`ContractDetails.subcategory`|保留|`ContractDetails.subcategory` optional。|
|`ContractDetails.timeZoneId`|保留|`ContractDetails.time_zone_id` optional。|
|`ContractDetails.tradingHours`|保留|`ContractDetails.trading_hours` optional 原文。|
|`ContractDetails.liquidHours`|保留|`ContractDetails.liquid_hours` optional 原文。|
|`ContractDetails.evRule`|保留|`ContractDetails.ev_rule` optional。|
|`ContractDetails.evMultiplier`|规范化|`ContractDetails.ev_multiplier` optional 十进制字符串。|
|`ContractDetails.secIdList`|保留|`ContractDetails.sec_id_list` 字符串键值；不是无类型响应。|
|`ContractDetails.aggGroup`|规范化|`ContractDetails.agg_group` optional 整数十进制字符串。|
|`ContractDetails.underSymbol`|保留|`ContractDetails.under_symbol` optional。|
|`ContractDetails.underSecType`|保留|`ContractDetails.under_sec_type` optional。|
|`ContractDetails.marketRuleIds`|保留|`ContractDetails.market_rule_ids` optional。|
|`ContractDetails.realExpirationDate`|保留|`ContractDetails.real_expiration_date` optional。|
|`ContractDetails.stockType`|保留|`ContractDetails.stock_type` optional。|
|`ContractDetails.minSize`|规范化|`ContractDetails.min_size` optional 十进制字符串。|
|`ContractDetails.sizeIncrement`|规范化|`ContractDetails.size_increment` optional 十进制字符串。|
|`ContractDetails.suggestedSizeIncrement`|规范化|`ContractDetails.suggested_size_increment` optional 十进制字符串。|
|`ContractDetails.fundName`|保留|`ContractDetails.fund_name` optional。|
|`ContractDetails.fundFamily`|保留|`ContractDetails.fund_family` optional。|
|`ContractDetails.fundType`|保留|`ContractDetails.fund_type` optional。|
|`ContractDetails.fundFrontLoad`|规范化|`ContractDetails.fund_front_load` optional 十进制字符串。|
|`ContractDetails.fundBackLoad`|规范化|`ContractDetails.fund_back_load` optional 十进制字符串。|
|`ContractDetails.fundBackLoadTimeInterval`|保留|`ContractDetails.fund_back_load_time_interval` optional。|
|`ContractDetails.fundManagementFee`|规范化|`ContractDetails.fund_management_fee` optional 十进制字符串。|
|`ContractDetails.fundClosed`|保留|`ContractDetails.fund_closed` optional。|
|`ContractDetails.fundClosedForNewInvestors`|保留|`ContractDetails.fund_closed_for_new_investors` optional。|
|`ContractDetails.fundClosedForNewMoney`|保留|`ContractDetails.fund_closed_for_new_money` optional。|
|`ContractDetails.fundNotifyAmount`|规范化|`ContractDetails.fund_notify_amount` optional 十进制字符串。|
|`ContractDetails.fundMinimumInitialPurchase`|规范化|`ContractDetails.fund_minimum_initial_purchase` optional 十进制字符串。|
|`ContractDetails.fundMinimumSubsequentPurchase`|规范化|`ContractDetails.fund_minimum_subsequent_purchase` optional 十进制字符串。|
|`ContractDetails.fundBlueSkyStates`|保留|`ContractDetails.fund_blue_sky_states` optional。|
|`ContractDetails.fundBlueSkyTerritories`|保留|`ContractDetails.fund_blue_sky_territories` optional。|
|`ContractDetails.fundDistributionPolicyIndicator`|保留|`ContractDetails.fund_distribution_policy_indicator` optional。|
|`ContractDetails.fundAssetType`|保留|`ContractDetails.fund_asset_type` optional。|
|`ContractDetails.cusip`|保留|`ContractDetails.cusip` optional。|
|`ContractDetails.issueDate`|保留|`ContractDetails.issue_date` optional。|
|`ContractDetails.ratings`|保留|`ContractDetails.ratings` optional。|
|`ContractDetails.bondType`|保留|`ContractDetails.bond_type` optional。|
|`ContractDetails.coupon`|规范化|`ContractDetails.coupon` optional 十进制字符串。|
|`ContractDetails.couponType`|保留|`ContractDetails.coupon_type` optional。|
|`ContractDetails.convertible`|保留|`ContractDetails.convertible` optional。|
|`ContractDetails.callable`|保留|`ContractDetails.callable` optional。|
|`ContractDetails.puttable`|保留|`ContractDetails.puttable` optional。|
|`ContractDetails.descAppend`|保留|`ContractDetails.desc_append` optional。|
|`ContractDetails.nextOptionDate`|保留|`ContractDetails.next_option_date` optional。|
|`ContractDetails.nextOptionType`|保留|`ContractDetails.next_option_type` optional。|
|`ContractDetails.nextOptionPartial`|保留|`ContractDetails.next_option_partial` optional。|
|`ContractDetails.bondNotes`|保留|`ContractDetails.bond_notes` optional。|
|`ContractDetails.ineligibilityReasonList`|保留|`ContractDetails.ineligibility_reasons[]`；每项 `id`、`description` 均保留。|
|`ContractDetails.eventContract1`|保留|`ContractDetails.event_contract_1` optional。|
|`ContractDetails.eventContractDescription1`|保留|`ContractDetails.event_contract_description_1` optional。|
|`ContractDetails.eventContractDescription2`|保留|`ContractDetails.event_contract_description_2` optional。|
|`ContractDetails.minAlgoSize`|规范化|`ContractDetails.min_algo_size` optional 十进制字符串。|
|`ContractDetails.lastPricePrecision`|规范化|`ContractDetails.last_price_precision` optional 十进制字符串。|
|`ContractDetails.lastSizePrecision`|规范化|`ContractDetails.last_size_precision` optional 十进制字符串。|
|`IneligibilityReason.id`|保留|`IneligibilityReason.id` optional。|
|`IneligibilityReason.description`|保留|`IneligibilityReason.description` optional。|
|`DeltaNeutralContract.conId`|不在范围|没有统一 Delta-neutral 语义；进入扩展与摘要。|
|`DeltaNeutralContract.delta`|不在范围|没有统一 Delta-neutral 语义；进入扩展与摘要。|
|`DeltaNeutralContract.price`|不在范围|没有统一 Delta-neutral 语义；进入扩展与摘要。|

**读面卡点（非字段级 unavailable）**：`IBroker` 只在 `broker.ts:502-513,587-597` 定义合约搜索、详情和可选历史 bar；订单簿在 `UTAAccountSDK.ts:174-175` 转发但不在 `IBroker` 公共接口，历史记录在 `history.ts` 而非 broker 读接口。当前不能据接口缺口推断某 broker 明确不提供这些读面，也不能凭名称补请求字段；实现前需补充权威读接口与能力字段。

### 3.2 Account、Position、子账户

|来源字段|处置|02 字段或原因|
|---|---|---|
|`AccountInfo.baseCurrency`|保留|`AccountInfo.base_currency`。|
|`AccountInfo.netLiquidation`|规范化|`AccountInfo.net_liquidation`，十进制字符串。|
|`AccountInfo.totalCashValue`|规范化|`AccountInfo.total_cash_value`，十进制字符串。|
|`AccountInfo.unrealizedPnL`|规范化|`AccountInfo.unrealized_pnl`，十进制字符串。|
|`AccountInfo.realizedPnL`|规范化|`AccountInfo.realized_pnl` optional 十进制字符串。|
|`AccountInfo.buyingPower`|规范化|`AccountInfo.buying_power` optional 十进制字符串。|
|`AccountInfo.initMarginReq`|规范化|`AccountInfo.init_margin_req` optional 十进制字符串。|
|`AccountInfo.maintMarginReq`|规范化|`AccountInfo.maint_margin_req` optional 十进制字符串。|
|`AccountInfo.dayTradesRemaining`|规范化|`AccountInfo.day_trades_remaining` optional 十进制字符串，不能用 `0` 表示缺失。|
|`Position.contract`|保留|`Position.contract`，逐叶见 3.1。|
|`Position.currency`|保留|`Position.currency`。|
|`Position.side`|保留|`Position.side` oneof 之外的原值约束由现有 `long/short` 语义承载。|
|`Position.quantity`|规范化|`Position.quantity` 十进制字符串。|
|`Position.avgCost`|规范化|`Position.avg_cost` 十进制字符串。|
|`Position.marketPrice`|规范化|`Position.market_price` 十进制字符串。|
|`Position.marketValue`|规范化|`Position.market_value` 十进制字符串。|
|`Position.unrealizedPnL`|规范化|`Position.unrealized_pnl` 十进制字符串。|
|`Position.realizedPnL`|规范化|`Position.realized_pnl` 十进制字符串。|
|`Position.multiplier`|规范化|`Position.multiplier` 十进制字符串。|
|`Position.avgCostSource`|保留|`Position.avg_cost_source` optional；`broker` / `wallet` 原值。|
|`Position.risk`|保留|`Position.risk` optional；叶字段见下。|
|`PositionRisk.leverage`|规范化|`PositionRisk.leverage` optional 十进制字符串。|
|`PositionRisk.liquidationPrice`|规范化|`PositionRisk.liquidation_price` optional 十进制字符串。|
|`PositionRisk.marginMode`|保留|`PositionRisk.margin_mode` optional，`cross` / `isolated`。|
|`SubAccountRef.id`|保留|`SubAccountRef.id`。|
|`SubAccountRef.label`|保留|`SubAccountRef.label`。|
|`SubAccountRef.kind`|保留|`SubAccountRef.kind`，`spot` / `derivatives` / `unified`。|

### 3.3 Quote、OrderBook、Bar、Clock

|来源字段|处置|02 字段或原因|
|---|---|---|
|`Quote.contract`|保留|`Quote.contract`。|
|`Quote.last`|规范化|`Quote.last` 十进制字符串。|
|`Quote.bid`|规范化|`Quote.bid` 十进制字符串。|
|`Quote.ask`|规范化|`Quote.ask` 十进制字符串。|
|`Quote.volume`|规范化|`Quote.volume` 十进制字符串。|
|`Quote.high`|规范化|`Quote.high` optional 十进制字符串。|
|`Quote.low`|规范化|`Quote.low` optional 十进制字符串。|
|`Quote.timestamp`|规范化|`Quote.timestamp` → `DataTime`。|
|`OrderBook.contract`|保留|`OrderBook.contract`。该类型当前只在 `ccxt-types.ts:70-75`，公共 broker 读接口卡点见 3.1。|
|`OrderBook.bids`|规范化|`OrderBookLevel[]`；每项价格和数量均为十进制字符串。|
|`OrderBook.asks`|规范化|`OrderBookLevel[]`；每项价格和数量均为十进制字符串。|
|`OrderBook.timestamp`|规范化|`OrderBook.timestamp` → `DataTime`。|
|`OrderBookLevel.price`|规范化|`OrderBookLevel.price` 十进制字符串。|
|`OrderBookLevel.amount`|规范化|`OrderBookLevel.amount` 十进制字符串。|
|`Bar.timestamp`|规范化|`Bar.timestamp` → `DataTime`。|
|`Bar.open`|规范化|`Bar.open` 十进制字符串。|
|`Bar.high`|规范化|`Bar.high` 十进制字符串。|
|`Bar.low`|规范化|`Bar.low` 十进制字符串。|
|`Bar.close`|规范化|`Bar.close` 十进制字符串。|
|`Bar.volume`|规范化|`Bar.volume` 十进制字符串。|
|`MarketClock.isOpen`|保留|`MarketClock.is_open`。|
|`MarketClock.nextOpen`|规范化|`MarketClock.next_open` optional → `DataTime`。|
|`MarketClock.nextClose`|规范化|`MarketClock.next_close` optional → `DataTime`。|
|`MarketClock.timestamp`|规范化|`MarketClock.timestamp` optional → `DataTime`。|

### 能力声明中的明确 unavailable 字段

这些不是把当前接口缺口猜成字段缺失，而是集成在能力声明中对某一已定义读/回票字段明确报告不提供时的四选一落点；IDL 字段仍可选，数据缺失不可用零值代替。

|能力字段路径|处置|02 字段或原因|
|---|---|---|
|`InstructionCapability.result_fields.broker_native_id`|明确 unavailable|该集成未发出原生 id；`BrokerPlacementResult.broker_native_id`、`OrderStateReceipt.broker_native_id`、`OpenOrder.broker_native_id` optional，能力字段为 `unsupported`。|
|`InstructionCapability.result_fields.state_receipt.*`|明确 unavailable|该集成未发出状态回票字段；`OrderStateReceipt` 各字段 optional，能力字段逐项为 `unsupported`。|
|`InstructionCapability.execution_receipts`|明确 unavailable|能力明确写 `逐笔成交：无` 时，不得提交 `ExecutionReceipt`；缺少 broker 成交 id 或能力不支持均返回对应 typed 拒绝并计入健康读的被拒记录数。|
|`InstructionCapability.result_fields.execution_receipt.*`|明确 unavailable|该集成未发出成交回票字段；`ExecutionReceipt` 各字段 optional，但 `broker_execution_id` 是成交回票必需 id。|
|`ReadCapability(Orders).broker_native_id`|明确 unavailable|订单读面没有原生字符串 id；`OpenOrder.broker_native_id` optional，数字观察值不替代。|
|`ReadCapability(Orders).filled_quantity`|明确 unavailable|订单读面没有累计成交量；使用 `OpenOrder.order.filled_quantity` optional，数字零不冒充缺失。|
|`ReadCapability(Orders).avg_fill_price`|明确 unavailable|订单读面没有平均成交价；`OpenOrder.avg_fill_price` optional。|
|`ReadCapability(HistoricalBars).*`|明确 unavailable|该集成没有历史 bar 读面；`ReadFailure.unsupported` 与能力字段共同呈现，不返回空数组。|
|`ReadCapability(Wallet).*`|明确 unavailable|该集成没有子账户枚举；`WalletData.sub_accounts` 不以空列表冒充，能力字段为 `unsupported`。|

### 3.4 Wallet 与 histories

|来源字段|处置|02 字段或原因|
|---|---|---|
|`HistoryContract.aliceId`|保留|`HistoryContract.alice_id` optional。|
|`HistoryContract.symbol`|保留|`HistoryContract.symbol` optional。|
|`HistoryContract.localSymbol`|保留|`HistoryContract.local_symbol` optional。|
|`HistoryContract.secType`|保留|`HistoryContract.sec_type` optional。|
|`HistoryContract.currency`|保留|`HistoryContract.currency` optional。|
|`HistoryContract.exchange`|保留|`HistoryContract.exchange` optional。|
|`HistoryContract.expiry`|保留|`HistoryContract.expiry` optional。|
|`HistoryContract.strike`|规范化|`HistoryContract.strike` optional 十进制字符串。|
|`HistoryContract.right`|保留|`HistoryContract.right` optional。|
|`HistoryContract.multiplier`|规范化|`HistoryContract.multiplier` optional 十进制字符串。|
|`OrderHistoryEntry.orderId`|保留|`OrderHistoryEntry.broker_native_id` optional；拒绝于提交前时可缺失。|
|`OrderHistoryEntry.timestamp`|规范化|`OrderHistoryEntry.timestamp` → `DataTime`。|
|`OrderHistoryEntry.resolvedAt`|规范化|`OrderHistoryEntry.resolved_at` optional → `DataTime`。|
|`OrderHistoryEntry.contract`|保留|`OrderHistoryEntry.contract`。|
|`OrderHistoryEntry.side`|保留|`OrderHistoryEntry.side`。|
|`OrderHistoryEntry.orderType`|保留|`OrderHistoryEntry.order_type` optional。|
|`OrderHistoryEntry.quantity`|规范化|`OrderHistoryEntry.quantity` optional 十进制字符串。|
|`OrderHistoryEntry.limitPrice`|规范化|`OrderHistoryEntry.limit_price` optional 十进制字符串。|
|`OrderHistoryEntry.stopPrice`|规范化|`OrderHistoryEntry.stop_price` optional 十进制字符串。|
|`OrderHistoryEntry.status`|保留|`OrderHistoryEntry.status` 原状态。|
|`OrderHistoryEntry.filledQty`|规范化|`OrderHistoryEntry.filled_quantity` optional 十进制字符串。|
|`OrderHistoryEntry.avgFillPrice`|规范化|`OrderHistoryEntry.average_fill_price` optional 十进制字符串。|
|`OrderHistoryEntry.source`|保留|`OrderHistoryEntry.source`。|
|`OrderHistoryEntry.commitHash`|保留|`OrderHistoryEntry.commit_hash`。|
|`OrderHistoryEntry.message`|保留|`OrderHistoryEntry.message`。|
|`OrderHistoryEntry.error`|保留|`OrderHistoryEntry.error` optional。|
|`TradeHistoryEntry.timestamp`|规范化|`TradeHistoryEntry.timestamp` → `DataTime`。|
|`TradeHistoryEntry.orderId`|保留|`TradeHistoryEntry.broker_native_id` optional。|
|`TradeHistoryEntry.contract`|保留|`TradeHistoryEntry.contract`。|
|`TradeHistoryEntry.side`|保留|`TradeHistoryEntry.side`。|
|`TradeHistoryEntry.quantity`|规范化|`TradeHistoryEntry.quantity` 十进制字符串。|
|`TradeHistoryEntry.price`|规范化|`TradeHistoryEntry.price` 十进制字符串。|
|`TradeHistoryEntry.value`|规范化|`TradeHistoryEntry.value` 十进制字符串。|
|`TradeHistoryEntry.source`|保留|`TradeHistoryEntry.source`。|
|`TradeHistoryEntry.commitHash`|保留|`TradeHistoryEntry.commit_hash`。|

`WalletData.sub_accounts` 逐项使用 `SubAccountRef`；缺少列表能力时是 `ReadCapability.fields` 的 `unsupported`，不是空列表。`HistoricalRecordsData` 同时承载订单历史与成交历史；两者的 optional 字段存在性不混为默认值。

## 4. 四个已知缺陷反向穿过 IDL

### E7：成交字段丢失

- 状态回票入口是 `OrderStateReceipt`：`original_status`、`mapped_state`、`cumulative_quantity`、`remaining`、`average_fill_price` 与完整 `OrderState` 分开保存；累计成交量是 broker 报告值，重复状态回票不做增量相加。
- 成交回票入口是 `ExecutionReceipt`：`broker_execution_id`、`single_quantity`、`single_price`、佣金与 broker 时间逐笔保存；成交回票不参与状态计算。
- 同一 broker 成交 id 且数量、价格相同的重复成交回票被忽略；佣金从无到有追加 `CommissionSupplement`；数量或价格冲突保留两张成交回票并追加 `ExecutionConflict`。
- 指令视图是 `InstructionView.tickets[]` 中显式的 `StateReceiptTicket`、`ExecutionReceiptTicket`、`CommissionSupplementTicket`、`ExecutionConflictTicket`；客户端分别读取状态回票与成交回票，不能只看最终状态。
- 账户帐票读是 `AccountTickets.tickets[]` 中同样的显式种类；订单对账中的 `OpenOrder.order.filled_quantity`、`avg_fill_price` 仍可读。N3 历史读还分别提供 `OrderHistoryEntry.filled_quantity`、`average_fill_price` 与 `TradeHistoryEntry.quantity`、`price`。
- 重启后只从持久化的显式帐票类型、`ReconciliationResponse` 与 history 消息读取；状态字段、成交字段、佣金补录、冲突、扩展与摘要不依赖当前集成对象。消费者以字段是否存在区分“broker 没给”与数值为零。

### E8：partial 与 broker 专有状态被压扁

- `OrderStateReceipt.mapped_state` 的 oneof 必须选择 `partially_filled`、`broker_specific` 或 `unmapped` 等状态分支；`OrderStateReceipt.original_status` 永远保留。表外状态不是被拒状态。
- 表外状态追加 `UnmappedStatusTicket`，其 `receipt` 是完整 `OrderStateReceipt`，含原状态、原生 id（若有）、状态字段、扩展与摘要；该 ticket 也在 `InstructionView` 和 `AccountTickets` 中可读。
- 消费者先按状态回票 oneof 分支区分 partial、已知专有状态和未映射状态，再读原始状态；成交回票不会覆盖状态回票。重启后按不可变 ticket 顺序重读，不能依赖状态映射表当前版本覆盖历史。

### E13：原生字符串 id 丢失

- `BrokerNativeId` 是 opaque 字符串；`OrderStateReceipt.broker_native_id`、`ExecutionReceipt.broker_native_id`、`OpenOrder.broker_native_id`、`BrokerPlacementResult.broker_native_id`、`PlaceOrderLeg.broker_native_id` 与历史的 `broker_native_id` 均为 optional transport 字段，但 broker 一旦声称已分配 id，该字段必须出现。
- `Order.orderId`、`Execution.orderId`、`OrderStatus.orderId` 等数字字段落到各自 `numeric_order_id` 十进制字符串，仅作原始观察；它们不能填充 `BrokerNativeId`，也不能替代 `ExecutionReceipt.broker_execution_id`。
- 消费者用状态回票或成交回票各自的原生 id 做关联，用 numeric 字段做诊断；重启后两者都在帐票、对账响应和扩展中可读。没有原生 id 时不把数字 0、空串或缺失误认为同一 id。

### E14：broker 不给 orderId / 成交数据

- 能力声明的 `InstructionCapability.result_fields` 对状态回票字段逐项给 `supported` 或 `unsupported`，`InstructionCapability.execution_receipts` 明确写 `逐笔成交：有 / 无`；能力声明本身不伪造数据。
- 状态回票没有 broker 事件 id 且无法在提交账户范围内组成 `(broker 原生 id, 状态原值, 累计成交量)` 指纹时返回 `MissingStateReceiptDedupeKeyRejected`；指纹不含时间戳。
- 成交回票必须有 `ExecutionReceipt.broker_execution_id`；缺少该 id 返回 `MissingExecutionIdRejected`。能力为 `逐笔成交：无` 却提交成交回票返回 `ExecutionReceiptUnsupportedRejected`；两种拒绝都计入 `AccountHealth.rejected_record_count`。
- 没有逐笔成交 id 的集成只能提交状态回票并声明 `逐笔成交：无`，不能用 broker 原生订单 id、client key、时间或业务字段拼造成交 id；成交回票不能退化为累计状态回票。
- 消费者以“能力字段为 unsupported + 具体字段 absent”识别 broker 未提供，以“能力 supported + 字段 absent”识别集成回票不完整；不能把 absent 解释为零、成交或被拒。
- 重启后能力声明版本、optional 缺失状态、状态去重键、成交 id、摘要与扩展均随帐票持久化；客户端仍可区分“无 id/无成交数据”与“记录损坏”。

## 5. 02 必须满足的反向检查

|检查|02 落点|
|---|---|
|每个数值不经过浮点|所有数量、价格、金额、倍率、成交量、数字 id、时间序列值使用十进制字符串；时间使用 `DataTime`。|
|每个缺失可区分|源 optional 映射为 IDL `optional`；不得用 proto3 标量默认值表示 unavailable。|
|每个私有字段可追溯|响应与回票提供 `TypedExtensions extensions` 与 `RawEvidenceSummary raw_evidence_summary`。|
|每个原生身份可重读|使用 `BrokerNativeId.value` 原样字符串；数字 id 只在 `numeric_*_id`。|
|每个状态不压扁|`ReceiptState` oneof + `original_status` + `UnmappedStatusTicket`。|
|N3 未定义读面不补造规则|订单簿与历史读面的现有字段可落 IDL；公共 broker 接口与能力覆盖不足处保留卡点，不能用 unavailable 偷换。|
