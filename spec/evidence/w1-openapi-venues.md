# Official venue API/spec evidence (UTA)

## Method and evidence rules

- **Observed:** URL mode of `functions.read` returned `URL reads are disabled by settings` for the candidate pages. I therefore used the installed `agent-browser read` **docs reader** for every URL below (not GitHub API/source reads), saved each response, and recorded status, final URL, content type, and scoped output.
- **Observed:** Raw JSON specs were parsed only after the docs-reader fetch; the extraction output is quoted below. `functions.read` was used for local line references to the extracted output.
- **Caveat:** A missing marker/link in a fetched page is a scoped absence, not proof that a vendor has no other artifact. No authenticated/live calls were made. GitHub source/spec URLs were not opened.

## Summary table

| Venue | Official fetched spec/reference URL and metadata | REST coverage / auth evidence | Streaming representation and caveat |
|---|---|---|---|
| **Alpaca** | `https://docs.alpaca.markets/us/openapi` (200; “All OpenAPI Files for v1.4”); raw `trading-api.json`, `market-data-api.json`, `auth.json` all 200. OpenAPI 3.1.2. | Trading 42 paths/59 methods; `/v2/orders`, `/v2/orders/{order_id}`, `/v2/orders:by_client_order_id`, `/v2/account`. Trading security is `API_Key` + `API_Secret` headers. Market spec is 44 GET paths across stocks/crypto/options/forex/news; key/secret or BasicAuth. | Market-data WS and trading `trade_updates` WS are prose docs; market spec also contains an SSE path. Index label `v1.4` differs from Trading info.version `2.0.1`; preserve both upstream values. |
| **Longbridge** | `https://open.longbridge.com/docs/api?page=overview` 200 but empty body. Official `https://open.longbridge.com/llms.txt` 200 points to current `open.longportapp.com` per-page docs; no standalone OpenAPI artifact was exposed by the fetched index. | Per-page docs show `POST /v1/trade/order` and `GET /v1/trade/order` with required `order_id`; OAuth 2.0 is default, with discovery endpoints. No OpenAPI security-scheme declaration was fetched. | Quote and trade feeds are WebSocket/TCP with protobuf commands (`wss://openapi-quote.longportapp.com`, `wss://openapi-trade.longportapp.com`). GitHub-hosted spec candidate was intentionally not read. |
| **IBKR Client Portal Web API** | `https://www.interactivebrokers.com/docs/openapi/api-reference.json` 200, raw JSON; OpenAPI 3.1.0, info 1.0.0, 168 paths/180 methods. Servers: local Gateway, production, sandbox. | Paths include `POST /v1/api/iserver/account/{accountId}/orders`, order status by `{orderId}`, and market-data snapshot. Parsed spec has no `securitySchemes`; docs separately describe Client Portal Gateway, OAuth 1.0a, and OAuth 2.0. | Official docs expose `wss://localhost:5000/v1/api/ws`; topics include `smd`, `sor`, `str`, etc. Spec security omission is a material fidelity caveat. |
| **Binance Spot** | `https://developers.binance.com/en/docs/llms.txt` 200; generated sections label Spot REST/WS API `1.0.0`. `rest-api.md`, `web-socket-streams.md`, and `user-data-stream.md` fetched 200. Swagger UI URL fetched 200 with empty body; no allowed standalone YAML fetch. | Official index lists `/api/v3/order` POST/GET/DELETE, `/api/v3/account`, `/api/v3/allOrders`, market endpoints. REST prose declares `NONE`, `TRADE`, `USER_DATA`, `SIGNED`, API-key/signature rules. | Market WS base `wss://stream.binance.com:9443`/`:443`; user stream emits `executionReport`. A GitHub-hosted YAML candidate was not opened because GitHub source reads are prohibited; treat standalone OpenAPI as unverified. |
| **OKX** | `https://www.okx.com/docs-v5/en/` 200 HTML (reader output truncated at configured limit); official REST/WS reference, no downloadable OpenAPI/Swagger link verified in the fetched page. | Observed `POST /api/v5/trade/order`, `GET /api/v5/trade/order` (ordId or clOrdId), account instruments; private REST requires `OK-ACCESS-KEY/SIGN/TIMESTAMP/PASSPHRASE`. Regional domains are required for US/AU and EU users. | Production public/private/business WS URLs are documented; subscribe messages use `op` and `args`, with account/positions channels. No formal OpenAPI version observed. |
| **Bybit** | `https://bybit-exchange.github.io/docs/v5/intro` and API Explorer fetched 200. Intro calls V5 a unified “set of specifications”; no standalone machine-readable spec link/version was exposed by fetched docs. | `POST /v5/order/create` and `GET /v5/market/tickers` are documented; API Explorer lists V5 categories and counts. Auth uses HMAC or RSA plus `X-BAPI-API-KEY`, timestamp, signature, and recv-window. | Public, private, and order-entry WS endpoints are documented, including auth, subscribe/unsubscribe, ping, and regional hosts. GitHub YAML source was not opened; OpenAPI version remains unavailable here. |
| **Bitget** | UTA catalog/order docs fetched 200; `https://www.bitget.com/docs/catalog` itself returned 200 with empty body, while Place Order resolved to `https://www.bitget.com/docs/catalog/trading/order-management`. No standalone OpenAPI/Swagger link was exposed. | `POST /api/v3/trade/place-order`, UTA trade read/write permission, and `clientOid` are documented; UTA auth uses `ACCESS-KEY/SIGN/TIMESTAMP/PASSPHRASE`. | UTA public/private WS domains and ticker/order/account/position channels are documented; order channel is update-only and REST is needed for pre-existing unfilled orders. No formal OpenAPI version observed. |
| **Hyperliquid** | Official GitBook `.../for-developers/api`, `info-endpoint`, and `exchange-endpoint` fetched 200 Markdown; no standalone OpenAPI/AsyncAPI artifact exposed by API landing/index. | One discriminated `POST https://api.hyperliquid.xyz/info` covers public/user info; one signed `POST .../exchange` covers orders, cancel, modify, transfers. Request body requires `action`, `nonce`, `signature`; cloid/order lookup is documented. | `wss://api.hyperliquid.xyz/ws` and testnet WS are documented with `subscribe` messages and reconnect/snapshot guidance. Prose uses many `type` variants rather than a formal OpenAPI document. |
| **LeverUp** | `https://developer-docs.leverup.xyz/`, `/api/overview`, and `/llms.txt` fetched 200; docs identify LeverUp as a decentralized perpetuals protocol on Monad. No OpenAPI/Swagger/AsyncAPI artifact was exposed. | REST base `https://service.leverup.xyz`; docs explicitly say read-only HTTP, no authentication, with prices/markets/positions/history/portfolio/leaderboards. Trading is a separate onchain Diamond or gasless EIP-712 intent path. | No WebSocket/AsyncAPI entry appeared in the fetched complete-doc index; do **not** infer protocol-wide absence. This is not a conventional CEX REST trading venue; coverage is protocol REST + onchain/gasless execution. |

## Scoped fetch evidence

### E1 — Alpaca

- `https://docs.alpaca.markets/us/openapi` returned `All OpenAPI Files for v1.4`, listing `auth.json`, `broker-api.json`, `market-data-api.json`, and `trading-api.json` (reader output).
- Raw extraction from `https://docs.alpaca.markets/us/openapi/trading-api.json` (HTTP 200, `application/json`): `openapi=3.1.2`, `info.title=Trading API`, `info.version=2.0.1`, paper/live servers, `paths=42 methods=59`, and security `API_Key`/`API_Secret` headers (`APCA-API-KEY-ID`, `APCA-API-SECRET-KEY`). It exposes `POST /v2/orders`, `GET /v2/orders/{order_id}`, `GET /v2/orders:by_client_order_id`, and `GET /v2/account` (scoped extraction `/tmp/w1-repro.txt:1-7`).
- Raw extraction from `https://docs.alpaca.markets/us/openapi/market-data-api.json` (HTTP 200): `openapi=3.1.2`, `info.title=Market Data API`, `info.version=1.1`, production/sandbox data servers, `paths=44 methods=44`, and `BasicAuth` plus API key/secret schemes. It exposes stock bars, crypto bars, news, and an SSE corporate-actions path (`/tmp/w1-repro.txt:8-13`).
- `https://docs.alpaca.markets/us/docs/streaming-market-data` reader output lines 24–47: `wss://stream.data.alpaca.markets/{version}/{feed}` and test stream `.../v2/test`; lines 66–79 describe WS auth; lines 160–179 describe subscribe messages. `https://docs.alpaca.markets/us/docs/websocket-streaming` lines 11–15 documents trade/account/order updates and `trade_updates`.

### E2 — Longbridge / LongPort

- `https://open.longbridge.com/llms.txt` reader output lines 33, 236–248, and 262–274 lists API reference, quote/trade docs, and socket endpoints. The same fetch has `openapi_token_count=0` and `swagger_token_count=0` (scoped marker scan; absence only).
- Current per-page reader fetch `https://open.longportapp.com/docs/trade/order/submit.md` lines 13, 29–30: order submission is for HK/US stocks, warrants, and options, `POST /v1/trade/order`; `.../order_detail.md` lines 29–40 gives `GET /v1/trade/order` and required `order_id`.
- `https://open.longbridge.com/docs/how-to-access-api` lines 8, 25–39 and 116–121: OAuth 2.0 is the default, discovery URLs are listed, and API calls use `Authorization: Bearer ACCESS_TOKEN`.
- `https://open.longportapp.com/docs/socket/hosts.md` lines 8–22 and `subscribe_quote.md` lines 16–21 document separate quote/trade WebSocket endpoints; `subscribe_trade.md` lines 8–14 documents the trade feed.

### E3 — IBKR

- Raw extraction from `https://www.interactivebrokers.com/docs/openapi/api-reference.json`: OpenAPI `3.1.0`, info `API Reference`/`1.0.0`, 168 paths/180 methods, servers local Gateway/production/sandbox, and `securitySchemes=None` (`/tmp/w1-repro.txt:18-23`).
- `https://ibkrcampus.com/docs/web-api/authentication/introduction.md` lines 6–10, 21–24, and 39–42 documents Client Portal Gateway, OAuth 1.0a, and OAuth 2.0. This is separate from the parsed spec’s empty security-scheme map.
- `https://ibkrcampus.com/docs/web-api/v1/ws/introduction.md` lines 6–12: session-required topics include `smd`, `sor`, `str`; URL is `wss://localhost:5000/v1/api/ws`. `subscribing-to-websocket-topics.md` lines 6–18 defines `TOPIC+TARGET+PARAMETERS` and `s`/`u` subscription prefixes.

### E4 — Binance Spot

- `https://developers.binance.com/en/docs/llms.txt` reader output lines 1775–1827 labels `Spot REST API (1.0.0)` and lists `/api/v3/account`, `/api/v3/allOrders`, `/api/v3/order` GET/POST/DELETE, and market data paths. Lines 1828–1886 label `Spot WebSocket API (1.0.0)` and list order/account/market operations.
- `https://developers.binance.com/en/docs/products/spot/rest-api.md` lines 4–29 list REST base endpoints; lines 148–169 define `NONE`, `TRADE`, `USER_DATA`, and signed API-key requests; lines 173–175 require `signature` on signed endpoints.
- `https://developers.binance.com/en/docs/products/spot/web-socket-streams.md` lines 2–24 document WS base URLs, raw/combined streams, and ping/pong. `user-data-stream.md` lines 40–49 says orders emit `executionReport`.
- Reader metadata: `https://binance.github.io/binance-api-swagger/` returned HTTP 200 with an empty body; this does not establish spec absence. Standalone YAML was not fetched from GitHub.

### E5 — OKX

- `https://www.okx.com/docs-v5/en/` reader output lines 1728–1746 says OKX provides REST and WebSocket APIs and requires regional domains; lines 2438–2462 list production/demo REST and public/private/business WS URLs.
- The same fetched page lines 1812–1850 documents private REST headers and HMAC-SHA256/Base64 signing. Lines 15645–15682 show `POST /api/v5/trade/order`; lines 17883–17948 show `GET /api/v5/trade/order`, `ordId`/`clOrdId`, and lookup precedence.
- WebSocket subscribe evidence is in the same official fetch lines 12943–13007 (`op=subscribe`, channel args, response) and lines 13723–13731 (`positions` channel). No formal OpenAPI version was present in the fetched reference.

### E6 — Bybit

- `https://bybit-exchange.github.io/docs/v5/intro` lines 43–71 identifies V5, says it is a unified set of specifications, and documents `POST /v5/order/create`; lines 91–110 divide paths into market/order/position/account/asset modules.
- `https://bybit-exchange.github.io/docs/v5/guide` lines 187–194 lists testnet/mainnet REST bases; lines 220–246 documents HMAC/RSA key types and `X-BAPI-API-KEY`, timestamp, signature, and recv-window headers.
- `https://bybit-exchange.github.io/docs/v5/ws/connect` lines 173–235 lists public/private/order-entry WS URLs; lines 277–295 show private-stream auth; lines 442–485 show subscribe/unsubscribe messages. API Explorer (`.../docs/api-explorer/v5/category`) lines 77–105 lists V5 categories and item counts.

### E7 — Bitget UTA

- `https://www.bitget.com/docs/catalog/trading/order-management` reader output lines 47–71: `POST https://api.bitget.com/api/v3/trade/place-order`, UTA trade read/write permission, and recommendation to provide `clientOid`; lines 132–166 include the clientOid constraint and timeout-recovery guidance.
- `https://www.bitget.com/docs/uta/quick-start` lines 112–136 list demo and production REST/WS domains; lines 212–224 list `ACCESS-KEY`, `ACCESS-SIGN`, `ACCESS-TIMESTAMP`, and `ACCESS-PASSPHRASE` headers.
- `https://www.bitget.com/docs/uta/best-practices-guide` lines 223–263 document the order channel, `GET /api/v3/trade/unfilled-orders`, clientOid, REST placement, and asynchronous acceptance; lines 325–339 require WS order updates for final state. `Ticker-Channel` lines 41–84 show public subscription shape.

### E8 — Hyperliquid

- `https://hyperliquid.gitbook.io/hyperliquid-docs/for-developers/api/info-endpoint` lines 5–13 says request bodies select different response schemas and applies to Perpetuals and Spot; lines 21–35 show `POST https://api.hyperliquid.xyz/info`; lines 332–344 show order-status lookup by `oid` or client order ID.
- `https://hyperliquid.gitbook.io/hyperliquid-docs/for-developers/api/exchange-endpoint` lines 21–35 document signed `POST https://api.hyperliquid.xyz/exchange`, TIF values, and optional 128-bit `cloid`; lines 43–51 require `action`, `nonce`, and `signature`; lines 121–138 show cancel requests.
- `https://hyperliquid.gitbook.io/hyperliquid-docs/for-developers/api/websocket` lines 3–23 documents mainnet/testnet WS URLs, `subscribe` messages, and reconnect/snapshot behavior. API landing page lines 3–17 provides mainnet/testnet HTTP URLs but no OpenAPI metadata.

### E9 — LeverUp

- `https://developer-docs.leverup.xyz/index.md` reader output lines 5–7 identifies LeverUp’s developer docs and its perpetuals protocol on Monad; lines 17–41 distinguish onchain, gasless, and REST integration paths.
- `https://developer-docs.leverup.xyz/api/overview.md` lines 1–15 explicitly describe read-only HTTP, no authentication, base `https://service.leverup.xyz`, and separate onchain/gasless trading; lines 32–46 give pagination; lines 48–104 list oracle/price, pairs, positions, history, portfolio, leaderboard, and competition paths.
- `https://developer-docs.leverup.xyz/llms.txt` lines 1–14, 16–39, and 41–49 list the complete docs sections, including raw trading ABI and EIP-712 gasless flow, but no WebSocket/AsyncAPI section. Scoped marker scan on this index: `openapi=0`, `swagger=0`; this is not proof of protocol-wide absence.
## Discovery-only machine-readable leads (not evidence)

The following URLs surfaced during discovery but were **not opened**: opening them would read GitHub-hosted source/spec content, which is outside this assignment’s allowed evidence path. They are follow-up leads only, not verified findings:

| Venue | Unverified lead |
|---|---|
| Longbridge | `https://raw.githubusercontent.com/longbridge/developers/main/openapi.yaml` |
| Binance Spot | `https://raw.githubusercontent.com/binance/binance-api-swagger/master/spot_api.yaml` |
| Bybit | `https://github.com/bybit-exchange/docs/tree/master/yml-folder/v5` |
| Hyperliquid | `https://github.com/hyperliquid-dex/hyperliquid-python-sdk/tree/master/api` |

Do not infer canonicality, version, coverage, or security declarations from these leads until they are read through an allowed source path.
