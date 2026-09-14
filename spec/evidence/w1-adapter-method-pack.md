# W1 adapter-method-pack evidence

## Scope and legend

- [OBSERVED] means the cited source was read at the stated path and line range.
- [INFERRED] is a conclusion drawn only from cited source shape/branches; it is not a provider guarantee.
- [UNKNOWN] means the inspected source did not establish the capability; it is not a claim that the provider cannot do it.
- Matrix status: `I` = concrete implementation; `D→x` = delegates to x/helper; `U` = explicit unsupported/loud refusal; `E` = empty/not-found/conditional result; `N` = no-op, constant, or simulator stub; `S` = input silently ignored; `A` = optional member absent.
- [OBSERVED] class-source ranges used for absence checks: `A = services/uta/src/domain/trading/brokers/alpaca/AlpacaBroker.ts:107-804`; `C = services/uta/src/domain/trading/brokers/ccxt/CcxtBroker.ts:167-1406`; `I = services/uta/src/domain/trading/brokers/ibkr/IbkrBroker.ts:73-921`; `L = services/uta/src/domain/trading/brokers/longbridge/LongbridgeBroker.ts:125-714`; `V = services/uta/src/domain/trading/brokers/others/leverup/LeverupBroker.ts:80-567`; `M = services/uta/src/domain/trading/brokers/mock/MockBroker.ts:164-979`.

## IBroker contract

- [OBSERVED `packages/uta-protocol/src/types/broker.ts:479-500`] `IBroker` requires identity, `init`, `close`, and declares only `setConnectionStateListener` as optional among lifecycle members.
- [OBSERVED `packages/uta-protocol/src/types/broker.ts:504-524`] search/details are required; `expandContract` and `refreshCatalog` are optional. The protocol documentation says missing expansion is loud-refused and SearchingCatalog brokers may omit refresh.
- [OBSERVED `packages/uta-protocol/src/types/broker.ts:528-553`] writes are required; sub-account enumeration and instrument-to-sub-account routing are optional.
- [OBSERVED `packages/uta-protocol/src/types/broker.ts:562-607`] account/positions/orders/quote/clock are required; `getOrder` accepts optional `symbolHint`; open-orders, historical, and asset-class methods are optional. Missing historical is expected to loud-refuse rather than return `[]`.
- [OBSERVED `packages/uta-protocol/src/types/broker.ts:611-621`] capabilities and native-key identity are required.

## Complete concrete-member matrix

| IBroker member | Alpaca | CCXT | IBKR | Longbridge | LeverUp | Mock |
|---|---|---|---|---|---|---|
| `id` | I [OBSERVED A:135-137] | I [OBSERVED C:227-230] | I [OBSERVED I:106-112] | I [OBSERVED L:155-158] | I [OBSERVED V:100-105] | I [OBSERVED M:181-184] |
| `label` | I [OBSERVED A:135-154] | I [OBSERVED C:227-249] | I [OBSERVED I:106-130] | I [OBSERVED L:155-166] | I [OBSERVED V:100-120] | I [OBSERVED M:181-208] |
| `brokerEngine?` | I [OBSERVED A:135] | I [OBSERVED C:227] | I [OBSERVED I:106] | I [OBSERVED L:155] | I [OBSERVED V:100] | I [OBSERVED M:181] |
| `meta?` | A [OBSERVED A-full; no declaration] | I [OBSERVED C:227-246] | A [OBSERVED I-full; no declaration] | A [OBSERVED L-full; no declaration] | I [OBSERVED V:100-104] | A [OBSERVED M-full; no declaration] |
| `init` | I [OBSERVED A:163-209] | I [OBSERVED C:326-413] | I [OBSERVED I:182-223] | I [OBSERVED L:180-228] | I [OBSERVED V:132-150] | N [OBSERVED M:254-257] |
| `close` | N [OBSERVED A:211-213; empty SDK-close body] | N [OBSERVED C:415-417; empty SDK-close body] | I [OBSERVED I:225-229; clears heartbeat/subscription and disconnects] | N [OBSERVED L:230-233; empty SDK-close body] | N [OBSERVED V:157-159; empty viem-close body] | N [OBSERVED M:256-257; records only] |
| `setConnectionStateListener?` | A [OBSERVED A-full; no declaration] | A [OBSERVED C-full; no declaration] | I [OBSERVED I:165-167; delegates to RequestBridge] | A [OBSERVED L-full; no declaration] | A [OBSERVED V-full; no declaration] | A [OBSERVED M-full; no declaration] |
| `searchContracts` | I [OBSERVED A:241-266; catalog/fuzzy or echo fallback] | I [OBSERVED C:434-503; loaded-market filtering/fuzzy ranking] | I [OBSERVED I:242-267; TWS matching plus hub expansion] | I [OBSERVED L:237-244; exact-search unavailable, echo fallback] | I [OBSERVED V:169-180; static pair list] | N [OBSERVED M:259-266; always default AAPL] |
| `getContractDetails` | I [OBSERVED A:268-285] | I [OBSERVED C:506-520] | I [OBSERVED I:289-312] | I [OBSERVED L:246-267] | I [OBSERVED V:183-189] | N [OBSERVED M:268-274; always mock contract] |
| `expandContract?` | I [OBSERVED A:702-721; stock options only] | A [OBSERVED C-full; no declaration] | I [OBSERVED I:391-476; issuer/options/futures expansion] | A [OBSERVED L-full; no declaration] | A [OBSERVED V-full; no declaration] | A [OBSERVED M-full; no declaration] |
| `refreshCatalog?` | I [OBSERVED A:223-239] | I [OBSERVED C:419-430] | A [OBSERVED I-full; no declaration] | A [OBSERVED L-full; no declaration] | A [OBSERVED V-full; no declaration] | A [OBSERVED M-full; no declaration] |
| `placeOrder` | I/U [OBSERVED A:290-370; asset/order restrictions refuse] | I/U [OBSERVED C:524-576; attached TP/SL refuses without override] | I/U [OBSERVED I:480-506; attached TP/SL refuses] | I/S [OBSERVED L:271-313; `_tpsl` ignored] | I [OBSERVED V:213-317; MKT/EIP-712 relayer] | I/S [OBSERVED M:278-325; `_record` only for TP/SL] |
| `modifyOrder` | I [OBSERVED A:373-399] | I [OBSERVED C:662-704] | I [OBSERVED I:509-543] | I [OBSERVED L:315-335] | U [OBSERVED V:319-321] | I [OBSERVED M:327-355] |
| `cancelOrder` | I [OBSERVED A:401-409] | I [OBSERVED C:643-660] | I [OBSERVED I:545-559] | I [OBSERVED L:337-344] | U [OBSERVED V:323-325] | I [OBSERVED M:357-367] |
| `closePosition` | D→place/SDK [OBSERVED A:412-447] | D→place [OBSERVED C:706-741] | D→place [OBSERVED I:561-585] | D→place [OBSERVED L:346-362] | I [OBSERVED V:327-374; relayer close; quantity unused] | D→place [OBSERVED M:369-383] |
| `listSubAccounts?` | A [OBSERVED A-full; no declaration] | I [OBSERVED C:747-784] | A [OBSERVED I-full; no declaration] | A [OBSERVED L-full; no declaration] | A [OBSERVED V-full; no declaration] | A [OBSERVED M-full; no declaration] |
| `subAccountForContract?` | A [OBSERVED A-full; no declaration] | I [OBSERVED C:758-784] | A [OBSERVED I-full; no declaration] | A [OBSERVED L-full; no declaration] | A [OBSERVED V-full; no declaration] | A [OBSERVED M-full; no declaration] |
| `getAccount` | I [OBSERVED A:451-475] | I/E [OBSERVED C:947-1046; keyless zero account and scoped reads] | I [OBSERVED I:606-660; cache-backed] | I [OBSERVED L:366-447] | I [OBSERVED V:378-414] | I [OBSERVED M:387-415; synthetic account] |
| `getPositions` | I [OBSERVED A:496-518] | I/E [OBSERVED C:1048-1112; keyless empty] | I [OBSERVED I:677-699; account cache] | I [OBSERVED L:465-509] | I/E [OBSERVED V:416-448; unknown pairs skipped] | I [OBSERVED M:417-435] |
| `getOrders` | D→getOrder [OBSERVED A:520-527] | D→getOrder [OBSERVED C:1114-1125] | I [OBSERVED I:759-764; open-list filter] | D→getOrder [OBSERVED L:613-620] | D→getOrder [OBSERVED V:450-458] | D→getOrder [OBSERVED M:437-445] |
| `getOrder` | I/E [OBSERVED A:529-536; SDK global lookup, null on error] | I/E [OBSERVED C:1127-1145; cache or symbolHint required] | D→open/completed [OBSERVED I:777-786] | I/E [OBSERVED L:622-628; SDK orderDetail, null on error] | E [OBSERVED V:460-492; in-memory tracking only] | E [OBSERVED M:447-452; in-memory map; hint ignored] |
| `getOpenOrders?` | I [OBSERVED A:538-547] | I/E [OBSERVED C:1193-1219; keyless/unsupported can return `[]`] | I/E [OBSERVED I:772-775; this clientId only] | A [OBSERVED L-full; no declaration] | A [OBSERVED V-full; no declaration] | I [OBSERVED M:454-462] |
| `getQuote` | I [OBSERVED A:549-589] | I [OBSERVED C:1222-1247] | I [OBSERVED I:811-839] | I [OBSERVED L:631-657] | I [OBSERVED V:494-514] | N [OBSERVED M:479-490; synthetic mark/default] |
| `getMarketClock` | I [OBSERVED A:666-678; SDK clock] | N [OBSERVED C:1328-1333; always open/now] | I [OBSERVED I:841-870; TWS time plus NYSE baseline] | I [OBSERVED L:659-672; tradingSession] | N [OBSERVED V:516-518; always open] | N [OBSERVED M:514-517; always open] |
| `getHistorical?` | I [OBSERVED A:597-654] | I [OBSERVED C:1254-1309] | A [OBSERVED I-full; no declaration] | A [OBSERVED L-full; no declaration] | A [OBSERVED V-full; no declaration] | N [OBSERVED M:492-512; deterministic bars] |
| `assetClassFor?` | A [OBSERVED A-full; no declaration] | I [OBSERVED C:1313-1318; always `crypto`] | A [OBSERVED I-full; no declaration] | A [OBSERVED L-full; no declaration] | A [OBSERVED V-full; no declaration] | A [OBSERVED M-full; no declaration] |
| `getCapabilities` | I [OBSERVED A:656-664] | I [OBSERVED C:1320-1325] | I [OBSERVED I:872-878] | I [OBSERVED L:674-680] | I [OBSERVED V:520-527] | N [OBSERVED M:104-108,519-521; static simulator values] |
| `getNativeKey` | I [OBSERVED A:681-685] | I [OBSERVED C:1335-1339] | I [OBSERVED I:881-896] | I [OBSERVED L:683-688] | I [OBSERVED V:529-533] | I [OBSERVED M:523-531] |
| `resolveNativeKey` | I [OBSERVED A:687-689] | I/U [OBSERVED C:1341-1355; skeletal unknown key] | I/U [OBSERVED I:898-918; issuer key refuses] | I [OBSERVED L:690-692] | I [OBSERVED V:535-547] | I [OBSERVED M:533-543] |

## Placement and protection behavior

- [OBSERVED `packages/uta-protocol/src/types/broker.ts:528-531`] `placeOrder` accepts only `(contract, order, tpsl?)`; the IBroker contract has no caller-supplied idempotency/client-order-key argument.
- [OBSERVED `services/uta/src/domain/trading/brokers/alpaca/AlpacaBroker.ts:304-318,333-351`] Alpaca sends symbol/side/type/time-in-force and qty/notional; no `client_order_id` is built from caller data. Attached two-leg TP/SL maps to `bracket`, one leg to `oto`; crypto TP/SL is refused at lines 296-301.
- [OBSERVED `services/uta/src/domain/trading/brokers/ccxt/CcxtBroker.ts:524-530,555-576`] concrete CCXT adds an `extraParams` argument, but the attached-protection path refuses unless a venue override is registered; [INFERRED] the `IBroker`-typed pack/factory path cannot supply that fourth argument as an interface capability.
- [OBSERVED `services/uta/src/domain/trading/brokers/ibkr/IbkrBroker.ts:491-502`] IBKR allocates `bridge.getNextOrderId()` and sends it through `EClient.placeOrder`; no caller idempotency key is accepted.
- [OBSERVED `services/uta/src/domain/trading/brokers/longbridge/LongbridgeBroker.ts:271-304`] Longbridge builds `SubmitOrderOptions` from symbol/order fields and ignores `_tpsl`; no caller idempotency field is constructed.
- [OBSERVED `services/uta/src/domain/trading/brokers/others/leverup/LeverupBroker.ts:245-265,276-305`] LeverUp includes TP/SL prices in open data, but generates a fresh `salt` and tracks the relayer `inputHash`; no caller-supplied idempotency value is accepted.
- [OBSERVED `services/uta/src/domain/trading/brokers/mock/MockBroker.ts:278-305`] Mock generates `mock-ord-${counter}`; `tpsl` is recorded in `_record` but does not alter the simulated fill path. 
- [INFERRED] Caller-supplied idempotent placement is not exposed by any of the six adapters through the `IBroker` contract. Provider-side primitive availability is separate and was not established as an adapter contract here.

## Read-by-key / restart behavior

- [OBSERVED `services/uta/src/domain/trading/brokers/alpaca/AlpacaBroker.ts:529-536`] Alpaca calls SDK `getOrder(orderId)` directly and returns null on any lookup error; the SDK path is not symbol-scoped in this adapter.
- [OBSERVED `services/uta/src/domain/trading/brokers/ccxt/CcxtBroker.ts:1127-1145,1147-1153`] CCXT resolves a symbol from its in-memory `orderSymbolCache` or the persisted `symbolHint`; with neither it returns null. Conversion and observed external orders seed the cache.
- [OBSERVED `services/uta/src/domain/trading/brokers/ibkr/IbkrBroker.ts:759-786`] IBKR filters current-client open orders by numeric order id, then searches completed orders; [OBSERVED `services/uta/src/domain/trading/brokers/ibkr/IbkrBroker.ts:766-770`] the open-list request is explicitly limited to this clientId, so manually entered TWS orders are not covered by this method.
- [OBSERVED `services/uta/src/domain/trading/brokers/longbridge/LongbridgeBroker.ts:622-628`] Longbridge calls SDK `orderDetail(orderId)`; [OBSERVED `services/uta/src/domain/trading/brokers/longbridge/LongbridgeBroker.ts:696-713`] mapped `OpenOrder.orderId` is set to `0` because native string ids are preserved through placement results instead.
- [OBSERVED `services/uta/src/domain/trading/brokers/others/leverup/LeverupBroker.ts:113-114,460-492`] LeverUp reads only its process-local `orderTracking` map and polls relayer status for tracked submitted orders; [INFERRED] a process restart loses that map because no persisted order index is present in the class source.
- [OBSERVED `services/uta/src/domain/trading/brokers/others/leverup/LeverupBroker.ts:327-370`] close-position returns a relayer `inputHash` but does not add the close result to `orderTracking`; [INFERRED] the returned close id cannot be read back through this adapter after the in-memory path unless another mechanism records it.
- [OBSERVED `services/uta/src/domain/trading/brokers/mock/MockBroker.ts:447-452`] Mock reads only `_orders` and ignores `symbolHint`.

## A7 observation cursor

- [OBSERVED `packages/uta-protocol/src/types/broker.ts:479-622`] no `IBroker` method accepts or returns a resumable observation cursor; `getOpenOrders()` has no cursor parameter and returns only `Promise<OpenOrder[]>`.
- [OBSERVED `services/uta/src/domain/trading/brokers/alpaca/AlpacaBroker.ts:702-721,733-753`] Alpaca uses local page tokens for option/bars HTTP pagination, consumes them internally, and returns contracts/bars without a cursor. These are pagination tokens, not an order/account observation cursor.
- [OBSERVED `services/uta/src/domain/trading/brokers/ccxt/CcxtBroker.ts:1284-1305`] CCXT uses a local timestamp `cursor` while walking OHLCV pages and returns bars only; no resumable account/order observation cursor is exposed.
- [OBSERVED `services/uta/src/domain/trading/brokers/ibkr/request-bridge.ts:113-120,200-229`] IBKR request ids and collector maps correlate one request; [OBSERVED `services/uta/src/domain/trading/brokers/ibkr/request-bridge.ts:309-347`] the account subscription is an internal cache. Neither is a public resumable cursor.
- [OBSERVED `services/uta/src/domain/trading/brokers/longbridge/LongbridgeBroker.ts:613-628,631-672`] adapter order/quote/clock reads have no cursor argument or return. [OBSERVED `services/uta/node_modules/longbridge/index.d.ts:2290-2384`] inspected SDK account/position/order methods likewise show no cursor in the used declarations.
- [OBSERVED `services/uta/src/domain/trading/brokers/others/leverup/reader-client.ts:80-90`] LeverUp hard-codes `page=0` on its REST position read; [OBSERVED `services/uta/src/domain/trading/brokers/mock/MockBroker.ts:185-203,447-462`] Mock state is process-local maps with no cursor.
- [UNKNOWN] A7 resumable observation cursor is not established for any adapter/provider from inspected source. This is an exposure gap finding, not a provider-impossibility claim.

## Provider primitives versus adapter exposure

### WebSocket / event primitives

- [OBSERVED `services/uta/node_modules/@alpacahq/alpaca-trade-api/dist/alpaca-trade-api.d.ts:1-45`] installed Alpaca SDK declares data/trade/crypto/option stream members. [OBSERVED `services/uta/node_modules/@alpacahq/alpaca-trade-api/dist/resources/websockets.d.ts:5-60`] its trade stream declares connect/reconnect/subscribe/unsubscribe and account/order callbacks; [OBSERVED `services/uta/src/domain/trading/brokers/alpaca/AlpacaBroker.ts:163-189,451-589`] adapter constructs the SDK and uses REST/account/data reads only, with no stream subscription or IBroker listener.
- [OBSERVED `services/uta/node_modules/ccxt/js/ccxt.d.ts:302-379`] installed CCXT declaration includes a `pro` WebSocket namespace/classes. [OBSERVED `services/uta/src/domain/trading/brokers/ccxt/CcxtBroker.ts:9-13,250-272,326-430`] adapter imports default CCXT, constructs the REST exchange class, and uses REST market/account methods; no `pro`/`watch*` path or IBroker event listener is declared.
- [OBSERVED `packages/ibkr/src/connection.ts:20-115`] IBKR transport is a framed TCP `net.Socket` connection, not WebSocket. [OBSERVED `packages/ibkr/src/wrapper.ts:48-79,126-183`] EWrapper exposes connection, tick, order, and account callbacks. [OBSERVED `services/uta/src/domain/trading/brokers/ibkr/request-bridge.ts:582-806`] RequestBridge consumes those callbacks into caches/one-shot collectors; [OBSERVED `services/uta/src/domain/trading/brokers/ibkr/IbkrBroker.ts:165-167`] only connection-state notification is exposed at IBroker level.
- [OBSERVED `services/uta/node_modules/longbridge/index.d.ts:1139-1228,2089-2130`] Longbridge SDK declares quote callbacks/subscriptions and trade order-change callbacks/subscriptions. [OBSERVED `services/uta/node_modules/longbridge/README.md:119-126`] SDK documents quote/trade WebSocket endpoints. [OBSERVED `services/uta/src/domain/trading/brokers/longbridge/LongbridgeBroker.ts:193-207,631-653`] adapter creates contexts and performs account/quote/depth reads but never calls `setOn*`, `subscribe`, or exposes a stream/listener.
- [OBSERVED `services/uta/src/domain/trading/brokers/others/leverup/reader-client.ts:80-100`] LeverUp reader uses HTTP/fetch and viem RPC. [OBSERVED `services/uta/src/domain/trading/brokers/others/leverup/relayer-client.ts:42-93`] relayer uses POST/GET and polling. [UNKNOWN] No provider WS/event primitive was established in the inspected LeverUp source.
- [OBSERVED `services/uta/src/domain/trading/brokers/mock/MockBroker.ts:546-563`] Mock simulator controls are imperative methods outside IBroker; no event/listener surface is declared in the class source.
- [INFERRED] Alpaca, CCXT, and Longbridge have provider-level WS declarations, and IBKR has callback transport, but only IBKR exposes a connection-state listener through IBroker; no adapter exposes market/account/order event streams.

### Official documentation / OpenAPI artifact availability

- [OBSERVED `services/uta/node_modules/@alpacahq/alpaca-trade-api/README.md:1-17`] Alpaca SDK points to official REST documentation at `https://docs.alpaca.markets`; no machine-readable OpenAPI artifact was read in the inspected SDK package.
- [OBSERVED `services/uta/node_modules/longbridge/package.json:20-24`] Longbridge package metadata identifies `https://github.com/longbridge/openapi`; [OBSERVED `services/uta/node_modules/longbridge/README.md:1-13`] it points to official Node.js docs at `https://longbridge.github.io/openapi/nodejs/index.html`; [UNKNOWN] a machine-readable OpenAPI schema was not read from the installed package.
- [UNKNOWN] A single official OpenAPI schema for CCXT itself was not established from the inspected CCXT package/docs; CCXT is an adapter library over venue APIs, so venue-specific specs require separate evidence.
- [UNKNOWN] An official OpenAPI schema for IBKR's inspected TWS socket transport was not established; the inspected transport source is TCP/EWrapper, not HTTP OpenAPI [OBSERVED `packages/ibkr/src/connection.ts:1-6,20-115`].
- [UNKNOWN] An official OpenAPI schema for LeverUp was not established; inspected relayer source only declares custom endpoint constants and HTTP calls [OBSERVED `services/uta/src/domain/trading/brokers/others/leverup/relayer-client.ts:1-11,42-71`].

## CCXT variants and override exposure

- [OBSERVED `packages/uta-protocol/src/brokers/preset-catalog.ts:110-154`] Binance presets map live/demo to engine `ccxt`, exchange `binance`, and `demoTrading`.
- [OBSERVED `packages/uta-protocol/src/brokers/preset-catalog.ts:156-189`] OKX presets map live/demo to engine `ccxt`; demo uses `sandbox: true` and password.
- [OBSERVED `packages/uta-protocol/src/brokers/preset-catalog.ts:191-224`] Bybit presets map live/testnet/demo; testnet uses sandbox and demo uses `demoTrading`.
- [OBSERVED `packages/uta-protocol/src/brokers/preset-catalog.ts:226-257`] Hyperliquid presets map live/testnet with wallet address/private key and sandbox for testnet.
- [OBSERVED `packages/uta-protocol/src/brokers/preset-catalog.ts:259-333`] Bitget presets map live/demo, and Custom accepts any exchange plus sandbox/demo and standard credential fields.
- [OBSERVED `services/uta/src/domain/trading/brokers/ccxt/overrides.ts:1-26,38-146`] override hooks cover credentials/balance, order lookup/cancel/place, positions, TP/SL, open-order reads, and subaccounts; comments define call-default/modify/replace conventions.
- [OBSERVED `services/uta/src/domain/trading/brokers/ccxt/overrides.ts:151-215`] defaults are regular `fetchBalance`, regular then `stop:true` `fetchOrder`, cancel, create order, positions, and all-open reads.
- [OBSERVED `services/uta/src/domain/trading/brokers/ccxt/overrides.ts:219-236`] Binance override enumerates spot/futures subaccounts and the registry is populated for binance, bitget, bybit, and hyperliquid.
- [OBSERVED `services/uta/src/domain/trading/brokers/ccxt/exchanges/bitget.ts:1-67`] Bitget adds strict private/open-order reads, spot/swap subaccounts, product-type balance/position calls, and merges regular/trigger/TP-SL/trailing order namespaces.
- [OBSERVED `services/uta/src/domain/trading/brokers/ccxt/exchanges/bybit.ts:1-52`] Bybit lookup tries open/closed regular and conditional namespaces; default cancellation remains in use.
- [OBSERVED `services/uta/src/domain/trading/brokers/ccxt/exchanges/hyperliquid.ts:104-213`] Hyperliquid merges ledgers, selects wallet modes for balance/positions, supplies market reference price, and recovers position mark prices.
- [OBSERVED `services/uta/src/domain/trading/brokers/ccxt/CcxtBroker.ts:170-188,200-222,242-309`] the dynamic adapter config accepts exchange, sandbox/demo, standard credential fields, and options; constructor resolves the exchange class and validates sandbox/demo endpoint conditions.

## Pack wrappers, identity, and load path

- [OBSERVED `packages/uta-broker-alpaca/src/index.ts:1-9`] Alpaca wrapper exports API version/engine, schema, and `createBroker` around `AlpacaBroker.fromConfig`; analogous wrappers are observed at `packages/uta-broker-ccxt/src/index.ts:1-9`, `packages/uta-broker-ibkr/src/index.ts:1-9`, `packages/uta-broker-leverup/src/index.ts:1-9`, and `packages/uta-broker-longbridge/src/index.ts:1-9`.
- [OBSERVED `services/uta/src/domain/trading/brokers/registry.ts:23-33,44-66`] registry validates a pack module shape and maps five installable engine names to workspace wrapper paths; [OBSERVED `services/uta/src/domain/trading/brokers/registry.ts:69-128`] it dynamically imports installed/workspace packs and checks API version, engine identity, schema, and factory.
- [OBSERVED `src/core/broker-packs.ts:10-19,86-135,151-172`] pack resolution validates active pointers, realpath containment, manifest schema/API, package name/version identity, and entry containment.
- [OBSERVED `services/uta/src/domain/trading/brokers/factory.ts:26-47`] factory resolves preset → engine config → pack, validates engine schema, passes `keyless`, and only duck-types `setFxService`; it returns `IBroker`.
- [OBSERVED `services/uta/src/domain/trading/brokers/alpaca/AlpacaBroker.ts:14`, `services/uta/src/domain/trading/brokers/ccxt/CcxtBroker.ts:9-13`, `services/uta/src/domain/trading/brokers/ibkr/IbkrBroker.ts:15`, `services/uta/src/domain/trading/brokers/longbridge/LongbridgeBroker.ts:19`, `services/uta/src/domain/trading/brokers/others/leverup/LeverupBroker.ts:12`, `services/uta/src/domain/trading/brokers/mock/MockBroker.ts:20`] all inspected adapters import `Decimal` from `decimal.js`.
- [OBSERVED `services/uta/node_modules/longbridge/index.d.ts:220-320`] Longbridge SDK also declares its own native `Decimal` class; [OBSERVED `services/uta/src/domain/trading/brokers/longbridge/LongbridgeBroker.ts:297-301`] adapter crosses that boundary with `as unknown as never` when passing decimal.js values. [UNKNOWN] Runtime numeric compatibility was not established by source inspection alone.
- [OBSERVED `packages/uta-broker-alpaca/tsup.config.ts:1-10`] Alpaca bundles all dependencies (`noExternal: [/.*/]`), while [OBSERVED `packages/uta-broker-ccxt/tsup.config.ts:1-15`, `packages/uta-broker-ibkr/tsup.config.ts:1-7`, `packages/uta-broker-leverup/tsup.config.ts:1-7`, `packages/uta-broker-longbridge/tsup.config.ts:1-7`] the other packs externalize ordinary node modules and bundle only workspace/protobuf patterns. [INFERRED] Decimal package identity at runtime can therefore differ by pack boundary; no runtime identity experiment was run.

## A7 status and unresolved facts

- [INFERRED] Caller-supplied idempotent placement: not exposed by the six `IBroker` adapters.
- [UNKNOWN] Provider primitive for caller-supplied idempotency was not established uniformly; adapter source does not wire one.
- [OBSERVED] Read-by-key exists for all six, but semantics differ: global SDK (Alpaca/Longbridge), symbol-scoped with hint (CCXT), current-client-plus-completed (IBKR), process-local tracking (LeverUp/Mock).
- [UNKNOWN] Resumable observation cursor is not established for any adapter; internal page/request/timestamp cursors are not public cursors.
- [OBSERVED] Provider WS/callback primitives exist for Alpaca, CCXT Pro, IBKR TCP callbacks, and Longbridge; only IBKR's connection-state listener is exposed through `IBroker`.
- [UNKNOWN] Official machine-readable OpenAPI availability is established only as documentation/repository pointers for Alpaca and Longbridge, not as a schema artifact read here; venue-specific CCXT/LeverUp/IBKR spec availability remains unknown from this inspection.
