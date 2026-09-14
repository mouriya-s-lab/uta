# 验收评审最终产出：ReviewCore

原始产物 `agent://ReviewCore` 的逐字导出。

```
{
  "conclusion": "disagree — the package and documents are not acceptance-ready; there are six blocker-level contradictions and multiple major ambiguities that can produce incompatible implementations.",
  "findings": [
    {
      "rank": 1,
      "severity": "blocker",
      "locations": [
        "plans/uta-refactor/spec/00-decision-register.md:123",
        "plans/uta-refactor/spec/03-ledger-and-persistence.md:79,84-85"
      ],
      "claim": "The binding register describes integrity as SHA-256 of the exact line bytes plus in-body prevHash, while the persistence contract defines lineHash as SHA-256 of canonical JSON bodyBytes only, explicitly excluding the prefix, space, and LF.",
      "impact": "Every frame hash, prevHash chain, HeadState hash, quarantine check, and recovery result differs depending on which rule is implemented.",
      "resolution": "Use one exact rule everywhere; recommend replacing the register wording with: SHA-256 of the canonical JSON body bytes (including its in-body prevHash), excluding the 64-hex prefix, separator, and terminating LF."
    },
    {
      "rank": 2,
      "severity": "blocker",
      "locations": [
        "plans/uta-refactor/spec/00-decision-register.md:79,94,96,98",
        "plans/uta-refactor/spec/03-ledger-and-persistence.md:19,104",
        "plans/uta-refactor/spec/types/ledger/entries.ts:13,15-17",
        "plans/uta-refactor/spec/types/provider/indexed.ts:10-14"
      ],
      "claim": "D3 requires every persisted/wire provider value to cross as ProviderEnvelope, but the canonical ledger types persist an existential RecoveryResult<ProviderTypes> directly and use different, undocumented encodings for attempt key, receipt, and observation. D4 lists ProviderKey and direct receipt/observation shapes, whereas the types use key:ProviderEnvelope, receipt:{receipt:ProviderEnvelope,result}, and {observation,envelope}. The type also requires provenance on rejected/unknown receipts although D4 only specifies it for accepted.",
      "impact": "A persisted recovery result has no explicit providerId/projectionVersion envelope and cannot be safely reassociated after type erasure; implementers must choose incompatible payload shapes and may violate the no-cast existential boundary.",
      "resolution": "Make the persistence representation normative: define explicit serializable DTOs for every provider-bearing field (including a ProviderEnvelope-wrapped found receipt and observation in recovery.resolved), then rewrite D4’s table to name those fields or provide a formal logical-to-persisted encoding."
    },
    {
      "rank": 3,
      "severity": "blocker",
      "locations": [
        "plans/uta-refactor/spec/04-provider-projections.md:74-88",
        "plans/uta-refactor/spec/types/provider/abi.ts:17-20"
      ],
      "claim": "The loader specification requires every load to collapse into LoaderValidationResult<loaded{module}|invalid{code,path}> with five stable failure codes, but PackLoader.load is typed as returning Result<ProviderEnvelope,ParseError> and accepts a consume callback.",
      "impact": "A conforming implementation cannot return the required loaded/invalid result, stable code, or path through the canonical interface; dynamic import failures can be exposed as the wrong error model.",
      "resolution": "Change PackLoader.load to return LoaderValidationResult (with a separate, explicitly typed consume/registry step if needed), and preserve the five codes/path as the only loader failure surface."
    },
    {
      "rank": 4,
      "severity": "blocker",
      "locations": [
        "plans/uta-refactor/spec/00-decision-register.md:156",
        "plans/uta-refactor/spec/03-ledger-and-persistence.md:99-104",
        "plans/uta-refactor/spec/05-protocol-and-replacement.md:146,149,191-193",
        "plans/uta-refactor/spec/types/wire/v2.ts:61,77",
        "plans/uta-refactor/spec/types/README.md:118"
      ],
      "claim": "The event and ledger/archive routes are described as returning ledger entries, but their canonical wire shape is entry:ProviderEnvelope inside ReadProjectionResponse. LedgerEntry includes non-provider intent.proposed, authorization.decided, and work.requested entries; ReadProjectionResponse also requires an upstream asOf/freshness that a ledger/archive read does not have. The ledger endpoint’s fromPosition/limit query has no canonical wire DTO.",
      "impact": "An implementer must invent a provider identity for UTA/legacy rows, fabricate an asOf, or drop required ledger fields; generated clients cannot consume the promised ledger stream consistently.",
      "resolution": "Define a serializable LedgerEntryWire/UnknownLedgerEntry (and LedgerEntriesQuery/Response) for event, ledger, and archive surfaces, preserving source/accountId/position, common entry fields, raw traceability, and recorded/occurred times. Reserve ProviderEnvelope and ReadProjectionResponse for provider observations; do not synthesize upstream asOf."
    },
    {
      "rank": 5,
      "severity": "blocker",
      "locations": [
        "plans/uta-refactor/spec/01-architecture.md:94,318",
        "plans/uta-refactor/spec/03-ledger-and-persistence.md:140",
        "plans/uta-refactor/spec/types/ledger/store.ts:13-18"
      ],
      "claim": "Drain/root shutdown and the persistence contract require AppendStore.close(), queue draining, lock release, and a structured post-close append failure, but the canonical AppendStore interface has only open, append, replay, and head, and AppendResult has no closed variant.",
      "impact": "The required shutdown sequence and lock lifecycle cannot be implemented through the canonical seam; implementations must add an unregistered method or throw an unspecified error.",
      "resolution": "Add close() to AppendStore and add a named Closed result/error (or a formally typed CloseResult plus closed append outcome), then align the register, architecture table, and shutdown text."
    },
    {
      "rank": 6,
      "severity": "blocker",
      "locations": [
        "plans/uta-refactor/spec/01-architecture.md:119",
        "plans/uta-refactor/spec/08-verification.md:185",
        "plans/uta-refactor/spec/05-protocol-and-replacement.md:85",
        "plans/uta-refactor/spec/types/wire/v2.ts:65"
      ],
      "claim": "ProviderUnknown is mapped to HTTP 503 in architecture and the P-3 verification lane, but the protocol error table assigns it HTTP 202. The wire type only exposes it as an ErrorCode and does not define which 202 success body would carry it.",
      "impact": "Clients will implement different retry and completion semantics (error/retry at 503 versus accepted asynchronous result at 202), and 202 conflicts with the stated non-2xx ErrorEnvelope/error mapping model.",
      "resolution": "Choose one protocol. Recommend make ProviderUnknown consistently 503 ErrorEnvelope (as architecture and verification already do), reserving 202 for recorded command/intent responses; update 05 and add the exact success/error body rule."
    },
    {
      "rank": 7,
      "severity": "major",
      "locations": [
        "plans/uta-refactor/spec/00-decision-register.md:109",
        "plans/uta-refactor/spec/03-ledger-and-persistence.md:521-535"
      ],
      "claim": "D4 says ConfigRevision hashes account config + policy + projection version, while the detailed derivation hashes a whole runtimeConfigDigest (all accounts, rules, projections, recovery, capacity, retention) plus Alice account row and projection version.",
      "impact": "Changing an unrelated account, queue capacity, or retention setting either changes every account revision or does not, depending on the implementation; snapshot and retry binding diverge.",
      "resolution": "Follow the binding per-account contract: define one exact effective-account preimage containing the account config, that account’s policy/runtime inputs, and active projection version; keep any whole-file runtime digest separate unless the register is explicitly amended."
    },
    {
      "rank": 8,
      "severity": "major",
      "locations": [
        "plans/uta-refactor/spec/03-ledger-and-persistence.md:521-535",
        "plans/uta-refactor/spec/types/ids.ts:44-45",
        "plans/uta-refactor/spec/types/fixtures/positive/replay.ts:23"
      ],
      "claim": "The detailed config contract requires ConfigRevision to be a lowercase 64-hex SHA-256 derived value, but parseConfigRevision accepts any nonempty opaque string and the positive fixture uses revision-1; no derivation function exists in the package.",
      "impact": "A caller can construct and persist a value that is not a revision hash, defeating the stated config snapshot/integrity binding while all current type fixtures pass.",
      "resolution": "Add the canonical derivation/parser boundary (64 lowercase hex, fixed canonical preimage), use it for entry creation, and replace the fixture’s arbitrary revision with a derived value; if the parser remains wire-generic, expose a separate trusted constructor and forbid that generic value in domain entries."
    },
    {
      "rank": 9,
      "severity": "major",
      "locations": [
        "plans/uta-refactor/spec/00-decision-register.md:169,175",
        "plans/uta-refactor/spec/03-ledger-and-persistence.md:510",
        "plans/uta-refactor/spec/types/policy.ts:7",
        "plans/uta-refactor/spec/types/fixtures/positive/replay.ts:82"
      ],
      "claim": "D11 requires financial values crossing boundaries as canonical decimal strings, yet maxNotional.limit is parsed as an unbounded finite JavaScript number; D10’s finite numeric wording does not override D11’s money rule.",
      "impact": "Large or fractional notional limits can round or lose precision before authorization, and JSON numeric limits are accepted contrary to the financial boundary.",
      "resolution": "Model maxNotional.limit as DecimalString (paired with currency) and parse it with the money parser; retain numeric milliseconds only for non-financial rule fields."
    },
    {
      "rank": 10,
      "severity": "major",
      "locations": [
        "plans/uta-refactor/spec/00-decision-register.md:103",
        "plans/uta-refactor/spec/types/ledger/fold.ts:6,64-105",
        "plans/uta-refactor/spec/types/wire/v2.ts:74"
      ],
      "claim": "D4 says a withdraw after attempt.started produces status recorded, but the normative IntentOutcome union in fold and wire has no recorded variant and the fold ignores that post-attempt withdraw for execution.",
      "impact": "The required status cannot be represented in detail responses or folds; implementers must either violate D4 or add a new variant across every consumer/client.",
      "resolution": "Remove recorded as an IntentOutcome literal and say the post-attempt decision remains an audit entry while the execution outcome is unchanged (preferred), or add recorded exhaustively to all status types and folds."
    },
    {
      "rank": 11,
      "severity": "major",
      "locations": [
        "plans/uta-refactor/spec/00-decision-register.md:97",
        "plans/uta-refactor/spec/06-interaction-and-issues.md:789-791",
        "plans/uta-refactor/spec/types/issues.ts:5-7",
        "plans/uta-refactor/spec/types/ledger/entries.ts:16"
      ],
      "claim": "The register calls manual recovery resolution a decision kind, while the interaction contract explicitly says it must use the separate manual-resolution variant and must not be substituted with DecisionAction; the type uses awaitingReview|abandon|continue, none of which is DecisionAction.",
      "impact": "A review endpoint implementer can incorrectly treat manual resolution as approve/reject/withdraw, or cannot serialize the documented manual resolution through the decision type.",
      "resolution": "State in D4 that manual recovery is a review-flow decision recorded as recovery.resolved.manual with the three named resolutions; DecisionAction only authorizes the next declared step and is not the resolution payload."
    },
    {
      "rank": 12,
      "severity": "major",
      "locations": [
        "plans/uta-refactor/spec/00-decision-register.md:96",
        "plans/uta-refactor/spec/04-provider-projections.md:502,509,540",
        "plans/uta-refactor/spec/types/provider/abi.ts:9",
        "plans/uta-refactor/spec/types/provider/indexed.ts:11-12"
      ],
      "claim": "The register and transport types restrict unknown receipt causes to timeout|disconnect|noResponse, but the provider taxonomy requires an already-sent parse failure to become an unknown outcome with cause parseFailure.",
      "impact": "The required two-layer result (unknown placement plus parseFailure cause) cannot be represented; implementers may incorrectly report a known ProviderParseFailure or lose the unknown placement state.",
      "resolution": "Add parseFailure to the canonical unknown-cause union and D4 receipt table while retaining ProviderParseFailure as the wire classification, or explicitly remove the unknown+parseFailure requirement; the former matches the verification taxonomy."
    },
    {
      "rank": 13,
      "severity": "major",
      "locations": [
        "plans/uta-refactor/spec/04-provider-projections.md:512,556",
        "plans/uta-refactor/spec/types/provider/abi.ts:8-10"
      ],
      "claim": "The provider contract mandates preservation of HTTP status, request ID, raw body, send evidence, and provider time, but TransportResult.response only has payload, notSent only has a string cause, and unknown only has a three-value cause.",
      "impact": "A plugin can satisfy the type while dropping the evidence needed to distinguish notSent from unknown, audit rejection, reconstruct parse failures, or preserve provider time.",
      "resolution": "Add an explicit serializable TransportEvidence record (status, requestId, raw payload/body, sendEvidence, provider time as applicable) to the transport result variants, and state which fields are mandatory for each variant."
    },
    {
      "rank": 14,
      "severity": "major",
      "locations": [
        "plans/uta-refactor/spec/01-architecture.md:135-143",
        "plans/uta-refactor/spec/04-provider-projections.md:549-556",
        "plans/uta-refactor/spec/types/provider/abi.ts:8-14"
      ],
      "claim": "Architecture requires the transport adapter to inject AbortSignal and enforce interruptible provider I/O, but the canonical HTTP/WS/Custom executor interfaces have no signal/cancellation parameter and D2 says Pack ABI arguments/results are serializable.",
      "impact": "An implementation cannot both honor the required cancellation path and obey the declared ABI; it must silently ignore cancellation, pass a non-serializable value through the pack boundary, or invent an out-of-band API.",
      "resolution": "Specify the boundary explicitly. Recommended: add a local, non-serialized cancellation context/signal to executor calls and amend D2 to exempt control signals from serializable business values; otherwise remove the injection requirement and define the adapter’s precise deadline/race/close behavior."
    },
    {
      "rank": 15,
      "severity": "major",
      "locations": [
        "plans/uta-refactor/spec/00-decision-register.md:155-157,176",
        "plans/uta-refactor/spec/01-architecture.md:157,166-172",
        "plans/uta-refactor/spec/03-ledger-and-persistence.md:388",
        "plans/uta-refactor/spec/types/README.md:77",
        "plans/uta-refactor/spec/types/consumer.ts:4"
      ],
      "claim": "D11 reserves Cursor for opaque provider/channel state, while architecture calls every internal ledger checkpoint Cursor and Reconciliation has both a ledger Cursor and provider Cursor; the type package defines Consumer.cursor as LedgerPosition and Cursor as a provider envelope.",
      "impact": "Implementers and generated clients cannot tell whether Cursor means ledger position or provider token, especially for Reconciliation and readiness checkpoint persistence.",
      "resolution": "Reserve Cursor for provider/channel-scoped opaque values; rename the internal ledger field to ConsumerPosition/LedgerCheckpoint (with sourcePosition/sourceHash) in architecture and consumer types, while keeping CursorMap as the wire account-position map."
    },
    {
      "rank": 16,
      "severity": "major",
      "locations": [
        "plans/uta-refactor/spec/01-architecture.md:158-160",
        "plans/uta-refactor/spec/03-ledger-and-persistence.md:398-406",
        "plans/uta-refactor/spec/types/consumer.ts:4",
        "plans/uta-refactor/spec/types/ledger/store.ts:10,16"
      ],
      "claim": "The document requires every Consumer to retain/report unknown kinds as unconsumed, but Consumer.rule can only receive Extract<LedgerEntry,...> for known kinds and replay returns a union including UnknownLedgerEntry; there is no unknown handler or dispatcher contract.",
      "impact": "A conforming consumer must bypass the canonical Consumer type or fabricate a known kind, making unknown handling silently implementation-specific.",
      "resolution": "Make the routing contract explicit and typed: either require Consumer.onUnknown(UnknownLedgerEntry,...) alongside rule, or define a separate mandatory replay dispatcher that records unknowns before invoking known-kind Consumers."
    },
    {
      "rank": 17,
      "severity": "major",
      "locations": [
        "plans/uta-refactor/spec/00-decision-register.md:79",
        "plans/uta-refactor/spec/04-provider-projections.md:17",
        "plans/uta-refactor/spec/types/provider/abi.ts:16",
        "plans/uta-refactor/spec/types/provider/indexed.ts:18-19"
      ],
      "claim": "The spec requires provider envelopes to be reassociated through ProjectionRegistry for provider-indexed values and recovery, but the canonical registry callback exposes only providerId/projectionVersion/decodeOperation; it has no decodeReceipt, decodeObservation, or recovery association path despite Translation defining all four decoders.",
      "impact": "Replay/recovery code has no registered, cast-free way to turn persisted receipt/observation/recovery envelopes back into P-indexed values.",
      "resolution": "Expose the complete provider projection/translation context (and the applicable recovery witness) through registry reassociation, or add separately named typed reassociateReceipt/reassociateObservation/reassociateRecovery APIs."
    },
    {
      "rank": 18,
      "severity": "major",
      "locations": [
        "plans/uta-refactor/spec/00-decision-register.md:182",
        "plans/uta-refactor/spec/05-protocol-and-replacement.md:77",
        "plans/uta-refactor/spec/01-architecture.md:110-123",
        "plans/uta-refactor/spec/types/wire/v2.ts:65"
      ],
      "claim": "ServiceDraining is a required lifecycle rejection and a declared ErrorCode, but it is omitted from architecture’s HTTP mapping table even though that table says the adapter maps only the listed declared errors.",
      "impact": "The mandated drain response has no status/body rule in the architecture contract; implementers may expose 500, ReadinessUnavailable, or an untyped error.",
      "resolution": "Add ServiceDraining → HTTP 503 and its ErrorEnvelope handling to the architecture mapping, matching 05 and the wire union."
    },
    {
      "rank": 19,
      "severity": "major",
      "locations": [
        "plans/uta-refactor/spec/01-architecture.md:150",
        "plans/uta-refactor/spec/03-ledger-and-persistence.md:137,203",
        "plans/uta-refactor/spec/05-protocol-and-replacement.md:161-169",
        "plans/uta-refactor/spec/types/ledger/store.ts:8",
        "plans/uta-refactor/spec/types/wire/v2.ts:65"
      ],
      "claim": "DurabilityFailure is a canonical AppendResult and must be exposed as a structured readiness failure, but it is not an ErrorCode and has no HTTP mapping; write endpoint tables only mention generic ReadinessUnavailable without saying how stage/result survives.",
      "impact": "A real append failure can be reported as an untyped 500, a successful command, or a misleading provider error, and clients lose the required stage diagnostics.",
      "resolution": "Map DurabilityFailure to ReadinessUnavailable/503 for HTTP while retaining stage/result in structured diagnostics and readiness, and state that mapping in the protocol and architecture tables (or add a dedicated wire code everywhere)."
    },
    {
      "rank": 20,
      "severity": "major",
      "locations": [
        "plans/uta-refactor/spec/05-protocol-and-replacement.md:68",
        "plans/uta-refactor/spec/01-architecture.md:123",
        "plans/uta-refactor/spec/types/wire/v2.ts:65-72"
      ],
      "claim": "The protocol requires every non-2xx /v2 response to be an ErrorEnvelope, while architecture allows an ordinary 500 for an unexpected Effect defect and ErrorCode has no internal/server-error variant.",
      "impact": "There is no valid canonical body for the required 500 path; mapping defects to an unrelated code would leak or misclassify failure semantics.",
      "resolution": "Add an InternalError (or similarly named) ErrorCode with explicit 500 ErrorEnvelope shape and safe-message rules, then map unexpected defects to it without serializing Cause/FiberFailure."
    },
    {
      "rank": 21,
      "severity": "major",
      "locations": [
        "plans/uta-refactor/spec/06-interaction-and-issues.md:789-791",
        "plans/uta-refactor/spec/types/issues.ts:7",
        "plans/uta-refactor/spec/types/wire/v2.ts:65"
      ],
      "claim": "The typed decision bridge says rejected responses may use Unauthorized, CapabilityUnsupported, ServiceDraining, and ReadinessUnavailable (as well as IntentExpired/DecisionConflict), but DecisionResponse.rejected.code only permits IntentExpired|DecisionConflict|InvalidRequest|ServiceDraining.",
      "impact": "The documented bridge error cases cannot be represented by the canonical DecisionResponse; callers must guess whether to receive rejected or ErrorEnvelope.",
      "resolution": "Define one exact decision-error union and use it consistently; recommend expanding the bridge’s rejected code to the listed decision-relevant ErrorCodes (or explicitly make ErrorEnvelope the failure type for this endpoint)."
    },
    {
      "rank": 22,
      "severity": "major",
      "locations": [
        "plans/uta-refactor/spec/04-provider-projections.md:699",
        "plans/uta-refactor/spec/types/provider/recovery.ts:35-36",
        "plans/uta-refactor/spec/types/README.md:97"
      ],
      "claim": "The recovery law says stillUnknown scheduling must not exceed maxUnknownDuration, but nextAction returns scheduleRecheck at arbitrary result.nextCheckAfter whenever now is before the budget; the type permits a value beyond the deadline or in the past.",
      "impact": "A malformed or adversarial witness can schedule review after the maximum unknown budget, violating the safety bound and delaying required review indefinitely.",
      "resolution": "Normalize/validate the schedule in nextAction: require a future time and clamp it to unknownSince+maxUnknownDuration; at or beyond that boundary return awaitingReview{UnknownDurationExceeded}."
    },
    {
      "rank": 23,
      "severity": "major",
      "locations": [
        "plans/uta-refactor/spec/01-architecture.md:316-318",
        "plans/uta-refactor/spec/03-ledger-and-persistence.md:140",
        "plans/uta-refactor/spec/types/ledger/store.ts:13-18"
      ],
      "claim": "Drain forbids starting new provider mutations and says queued commands without a permit remain durable for restart, while AppendStore.close waits for the per-account queue to become empty. No operation removes/rejects in-memory queued commands or defines how close can complete.",
      "impact": "A normal drain can hang forever, or an implementer can drop queued commands/mark them abandoned contrary to D12.",
      "resolution": "On the atomic draining transition, stop intake, reject/remove only non-durable queued commands with ServiceDraining, retain already durable intent/authorization for restart, and define that close waits only after the queue has reached this deterministic empty state."
    },
    {
      "rank": 24,
      "severity": "major",
      "locations": [
        "plans/uta-refactor/spec/03-ledger-and-persistence.md:490-495",
        "plans/uta-refactor/spec/01-architecture.md:279-283"
      ],
      "claim": "The persistence startup baseline requires reading MigrationMarker/archive state before opening the fresh ledger, but the architecture’s Ledger open stage immediately resolves paths, acquires the lock, calls AppendStore.open, and replays without that prerequisite.",
      "impact": "Implementations can replay or expose a fresh ledger before detecting marker/archive mismatch, changing readiness and migration safety outcomes.",
      "resolution": "Add marker/archive/digest validation as the first substep of Ledger open before AppendStore.open/replay (or explicitly move that check into the preceding startup stage and state the ordering equivalently)."
    },
    {
      "rank": 25,
      "severity": "major",
      "locations": [
        "plans/uta-refactor/spec/00-decision-register.md:41-43",
        "plans/uta-refactor/spec/01-architecture.md:87,129-131",
        "plans/uta-refactor/spec/04-provider-projections.md:542-556"
      ],
      "claim": "D2 says Effect never touches generated clients, while the architecture puts Promise-facing TransportPlugin calls through an Effect adapter and the provider contract says HTTPExecutor faces generated OpenAPI clients.",
      "impact": "The pack/core ownership boundary is unclear: implementers may import generated clients into Effect core, put generated clients inside packs, or violate the stated no-touch rule.",
      "resolution": "Clarify that generated client source/types remain Effect-free and cross no boundary, while the only permitted invocation is an opaque Promise-facing generated-client wrapper at the provider transport adapter; no generated client or SDK value enters core serialization."
    },
    {
      "rank": 26,
      "severity": "minor",
      "locations": [
        "plans/uta-refactor/spec/00-decision-register.md:73,75,103,115",
        "plans/uta-refactor/spec/00-decision-register.md:162",
        "plans/uta-refactor/spec/types/issues.ts:9",
        "plans/uta-refactor/spec/types/provider/recovery.ts:23"
      ],
      "claim": "Machine-looking literals are inconsistent: prose uses awaiting-review and review.unknown-outcome, while canonical status/work-request/recovery values use awaitingReview and review.unknownOutcome.",
      "impact": "An implementer can emit different wire/ledger discriminants from the walkthrough versus the registered vocabulary; UI copy and protocol tokens are not distinguished.",
      "resolution": "Normalize protocol/ledger/status tokens to the camelCase registered values and label hyphenated forms explicitly as UI display text only."
    },
    {
      "rank": 27,
      "severity": "minor",
      "locations": [
        "plans/uta-refactor/spec/00-decision-register.md:39,193",
        "plans/uta-refactor/spec/types/README.md:187",
        "plans/uta-refactor/spec/types/package.json:10"
      ],
      "claim": "D2 says effect is a dependency of services/uta only, while the binding type-package clause and package manifest permit effect as a dev dependency for boundary fixtures.",
      "impact": "Dependency audits and implementers may either reject the required fixture package or add Effect as a runtime dependency to the library package.",
      "resolution": "Change D2 to say Effect is a runtime dependency only of services/uta; the private types package may have the pinned dev-only fixture dependency and no production import."
    }
  ],
  "verification": "Read-only document/type review only. No files were modified and no formatter, linter, or project-wide tests were run, per the acceptance constraints; the user-provided typecheck/fixture pass was not re-run."
}
```
