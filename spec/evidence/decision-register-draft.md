# New UTA — Draft Decision Register (Main's positions, v0)

Evidence packet: `local://w1-surface-map.md`, `local://w1-interaction-audit.md`, `local://w1-ledger-persistence.md`, `local://w1-provider-capability.md`, `local://w1-effect-evaluation.md`, `local://w1-language-runtime.md`, `local://w1-openapi-projection.md`, `local://w1-issue-system.md`, `local://w1-lifecycle-topology.md`, `local://w1-trace-matrix.md`. Axioms: `plans/uta-refactor/design/00-core-contract.md`, `01-detail-constraints.md`.

Each decision: position, why, what would falsify it.

## D1 Language: TypeScript (Node default, Bun 1.4.0 supported); no Rust core

- Benchmark: 100–200 upstream WS at 5–10k msg/s costs <10% CPU on both Node and Bun with zero loss; queue growth only appears near ~22k processed msg/s. Target is far inside the envelope.
- Rust would change all five launchers (dev/prod Guardian, Electron `ELECTRON_RUN_AS_NODE`, Bun `--internal-role uta`, Docker), the ASAR assertion, macOS/Windows signing, the release matrix, and would break the JS Broker Pack loading model; Pingora is an inbound proxy framework, not the primitive for outbound provider subscriptions; Rust has no stable ZIO-like effect system.
- Escape hatch kept by design: the provider *transport executor* seam is serializable (records, decimal strings, tagged errors, raw envelopes). A Rust transport sidecar can later be attached behind the same capability witness without touching the domain.
- Falsifier: representative provider replay needing ≥25k processed msg/s sustained with p99 ≤5µs per message through fan-out, or a Rust prototype meeting that while TS fails.

## D2 Effect-ts: Option B — Effect inside the UTA process only

- Effect owns: composition root (`Layer`) = A3 static requirement check (missing handler → TS2345 proven); `Scope`/`acquireRelease` for every provider connection/subscription; bounded `Queue`/`PubSub` for fan-out with explicit backpressure; one-permit `Semaphore` per account write channel; `Schedule` only for *classified* retryable failures; `TestClock` for reconnect/poll/expiry tests; `ManagedRuntime` at process root; `uninterruptibleMask` around the attempt-append → provider-call → receipt-append critical section.
- Effect does NOT appear in: `@traderalice/uta-protocol` (wire types, Zod 4 schemas), the ledger record format, the Pack/projection ABI (Promise-facing), the UI, Alice SDK/tools. Conversion at exactly two seams: HTTP ingress (Hono route → Zod decode → `runtime.runPromise`) and provider adapters (`Effect.tryPromise` + error classification).
- Effect Schema / HttpApi: NOT adopted. UTA publishes OpenAPI 3.1 from protocol Zod schemas (`zod-openapi`); Alice/UI clients generated from that document (kills the UI hand-copies).
- Falsifier: loading a real compiled Pack under pinned Bun 1.4.0 forces Effect types across the Pack boundary, or the Effect closure breaks the measured startup/RSS budget.

## D3 Provider model: Projection = declaration + codec; Transport = generic executors + transport plugins

- A `ProviderProjection` release = immutable vendor source (OAS or declared non-OAS) + `x-uta` overlay (versioned) + stream manifest (AsyncAPI-like) + generated typed client/parsers + translation module. Distributed/activated through the existing Broker Pack lifecycle (immutable release dir, `active.json` atomic swap, checksum/manifest validation, no runtime package management).
- Capability declarations are *semantic variants*, never booleans: `IdempotencyContract` (`cachedReplay{retention}` | `uniqueKeyRejectsDuplicate{scope, reuseAfterTerminal}` | `none`), `ReadByKeyContract` (`byClientKey{coverage}` | `byProviderIdOnly` | `none`), `ObservationCursorContract` (`resumable{encoding, replayWindow}` | `snapshotOnly` | `none`). Unknown → declared `none`/`unsupported`; build rejects a `supported` write lacking idempotency+read-by-key declarations.
- Retry policy for `unknown` receipts is *derived* from these contracts, per account. Providers with `none/none` never auto-retry; they stop at `awaiting-review`.
- Legacy SDK adapters (CCXT per venue, IBKR TWS, LeverUp, Mock) become *manual projections*: hand-written declaration + translation + transport plugin behind the same ABI; they must declare honestly (today: no A7-conforming adapter exists).
- Existential boundary: `P` is retained from projection lookup through handler execution inside the process. Storage and wire carry `ProviderEnvelope<{providerId, projectionVersion, payload}>`; re-association via the projection registry, never `as`.
- Falsifier: a generator fixture shows `x-uta` cannot be preserved/validated alongside vendor `$ref`/unions, or a venue's WS semantics cannot be expressed in the stream manifest.

## D4 Command lifecycle (normative state machine)

Entry kinds (all append-only, per-account stream, each with `entryId, position, occurredAt, recordedAt, source, kind, idempotencyKey, why, payload`):

1. `intent.proposed` — immutable; carries `intentId`, proposer `Principal`, `Origin`, scope (account, operation kind, provider-indexed operation payload), `expiresAt`. Durable before anything else may happen.
2. `authorization.decided` — `approve | reject | withdraw`, actor `Principal`, bound to `intentId + intentHash`, `policyResult`. Multiple decisions allowed; the *effective* decision is the fold. Human one-shot (UI form) = `intent.proposed` + `authorization.decided(approve, actor=human)` in one append batch, only when the principal is human and account policy allows self-authorization. `allowAiTrading` → replaced by per-account `AuthorizationPolicy` (grants: which principal kinds may self-approve, limits); a policy approval is recorded with actor `policy:<id>`.
3. `attempt.started` — durable *before* the provider call; `attemptNo`, same idempotency key, provider key placement per projection contract.
4. `receipt.recorded` — `accepted{providerRef}` | `rejected{providerCode, requestId, raw}` | `unknown{cause: timeout|disconnect|no-response}`; transport failure with proof-of-not-sent → `attempt.abandoned{reason}` (may re-attempt).
5. `observation.recorded` — upstream facts only (order state, fills, balances), with `asOf`, provider source, correlation to intent/attempt when known.
6. `reversal.appended` — consumer-local "not continuing" with nested structured reason; references target entry; never claims remote rollback.
7. Compensation = new `intent.proposed` with `causedBy`.

Consumers (kind-subscribed, own cursor, pure rule sets, structured `allow | reject(reasonTree) | pending(reason)`): Authorization consumer, Execution consumer (the single write entry), Reconciliation consumer (subscribes `receipt.unknown`, `receipt.accepted` without observation within deadline, `attempt.started` without receipt after restart), Projection consumer (materialized views), Outbox consumer (event stream), Expiry consumer (intent past `expiresAt` → reversal).

Crash matrix (restart outcome): no intent → nothing; intent w/o decision → still pending, visible; decision w/o attempt → Execution consumer resumes; attempt w/o receipt → Reconciliation via read-by-key contract, else `awaiting-review`; receipt w/o observation → poll; observation → fold. No blind retry after `unknown`.

## D5 Persistence contract

- Ledger: `data/trading/<accountId>/ledger/{manifest.json, segments/segment-NNNNNN.jsonl, quarantine/}` + `ledger.lock/` (runtime-lock style dir lock with owner token/heartbeat). Frame = `len sha256 json\n`; body has `prevHash`; `append(entries, expectedPosition)` → `Appended | Conflict{expected, actual} | DurabilityFailure`. Durability = segment fsync → manifest tmp+rename → dir fsync (best effort on Windows, documented) → then publish. Torn tail → quarantine + structured `TornTail` result; interior corruption → account ledger open fails structured.
- Views/checkpoints: separate files with `sourcePosition/sourceHash`; rebuildable; failure never touches ledger.
- Migration `0044`: convert each `commit.json` into `legacy.trading-git.commit` entries (opaque payload, kind namespaced), archive original under `data/trading/<id>/legacy/`, per-account completion marker; ephemeral accounts excepted (existing wipe rule stays as explicit exception).
- Config: `accounts.json` stays Alice-owned and sealed (UTA reads); UTA-owned runtime config (policies, projections activation) is a separate whole-replace atomic file with schema; restart flag protocol unchanged.
- Snapshots: keep as observation-derived store, separate from ledger; fix chunk/index invariants; never a second truth.

## D6 Replacement envelope: atomic in-repo migration, no HTTP facade

- Preserved exactly: process contract (`OPENALICE_UTA_PORT`, loopback, `GET /__uta/health {ok,startedAt,utas}`, SIGTERM/SIGINT, `restart-uta.flag`, `OPENALICE_HOME`/`OPENALICE_APP_HOME` split, Bun `--internal-role uta`, Node `dist/uta.js` entry), sealed `accounts.json` reading, Broker Pack release layout, ledger history via migration, `alice-uta` CLI command names, `aliceId` instrument identity, decimal-string wire discipline, mode semantics (`lite/readonly/pro`).
- Replaced: all `/api/trading/*` and `/api/simulator/*` routes → versioned `/v2/*` surface published as OpenAPI; SDK, 26 tools, BFF, connector, bars gateway, UI api layer, demo handlers migrate in the same change set. The legacy `GET /uta/:id/quote/:symbol` is dropped. Facade rejected: every consumer is in this monorepo and ships together; a facade would be weightless code with no in-repo caller after cutover.
- Falsifier: an out-of-repo caller of `/api/trading/*` is identified.

## D7 Security and principals

- UTA remains loopback but stops trusting loopback alone: Guardian generates a per-run bearer token, injects it to both Alice and UTA (`OPENALICE_UTA_TOKEN`); every UTA request carries it plus a `Principal` header set by Alice from its own auth context (browser session → `human`, CLI/MCP `x-openalice-run/session` → `agent{workspaceId,resumeId,runId}`, connector → `connector{connectorId, externalUserId?}`, scheduler → `schedule{issueId}`, policy → `policy{id}`). UTA records principal on intents/decisions; unauthenticated requests are 401.
- Resource ids: `accountId` must resolve through the registry before any path/FS use; no filesystem path derived from raw input (closes the traversal class).
- BFF gates become explicit route tables generated from the OpenAPI (`x-uta-channel`), not substring matching.

## D8 Event stream

- UTA publishes a durable, cursor-addressable event stream per home derived from ledger appends, observations, health and readiness: `GET /v2/events?after=<cursor>` (long-poll) and SSE. Consumers own cursors. UI live stores consume via BFF SSE relay (replaces the seven polling intervals); the Issue bridge and connector consume the same stream.

## D9 Issue integration (UTA consumes AI via Alice Issues)

- UTA never writes Alice's issue files. It appends `work.requested` events (`review.unknown-outcome`, `review.guard-rejection`, `reconcile.discrepancy`, `watch.triggered`, `news.received`, `intent.awaiting-authorization`) with `requestId`, `accountId`, optional `intentId`, canonical Markdown `what`, `expiresAt`.
- Alice runs a **UTA desk bridge** (pattern: connector action-bridge + `connectorDesk`): per account or per home a configured desk Workspace; bridge consumes the event stream with its cursor, creates Issue `uta-<accountId>-<requestId>` (deterministic id → idempotent), stores the link in a launcher-owned store (not YAML frontmatter), and comments receipts/outcomes with deterministic comment ids.
- Decisions flow back only through a typed endpoint: `POST /api/uta/intents/:id/decisions` on Alice → UTA `authorization.decided` with principal `agent{resumeId}` or `human`. Whether an agent decision carries authority is decided by the account `AuthorizationPolicy`; otherwise it is recorded as a recommendation and the intent stays pending human decision. Issue `done` never means executed; Issue closure follows UTA outcome events.
- Agents may also create new intents through the existing tools; those carry `Origin{issueId, resumeId, runId}`.

## D10 Rules/guards

- Rule sets are consumer-local pure functions over parsed inputs (`Decimal` quantities, branded ids, `asOf` observations). Config for rules is parsed at load with fail-closed semantics (NaN/empty whitelist rejected at config write). Results are `allow | reject(ReasonTree) | pending(Reason)`; `ReasonTree` nests child rule failures with constructors preserved. Unknown intent kind → `reject(UnknownKind)`.

## D11 Identity, numbers, time

- Branded types for `AccountId, ProviderId, ProjectionVersion, IntentId, AttemptNo, EntryId, LedgerPosition, IdempotencyKey, ProviderOrderRef, ClientOrderKey, AliceId, NativeKey, Instant, AsOf, Cursor`. Money/quantity as decimal strings on wire and `Decimal` in-process (branded `Qty`, `Money{currency}`). `occurredAt` (upstream/business time) vs `recordedAt` (append time) always separate. FX rates are read-only observations with `asOf` and staleness variants.

## D12 Operations

- Readiness: `/__uta/health` unchanged (process); `/v2/readiness` per account: `{process, transport, readable, writable, capabilities, observationFreshness}`. Drain on SIGTERM: stop accepting intents, let the uninterruptible attempt section finish, close scopes, flush cursors. Crash restart policy: unchanged (Guardian does not auto-retry) — recorded as an explicit non-change.

## D13 Scope

- In: everything above, Mock as manual projection, simulator on `/v2/simulator`, FX as read-only channel, snapshots retained. Out: Futu/OpenD, Alice-wide Issue redesign, portfolio strategy, new venues beyond declaring existing ones honestly.
