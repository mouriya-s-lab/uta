# New UTA — Binding Decision Register

Status: **frozen for authoring** (2026-09-14); **amended after acceptance review** (2026-09-15, amendments A1–A11 folded into the decisions below). This document is the single place where cross-cutting design decisions are made. Spec documents `01`–`09` and the type package under `spec/types/` elaborate these decisions; they may not contradict them. Axioms A1–A8 in [[plans/uta-refactor/design/00-core-contract.md]] and the constraints in [[plans/uta-refactor/design/01-detail-constraints.md]] are fixed inputs and are not restated here.

Evidence behind each decision comes from the Wave-1 investigation (surface map, interaction audit, ledger persistence, provider capability, Effect evaluation, language/runtime benchmark, OpenAPI projection, Issue system, lifecycle topology) and two discussant rounds. Where a decision rests on a measurement, the measurement and its limits are named. Each decision states what would falsify it.

## Vocabulary used below

| Term | Meaning |
|---|---|
| **Account** | One configured provider account (`AccountId`), the unit of the ledger stream and of the write channel. |
| **Projection (provider)** | A Broker Pack release whose module exports a capability *declaration* (data), *translation* (raw ↔ typed), and *transport plugin(s)*; see D3. |
| **Intent** | Immutable request to change upstream state (`intent.proposed`). |
| **Proposal** | Optional grouping of intents proposed in one action (`proposalId`); the unit a human clicks. |
| **Decision** | `authorization.decided` entry: `approve | reject | withdraw` by a `Principal`. |
| **Attempt** | One provider placement for one intent, durably recorded before the call. |
| **Receipt** | Provider's answer at the attempt boundary: `accepted | rejected | unknown`. |
| **Observation** | Upstream fact read later (`asOf`, source). |
| **Reversal** | Consumer-local structured "not continuing" entry targeting a prior entry. |
| **Consumer** | Kind-subscribed reader of the ledger with its own cursor and pure rule set. |
| **Principal** | Who acted: `human | agent | connector | schedule | policy | system`. |

---

## D1 Language and runtime: TypeScript; Node default, Bun 1.4.0 supported. No Rust core.

**Decision.** New UTA is TypeScript, launched exactly as today (`node dist/uta.js`, `tsx` in dev, Bun `--internal-role uta`, Electron `ELECTRON_RUN_AS_NODE`, Docker Guardian). Node/Bun selection stays a deployment fact, not a source fork.

**Why.** Two-process synthetic benchmark on this host (exact scripts in the Wave-1 report): 100 and 200 upstream WebSockets at 5k/10k requested msg/s (~200-byte JSON, three in-process consumers) ran on Node 26.8 and Bun 1.4.0 with zero sequence gaps, zero parse errors, ≤10% mean CPU, p99 processing ≤12.5 µs; queue growth appeared only near ~22k processed msg/s. The target load is inside the envelope with an order of magnitude of headroom. A Rust core would change all five launchers, the ASAR assertion, macOS/Windows signing, the release matrix and the JS Broker Pack loading model; Pingora is an inbound proxy framework (Linux tier-1, Windows preliminary) and not the primitive for outbound provider subscriptions; Rust has no stable ZIO-like effect system.

**Limits of the evidence.** The probe establishes feasibility at the target load, not a capacity SLO: localhost, no TLS/proxy, no provider decoding, no reconnect storms, no ledger writes. Before the implementation is called accepted, run the full UTA with a compiled active Pack through the file-URL loader under a provider-like replay (see `08-verification.md`).

**Boundary invariant (not an escape hatch).** The provider transport seam carries only serializable values (records, decimal strings, tagged errors, raw envelopes) and receives credentials as **injected values**; no transport plugin reads `accounts.json`, `sealing.key`, or the sealed envelope. This is required independently by A7 and by the Pack class-identity rule; it is not a commitment to a future sidecar.

**Falsifier.** A representative provider replay needing ≥25k processed msg/s sustained for 10 minutes with p99 ≤5 µs through fan-out and bounded queues, on which the TypeScript implementation fails.

## D2 Effect-ts: adopted inside the UTA process only (Option B)

**Decision.** `effect` (v3, pinned) is a dependency of `services/uta` only. Effect owns the **runtime disciplines**: the composition root (`Layer`/`ManagedRuntime`), `Scope`/`acquireRelease` for every provider connection and subscription, bounded `Queue`/`PubSub` for fan-out with explicit backpressure, one-permit `Semaphore` per account write channel, `Schedule` applied only after failure classification, `TestClock` for reconnect/poll/expiry tests, and `Effect.uninterruptibleMask` around **local durable appends** (attempt append, receipt append). Provider I/O inside that region stays deadline-bounded and interruptible so drain cannot hang.

**What Effect never touches.** `@traderalice/uta-protocol` (wire types and Zod 4 schemas), the ledger record format, the Pack ABI (Promise-facing), the UI, the Alice SDK/tools, generated clients. `Cause`, `FiberFailure`, `Layer`, `Schema` values never serialize. `effect` is a runtime dependency of `services/uta` only; the private types package has it as a dev-only fixture dependency. Generated OpenAPI clients are Effect-free and are invoked only through the transport adapter's Promise wrapper; no generated-client value enters core serialization. **Control values** (`AbortSignal`/cancellation context, deadlines) are exempt from the "serializable business values" rule at the Pack ABI: they cross the ABI as arguments and never persist.

**Three containment points** (not two): (1) HTTP ingress — Hono route → Zod decode → `runtime.runPromise`, one adapter maps expected domain errors to wire errors and HTTP status; (2) Pack loader ABI — an extended module-shape validator normalizes returned values and thrown errors into serializable provider-indexed records before the core sees them; (3) provider transport adapter — `Effect.tryPromise` + error classification.

**Where A3's static check stops.** Inside the process, "every declared capability has a handler" is a compile-time fact: conditional types make write handlers unconstructible for read-only providers (ordinary TypeScript), and Layer `Requirements` make a missing service a compile error at the composition root (TS2345 proven on the sketch). **Across the dynamic Pack boundary the check is a runtime validator, permanently**; a Pack that omits a declared capability fails at load, never at call time.

**Schema.** Effect Schema and `@effect/platform` HttpApi are **not** adopted. UTA publishes OpenAPI 3.1 from the protocol Zod schemas (`zod-openapi`); Alice/UI clients are generated from that document, which retires the hand-copied UI types.

**Falsifier (open).** Loading a real compiled Pack under pinned Bun 1.4.0 forces Effect types across the Pack boundary, or the Effect closure breaks the measured startup/RSS budget. Neither has been tested; both are acceptance gates.

## D3 Provider model: a projection release *is* a Broker Pack release (ABI v2)

**Decision.** There is no new distribution mechanism. A provider projection ships as a Broker Pack release: same immutable release directory, `active.json` atomic swap, manifest/checksum/realpath-containment validation, no runtime package management. `BROKER_PACK_API_VERSION` increments to 2 and all five existing wrappers migrate in one cutover. The ABI changes from `createBroker → IBroker` to three exports:

1. **Declaration** (plain data): provider identity, projection version, source digest, a **venue-keyed capability table** (the effective tuple is resolved when account config loads, not when the release loads — `ccxt-custom` is a function of venue × config), transport family, auth scheme *names/locations* (never secrets).
2. **Translation**: pure functions raw ↔ `Operation<P>` / `Receipt<P>` / `Observation<P>`; raw payload retained at the boundary.
3. **Transport plugin(s)**: HTTP executor driven by the generated client; stream executor for WS; custom plugins for TWS TCP / on-chain. Credentials arrive as injected values.

**Module typing and loader.** `PackModule<D extends ProviderDeclaration>`: handlers (`recovery`, transport bindings) are keyed by the write kinds whose status is `supported` in `D` (a mapped type), so a Pack's own typecheck fails when a supported kind lacks its witness. The loader mirrors this at runtime and returns `LoaderValidationResult = loaded{module} | invalid{code: ApiVersion | DeclarationInvalid | MissingHandler | NonSerializable | ProjectionIdentity, path}`; a `supported` write row without a constructible witness is a load-time `MissingHandler`.

**OpenAPI is a build-time input.** The vendor OAS (or declared non-OAS source) is pinned by digest, the `x-uta` overlay is applied and validated in the pack build (each wrapper already has a `tsup` build), typed client and parsers are generated, and the release ships compiled code plus the declaration. **Runtime parses no vendor document.** Stream semantics live in a UTA-owned stream manifest (AsyncAPI-shaped) validated the same way; no venue publishes AsyncAPI, so each is authored from documented/probed behavior.

**Stream items.** A stream executor yields a tagged `StreamItem`: `event{channel, eventId?, asOf, providerTime?, cursor?, payload}` | `gap{channel, reason: cursorExpired | resubscribed | providerReset, lastCursor?}` | `ended{channel, reason: closedByProvider | deadline | drain | authRevoked}`. Observations are constructed only from `event`; `gap` triggers the manifest's backfill/resnapshot rule; `ended` feeds reconnect classification. The manifest's end-reason vocabulary is this same set. Rejected alternative: a payload-only event callback with separate `endReasons` (`exhausted | disconnect | cancelled | timeout`) — it let transport failures masquerade as orderly ends and gave the Observation constructor no evidence of gaps.

**Capability declarations are semantic variants with explicit unknowns.** Booleans are forbidden. For every write kind the declaration carries:

- `IdempotencyContract`: `cachedReplay{keyField, scope, retention}` | `uniqueKey{keyField, scope, window: Duration | 'untilTerminal' | 'unverified', reuseAfterTerminal: boolean | 'unverified', duplicateResponse: 'rejectsWithCode' | 'returnsExisting' | 'unverified'}` | `none`.
- `ReadByKeyContract`: `byClientKey{operation, coverage: 'openOnly' | 'openAndHistory' | 'unverified', historyWindow: Duration | 'unverified', ambiguity: 'unique' | 'latestMatch'}` | `byProviderRef{coverage: 'openOnly' | 'openAndHistory' | 'unverified', historyWindow: Duration | 'unverified'}` | `none`. `VerifiedReadByKey` is the refinement with no `'unverified'` field; only it can produce `confirmedAbsent`.
- `ObservationCursorContract`: `resumable{channel, encoding, replayWindow}` | `snapshotOnly` | `none`.

Observed values (Wave-1): Longbridge `cachedReplay{client_request_id, account, 10m}` + `byProviderRef`; Alpaca `uniqueKey{client_order_id, …, duplicateResponse unverified}` + `byClientKey{coverage unverified}`; OKX `uniqueKey{clOrdId, window untilTerminal, reuseAfterTerminal true}` + `byClientKey{ambiguity latestMatch}`; IBKR CP `uniqueKey{cOID, 24h, reuseAfterTerminal unverified}` + `byProviderRef`; Bybit `uniqueKey{orderLinkId, duplicateResponse rejectsWithCode}` + `byClientKey{openOnly}`. **No current adapter exposes any of these**; until a pack does, every account is statically read-only for writes.

**`PlacementRecovery<P>` witness.** A write capability for kind `k` is constructible only if the pack supplies `recover(attempt, now)` returning exactly one of: `found{receipt (provenance: recovered{via: replay | keyedRead | providerRefRead}), observation}` | `confirmedAbsent{proof: keyedLookupMiss{coverage, window}, validUntil}` | `ambiguous{candidates}` | `stillUnknown{nextCheckAfter, reason}`. Laws, provider-name free:

- `found` → append late receipt + observation; intent proceeds.
- `confirmedAbsent` is constructible only when the read-by-key contract's coverage and window are *verified* and the attempt time lies inside them. A duplicate-probe rejection proves **presence**, never absence.
- Re-attempt with the **same** key after `confirmedAbsent` is allowed only if idempotency ∈ {`cachedReplay` within retention, `uniqueKey` with `duplicateResponse ∈ {rejectsWithCode, returnsExisting}` and inside `window`}. Otherwise → `awaitingReview`.
- `ambiguity: latestMatch` results are `ambiguous` unless a correlation proof (provider echo of our key with matching payload hash) upgrades them to `found`.
- `stillUnknown` reschedules until the account's `maxUnknownDuration`, then → `awaitingReview`.
- Declaration `none`/`none` cannot construct the witness → the kind is `unsupported`; account `writable` is `blocked{NoPlacementRecovery}`.
- `confirmedAbsent` is the canonical name for the constraint document's `MissingRemote`.
- Longbridge's "resubmit the same `client_request_id` within 10 minutes and read the original response" is a legitimate `found{provenance: recovered{via: replay}}` because the provider contract guarantees no new order; it is recorded as a recovery, never as a fresh attempt.

**Existential boundary.** `P` is retained from projection lookup through handler execution in-process (`ProviderKey<P>`, `ProviderOrderRef<P>`, `Operation<P>`…); `P`'s phantom is the pair `(ProviderId, ProjectionVersion)`. Storage and wire carry envelopes `{providerId, projectionVersion, kind: OperationKind, payload}` that are **role-indexed** — `OperationEnvelope`, `KeyEnvelope`, `ReceiptEnvelope`, `ObservationEnvelope`, `ConfigEnvelope` — so roles cannot be swapped; re-association happens through the `ProjectionRegistry` (`decodeReceipt`, `decodeObservation`, `recoveryFor`), never through `as`.

**Legacy adapters** (CCXT per venue, IBKR TWS, LeverUp, Mock) become manual projections behind the same ABI and must declare honestly. IBKR CP Web API evidence must not be attributed to the TWS adapter.

**Falsifier.** A generator fixture proves `x-uta` cannot be preserved/validated alongside vendor `$ref`/unions; or a vendor's terms require the document at activation time rather than vendored at build.

## D4 Command lifecycle: an event algebra per account, not a linear state machine

**Entries** (append-only, one account stream; every entry has `entryId, position, occurredAt, recordedAt, source, kind, idempotencyKey, why, causedBy?, correlation{intentId?, attemptNo?, providerRef?}`):

| Kind | Payload essentials | Rule |
|---|---|---|
| `intent.proposed` | `intentId, proposalId?, intentHash, principal, origin, scope{accountId, operationKind, ProviderEnvelope}, expiresAt, configRevision` | Immutable; durable before anything else. Ingress validates shape and that the account declares the kind; an unsupported kind is a **response** (`CapabilityUnsupported`), not an entry. |
| `authorization.decided` | `decisionId (caller idempotency key), scope{proposalId? \| intentIds[]}, scopeHash, action approve\|reject\|withdraw, principal, policyResult` | Duplicate `decisionId` → same result, no new entry. Decision after `intent.expired` → append rejected with `IntentExpired`. |
| `intent.expired` | `intentId` | Appended by the Expiry consumer or by the Execution consumer when it finds `expiresAt` passed before attempting. Expiry applies until `attempt.started`. |
| `attempt.started` | `intentId, attemptNo, key: KeyEnvelope, configRevision, deadline` | Durable **before** the provider call; one attempt in flight per account write channel; same key for every attempt of an intent. |
| `attempt.abandoned` | `intentId, attemptNo, reason: notSent{cause} \| confirmedAbsent{proof}` | Only from proof-of-not-sent or a witness `confirmedAbsent`. |
| `receipt.recorded` | `intentId, attemptNo, receipt: ReceiptEnvelope` — `accepted{providerRef, provenance original\|recovered}` \| `rejected{code, requestId, raw}` \| `unknown{cause: timeout\|disconnect\|noResponse\|processRestart\|parseFailure}` | Provenance is meaningful only on `accepted`. A response with embedded fills yields receipt + separate `observation.recorded` in the **same append frame**. `parseFailure` = a response was received after send evidence but could not be decoded. |
| `recovery.resolved` | `intentId, attemptNo, result: found{receiptEntryId, observationEntryId} \| confirmedAbsent{…} \| ambiguous{…} \| stillUnknown{…} \| manual{resolution: abandon\|continue\|awaitingReview, principal}` | `found` refers to a `receipt.recorded{provenance: recovered}` and an `observation.recorded` appended in the **same frame**; folds read only those two kinds. Manual resolution is a review-flow action distinct from `DecisionAction`. |
| `observation.recorded` | `source, asOf, providerRef?, observation: ObservationEnvelope` | Upstream facts only. |
| `reversal.appended` | `target: entryId, consumer, reasonTree` | Legal only while the target has not been consumed by Execution (`attempt.started` absent). After that, a consumer proposes a compensation intent instead; a late reversal append fails with `ReversalTargetAlreadyExecuted`. |

**Folds** (pure, position-ordered):

- `intentOutcome(intentId)`: effective authorization = latest `approve|reject` by position; `withdraw` before `attempt.started` → withdrawn, after → an audit entry only, execution outcome unchanged. Status ∈ `proposed | authorized | withdrawn | rejected | expired | attempting | accepted | rejectedByProvider | unknown | awaitingReview | filled | partiallyFilled | cancelled | abandoned`.
- `proposalOutcome(proposalId)`: over member intents, so a UI renders one click and partial approval is expressible. A proposal provides **no provider-level atomicity**; multi-order atomicity lives inside one operation payload or a declared batch capability.
- `orderProjection(providerRef)`: current state = observation with the latest `asOf`; identical duplicates are idempotent; late lower-`asOf` observations never regress state; equal `asOf` with different content → keep later position and flag `observationConflict`.

**Serialisation.** Intents append immediately (no queueing at ingress). The Execution consumer processes authorized intents in position order with one provider call in flight per account write channel; a second intent waits, it is never "lost". Expected-position CAS conflicts are writer conflicts (migration/tooling), not intent outcomes.

**Config binding.** `configRevision` is the per-account value defined in D5 (`sha256(canonical {runtimeConfigDigest, accountConfigDigest, projectionVersion})`). It is snapshotted on `intent.proposed` and again on `attempt.started`; an attempt runs under its own snapshot; a config replacement during an in-flight attempt affects the next attempt only.

**Authorization policy replaces `allowAiTrading`.** Per account `AuthorizationPolicy{selfApprove: PrincipalKind[], agentDecisionAuthority: binding | recommendation, limits}`; `selfApprove` may not contain `system`. Human one-shot (UI form) = `intent.proposed` + `authorization.decided(approve, human)` in one frame, only if `selfApprove` includes `human`. A policy approval is recorded with principal `policy:<id>`.

**Crash matrix (restart outcome).** No intent → nothing. Intent without decision → still `proposed`, visible. Decision without attempt → Execution resumes. `attempt.started` without receipt → on startup the Reconciliation consumer first appends `receipt.recorded{unknown{cause: 'processRestart'}}`, then runs the witness; outcome per D3 laws; never a blind retry. Response received but receipt not durable → treated exactly as "attempt without receipt". Receipt without observation → observation polling. Observation → fold. Torn tail → D5.

**Walkthrough results** (must hold in the model tests): (A) attempt, crash → `receipt.recorded{unknown{processRestart}}`, witness `stillUnknown` until budget → `awaitingReview`, UI shows "unknown outcome — review", bridge emits `review.unknownOutcome`. (B) accepted + observations t1(0), t3(100), t2(40) late, t3 duplicate → `filled 100`, no regression, no duplicate effect. (C) rejected → terminal `rejectedByProvider`; compensation intent accepted; a risk-rule reversal appended after its attempt is refused with `ReversalTargetAlreadyExecuted`.

## D5 Persistence: contract first, files behind one seam

**Contract (normative).** Per account: append-only; total order assigned by one writer; `append(entries, expectedPosition)` returns `Appended{position} | Conflict{expected, actual} | DurabilityFailure{result}`; **a multi-entry append is one frame** (atomic, no separate batch marker); durability completes before exposure; a corrupt or uncommitted record is observable, never silently absent; replay follows persisted order; unknown kinds are preserved and reported unconsumed.

**Seam.** `AppendStore` with `open() → Open | TornTail{quarantined} | Corrupt{at}`, `append`, `replay(fromPosition)`, `head()`. Storage engine is an implementation detail behind it.

**Chosen engine: files.** `data/trading/<accountId>/ledger/{head.json, segments/segment-NNNNNN.jsonl, quarantine/}`. Frame = one line per append: `<64-hex sha256><space><canonical JSON body>\n`; the digest is SHA-256 of the canonical JSON body bytes only (the body contains `prevHash` and `position`); prefix, space and LF are excluded from the digest; no byte-length prefix. Positions are `EntryPosition` (≥1) for entries and `HeadPosition` (≥0) for the head. Durability sequence: segment `fsync` → `head.json` tmp+rename → directory `fsync`; the result is a named variant `Durable | DurableWeak{platform, reason}` (Windows directory durability is untested and must not be reported as `Durable`). Recovery: contiguous valid prefix up to `head.position`; valid frames beyond head are `UncommittedTail` and quarantined with a structured report; interior corruption fails `open()` structurally; a known kind whose payload fails its parser is `Corrupt`, not `Unknown`. Lock: a boot-time `mkdir` lock with owner metadata reclaimed by the sole owner at boot; its only named second writer is migration tooling; heartbeat/takeover semantics are not inherited.

**SQLite rejected for now**, with a written criterion: adopt SQLite-behind-the-seam if a driver exists that requires no native build under Node 22 (prod/Docker), Electron 39's Node ABI, and pinned Bun 1.4.0 across the five launchers and three platforms. Until then it would be UTA's first binary-dependency axis.

**Views.** Materialized projections and consumer cursors are separate files with `sourcePosition/sourceHash`; rebuildable; their failure never touches the ledger.

**Migration 0044 (registry).** Freeze `commit.json` as a **read-only archive** (`data/trading/<id>/legacy/commit.json` + digest); start a fresh ledger; serve legacy history through a read-only history projection tagged `source: legacy-archive` so UI history stays continuous. **No conversion into ledger entries**; imported records never carry authority and never authorize placement. Per-account completion marker; ephemeral accounts keep the existing wipe rule as an explicit exception. Legacy `crypto.guards`/`securities.guards` sections are removed with backup by the same migration.

**First-observation gate.** After every start, an account is `writable: blocked{FirstObservationRequired}` until its first successful upstream observation pass completes (this also closes the legacy pending-order gap).

**Config.** `accounts.json` remains Alice-owned and sealed (UTA reads). Runtime config (`data/config/uta-runtime.json`: authorization policies, rule sets, projection activation) is **Alice-written** through the Alice config routes and **UTA-read with hot reload**; it is one whole-replace atomic file with a strict schema. `configRevision` is never persisted; it is derived at two levels: `runtimeConfigDigest = sha256(canonical semantic object excluding updatedAt)`; per account `ConfigRevision = sha256(canonical {runtimeConfigDigest, accountConfigDigest, projectionVersion})` with `accountConfigDigest = sha256(canonical non-secret account row)`. The restart-flag protocol is unchanged.

## D6 Replacement envelope: atomic in-repo migration, no HTTP facade

**Preserved exactly.** Process contract (`OPENALICE_UTA_PORT`, loopback bind, `GET /__uta/health {ok, startedAt, utas}`, SIGTERM/SIGINT, `data/control/restart-uta.flag`, `OPENALICE_HOME`/`OPENALICE_APP_HOME` split, Node `dist/uta.js` entry, Bun `--internal-role uta`), sealed `accounts.json` reading, Broker Pack release layout, ledger history (archive projection), `alice-uta` command names, `aliceId` instrument identity, decimal-string wire discipline, mode semantics (`lite | readonly | pro`).

**Route accounting (49 → 41).** Of the 49 old registrations: 31 are replaced 1:1 by a `/v2` route; 16 (the staging/approval/one-shot and simulator mutation family) merge into 4 `/v2` routes (`POST /v2/accounts/:id/intents`, `POST /v2/accounts/:id/intents/one-shot`, `POST /v2/intents/:id/decisions`, `POST /v2/simulator/accounts/:id/actions`); 1 (the legacy quote path) is removed with 410; 1 (`/__uta/health`) is preserved. Five routes are new with no legacy ancestor (`/v2/readiness`, `/v2/events`, `/v2/openapi.json`, `GET /v2/intents/:id`, `GET /v2/accounts/:id/proposals`). 31 + 4 + 5 = 40 `/v2` routes + health = the 41-row `ROUTES` catalogue; the per-registration disposition table is `05-protocol-and-replacement.md` §7.

**Replaced.** All UTA-served routes — the 49 registrations in `services/uta/src/http/**` (39 `/api/trading/*`, 9 `/api/simulator/*`, plus `/__uta/health`, which is preserved rather than moved) — become `/v2/*` published as OpenAPI; SDK, 26 tools, BFF, connector `uta-review`, bars gateway, UI API layer and hand-copied types, demo handlers migrate in the same change set (the consumer-by-consumer checklist is in `05-protocol-and-replacement.md`). The legacy `GET /api/trading/uta/:id/quote/:symbol` is dropped. Alice-owned `/api/trading/config/*` routes (`src/webui/routes/trading-config.ts`) are **not** UTA routes: they stay on Alice, keep their paths, and gain `GET/PUT /api/trading/config/runtime` for `uta-runtime.json`.

**Why no facade.** Every consumer is in this monorepo and ships together. D7 makes any out-of-repo caller impossible by construction (bearer token), so a facade would serve nobody. Unknown legacy paths return `410 {code: LegacyRouteRemoved, replacement}`.

**Falsifier.** Deployment evidence of a staggered or external caller before D7 lands; then a full one-release adapter is required (a read-only facade is insufficient because old surfaces include writes).

## D7 Security and principals

- Guardian generates one cryptographically random bearer per Guardian run (overriding any supplied value), injects it into Alice and UTA (`OPENALICE_UTA_TOKEN`), keeps it stable across UTA respawns. A UTA started without it fails closed. Every request carries `Authorization: Bearer`; otherwise 401. `/__uta/health`, `/v2/readiness` and `/v2/openapi.json` are bearer-only (no business Principal); every other route requires bearer + Principal.
- `Principal` is an Alice-authenticated, **server-stamped** assertion header, never an arbitrary caller header: browser session → `human{sessionId}`, CLI/MCP `x-openalice-run/session` → `agent{workspaceId, resumeId, runId}`, connector → `connector{connectorId, externalUserId?}`, scheduler → `schedule{workspaceId, issueId}`, policy → `policy{policyId}`, Alice internals → `system`. UTA records the principal on intents and decisions.
- Non-loopback `OPENALICE_UTA_URL` is rejected while bearer mode is the only transport; a remote UTA needs a separately specified authenticated transport and never receives the local token.
- `accountId` and every resource id resolve through the registry before any filesystem or provider use; no path is derived from raw input. `intentId`, `proposalId`, `decisionId` are caller-generated stable identities; UTA validates and records them, never allocates them.
- BFF gates are generated from the published OpenAPI: `x-uta-channel ∈ {read-only, read-write}` only; lifecycle/simulator classification lives in `x-uta-kind`, not in the channel.
- **Status and error mapping (normative).** `POST /v2/accounts/:id/intents` new = 201, duplicate = 200; body `accountId` ≠ path `accountId` = 409 `ScopeMismatch` (the body stays self-describing because it is hashed into `intentHash`; `DecisionConflict` remains the code for a decision `scopeHash` mismatch); `POST /v2/intents/:id/decisions` new = 202, same `decisionId` = 200; `CapabilityUnsupported` = 422 only; `ProviderUnknown`, `ServiceDraining`, `ProviderTransportFailure` = 503; `DurabilityFailure` surfaces as 503 `ReadinessUnavailable` with structured diagnostics in readiness (`ReadinessUnavailable` is an HTTP-level code only); `InternalError` = 500 with a safe message; `Timeout` = 504 only for server-side deadline overrun (provider-backed reads/commands and `/v2/events`), never for readiness, health, OpenAPI or ledger-local reads. `ProviderUnknown` has exactly one meaning — the placement outcome is unknown after send evidence (A1's unknown receipt) — it is never "provider not registered" (`AccountNotFound`) or "provider unreachable before send" (`ProviderTransportFailure`). Decision failures are `ErrorEnvelope` non-2xx responses, never a `rejected` DTO. Config-invalid errors are `ConfigError{path, code, message}[]`, never `string[]`.

## D8 Event stream: the ledger is the stream

- No outbox, no second durable event store, no durable health journal. Per account, cursor = ledger `position`.
- `GET /v2/events?cursors=<accountId:position,...>&wait=<ms>` is a read-only **fan-in**: ledger entries per account after each cursor plus one current readiness snapshot; every item is `{source, accountId, position, entry: LedgerEntryWire | UnknownLedgerEntryWire}` (never a bare provider envelope; correlation lives inside the entry); response carries the next cursor map. Long-poll only; `wait` ≤ 25000 ms; reaching the wait deadline returns 200 with empty `items` and unchanged `nextCursors` (`Timeout` only on server overrun); omitted cursor = from head; `limit` default 500, max 2000. No SSE in this release (the tightest human cadence today is 3 s).
- UI live stores consume it through a BFF relay and retire the seven independent polling intervals; the Issue bridge and connector consume the same endpoint with their own persisted cursors, which are allowed to lag.
- `data/event-log/events.jsonl` is not reused for anything durable.

## D9 UTA consumes AI through Alice Issues: pull direction, typed decisions

- UTA never writes Alice Issue files. It appends `work.requested` entries (kinds: `review.unknownOutcome`, `review.ambiguousRecovery`, `review.ruleRejection`, `reconcile.discrepancy`, `intent.awaitingAuthorization`, `watch.triggered`, `news.received`) carrying: `requestId`, `accountId`, `kind` (discriminant), causal `{entryId, position, why}`, `intentId?/idempotencyKey?`, `authority: binding | recommendation` (derived from the account policy at emit time), `admissibleDecisions` and the `scopeHash/intentHash` they must bind to (non-empty for `review.*`, `reconcile.*`, `intent.awaitingAuthorization`; may be empty for `watch.triggered`/`news.received`), `freshness{asOf, …}` of the last relevant observation (nested; no top-level `asOf`), `providerId/projectionVersion` and the operation payload in decimal strings, canonical Markdown `what`, `expiresAt`. `review.ruleRejection` additionally carries `{consumer, ruleId, configRevision, reasonTree}`. These literals are the protocol/ledger tokens; hyphenated forms are UI copy only. Positions/balances/health are **not** embedded (addressability, not completeness).
- Alice runs a **UTA desk bridge** (pattern: connector action-bridge + `connectorDesk`): per account a configured desk Workspace and one long-lived **desk Issue** as the timeline; the bridge consumes `/v2/events` with a persisted cursor (temp+rename, launcher-owned) and: (a) for `authority: binding` review/reconcile kinds creates a **per-request Issue** with a charset-safe, length-capped derived id, reading its own link store first and treating `createIssue` `conflict` as success-by-reference; (b) for notifications and recommendations appends a desk comment with a deterministic comment id. Cursor advances only after the Issue file is durable and re-readable.
- Decisions return only through a typed endpoint `POST /api/uta/intents/:intentId/decisions` on Alice with `DecisionRequest{decisionId, action, scopeHash}`; Alice stamps the principal (`agent{resumeId}` or `human`), resolves the scope, and forwards `AuthorizationDecisionRequest{decisionId, action, scope, scopeHash}` to UTA. Whether an agent decision is binding is the account policy's call; otherwise it is recorded as a recommendation and the intent stays pending a human. Issue `done` never means executed; outcome comments follow ledger entries. The bridge long-polls with `wait = min(pollIntervalMs, 25000)`.
- Agents may still create intents through the tools; those carry `origin{issueId, resumeId, runId}`. `Origin` also has `watch{sourceEntryId, sourcePosition, cursor?, eventId?}` and `news{sourceEntryId, sourcePosition, cursor?, eventId?}` variants for intents derived from `watch.triggered`/`news.received`.

## D10 Rules and guards

- One strict, kind-aware parser at three points: config write, startup, and before provider construction. Rule options are finite numeric domains and non-empty validated string sets; unknown kinds are errors. Invalid account config → account `config-invalid`, `writable: blocked{ConfigInvalid}`, structured `ConfigError[]` surfaced in readiness; invalid global config blocks startup. Never warn-and-skip.
- Rule sets are consumer-local pure functions over parsed inputs; results `allow | reject(ReasonTree) | pending(Reason)`; `ReasonTree` nests child failures with constructors preserved. Unknown intent kind at a consumer → `reject(UnknownKind)`.

## D11 Identity, numbers, time

- Branded types for `AccountId, ProviderId, ProjectionVersion, IntentId, ProposalId, DecisionId, AttemptNo, EntryId, LedgerPosition, IdempotencyKey, ProviderKey<P>, ProviderOrderRef<P>, AliceId, NativeKey, Instant, AsOf, Cursor, ConfigRevision, RequestId`. One parser/constructor per boundary (HTTP, file/ledger, env, Pack ABI); brands are erased at runtime, so nothing enters the domain unparsed.
- Financial values cross every boundary as canonical decimal strings; non-finite values, provider sentinels, invalid scale, and numeric JSON for money are rejected at the parser. In-process `Decimal` never crosses the Pack ABI. `Money` carries currency; `Qty` carries unit context.
- `occurredAt` (upstream/business time) and `recordedAt` (append time) are always separate; replay order is `position`, never wall-clock. `Cursor` is provider/channel-scoped and opaque.
- FX is a read-only observation with `asOf` and a freshness variant; a default or 1:1 fallback is display-only and cannot feed a rule or authorization.

## D12 Operations

- `/__uta/health` unchanged (process liveness). `/v2/readiness` per account: `{process, transport, readable, writable, capabilities, observationFreshness, config}`; it is a local snapshot answered 200 or 503, never 504. `transport` = `connected | connecting | disconnected{since, lastError?}`; `readable` = `ok{headPosition} | unavailable{open: TornTail | Corrupt | DurabilityFailure, at?}`; `writable` = `ok | blocked{reason, secondary[]}` over the finite tagged set `Draining | ConfigInvalid | LedgerUnreadable | TransportDisconnected | FirstObservationRequired | QuarantineCapacityExceeded | NoPlacementRecovery`. When several apply, `reason` is the highest by the normative precedence `Draining > ConfigInvalid > LedgerUnreadable > TransportDisconnected > FirstObservationRequired > QuarantineCapacityExceeded > NoPlacementRecovery` and `secondary` lists the rest in that order (exported as `WRITABLE_BLOCK_PRECEDENCE`). Readiness is cross-field consistent by construction (`makeAccountReadiness(input, declaredKinds)`: `process: draining ⇒ Draining`, `config: invalid ⇒ ConfigInvalid`, `readable: unavailable ⇒ LedgerUnreadable` with identical `{open, at}`, `transport: disconnected ⇒ TransportDisconnected`; `capabilities` has exactly one row per declared kind). Rejected alternative: independent boolean dimensions with a free-text reason — it permitted `draining` + writable and gave operators no ordering when blockers coexist. `maxProviderConnections` has a floor of 100 (`MIN_PROVIDER_CONNECTIONS`).
- Lifecycle `running → draining → stopped`: on SIGTERM the transition to `draining` is atomic — non-durable queued commands are rejected with `ServiceDraining`, durable intents/decisions remain for restart, pollers/streams stop. While draining, the effective attempt deadline = `min(provider-declared deadline, drainBudget)` with `drainBudget = launcherGrace − 1000 ms`; an attempt unresolved at the cap exits with the process and its durable `attempt.started` is classified as `unknown{processRestart}` on restart (D4). Then scopes close, cursors flush, and `AppendStore.close()` waits only for the now-deterministic empty queue. On force-kill, restart runs the same classification and the witness before anything becomes writable.
- Crash restart policy: **unchanged** — Guardian does not auto-respawn; flag/operator restart remains. Recorded as an explicit non-change with the trade-off named.

## D13 Scope

In: everything above; Mock as a manual projection; simulator on `/v2/simulator`; FX as a read-only channel; snapshots retained as observation-derived store with their index invariants fixed. Out: Futu/OpenD; Alice-wide Issue redesign; portfolio strategy; new venues beyond declaring existing ones honestly; automatic crash respawn.

## Residual rulings (post-review, binding)

Small decisions the traceability matrix found undecided. Each is a single ruling; documents cite `R<n>`.

- **R1 Duplicate account ids.** `accounts.json` is Alice-owned; the Alice config route rejects a duplicate `accountId` with 409 `DuplicateAccountId`. If UTA nevertheless loads a file with duplicates, every row sharing the id is `config-invalid` with `ConfigError{code: DuplicateAccountId}` and `writable: blocked{ConfigInvalid}`; startup is not blocked and no row is silently chosen.
- **R2 Mode precedence.** Effective mode = `OPENALICE_UTA_MODE` env if set, else `uta-runtime.json.mode`, else `lite`; resolved once at startup, never hot-reloaded (a mode change requires the restart flag). Readiness reports `process.mode{value, source: env | config | default}`.
- **R3 Ephemeral accounts.** An ephemeral account must bind the Mock projection; any other projection makes the row `config-invalid{EphemeralRequiresMock}`. The existing wipe-on-start rule for ephemeral accounts is retained (D5).
- **R4 No echo fallbacks.** Contract search and every other read kind is a declared capability row (`contract.search`, `account.balances`, …). A provider that cannot serve a read kind declares it `unsupported` and the route answers 422 `CapabilityUnsupported`; a projection may never synthesize a result from the request (the legacy Alpaca "echo the query" fallback is forbidden because it fabricates an observation). Catalog availability is therefore visible in readiness `capabilities`, separately from transport health.
- **R5 Unresolved venue/routing.** If the effective capability tuple cannot be resolved from declaration × account config (unknown venue, unknown mode), the account is `config-invalid{UnresolvedVenue}`; provider-backed reads answer 503 `ReadinessUnavailable` with the `ConfigError[]`, ledger-local reads keep working.
- **R6 FX.** FX rates are observations on the `fx.rates` read kind (provider-declared or the UTA FX service, each with its own `source`); each rate is `{base, quote, rate: DecimalString, asOf, source}` and its freshness variant (`fresh | stale{age} | missing`) is computed against runtime config `fx.maxAge: Duration`. There are no separate warning levels: the freshness variant is the warning. Only `fresh` rates may feed rules or authorization; `stale`/`missing` are display-only with the variant shown (D11). The parser rejects non-decimal, non-positive or pair-less rates.
- **R7 Contract identity.** `aliceId` is the sole cross-provider contract identity; provider-native keys (`NativeKey`) are attached per provider and never used as a cross-provider join. Search dedups on `aliceId`; `assetClass` is a search filter and a declaration-level capability qualifier, not an identity component; normalization of symbols happens in each projection's translation, never in Alice.

---

## Type vocabulary and module layout (binding for authors)

Package `plans/uta-refactor/spec/types/` (private, `tsc --noEmit` only, no Alice `src/**` imports, `effect` as a dev dependency for the boundary fixtures only; negative fixtures use a `// Expected TSxxxx` header, never `@ts-expect-error`):

| Module | Owns |
|---|---|
| `ids.ts` | brands + parsers listed in D11 (plus `ScopeHash`, `IntentHash`, `Sha256`, `SourceDigest`, `SegmentName`, `EntryPosition`, `HeadPosition`, `Source`, `ProviderErrorCode`, `ByteOffset`) |
| `result.ts` | `Result<T, E>` used by every parser and by `Translation.encodeOperation` |
| `money.ts` | `DecimalString`, `Money`, `Qty`, parsers |
| `time.ts` | `Instant`, `AsOf`, `Duration`, freshness variants |
| `principal.ts` | `Principal`, `Origin` |
| `provider/declaration.ts` | `ProviderDeclaration`, capability table, `IdempotencyContract`, `ReadByKeyContract`, `ObservationCursorContract`, `CapabilityStatus` |
| `provider/indexed.ts` | `Operation<P>`, `Receipt<P>`, `Observation<P>`, `ProviderKey<P>`, `ProviderOrderRef<P>`, role-indexed envelopes (`OperationEnvelope`, `KeyEnvelope`, `ReceiptEnvelope`, `ObservationEnvelope`, `ConfigEnvelope`) |
| `provider/recovery.ts` | `PlacementRecovery<P>`, `RecoveryResult<P>`, `nextAction` law |
| `provider/abi.ts` | `PackModule<D>` (mapped supported-kind handlers), transport plugin interfaces, `TransportRequest`/`TransportResult`, `StreamItem` (`event | gap | ended`), `LoaderValidationResult` |
| `channel.ts` | `ReadOnly<T>`, `ReadWrite<E, T>`, capability = type + handler pairing, read-only unconstructibility |
| `ledger/entries.ts` | entry kinds from D4, `LedgerEntry`, `ReasonTree` |
| `ledger/store.ts` | `AppendStore` (incl. `close(): Promise<CloseResult>`), `AppendResult`, `OpenResult`, durability variants |
| `ledger/fold.ts` | `intentOutcome`, `proposalOutcome`, `orderProjection` signatures and status unions |
| `consumer.ts` | `Consumer`, subscription declaration, `RuleResult` |
| `policy.ts` | `AuthorizationPolicy`, rule config parsers |
| `readiness.ts` | `AccountReadiness`, `WritableBlockedReason` (seven kinds, `WRITABLE_BLOCK_PRECEDENCE`), `ConfigError`, `makeAccountReadiness`, `readinessResponseSchema`, `MIN_PROVIDER_CONNECTIONS` |
| `wire/v2.ts` | barrel: error codes and re-exports of `wire/projection.ts` + `wire/routes.ts` |
| `wire/projection.ts` | shared wire wrappers: `readProjectionResponseSchema(valueSchema)`, `LedgerEntryWire`/`UnknownLedgerEntryWire`, events/ledger query+response schemas, `CommandResponse` |
| `wire/routes.ts` | one named request/response schema pair per route plus the `ROUTES` catalogue (41 rows: 40 `/v2` + `/__uta/health`) with method, path, `x-uta-channel`, `x-uta-kind`, principal requirement, params schema and status set |
| `issues.ts` | `WorkRequested`, `UtaDeskSettings`/`DeskBridgeLinkStore` parsers, `DecisionRequest` (Alice) and `AuthorizationDecisionRequest` (UTA) |
| `fixtures/` | positive compile fixtures and `// Expected TSxxxx` negatives for every static guarantee named in D2/D3/D4 |

Naming is fixed here; authors extend but do not rename.
