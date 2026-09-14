# New UTA OpenAPI projection evidence

## Decision questions answered

### DQ1 — Is provider-supplied OpenAPI a viable common input when providers are not required to share one model?

**[Inferred answer]** Yes, as a provider-indexed projection input, not as a universal domain model.

- **[Observed — session requirement]** UTA is expected to consume a pre-built OpenAPI projection for most providers, or the provider's own OpenAPI directly. Projections may differ, provided the extensions follow a principled contract.
- **[Observed — `plans/uta-refactor/design/00-core-contract.md:17-23,65-75`]** The design anchors abstractions in runtime IO, allows providers to expose only partial capabilities, and requires provider-indexed `Operation<P>`, `Receipt<P>`, and `Observation<P>` rather than a lowest-common-denominator provider object.
- **[Observed — scoped fetch output `/tmp/w1-openapi-fetch-evidence.txt:2-10`]** The nine candidate venues have materially different machine-readable coverage: Alpaca, Longbridge, IBKR, Binance, and Bybit expose parsed OpenAPI artifacts; OKX, Bitget, Hyperliquid, and LeverUp were represented by prose references in the fetched official surfaces. Their auth, order identity, and stream models also differ.
- **[Inferred]** A projection must preserve the vendor document and add an explicit UTA capability manifest. It must not silently normalize every venue into one request/response shape. A venue without machine-readable OpenAPI can remain a manually declared adapter or be marked unavailable; it must not receive a fabricated claim of OpenAPI compatibility.

### DQ2 — Which artifacts establish HTTP operations, authentication, idempotency/read-by-key, and streaming semantics?

**[Inferred answer]** HTTP coverage and stream coverage are separate evidence tracks.

- **[Observed — `/tmp/w1-openapi-fetch-evidence.txt:2-6`]** The parsed OpenAPI artifacts expose normal HTTP operations and security declarations with uneven fidelity. Alpaca has `POST /v2/orders` and `GET /v2/orders:by_client_order_id`; Longbridge's fetched YAML has `GET/PUT/DELETE /v1/trade/order` but no normal place operation even though its current per-page docs document `POST /v1/trade/order`; IBKR's current artifact includes an order POST and WebSocket handshake but no `securitySchemes`; Binance and Bybit expose order and read operations in split/large documents.
- **[Observed — `/tmp/w1-openapi-fetch-evidence.txt:7-10`]** OKX, Bitget, Hyperliquid, and LeverUp official references were prose in this scoped fetch. Their pages document order identity or signed actions in different ways, but do not establish a shared OAS schema.
- **[Observed — `/tmp/w1-openapi-fetch-evidence.txt:2-12`]** No fetched venue stream reference was an AsyncAPI document. Alpaca, Longbridge, IBKR, Binance, Bybit, OKX, Bitget, and Hyperliquid document WebSocket behavior in prose; Alpaca's market-data OAS also has an SSE path. LeverUp's fetched index had no WebSocket/AsyncAPI entry, but that is only a scoped absence.
- **[Inferred]** The projection build must inspect both HTTP operations and a separate stream artifact. OpenAPI alone cannot prove WebSocket message schemas or reconnect/snapshot semantics. Use an AsyncAPI sidecar (or a deliberately equivalent typed stream manifest) for WebSocket channels, and make stream capability status independent from REST capability status.

### DQ3 — Can UTA publish its own OpenAPI through the same schema mechanism?

**[Inferred answer]** Yes for UTA HTTP; use a separate AsyncAPI document for WebSockets.

- **[Observed — `packages/uta-protocol/src/schemas/index.ts:1-10`]** The protocol package already states that one Request/Response Zod schema pair should be the source of truth for Alice parsing and UTA Hono validation, but the schema module currently exports nothing.
- **[Observed — `services/uta/src/http/routes-trading.ts:14-47`]** Current order-entry schemas are declared in the route module, including decimal-as-string fields and a cross-field quantity refinement.
- **[Observed — `services/uta/src/main.ts:145-177`]** UTA is a Hono HTTP app with `/api/trading` and `/api/simulator` routes bound to localhost, so an additional generated-document route is compatible with the present boundary.
- **[Observed — `packages/uta-protocol/package.json:1-29`]** `@traderalice/uta-protocol` is already the wire-protocol package and depends on Zod `^4.3.6`; it has no OpenAPI generator dependency today.
- **[Observed — `local://w1-openapi-tooling.md:52-60`]** Effect `HttpApi` is code-first and can produce OpenAPI 3.1 plus an Effect-validated client. It is not documented as an importer of provider OpenAPI. `typed-openapi` is the OpenAPI-in alternative with Zod and Effect runtime adapters.
- **[Inferred]** Complete the protocol Zod schemas once, annotate their HTTP metadata, generate UTA's OAS 3.1 at build time, and serve the immutable document from UTA. Generate Alice/UI clients from that document. Keep this UTA-facing contract separate from each provider's projection; otherwise a vendor response could be mistaken for UTA truth.

### DQ4 — What extension and lifecycle contract is sufficient for implementation guidance?

**[Inferred answer]** Use a versioned `x-uta` namespace, immutable source-plus-overlay artifacts, explicit capability variants, and build/runtime rejection of unsafe claims.

- **[Observed — OpenAPI 3.1.2 specification §4.9](https://spec.openapis.org/oas/v3.1.2.html)** Specification extensions are patterned properties prefixed by `x-`; extension values may be any JSON; support for any extension is optional. `x-oai-*` and `x-oas-*` are reserved.
- **[Observed — `docs/broker-packs.md:8-30,32-65,79-123`]** The current pack system already separates UTA orchestration from versioned provider implementations, loads only an activated immutable release in production, verifies checksums/manifests, atomically swaps an active pointer, and leaves malformed releases visible rather than silently trusting them.
- **[Inferred]** Apply that existing pack lifecycle to projection artifacts: pin the upstream document and digest, apply a deterministic overlay, validate references and UTA extensions in CI/build, package generated clients/parsers plus the manifest, and activate atomically. A missing capability is an explicit partial capability, not a failed installation; an unsafe positive claim (write without idempotency/read-by-key evidence) rejects the release or that capability.

## Observed facts

### 1. Venue matrix

The following is a scoped matrix, not a claim that each vendor's entire platform is covered. Counts are from a Bun runtime probe using `fetch` followed by JSON/YAML parsing; compact output is at `/tmp/w1-openapi-fetch-evidence.txt:1-12`.

| Venue | Official machine-readable HTTP evidence | Auth / order identity evidence | Streaming evidence |
|---|---|---|---|
| **Alpaca** | **[Observed]** [`trading-api.json`](https://docs.alpaca.markets/us/openapi/trading-api.json) is OAS 3.1.2, info version 2.0.1, 42 paths/59 operations; the official index is [`/us/openapi`](https://docs.alpaca.markets/us/openapi) and labels the collection v1.4. **[Observed]** [`market-data-api.json`](https://docs.alpaca.markets/us/openapi/market-data-api.json) is OAS 3.1.2, 44 paths/44 operations. | **[Observed]** Trading security schemes are API-key headers `APCA-API-KEY-ID` and `APCA-API-SECRET-KEY`. The trading document exposes `POST /v2/orders`, order detail, account, and `GET /v2/orders:by_client_order_id`. **[Observed]** The create-order schema has optional `client_order_id` (maximum length 128); the by-client operation requires the corresponding query key. | **[Observed]** [`streaming-market-data`](https://docs.alpaca.markets/us/docs/streaming-market-data) and [`websocket-streaming`](https://docs.alpaca.markets/us/docs/websocket-streaming) are prose WebSocket references for market data and `trade_updates`; the market-data OAS also contains an SSE path. No AsyncAPI document was found in this scoped fetch. |
| **Longbridge / LongPort** | **[Observed]** The official [`openapi.yaml`](https://raw.githubusercontent.com/longbridge/developers/main/openapi.yaml) is OAS 3.0.3, info version 1.0.0, 33 paths/40 operations, with Global and China servers. **[Observed]** It contains order detail/modify/cancel operations but no normal place operation or quote/kline/depth path in this file. **[Observed]** Current [`submit`](https://open.longportapp.com/docs/trade/order/submit.md) prose documents `POST /v1/trade/order`. | **[Observed]** The YAML declares OAuth2 authorization-code security with `openapi` scope. Current access docs also describe OAuth2 bearer usage. **[Observed]** Order identity is represented by `order_id` in the detail path/docs. | **[Observed]** [`socket/hosts`](https://open.longbridge.com/docs/socket/hosts) and [`subscribe_quote`](https://open.longbridge.com/docs/socket/subscribe_quote) document separate quote/trade WebSockets and binary protobuf messages; [`subscribe_trade`](https://open.longbridge.com/docs/socket/subscribe_trade) documents trade pushes. No AsyncAPI document was found. |
| **IBKR Client Portal Web API** | **[Observed]** Current [`api-reference.json`](https://www.interactivebrokers.com/docs/openapi/api-reference.json) is OAS 3.1.0, info version 1.0.0, 168 paths/180 operations, with local Gateway, production, and QA servers. It includes order, account, market-data snapshot, and WebSocket-handshake paths. **[Observed]** The official Campus page says not all Web API endpoints are currently included in this reference: [`webapi-doc`](https://ibkrcampus.com/campus/ibkr-api-page/webapi-doc/). | **[Observed]** The parsed document has no `securitySchemes`; separate Campus authentication docs describe Client Portal Gateway, OAuth 1.0a, and OAuth 2.0. **[Observed]** `POST /v1/api/iserver/account/{accountId}/orders` and order-status/read paths are present. | **[Observed]** [`Web API WebSocket introduction`](https://ibkrcampus.com/docs/web-api/v1/ws/introduction.md) documents `wss://localhost:5000/v1/api/ws` and session-required topics such as `smd`, `sor`, and `str`; the OAS path is a handshake, not a complete message schema. |
| **Binance Spot** | **[Observed]** Official [`spot_api.yaml`](https://raw.githubusercontent.com/binance/binance-api-swagger/master/spot_api.yaml) is OAS 3.0.2, info version 1.0, 316 paths/340 operations, with production and testnet servers and Market/Trade/Stream tags. The linked documentation repository is [`binance-spot-api-docs`](https://github.com/binance/binance-spot-api-docs). | **[Observed]** `ApiKeyAuth` uses `X-MBX-APIKEY`; the REST document exposes `POST/GET/DELETE /api/v3/order`, `GET /api/v3/account`, and client/order identifiers. Signature, permission, and API-key rules are documented in the [`Spot REST reference`](https://developers.binance.com/en/docs/products/spot/rest-api). | **[Observed]** [`Spot WebSocket Streams`](https://developers.binance.com/en/docs/binance-spot-api-docs/web-socket-streams) documents market streams; [`user-data-stream`](https://developers.binance.com/en/docs/binance-spot-api-docs/user-data-stream) documents `executionReport` and listen-key lifecycle. No AsyncAPI document was found. |
| **Bybit V5** | **[Observed]** The official YAML suite—[`market.yaml`](https://raw.githubusercontent.com/bybit-exchange/docs/master/yml-folder/v5/market.yaml), [`account.yaml`](https://raw.githubusercontent.com/bybit-exchange/docs/master/yml-folder/v5/account.yaml), [`position.yaml`](https://raw.githubusercontent.com/bybit-exchange/docs/master/yml-folder/v5/position.yaml), [`trade.yaml`](https://raw.githubusercontent.com/bybit-exchange/docs/master/yml-folder/v5/trade.yaml), [`asset.yaml`](https://raw.githubusercontent.com/bybit-exchange/docs/master/yml-folder/v5/asset.yaml), and [`user.yaml`](https://raw.githubusercontent.com/bybit-exchange/docs/master/yml-folder/v5/user.yaml)—are each OAS 3.0.0/info 3.0.0. Parsed counts were 16, 11, 11, 10, 16, and 4 paths respectively. | **[Observed]** The suite exposes `POST /v5/order/create`, order amend/cancel/realtime/history, market kline, and account balance operations. The YAML uses inline `apiKey`/`secret` header parameters rather than reusable security schemes; official [`authentication guide`](https://bybit-exchange.github.io/docs/v5/guide) documents HMAC/RSA and `X-BAPI-API-KEY`, timestamp, signature, and recv-window headers. | **[Observed]** [`WebSocket connect`](https://bybit-exchange.github.io/docs/v5/ws/connect) documents public, private, and order-entry WebSockets, auth, subscribe/unsubscribe, and regional endpoints. No AsyncAPI document was found. |
| **OKX** | **[Observed]** The official [`API V5 reference`](https://www.okx.com/docs-v5/en/) was fetched as HTML. Scoped marker scan found no standalone OpenAPI/Swagger/AsyncAPI label or formal OAS version. The prose documents REST V5 operations including `POST /api/v5/trade/order`, `GET /api/v5/trade/order`, and account balance. | **[Observed]** Private REST uses `OK-ACCESS-KEY`, `OK-ACCESS-SIGN`, `OK-ACCESS-TIMESTAMP`, and `OK-ACCESS-PASSPHRASE`; signing is HMAC-SHA256/Base64 over timestamp, method, path, and body. Order reads accept `ordId` or `clOrdId`. | **[Observed]** The same reference documents public/private/business WebSocket URLs, `op=subscribe` plus `args`, and account/positions channels. No AsyncAPI document was found. |
| **Bitget UTA** | **[Observed]** Current [`order management`](https://www.bitget.com/docs/catalog/trading/order-management) prose documents UTA V3 `POST https://api.bitget.com/api/v3/trade/place-order`; no standalone OpenAPI/Swagger link appeared in the fetched UTA docs. | **[Observed]** UTA uses `ACCESS-KEY`, `ACCESS-SIGN`, `ACCESS-TIMESTAMP`, and `ACCESS-PASSPHRASE`; `clientOid` is recommended for placement and unfilled-order REST supports recovery. | **[Observed]** [`UTA quick start`](https://www.bitget.com/docs/uta/quick-start) and [`best practices`](https://www.bitget.com/docs/uta/best-practices-guide) document public/private v3 WebSockets, order/account/position channels, and update-only order events requiring REST for an initial unfilled snapshot. No AsyncAPI document was found. |
| **Hyperliquid** | **[Observed]** Official [`info endpoint`](https://hyperliquid.gitbook.io/hyperliquid-docs/for-developers/api/info-endpoint) and [`exchange endpoint`](https://hyperliquid.gitbook.io/hyperliquid-docs/for-developers/api/exchange-endpoint) are prose references: public/user info is `POST /info`; signed actions are `POST /exchange`; no OAS/Swagger/AsyncAPI artifact or formal version appeared in the fetched API pages. | **[Observed]** Exchange requests require an action, nonce, and signature; optional 128-bit `cloid` and lookup by order/client ID are documented. | **[Observed]** [`WebSocket API`](https://hyperliquid.gitbook.io/hyperliquid-docs/for-developers/api/websocket) documents mainnet/testnet `wss://api.hyperliquid.xyz/ws`, JSON subscribe messages, reconnect, and snapshot behavior. No AsyncAPI document was found. |
| **LeverUp** | **[Observed]** [`API overview`](https://developer-docs.leverup.xyz/api/overview) documents unauthenticated, read-only HTTP at `https://service.leverup.xyz`; no OpenAPI/Swagger/AsyncAPI marker appeared in the fetched docs index. | **[Observed]** HTTP covers prices, markets, positions, history, portfolio, and leaderboards. Trading is a separate on-chain Diamond or gasless EIP-712 intent path; this is not an ordinary CEX REST order endpoint. | **[Observed]** No WebSocket/AsyncAPI section appeared in the fetched complete-doc index. This is a scoped absence and not evidence that the protocol has no other streaming mechanism. |

### 2. OpenAPI/AsyncAPI boundary facts

- **[Observed — OpenAPI 3.1.2 §4.9](https://spec.openapis.org/oas/v3.1.2.html)** Any extension may carry valid JSON, but consumers may ignore extensions. Therefore a generated client cannot be treated as proof that `x-uta` was understood.
- **[Observed — AsyncAPI 3.0.0](https://www.asyncapi.com/docs/reference/specification/v3.0.0)** AsyncAPI models channels, messages, and `send`/`receive` operations, including WebSocket bindings for connection/query/headers. One connection can carry multiple message types.
- **[Inferred]** OAS should describe request/response HTTP transport and AsyncAPI should describe WebSocket message contracts. A provider projection may link the two documents, but should not encode a WebSocket event union as fake REST paths.

### 3. Generator/runtime evidence

The detailed tool report is at [`local://w1-openapi-tooling.md:1-96`]. The following findings are the relevant implementation constraints.

| Tool | Observed behavior | Projection implication |
|---|---|---|
| `openapi-typescript` + `openapi-fetch` | **[Observed — `local://w1-openapi-tooling.md:15-25`]** OAS 3.0/3.1 becomes runtime-free TypeScript declarations; `openapi-fetch` types paths and uses Fetch, but successful responses use native JSON parsing and there is no value-validator hook. | **[Inferred]** Good type-only base, but parser-out must be a separate Zod/Effect boundary. Do not equate generated TS types with validation. |
| Orval | **[Observed — `local://w1-openapi-tooling.md:27-38`]** Generates Fetch/other clients and can emit Zod schemas; Fetch response validation is opt-in and skipped for primitive/void, non-JSON/NDJSON, or custom-mutator cases. It can emit Effect schemas separately. | **[Inferred]** Viable if generated Fetch plus explicit Zod wiring is accepted; custom transports need their own parser. |
| `@hey-api/openapi-ts` | **[Observed — `local://w1-openapi-tooling.md:40-50`]** Generates SDK operations and optional Zod request/response/definition schemas; SDK validation is configurable and off by default; the documented Effect client is planned, not available. | **[Inferred]** Viable TypeScript provider-pack SDK with Zod parser-out; pin the generator and test vendor extension/read-write behavior. |
| `typed-openapi` | **[Observed — `local://w1-openapi-tooling.md:62-72`]** Consumes an OAS path/URL, emits one client, supports Zod and Effect runtime adapters, and has explicit `--validate-side none | input | output | both`; Effect mode preserves status/transport/decode distinctions. SSE output validation is bypassed. | **[Inferred]** Best first candidate when one provider-pack module needs spec-in, typed client, and parser-out in one artifact. It still needs a separate WebSocket/AsyncAPI parser. |
| `@effect/platform` `HttpApi` | **[Observed — `local://w1-openapi-tooling.md:52-60`]** Code-first Effect `HttpApi` generates OAS 3.1 and an Effect-validated client; it is not documented as a provider-OAS importer. | **[Inferred]** Use for a UTA API authored in Effect, not as the primary provider projection importer. If the core is Effect, use `typed-openapi --runtime effect` for provider input. |

- **[Observed — `local://w1-openapi-tooling.md:74-88`]** No reviewed generator advertises UTA-specific partial capabilities, mandatory idempotent placement, read-by-key declarations, or arbitrary `x-uta` preservation. No reviewed candidate documents AsyncAPI/WebSocket generation.
- **[Inferred]** Before locking a generator, run one fixture through the selected version. The fixture must include `$ref`, readOnly/writeOnly fields, unions, multiple statuses/content types, security, and an `x-uta` marker; assert the raw extension manifest remains available to the pack validator.

### 4. Existing OpenAlice boundaries

- **[Observed — `AGENTS.md:58-78`]** Alice owns `src/` and the UTA client boundary; `services/uta/` owns brokers, accounts, approvals, snapshots, FX, and trading writes; secrets belong to Alice/configuration and never tracked files.
- **[Observed — `docs/broker-packs.md:18-30,43-56`]** UTA Core owns orchestration/HTTP/interface and packs provide external broker implementations; the registry loads an active pack only when an account needs its engine.
- **[Observed — `docs/broker-packs.md:94-123`]** Alice performs catalog selection, download, checksum/manifest validation, immutable release placement, atomic `active.json` replacement, and restart request. UTA never runs a package manager.
- **[Observed — `src/services/uta-client/UTAAccountSDK.ts:1-10,68-72`]** Alice's SDK delegates to UTA HTTP routes and is deliberately not a subclass of the in-process UTA account; missing routes currently throw a named error.
- **[Observed — `services/uta/src/http/routes-trading.ts:14-47`]** Route-local Zod schemas are currently the only visible source for several trading request shapes. This duplicates the intended protocol-package source-of-truth claim and should be resolved before publishing a durable document.

## Recommendations

### 1. Projection artifact and extension contract

The following is a recommendation, not an observed vendor contract.

- **[Inferred]** Store each provider's original OAS (or a declared non-OAS source) immutably, then apply a deterministic UTA overlay. Never rewrite the vendor source in place. Record provider, source URL/ref, source content digest, upstream `info.version`, and projection schema version separately.
- **[Inferred]** Reserve one root namespace, `x-uta`, for the manifest and use operation-level extensions only for bindings. The root manifest is the index; operation annotations are the executable details. A validator must reject disagreement rather than choose one duplicate value.
- **[Inferred]** Model capability availability as a discriminated variant: `supported`, `unsupported` with a structured reason, or `conditional` with an explicit condition. Do not use an optional method, an empty response, or a boolean matrix to imply support.
- **[Inferred]** For every write capability, require explicit declarations for idempotency and read-by-key, even when the provider offers neither. For a positive placement capability, require a stable key location/scope and a read operation that can recover the attempt after an unknown response. A provider field that is merely optional upstream (for example Alpaca `client_order_id`) can be made required by the UTA projection only when the adapter generates/injects it and the mapping is validated.
- **[Inferred]** Represent `asOf` with a response JSON Pointer and provider-reported semantics; never use local receive time as the upstream observation time. Represent paged/streamed observations with request/response cursor bindings and an explicit disconnect/end reason.
- **[Inferred]** Keep standard `x-*` vendor metadata opaque to generated clients. The pack validator must parse `x-uta` before client construction and preserve the raw source/response payload at the adapter boundary.

Suggested extension fields (names are recommendations):

| Extension | Required meaning |
|---|---|
| `x-uta.schemaVersion` | **[Inferred]** Version of the UTA extension schema, independent of OAS and vendor info versions. |
| `x-uta.provider` | **[Inferred]** Stable provider identifier; not an engine class name. |
| `x-uta.capabilities` | **[Inferred]** Provider-indexed capability index with status, operation references, and semantic requirements. |
| `x-uta-channel` | **[Inferred]** `read-only` or `read-write`; a type-level direction, not an arbitrary runtime flag. |
| `x-uta-kind` | **[Inferred]** Stable open kind identifier such as `order.place` or `order.observe`; do not make a closed central enum of provider verbs. |
| `x-uta-idempotency-key` | **[Inferred]** Structured location, name/path, scope, and whether UTA must supply it. Required for every declared write. |
| `x-uta-read-by-key` | **[Inferred]** Operation ID plus key mapping that can recover the same intent after timeout/unknown. Required for every declared write. |
| `x-uta-as-of` | **[Inferred]** Observation response pointer and source-time semantics. |
| `x-uta-observation-cursor` | **[Inferred]** Request/response cursor bindings, retention/replay rules, and stream end semantics. |

### 2. Illustrative real-venue projection excerpt (Alpaca)

This is a deliberately partial overlay/excerpt. The vendor `components` referenced below remain from Alpaca's original `trading-api.json`; `x-uta-*` values are UTA-owned metadata.

```yaml
openapi: 3.1.2
info:
  title: Alpaca Trading API (UTA projection)
  version: 2.0.1+uta.1
servers:
  - url: https://paper-api.alpaca.markets
x-uta:
  schemaVersion: 1
  provider: alpaca
  capabilities:
    order.place:
      status: supported
      operationId: postOrder
      readByKeyOperationId: getOrderByClientOrderId
      idempotency: required
      readByKey: required
paths:
  /v2/orders:
    post:
      operationId: postOrder
      x-uta-channel: read-write
      x-uta-kind: order.place
      x-uta-idempotency-key:
        in: body
        path: /client_order_id
        requiredByUta: true
        scope: account
      x-uta-read-by-key: getOrderByClientOrderId
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/CreateOrderRequest'
      responses:
        '200':
          description: Provider response; UTA records it as receipt plus observation
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Order'
  /v2/orders:by_client_order_id:
    get:
      operationId: getOrderByClientOrderId
      x-uta-channel: read-only
      x-uta-kind: order.observe-by-client-key
      x-uta-as-of:
        response: /updated_at
        semantic: provider-reported
      parameters:
        - in: query
          name: client_order_id
          required: true
          schema:
            type: string
            maxLength: 128
      responses:
        '200':
          description: Upstream order observation
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Order'
```

- **[Observed — `/tmp/w1-openapi-fetch-evidence.txt:2`]** Alpaca's upstream operation and schema names used in this excerpt are present in the fetched document; `client_order_id` is optional upstream, so the UTA-required mapping is an overlay decision.
- **[Inferred]** A `200` order response must not by itself be exposed as final fill/position truth. The adapter may retain returned provider fields as an observation associated with the placement receipt, while later reads/stream events establish subsequent upstream state.

### 3. Runtime surface semantics

These recommendations follow the UTA axioms and are the behavior the generated adapter must preserve.

| Surface concern | Recommended semantic contract |
|---|---|
| Caller | **[Inferred]** UI, AI tools, Telegram, and issue callbacks submit an intent to one UTA read-write capability entry; none calls a generated provider mutation function directly. Read-only consumers may share market/account observation handlers. |
| Trust principal | **[Inferred]** Alice-owned configuration supplies provider credentials; the projection contains auth scheme names and locations, never secrets. Raw provider payloads are untrusted until parsed at the adapter boundary. |
| Request identity | **[Inferred]** One UTA intent ID and stable idempotency key survive every attempt, timeout confirmation, and replay. The provider request ID/order ID is retained as a separate receipt/observation identity. |
| Concurrency | **[Inferred]** Serialize writes per provider/account/channel; allow independent reads and stream consumers to run concurrently. For the target of at least 100 upstream WebSocket connections, use a bounded supervisor/backpressure policy and keep stream fan-out separate from the write serializer. |
| Retry | **[Inferred]** Retry a transport failure only when there is no evidence of send or the projection declares safe idempotency. For `unknown`, recover by the declared read-by-key path before any new placement; explicit provider rejection is not an automatic retry. |
| Error translation | **[Inferred]** Preserve distinct variants for request-not-sent transport failure, sent-without-response `unknown`, provider rejection (code/request ID/raw body), parser failure, and unsupported capability. Never collapse them to a boolean or string. |
| Final side effect | **[Inferred]** The generated HTTP/WS client only performs the provider IO. Placement returns a receipt; final order/fill/balance state arrives through provider observations, reconciliation, or stream messages and is recorded separately. |
| Persistence | **[Inferred]** Store immutable source/projection/pack metadata as configuration/runtime state. Store intent, attempt, receipt, raw response/event, observation, unknown, and reversal records in the append-only ledger; do not mix the projection artifact into ledger facts. |

### 4. Build, activation, and failure behavior

- **[Inferred]** Build input: fetch or receive a pinned vendor OAS, verify its digest, parse/resolve it, apply only the allowed UTA overlay, validate every referenced operation/schema/pointer, and generate the typed HTTP client plus response parsers. Build a separate AsyncAPI/stream parser artifact where streams exist.
- **[Inferred]** Build rejection: fail a projection with malformed OAS, unresolved refs, invalid extension variants, duplicate capability identities, operation-direction conflict, or a supported write lacking mandatory idempotency/read-by-key declarations. Do not downgrade such errors to `unsupported` automatically, because that hides a claimed safety guarantee.
- **[Inferred]** Runtime activation: package the projection and generated code as an immutable provider release. Alice verifies manifest/checksum and atomically activates it using the existing Broker Pack transaction. UTA loads the active release and never downloads or installs dependencies at runtime.
- **[Inferred]** Runtime partiality: an explicitly unsupported or unavailable capability remains visible and returns structured `Unsupported { provider, kind, reason }`; it must not return an empty success, `undefined`, or a fabricated handler. Static consumers still declare their required handler types separately from runtime provider capability.
- **[Inferred]** Runtime drift: if an upstream response fails the generated parser, retain raw input and return a structured parse/schema failure; do not coerce it into a successful observation. If the source digest/version changes, produce a new immutable release rather than mutating the active one.

### 5. UTA-owned published contract

- **[Inferred]** Move durable HTTP request/response/error schemas into `@traderalice/uta-protocol`, retaining Zod 4 as the canonical boundary because that is the package's existing dependency and stated purpose.
- **[Inferred]** Generate UTA's OAS 3.1 with a Zod-to-OpenAPI generator such as [`zod-openapi`](https://github.com/samchungy/zod-openapi), then serve a versioned `/openapi.json` from the Hono app. Generate Alice/UI clients at build time rather than hand-copying response types. This is an API publication step, not provider projection ingestion.
- **[Inferred]** If the core adopts Effect, an Effect `HttpApi` can become the canonical UTA HTTP definition and produce both OAS and an Effect-validated client. Do not mix Effect v3/v4 imports; the reviewed `@effect/platform` evidence is peer-coupled to Effect v3 (`local://w1-openapi-tooling.md:9,52-60`).
- **[Inferred]** Publish a companion AsyncAPI document for UTA WebSockets, including message variants, connection/auth metadata, cursor/reconnect behavior, and send/receive direction. Keep generated HTTP and stream clients separate so a market watcher cannot accidentally obtain a write handler.
- **[Inferred]** The UTA published contract should describe UTA receipts, observations, and errors—not expose a vendor's `Order` as a universal final-state type. Vendor payloads remain provider-indexed/opaque outside their adapter.

## Unavailable / contradictions

- **[Unavailable]** No live authenticated provider call was made. The evidence establishes document shape and documented operations, not account permissions, rate limits, actual error bodies, or whether a particular account's credentials can use an operation.
- **[Unavailable]** No common fixture has yet been generated with every candidate tool. Arbitrary `x-uta` preservation, `$ref` handling across generator versions, unions, multiple response statuses, and readOnly/writeOnly behavior remain to be proven by a scoped fixture before selecting a generator.
- **[Unavailable]** The venue matrix does not establish complete platform coverage. IBKR explicitly warns its reference omits some endpoints; Longbridge's machine-readable YAML and current per-page docs disagree on the presence of normal order placement; Bybit's suite is split and its auth declarations are less reusable than its prose guide.
- **[Unavailable]** A scoped absence of an OpenAPI/AsyncAPI marker in OKX, Bitget, Hyperliquid, or LeverUp docs is not proof that no private artifact exists elsewhere. Their current fetched references should be treated as prose/manual-adapter inputs until a canonical machine-readable source is supplied.
- **[Unavailable]** WebSocket event schemas, ordering guarantees, replay windows, reconnect tokens, rate limits, and regional failover details were not normalized for all nine venues. AsyncAPI sidecars must be authored from each venue's documented/probed stream behavior.
- **[Unavailable]** Rust generator/runtime selection was not investigated in this slice. The projection artifact and `x-uta` manifest should remain language-neutral; the Rust-specific client/parser choice belongs to the language/runtime decision.
- **[Contradiction to resolve]** `packages/uta-protocol/src/schemas/index.ts:1-10` promises shared Zod source-of-truth schemas, while `services/uta/src/http/routes-trading.ts:14-47` still owns route-local schemas. Publishing OAS before reconciling those sources would create a second contract.
- **[Contradiction to resolve]** Existing Broker Pack docs list live SDK wrappers as the current transport boundary (`docs/broker-packs.md:32-56`), while the new input direction favors generated OpenAPI clients. The pack API can host generated clients, but the migration boundary and whether SDK wrappers remain for venues lacking OAS must be decided explicitly rather than assumed.
