# W1 Requirements Traceability Matrix (skeleton)

One row per axiom / constraint / invariant / open question / surface / store / nonfunctional item. All `Status` values are `open`; `Resolved by` and `Verification scenario` are filled by Main later.

| ID | Source | Requirement (≤20 words) | Category (axiom/constraint/invariant/open-question/surface/store/nonfunctional) | Status | Resolved by (spec section / type) | Verification scenario |
|---|---|---|---|---|---|---|
| AX-1 | 00-core-contract §1 | Upstream alone is truth; downstream records intents, never fabricates upstream facts | axiom | open | | |
| AX-2 | 00-core-contract §1 | Abstract from runtime-exchanged IO semantics, not data shapes or providers | axiom | open | | |
| AX-3 | 00-core-contract §1 | Channels are type-constructor specializations; each capability is type-plus-handler pair | axiom | open | | |
| AX-4 | 00-core-contract §1 | Read-only vs read-write separated in types; writes via single constrained entry | axiom | open | | |
| AX-5 | 00-core-contract §1 | Ledger is the T of trading IO; append-only entries, replayable explanations | axiom | open | | |
| AX-6 | 00-core-contract §1 | Entries routed by kind; rule rejection appends structured reversal entry | axiom | open | | |
| AX-7 | 00-core-contract §1 | Interfaces defined downstream; partial capability allowed, never faked | axiom | open | | |
| AX-8 | 00-core-contract §1 | Persistence IO determines ownership; config persistence separate from ledger | axiom | open | | |
| DC-0-1 | 01-detail-constraints §0 | Ledger and write constraints never auto-extend to heartbeat, config, plain reads | constraint | open | | |
| DC-1.1-1 | 01-detail-constraints §1.1 | Abstraction states IO direction, external effect, order, time, confirmation semantics | constraint | open | | |
| DC-1.1-2 | 01-detail-constraints §1.1 | Payload shapes are not public-abstraction prerequisites; parsed on demand | constraint | open | | |
| DC-1.1-3 | 01-detail-constraints §1.1 | Consumers are not part of IO definition; zero or many allowed | constraint | open | | |
| DC-1.1-4 | 01-detail-constraints §1.1 | Channels are type specializations by IO direction, not preset account objects | constraint | open | | |
| DC-1.1-5 | 01-detail-constraints §1.1 | Direction and side-effect nature are type boundaries, not runtime flags | constraint | open | | |
| DC-1.2-1 | 01-detail-constraints §1.2 | Read-only IO only fetches observations; initiating reads is still action | constraint | open | | |
| DC-1.2-2 | 01-detail-constraints §1.2 | Read-write IO casts intents; return value is receipt only, never fact | constraint | open | | |
| DC-1.2-3 | 01-detail-constraints §1.2 | Read-write kind set stays open; no closed central action enum | constraint | open | | |
| DC-1.2-4 | 01-detail-constraints §1.2 | External results flow back via observation or ledger paths; no disguising | constraint | open | | |
| DC-1.3-1 | 01-detail-constraints §1.3 | Heartbeat, health, config, snapshots never stuffed into trading ledger | constraint | open | | |
| DC-1.3-2 | 01-detail-constraints §1.3 | No ledger, routing, reversal, replay semantics imposed on plain read values | constraint | open | | |
| DC-1.3-3 | 01-detail-constraints §1.3 | No transformer or wrapper terminology substituting agreed type boundaries | constraint | open | | |
| DC-1.3-4 | 01-detail-constraints §1.3 | No second upstream-downstream axis; single truth and observation direction | constraint | open | | |
| DC-1.3-5 | 01-detail-constraints §1.3 | Providers never pre-projected into IBKR, HTTP, TCP, SDK, MQL object model | constraint | open | | |
| DC-1.3-6 | 01-detail-constraints §1.3 | Unimplemented never faked as empty success, list, method, swallowed error | constraint | open | | |
| DC-2.1-1 | 01-detail-constraints §2.1 | Ledger is trading IO's T: intents, receipts, unknowns, observations, explanations | constraint | open | | |
| DC-2.1-2 | 01-detail-constraints §2.1 | Ledger is append-only; corrections always append new entries | constraint | open | | |
| DC-2.1-3 | 01-detail-constraints §2.1 | Ledger records occurred IO only; local estimates never upstream facts | constraint | open | | |
| DC-2.1-4 | 01-detail-constraints §2.1 | Every entry carries why; no empty, generic, or log-only substitutes | constraint | open | | |
| DC-2.2-1 | 01-detail-constraints §2.2 | Entry keeps upstream and local-append times distinct; no now-masquerading | constraint | open | | |
| DC-2.2-2 | 01-detail-constraints §2.2 | Entry source explicit; original sources preserved across merges | constraint | open | | |
| DC-2.2-3 | 01-detail-constraints §2.2 | Entry kind lives in extensible namespace; no closed action set | constraint | open | | |
| DC-2.2-4 | 01-detail-constraints §2.2 | Retries reuse one idempotency key; no new keys masking same placement | constraint | open | | |
| DC-2.2-5 | 01-detail-constraints §2.2 | Why persists with entry, never in logs, exceptions, memory only | constraint | open | | |
| DC-2.2-6 | 01-detail-constraints §2.2 | Opaque payload preserved untampered; core never invents provider fields | constraint | open | | |
| DC-2.2-7 | 01-detail-constraints §2.2 | Entries need stable identity for reversal, cursors, replay, reconcile | constraint | open | | |
| DC-2.2-8 | 01-detail-constraints §2.2 | Intent, attempt, receipt, observation distinct; reconcile ignores receipts | constraint | open | | |
| DC-2.2-9 | 01-detail-constraints §2.2 | Entry shape is type sketch only; serialization field names not prescribed | constraint | open | | |
| DC-2.3-1 | 01-detail-constraints §2.3 | Raw responses or materialized state never facts without sourced observation | constraint | open | | |
| DC-2.3-2 | 01-detail-constraints §2.3 | Payload never substitutes kind, source, why, or idempotency key | constraint | open | | |
| DC-2.3-3 | 01-detail-constraints §2.3 | No defaults or sentinels fabricating success; unknown stays explicit | constraint | open | | |
| DC-2.3-4 | 01-detail-constraints §2.3 | Append order, event time, idempotency identity are three distinct constraints | constraint | open | | |
| DC-2.3-5 | 01-detail-constraints §2.3 | Retries link original entry via key; new intent only when genuinely new | constraint | open | | |
| DC-2.4-1 | 01-detail-constraints §2.4 | Reversal is newly appended entry; originals never edited or hidden | constraint | open | | |
| DC-2.4-2 | 01-detail-constraints §2.4 | Reversal references target identity with source, time, kind, why | constraint | open | | |
| DC-2.4-3 | 01-detail-constraints §2.4 | Partial reversals state scope; compensations are new intent entries | constraint | open | | |
| DC-2.4-4 | 01-detail-constraints §2.4 | Rejections, vetoes, unsupported, corrections keep distinct sources and reasons | constraint | open | | |
| DC-2.4-5 | 01-detail-constraints §2.4 | Reversal only ends local continuation; never claims upstream rollback | constraint | open | | |
| DC-2.5-1 | 01-detail-constraints §2.5 | Replay is pure deterministic repeatable fold; no ambient inputs | constraint | open | | |
| DC-2.5-2 | 01-detail-constraints §2.5 | Materialized views are pure derivations; no second fact writer | constraint | open | | |
| DC-2.5-3 | 01-detail-constraints §2.5 | View persistence failure never mutates entries; re-materialize later | constraint | open | | |
| DC-2.5-4 | 01-detail-constraints §2.5 | Replay follows persistence order with explicit reversal handling | constraint | open | | |
| DC-2.5-5 | 01-detail-constraints §2.5 | Unknown or unconsumed kinds never silently dropped; state stays observable | constraint | open | | |
| DC-2.5-6 | 01-detail-constraints §2.5 | Views never generate upstream facts; follow-ups are explicit intents | constraint | open | | |
| DC-3.1-1 | 01-detail-constraints §3.1 | Read initiation is impure IO; returned T is pure value, never ledger | constraint | open | | |
| DC-3.1-2 | 01-detail-constraints §3.1 | Read-only types express no-upstream-state-change direction | constraint | open | | |
| DC-3.1-3 | 01-detail-constraints §3.1 | Read results carry source time or asOf, never client-receive time | constraint | open | | |
| DC-3.1-4 | 01-detail-constraints §3.1 | Subscriptions and pages carry resumable cursors and terminal end reasons | constraint | open | | |
| DC-3.1-5 | 01-detail-constraints §3.1 | Read resources acquired and released in pairs; release preserves observations | constraint | open | | |
| DC-3.2-1 | 01-detail-constraints §3.2 | Freshness is explicit result info; stale neither masked nor always errors | constraint | open | | |
| DC-3.2-2 | 01-detail-constraints §3.2 | Read failure and stale-value-with-asOf are semantically separated | constraint | open | | |
| DC-3.2-3 | 01-detail-constraints §3.2 | Cached, retried, concurrent reads never imply intent placement | constraint | open | | |
| DC-3.2-4 | 01-detail-constraints §3.2 | Read retries never append reversal or unknown entries unless consumed | constraint | open | | |
| DC-3.2-5 | 01-detail-constraints §3.2 | Market-driven trading stays thin wrapper: subscribe, pure intent, single entry | constraint | open | | |
| DC-3.3-1 | 01-detail-constraints §3.3 | Any program part may consume read-only channels; no write-gate monopoly | constraint | open | | |
| DC-3.3-2 | 01-detail-constraints §3.3 | Open consumption keeps identity, permission, masking, audit requirements intact | constraint | open | | |
| DC-3.3-3 | 01-detail-constraints §3.3 | Consumers share results or rebuildable views, never mutable ownership | constraint | open | | |
| DC-3.3-4 | 01-detail-constraints §3.3 | Consumers choose caching and staleness; no central health verdict | constraint | open | | |
| DC-3.4-1 | 01-detail-constraints §3.4 | Heartbeat follows read-only semantics; value carries no hidden state machine | constraint | open | | |
| DC-3.4-2 | 01-detail-constraints §3.4 | Liveness thresholds belong to consumers, not heartbeat type globals | constraint | open | | |
| DC-3.4-3 | 01-detail-constraints §3.4 | Heartbeat-triggered actions belong to consumer IO, never ledger entries | constraint | open | | |
| DC-3.4-4 | 01-detail-constraints §3.4 | Heartbeat is instantaneous observation; ledger is appendable replayable structure | constraint | open | | |
| DC-4.1-1 | 01-detail-constraints §4.1 | Each read-write channel has one logical intake; no bypass path | constraint | open | | |
| DC-4.1-2 | 01-detail-constraints §4.1 | External mutations serialized per channel within side-effect boundary | constraint | open | | |
| DC-4.1-3 | 01-detail-constraints §4.1 | Single entry unifies rules, capability, idempotency, receipts, unknowns | constraint | open | | |
| DC-4.1-4 | 01-detail-constraints §4.1 | Cross-cutting layer order fixed by root; providers supply terminal handler | constraint | open | | |
| DC-4.2-1 | 01-detail-constraints §4.2 | Intent contract fixes boundaries only; new kinds avoid central changes | constraint | open | | |
| DC-4.2-2 | 01-detail-constraints §4.2 | Every intent carries stable key across retries, confirms, replays | constraint | open | | |
| DC-4.2-3 | 01-detail-constraints §4.2 | Keys identify repeats only; never prove acceptance or fill | constraint | open | | |
| DC-4.2-4 | 01-detail-constraints §4.2 | Unknown kinds rejected by default with structured reason and reversal | constraint | open | | |
| DC-4.3-1 | 01-detail-constraints §4.3 | Write actions return receipts only, never positions or balances | constraint | open | | |
| DC-4.3-2 | 01-detail-constraints §4.3 | Receipts never carry fill, price, balance facts; co-returned data are observations | constraint | open | | |
| DC-4.3-3 | 01-detail-constraints §4.3 | Outcomes flow back via observations or ledger; awaited fields prove nothing | constraint | open | | |
| DC-4.4-1 | 01-detail-constraints §4.4 | Timeout, disconnect, lost response enter explicit unknown semantics | constraint | open | | |
| DC-4.4-2 | 01-detail-constraints §4.4 | Unknown states recorded as entries; never deleted or overwritten | constraint | open | | |
| DC-4.4-3 | 01-detail-constraints §4.4 | Reconcile path: unknown entry, query upstream, append observation or intent | constraint | open | | |
| DC-4.4-4 | 01-detail-constraints §4.4 | Retry permission from receipt variant plus capability, never generic backoff | constraint | open | | |
| DC-5.1-1 | 01-detail-constraints §5.1 | Consumers declare subscribed kinds as checkable contract | constraint | open | | |
| DC-5.1-2 | 01-detail-constraints §5.1 | Routing by kind only; namespace open without disturbing subscribers | constraint | open | | |
| DC-5.1-3 | 01-detail-constraints §5.1 | Kinds shared; each consumer keeps own cursor, rules, failures | constraint | open | | |
| DC-5.1-4 | 01-detail-constraints §5.1 | Execution is one consumer among others, owning no entries | constraint | open | | |
| DC-5.2-1 | 01-detail-constraints §5.2 | Rule sets are consumer-local with identifiable inputs and reasons | constraint | open | | |
| DC-5.2-2 | 01-detail-constraints §5.2 | Rule evaluation is pure: same entry, context, time, same result | constraint | open | | |
| DC-5.2-3 | 01-detail-constraints §5.2 | External state enters rules only as explicit asOf observations | constraint | open | | |
| DC-5.2-4 | 01-detail-constraints §5.2 | Rule results structured: pass, reject with reason, explicit pending | constraint | open | | |
| DC-5.3-1 | 01-detail-constraints §5.3 | Rejection appends reversal with consumer, rule, time, reference, why | constraint | open | | |
| DC-5.3-2 | 01-detail-constraints §5.3 | Rejection stops local side effects only; never global fact | constraint | open | | |
| DC-5.3-3 | 01-detail-constraints §5.3 | Reversal reasons serve human audit and machine routing; no bare strings | constraint | open | | |
| DC-5.4-1 | 01-detail-constraints §5.4 | Unsubscribed entries remain valid, retained, replayable, queryable | constraint | open | | |
| DC-5.4-2 | 01-detail-constraints §5.4 | Unavailable consumers or unknown kinds preserve entries and unconsumed state | constraint | open | | |
| DC-5.4-3 | 01-detail-constraints §5.4 | Unhandleable kinds report Unsupported; never fake success entries | constraint | open | | |
| DC-5.4-4 | 01-detail-constraints §5.4 | Consumer or rule failure never rolls back appended entries | constraint | open | | |
| DC-6.1-1 | 01-detail-constraints §6.1 | Downstream declares needed IO first; providers declare fillable channels | constraint | open | | |
| DC-6.1-2 | 01-detail-constraints §6.1 | Providers never implement a pre-guessed complete interface | constraint | open | | |
| DC-6.1-3 | 01-detail-constraints §6.1 | Capabilities observable per channel and kind; absence explicit or Unsupported | constraint | open | | |
| DC-6.1-4 | 01-detail-constraints §6.1 | Transport and session mechanisms stay out of public abstraction | constraint | open | | |
| DC-6.1-5 | 01-detail-constraints §6.1 | Onboarding splits into declaration, translation, transport execution independently | constraint | open | | |
| DC-6.1-6 | 01-detail-constraints §6.1 | Idempotent-placement and keyed-readback support are mandatory declared capabilities | constraint | open | | |
| DC-6.2-1 | 01-detail-constraints §6.2 | Raw provider data retained at translation boundary for re-parse, traceability | constraint | open | | |
| DC-6.2-2 | 01-detail-constraints §6.2 | Translation is provider- and channel-local; no forced shared fields | constraint | open | | |
| DC-6.2-3 | 01-detail-constraints §6.2 | Partial translation keeps boundary; no silent drops or defaults | constraint | open | | |
| DC-6.2-4 | 01-detail-constraints §6.2 | Unanswerable questions get explicit unsupported; no fake stubs | constraint | open | | |
| DC-6.2-5 | 01-detail-constraints §6.2 | Rejection, transport, parse, unknown errors keep distinct semantics, identities | constraint | open | | |
| DC-7.1-1 | 01-detail-constraints §7.1 | Ownership decided by persisting writer, never in-memory holders | constraint | open | | |
| DC-7.1-2 | 01-detail-constraints §7.1 | Each persisted datum has explicit writer, readers, propagation semantics | constraint | open | | |
| DC-7.1-3 | 01-detail-constraints §7.1 | Config and ledger writers independent; neither writes other's store | constraint | open | | |
| DC-7.2-1 | 01-detail-constraints §7.2 | Config is whole-replacement expected-state persistence with pre-write validation | constraint | open | | |
| DC-7.2-2 | 01-detail-constraints §7.2 | Ledger is append-only persistence with order, replay, routing, reversal | constraint | open | | |
| DC-7.2-3 | 01-detail-constraints §7.2 | Restart and reconnect actions are neither config entries nor ledger facts | constraint | open | | |
| DC-7.2-4 | 01-detail-constraints §7.2 | Health, handles, caches, views stay out of ledger unless genuine entries | constraint | open | | |
| DC-7.3-1 | 01-detail-constraints §7.3 | Config and ledger files each have explicit single-writer boundary | constraint | open | | |
| DC-7.3-2 | 01-detail-constraints §7.3 | Config replacement atomic; readers never see truncated content | constraint | open | | |
| DC-7.3-3 | 01-detail-constraints §7.3 | Appends have recoverable boundaries; corrupt tails never valid or silent | constraint | open | | |
| DC-7.3-4 | 01-detail-constraints §7.3 | Appends serialize order and persist before exposure; failures stay visible | constraint | open | | |
| DC-7.3-5 | 01-detail-constraints §7.3 | Appends carry expected position; conflicts fail as structured results | constraint | open | | |
| DC-7.3-6 | 01-detail-constraints §7.3 | Restart recovers order from ledger and rebuilds views; caches never truth | constraint | open | | |
| DC-7.3-7 | 01-detail-constraints §7.3 | No implicit cross-store transaction between config and ledger | constraint | open | | |
| DC-7.4-1 | 01-detail-constraints §7.4 | Config may reuse Alice boundary, schema, writer, propagate discipline | constraint | open | | |
| DC-7.4-2 | 01-detail-constraints §7.4 | Ledger may reuse Alice single-writer append-only replay-cursor discipline | constraint | open | | |
| DC-7.4-3 | 01-detail-constraints §7.4 | Ledger adds reversal-as-new-entry and declared-kind subscription routing | constraint | open | | |
| DC-7.4-4 | 01-detail-constraints §7.4 | Unconsumed entries retained for future consumption, audit, replay | constraint | open | | |
| DC-7.4-5 | 01-detail-constraints §7.4 | Shared implementation never merges config, heartbeat, ledger ownership | constraint | open | | |
| DC-8.1-1 | 01-detail-constraints §8.1 | Identity-like values use nominal types; no interchangeable raw strings | constraint | open | | |
| DC-8.1-2 | 01-detail-constraints §8.1 | Intent, receipt, observation, rule, reversal, config, heartbeat are distinct types | constraint | open | | |
| DC-8.1-3 | 01-detail-constraints §8.1 | External inputs enter as unknown; only named domain types flow inward | constraint | open | | |
| DC-8.1-4 | 01-detail-constraints §8.1 | Opaque payloads bounded by source and kind; no rule use unparsed | constraint | open | | |
| DC-8.2-1 | 01-detail-constraints §8.2 | Exclusive states use discriminated unions, never boolean combinations | constraint | open | | |
| DC-8.2-2 | 01-detail-constraints §8.2 | Routing, rules, replay, capability, errors exhaust variants with never-assertions | constraint | open | | |
| DC-8.2-3 | 01-detail-constraints §8.2 | No default branches silently absorbing unknown kinds or variants | constraint | open | | |
| DC-8.2-4 | 01-detail-constraints §8.2 | Types prove local composition only; receipt and reconcile boundaries remain | constraint | open | | |
| DC-8.3-1 | 01-detail-constraints §8.3 | No any, Object, or raw maps as domain or cross-boundary types | constraint | open | | |
| DC-8.3-2 | 01-detail-constraints §8.3 | No casts hiding missing capability, mismatch, direction error, gaps | constraint | open | | |
| DC-8.3-3 | 01-detail-constraints §8.3 | Boundary parsing returns structured results or errors; no boolean-only validators | constraint | open | | |
| DC-8.3-4 | 01-detail-constraints §8.3 | Serialization preserves unknown kinds and raw payload traceability | constraint | open | | |
| DC-8.4-1 | 01-detail-constraints §8.4 | IO T comes from exchanged data; direction specializes afterward | constraint | open | | |
| DC-8.4-2 | 01-detail-constraints §8.4 | Ledger types constrain trading channels only; reads never inherit them | constraint | open | | |
| DC-8.4-3 | 01-detail-constraints §8.4 | Open kinds use kind and payload extension, never central interface edits | constraint | open | | |
| DC-8.5-1 | 01-detail-constraints §8.5 | Providers and consumers tested against shared state-machine model | constraint | open | | |
| DC-8.5-2 | 01-detail-constraints §8.5 | Model tests cover Unsupported, rejection, unknown, crash recovery, conflicts | constraint | open | | |
| DC-8.5-3 | 01-detail-constraints §8.5 | Passing model tests never substitutes sandbox observation and ledger evidence | constraint | open | | |
| DC-9-1 | 01-detail-constraints §9 | Diagram arrows are semantic directions, not module or process prescriptions | constraint | open | | |
| DC-9-2 | 01-detail-constraints §9 | Read paths become ledger paths only when output enters single write entry | constraint | open | | |
| DC-9-3 | 01-detail-constraints §9 | Receipt, observation, materialized view stay distinct, non-substitutable | constraint | open | | |
| DC-9-4 | 01-detail-constraints §9 | Config, ledger, runtime state keep separate ownership despite co-persistence | constraint | open | | |
| INV-7.1-1 | 00-overview §7.1 | UTA binds loopback only; port comes from OPENALICE_UTA_PORT | invariant | open | | |
| INV-7.1-2 | 00-overview §7.1 | Single account init failure never blocks others or HTTP surface | invariant | open | | |
| INV-7.1-3 | 00-overview §7.1 | startedAt immutable per process; serves as restart signal | invariant | open | | |
| INV-7.1-4 | 00-overview §7.1 | restart-uta.flag sole cross-process restart signal; no hot reload | invariant | open | | |
| INV-7.1-5 | 00-overview §7.1 | Account ids unique per manager; duplicate add throws | invariant | open | | |
| INV-7.1-6 | 00-overview §7.1 | Health endpoint ok:true means listening only, never readiness | invariant | open | | |
| INV-7.1-7 | 00-overview §7.1 | Connection and request paths concurrency-safe via grace and fast-fail | invariant | open | | |
| INV-7.2-1 | 00-overview §7.2 | Money and quantity cross wire as strings via Decimal export | invariant | open | | |
| INV-7.2-2 | 00-overview §7.2 | aliceId ownership unique; cross-account resolution throws | invariant | open | | |
| INV-7.2-3 | 00-overview §7.2 | push and reject require current pendingHash; missing or conflict is 409 | invariant | open | | |
| INV-7.2-4 | 00-overview §7.2 | Stage errors never persist; commit failure never rolls back staging | invariant | open | | |
| INV-7.2-5 | 00-overview §7.2 | GET idempotent; stage, commit, sync, push, reject, simulate non-idempotent | invariant | open | | |
| INV-7.2-6 | 00-overview §7.2 | external-deposit quasi-idempotent via overwrite; semantics dangerous | invariant | open | | |
| INV-7.2-7 | 00-overview §7.2 | Health gate only on queryAccount path; other reads skip recovery | invariant | open | | |
| INV-7.2-8 | 00-overview §7.2 | Multi-account traversal reads lack transaction or snapshot isolation | invariant | open | | |
| INV-7.3-1 | 00-overview §7.3 | aliceId ownership unique; broker contracts always stamped over | invariant | open | | |
| INV-7.3-2 | 00-overview §7.3 | Absence never equals terminal state; disappearance needs reconfirmation | invariant | open | | |
| INV-7.3-3 | 00-overview §7.3 | One pending batch per account; commit can silently overwrite token | invariant | open | | |
| INV-7.3-4 | 00-overview §7.3 | Every commit carries stateAfter from post-execution getGitState | invariant | open | | |
| INV-7.3-5 | 00-overview §7.3 | Account unrealizedPnL equals position sum; getState path may differ | invariant | open | | |
| INV-7.3-6 | 00-overview §7.3 | Position math has single implementation for derive and pnl | invariant | open | | |
| INV-7.3-7 | 00-overview §7.3 | Cost basis rebuilt on wallet positions only | invariant | open | | |
| INV-7.3-8 | 00-overview §7.3 | In-flight orders suppress drift accounting until settlement | invariant | open | | |
| INV-7.3-9 | 00-overview §7.3 | sync, observe, reconcile bypass write mutex and mutate head | invariant | open | | |
| INV-7.3-10 | 00-overview §7.3 | Commits immutable, head mutable; show by hash is read-only | invariant | open | | |
| INV-7.4-1 | 00-overview §7.4 | Pending forbids further stage; single pending each account; overwrite possible | invariant | open | | |
| INV-7.4-2 | 00-overview §7.4 | Write-in-progress bars four actions but not sync, reconcile, observed | invariant | open | | |
| INV-7.4-3 | 00-overview §7.4 | push and reject present current pendingHash via beginWrite plus 409 | invariant | open | | |
| INV-7.4-4 | 00-overview §7.4 | Commits append-only; no compaction or history mutation APIs | invariant | open | | |
| INV-7.4-5 | 00-overview §7.4 | HEAD equals last written hash except across persistence-failure windows | invariant | open | | |
| INV-7.4-6 | 00-overview §7.4 | Ledger money strings; orderIds raw; legacy numbers rehydrated | invariant | open | | |
| INV-7.4-7 | 00-overview §7.4 | Approval writes nothing externally; reject still performs broker reads | invariant | open | | |
| INV-7.4-8 | 00-overview §7.4 | Execute, snapshot, record, persist, clear chain lacks transaction, compensation | invariant | open | | |
| INV-7.4-9 | 00-overview §7.4 | Approval actor, TTL, partial approval, withdrawal all absent | invariant | open | | |
| INV-7.5-1 | 00-overview §7.5 | Snapshots persist only with real data; keyless zeros are legitimate empties | invariant | open | | |
| INV-7.5-2 | 00-overview §7.5 | Money fields strings; equity-curve sums Numbers before stringifying | invariant | open | | |
| INV-7.5-3 | 00-overview §7.5 | positions marketValue always positive; direction expressed by side | invariant | open | | |
| INV-7.5-4 | 00-overview §7.5 | openOrders holds active orders only; terminal truth in history | invariant | open | | |
| INV-7.5-5 | 00-overview §7.5 | Chunk files hold at most 50 lines; deletion causes collisions | invariant | open | | |
| INV-7.5-6 | 00-overview §7.5 | Index chunks keep startTime ≤ endTime; concurrent writes break it | invariant | open | | |
| INV-7.5-7 | 00-overview §7.5 | Guards only pass or reject; never rewrite operations | invariant | open | | |
| INV-7.5-8 | 00-overview §7.5 | Guard short-circuit order follows configuration array order | invariant | open | | |
| INV-7.5-9 | 00-overview §7.5 | Guards miss sync, reconcile, observe, simulator mutation paths | invariant | open | | |
| INV-7.6-1 | 00-overview §7.6 | UTA performs no package management; only reads and imports | invariant | open | | |
| INV-7.6-2 | 00-overview §7.6 | Pack compatibility from API version; old product-version packs load | invariant | open | | |
| INV-7.6-3 | 00-overview §7.6 | Immutable releases never modified; repair creates new release | invariant | open | | |
| INV-7.6-4 | 00-overview §7.6 | Failed installs leave previous active pointer untouched | invariant | open | | |
| INV-7.6-5 | 00-overview §7.6 | Same-engine installs mutually excluded via directory lock | invariant | open | | |
| INV-7.6-6 | 00-overview §7.6 | Auto-reconcile only updates existing packs; never installs new | invariant | open | | |
| INV-7.6-7 | 00-overview §7.6 | Mock is the sole built-in engine | invariant | open | | |
| INV-7.6-8 | 00-overview §7.6 | Cross-pack boundary never relies on class identity; stable codes | invariant | open | | |
| INV-7.6-9 | 00-overview §7.6 | Bridge write methods throw when disconnected; never silent no-op | invariant | open | | |
| INV-7.6-10 | 00-overview §7.6 | Search availability differs from catalog readiness; Alpaca echoes fallback | invariant | open | | |
| INV-7.7-1 | 00-overview §7.7 | Alice holds no broker connections or caches, with four leaks | invariant | open | | |
| INV-7.7-2 | 00-overview §7.7 | Git commit hash sole persistent identity for trade decisions | invariant | open | | |
| INV-7.7-3 | 00-overview §7.7 | Decimal precision end-to-end; tools accept strings only | invariant | open | | |
| INV-7.7-4 | 00-overview §7.7 | Approval wall un-bypassable; AI direct push lacks recorded actor | invariant | open | | |
| INV-7.7-5 | 00-overview §7.7 | Mode env var overrides config once set | invariant | open | | |
| INV-7.7-6 | 00-overview §7.7 | IBKR unset sentinel values stripped before use | invariant | open | | |
| INV-7.8-1 | 00-overview §7.8 | accounts.json sealed envelope or legacy array; writes via single function | invariant | open | | |
| INV-7.8-2 | 00-overview §7.8 | sealing.key lives outside data subtree | invariant | open | | |
| INV-7.8-3 | 00-overview §7.8 | Migration bodies idempotent via journal plus body self-check | invariant | open | | |
| INV-7.8-4 | 00-overview §7.8 | Migration failure halts boot; snapshots cover data config only | invariant | open | | |
| INV-7.8-5 | 00-overview §7.8 | Boot purge writes accounts.json when ephemeral accounts exist | invariant | open | | |
| INV-7.8-6 | 00-overview §7.8 | Pack releases immutable; activation only via atomic pointer replacement | invariant | open | | |
| INV-7.8-7 | 00-overview §7.8 | Pack compatibility decided by API version at load time | invariant | open | | |
| INV-7.8-8 | 00-overview §7.8 | Snapshot store writes serialized with atomic index replacement | invariant | open | | |
| INV-7.8-9 | 00-overview §7.8 | git-persistence lacks serialization and atomic rename discipline | invariant | open | | |
| INV-7.8-10 | 00-overview §7.8 | Ephemeral UTAs restricted to mock-simulator preset | invariant | open | | |
| INV-7.8-11 | 00-overview §7.8 | live-paper run records exclude ids, balances, credentials, payloads | invariant | open | | |
| INV-7.9-1 | 00-overview §7.9 | Live verification uses demo or paper accounts after mode check | invariant | open | | |
| INV-7.9-2 | 00-overview §7.9 | Agent surface only; HTTP push stands in for approval click | invariant | open | | |
| INV-7.9-3 | 00-overview §7.9 | Never trust ledger over venue; verify material orders venue-side | invariant | open | | |
| INV-7.9-4 | 00-overview §7.9 | ccxt is SDK not semantics; per-venue availability differs | invariant | open | | |
| INV-7.9-5 | 00-overview §7.9 | Balance routing authoritative; unknown mode query fails account read | invariant | open | | |
| INV-7.9-6 | 00-overview §7.9 | Zero balance is discovery, cross-checked with venue UI | invariant | open | | |
| INV-7.9-7 | 00-overview §7.9 | Close-out leaves flat account: no pendings, clean status, baseline | invariant | open | | |
| INV-7.9-8 | 00-overview §7.9 | Limit prices inside venue bands with fresh quotes before push | invariant | open | | |
| INV-7.9-9 | 00-overview §7.9 | Every bug fixed in place or filed; fixes add regression spec | invariant | open | | |
| OQ-1 | 00-overview §9 | Failure atomicity: pre-write intents, broker idempotency, or compensating commits | open-question | open | | |
| OQ-2 | 00-overview §9 | Pending and staging persistence boundary, restart shape, MockBroker replay API | open-question | open | | |
| OQ-3 | 00-overview §9 | Push-time fill booking: immediate response data or sync-only pending | open-question | open | | |
| OQ-4 | 00-overview §9 | Write-lock scope for synthetic commits; multi-process file locking strategy | open-question | open | | |
| OQ-5 | 00-overview §9 | Approval upgrades: actor, TTL, partial approval, withdrawal, ledger migration | open-question | open | | |
| OQ-6 | 00-overview §9 | Crash recovery owner, restart policy, backoff, health criteria, merged contracts | open-question | open | | |
| OQ-7 | 00-overview §9 | Code boundary: reverse deps cut, credential schema home, test-connection | open-question | open | | |
| OQ-8 | 00-overview §9 | Type single-sourcing: UI protocol adoption, snapshot types, naming | open-question | open | | |
| OQ-9 | 00-overview §9 | Guard contract: failure semantics, coverage, validation layer, config fixes | open-question | open | | |
| OQ-10 | 00-overview §9 | Security: snapshot traversal fix, long-term zero-auth, BFF mutation list | open-question | open | | |
| OQ-11 | 00-overview §9 | Pack boundary reality: wrapper vs package, version checks, restart-free | open-question | open | | |
| OQ-12 | 00-overview §9 | Ledger and snapshot capacity: inline vs archive, indexes, retention, keyless | open-question | open | | |
| OQ-13 | 00-overview §9 | Contract identity: dedup ownership, assetClass hooks, hub shape, normalization | open-question | open | | |
| OQ-14 | 00-overview §9 | Capability negotiation: stage gate, missing-engine UI visibility | open-question | open | | |
| OQ-15 | 00-overview §9 | FX expiry semantics: source values, warning levels, hub validation | open-question | open | | |
| OQ-16 | 00-overview §9 | Consumer convergence: polling scheduler, dead SDK methods, demo fidelity | open-question | open | | |
| OQ-17 | 00-overview §9 | Scope rulings: Futu and OpenD inclusion, legacy removal, shape changes | open-question | open | | |
| OQC-1 | 00-core-contract §6 | Ledger append atomicity and single-writer implementation under file persistence | open-question | open | | |
| OQC-2 | 00-core-contract §6 | Dynamic provider loading vs type-safety boundary: runtime or compile-time | open-question | open | | |
| OQC-3 | 00-core-contract §6 | Unknown-receipt retry permission tied to mandatory idempotency declarations | open-question | open | | |
| SF-ui | 00-overview §2.1 | Browser trading pages consume via BFF proxy and config routes | surface | open | | |
| SF-bff | 00-overview §2.1 | BFF proxies trading routes with lite and readonly gates | surface | open | | |
| SF-sdk | 00-overview §2.1 | SDK is Alice HTTP-only client holding no broker connections | surface | open | | |
| SF-aitools | 00-overview §2.1 | AI trading tools share SDK HTTP surface behind approval wall | surface | open | | |
| SF-cli | 00-overview §7.9 | alice-uta CLI drives agent-surface trading flows end to end | surface | open | | |
| SF-telegram | 00-overview §2.1 | Telegram connector carries approval requests via review client | surface | open | | |
| SF-bars | 00-overview §2.1 | Bars gateway feeds market data through SDK-backed path | surface | open | | |
| SF-issues | W1 brief consumer surfaces | Issue callback surface receives subscription side-effect fan-out | surface | open | | |
| ST-accounts | 00-overview §2.2 | accounts.json sealed credential envelope with Alice-owned boundary | store | open | | |
| ST-commit | 00-overview §2.2 | commit.json append-only ledger; full-rewrite single persisted truth | store | open | | |
| ST-snapshots | 00-overview §2.2 | Snapshots chunked time-series store with index; UTA-owned | store | open | | |
| ST-events | 00-overview §2.2 | events.jsonl append-only log; UTA writes, never reads back | store | open | | |
| ST-restartflag | 00-overview §2.1 | restart-uta.flag sole cross-process restart signal with debounce | store | open | | |
| ST-packs | 00-overview §2.2 | Broker pack releases immutable; activation via active pointer | store | open | | |
| ST-sealingkey | 00-overview §2.2 | sealing.key kept outside data subtree | store | open | | |
| NF-WS100 | W1 brief nonfunctional | At least 100 concurrent upstream WebSocket market-watch connections | nonfunctional | open | | |
| NF-LANG | W1 brief nonfunctional | Implementation language decision: TypeScript (Node or Bun) vs Rust | nonfunctional | open | | |
| NF-EFFECT | W1 brief nonfunctional | ZIO-like effect-system boundary decision; Effect-TS v3 candidate | nonfunctional | open | | |
| NF-OPENAPI | W1 brief nonfunctional | Provider OpenAPI projection contract with principled extension rules | nonfunctional | open | | |
| NF-AUTH | W1 brief nonfunctional | Ingress authentication and trust principals for new UTA surface | nonfunctional | open | | |
| NF-REPLACE | W1 brief nonfunctional | Seamless replacement and cutover of old UTA without disruption | nonfunctional | open | | |
