# 验收评审最终产出：ReviewAxioms

原始产物 `agent://ReviewAxioms` 的逐字导出。

```
{
  "verdict": "Not acceptance-ready. Main's adjudication closes the ProviderUnknown HTTP-status conflict in favor of 503; the remaining findings below still need one canonical resolution each.",
  "closed_adjudication": [
    {
      "severity": "blocker",
      "claim": "The HTTP mapping was contradictory: architecture grouped ProviderUnknown with 503, while protocol mapped it to 202.",
      "sideA": "plans/uta-refactor/spec/01-architecture.md:118-121",
      "sideB": "plans/uta-refactor/spec/05-protocol-and-replacement.md:82-87",
      "resolution": "Use ProviderUnknown=503 as adjudicated by Main; update protocol document in the reconciliation wave, retaining 504 only for server deadline overrun."
    }
  ],
  "blockers": [
    {
      "id": "B1",
      "claim": "The public PackLoader contract has two incompatible result protocols: the provider spec requires LoaderValidationResult.loaded|invalid with stable loader codes, while the type package exposes PackLoader.load returning Result<ProviderEnvelope,ParseError> and a callback that consumes a typed module.",
      "sideA": "plans/uta-refactor/spec/04-provider-projections.md:73-88",
      "sideB": "plans/uta-refactor/spec/types/provider/abi.ts:18-20",
      "resolution": "Make PackLoader.load return LoaderValidationResult<P> (or explicitly rename/separate this lower-level seam); preserve the five stable invalid codes and path."
    },
    {
      "id": "B2",
      "claim": "The acceptance oracle calls a no-match state MissingRemote, but the frozen register, provider types, and verification use confirmedAbsent/abandoned. This changes the variant/outcome names an implementer and test oracle must produce.",
      "sideA": "plans/uta-refactor/design/01-detail-constraints.md:303-305",
      "sideB": "plans/uta-refactor/spec/00-decision-register.md:69-76; plans/uta-refactor/spec/types/provider/recovery.ts:11-14; plans/uta-refactor/spec/08-verification.md:187-193",
      "resolution": "Because the register is binding, canonicalize §8.5 and verification prose to confirmedAbsent, and state that the resulting ledger outcome is abandoned; do not introduce a second variant."
    }
  ],
  "majors": [
    {
      "id": "M1",
      "claim": "ProviderTypes and provider payloads are structurally raw Serializable values, while the type constraint requires opaque payloads to be associated with source/kind and forbids an understood raw map from entering domain rules/materialization.",
      "sideA": "plans/uta-refactor/design/01-detail-constraints.md:276-279,290-293",
      "sideB": "plans/uta-refactor/spec/types/provider/indexed.ts:4-16; plans/uta-refactor/spec/types/ledger/entries.ts:10-17",
      "resolution": "Keep Serializable only at an explicit raw boundary; introduce named per-kind/provider payload records or an opaque `{providerId,projectionVersion,kind,payload}` boundary, and require kind-specific parsing before folds/rules."
    },
    {
      "id": "M2",
      "claim": "ProviderEnvelope carries only providerId, projectionVersion, and arbitrary payload; the protocol requires EventItem/ledger exposure to retain entry kind, why, correlation, provider identity, and raw traceability. The type permits an implementation to omit all of these from the envelope.",
      "sideA": "plans/uta-refactor/spec/05-protocol-and-replacement.md:191-193; plans/uta-refactor/spec/06-interaction-and-issues.md:69-78",
      "sideB": "plans/uta-refactor/spec/types/provider/indexed.ts:16; plans/uta-refactor/spec/types/wire/v2.ts:61-64",
      "resolution": "Define the existential envelope/event item so kind, why, correlation, source, and provider identity are mandatory at the relevant boundary; do not rely on an unconstrained payload map to carry required fields."
    },
    {
      "id": "M3",
      "claim": "D3 requires a found recovery to append a late receipt and a separate observation entry, but RecoveryResult.found merely nests both values and the package fixture does not exercise the required two-entry append.",
      "sideA": "plans/uta-refactor/spec/00-decision-register.md:69-73; plans/uta-refactor/spec/03-ledger-and-persistence.md:545-549",
      "sideB": "plans/uta-refactor/spec/types/provider/recovery.ts:11; plans/uta-refactor/spec/types/ledger/entries.ts:15-17; plans/uta-refactor/spec/types/fixtures/positive/replay.ts:34-50",
      "resolution": "Keep the recovery result as a witness if desired, but define/verify the caller transition that emits two distinct entries in one frame (recovered receipt first, observation second), and add that transition to the acceptance fixture."
    },
    {
      "id": "M4",
      "claim": "The declaration's supported write row and PackModule.recovery map are independent types: Record<string,PlacementRecovery<P>> neither keys recovery by OperationKind nor proves every supported write kind has a matching witness. The generic Handlers fixture also accepts an arbitrary hand-authored Spec unrelated to declarations.",
      "sideA": "plans/uta-refactor/spec/04-provider-projections.md:58-65,81-86",
      "sideB": "plans/uta-refactor/spec/types/provider/declaration.ts:14-18; plans/uta-refactor/spec/types/provider/abi.ts:17-20; plans/uta-refactor/spec/types/fixtures/positive/composition.ts:8-12",
      "resolution": "Tie the recovery map to declared write kinds (and contracts) in the package type, then keep runtime loader validation for dynamic modules; make the composition fixture consume a declaration-derived spec rather than an unrelated local type."
    },
    {
      "id": "M5",
      "claim": "CapabilityStatus permits a supported write with idempotency=none and readByKey=none; PlacementRecovery separately rejects none/none, so a status row can claim supported while its required witness is unconstructible.",
      "sideA": "plans/uta-refactor/spec/04-provider-projections.md:194-198",
      "sideB": "plans/uta-refactor/spec/types/provider/declaration.ts:2-18; plans/uta-refactor/spec/types/provider/recovery.ts:15; plans/uta-refactor/spec/types/fixtures/negative/no-recovery.ts:1-3",
      "resolution": "Encode the status/contract invariant: supported write rows require a constructible PlacementRecovery contract; otherwise only unsupported/conditional may be represented."
    },
    {
      "id": "M6",
      "claim": "Handler's conditional type is distributive over a union. A capability typed ReadOnly<T>|ReadWrite<E,T> can therefore accept a read-only-looking handler while also admitting a write branch, weakening the claimed static direction boundary.",
      "sideA": "plans/uta-refactor/design/00-core-contract.md:102-108",
      "sideB": "plans/uta-refactor/spec/types/channel.ts:1-5",
      "resolution": "Make the conditional non-distributive or reject union capability specs, and add a negative fixture specifically for a ReadOnly|ReadWrite union."
    },
    {
      "id": "M7",
      "claim": "The positive composition advertises Heartbeat as bare number, contradicting the distinct domain-type requirement for heartbeat versus transaction intent/receipt/observation/config values.",
      "sideA": "plans/uta-refactor/design/01-detail-constraints.md:276-278; plans/uta-refactor/design/00-core-contract.md:91-97",
      "sideB": "plans/uta-refactor/spec/types/fixtures/positive/composition.ts:8-12",
      "resolution": "Add a named Heartbeat domain type and use it in ReadOnly<Heartbeat>; do not use number as the proof of the ordinary-IO boundary."
    },
    {
      "id": "M8",
      "claim": "Translation encode is typed as returning bare Serializable, although the provider contract requires an encoding failure to remain a structured ParseError/ErrorEnvelope rather than becoming an empty request or an unobservable throw.",
      "sideA": "plans/uta-refactor/spec/04-provider-projections.md:62-65,491-512",
      "sideB": "plans/uta-refactor/spec/types/provider/abi.ts:16",
      "resolution": "Return Result<Serializable,ParseError> (or another named structured encode error) and map it explicitly to notSent/ProviderTransportFailure at the transport boundary."
    },
    {
      "id": "M9",
      "claim": "TransportResult has only response, notSent, and unknown. The provider taxonomy requires distinct rejected and parseFailure outcomes and, for notSent/unknown, auditable send evidence, provider code/request ID/raw body, and cause layering.",
      "sideA": "plans/uta-refactor/spec/04-provider-projections.md:500-518,534-556",
      "sideB": "plans/uta-refactor/spec/types/provider/abi.ts:7-15",
      "resolution": "Expand the ABI result union with the five declared failure semantics or define a typed evidence record carried alongside the result; preserve status/requestId/raw/send evidence without stringly-typed causes."
    },
    {
      "id": "M10",
      "claim": "WSExecutor events expose only payload and an optional Cursor, while the stream manifest and observation contract require provider event identity, provider ordering/time, source/channel, gap/end reason, and reconnect/backfill semantics.",
      "sideA": "plans/uta-refactor/spec/04-provider-projections.md:397-429,490-496; plans/uta-refactor/spec/06-interaction-and-issues.md:506-510",
      "sideB": "plans/uta-refactor/spec/types/provider/abi.ts:10-15; plans/uta-refactor/spec/types/provider/indexed.ts:15",
      "resolution": "Define a typed stream event/evidence record carrying source/channel/event identity/provider time/order and explicit gap/end reason, or state that those are mandatory fields inside a named manifest-bound payload and validate them before Observation construction."
    },
    {
      "id": "M11",
      "claim": "Projection identity is a provider/projection-version pair at the existential boundary, but ProviderKey, ProviderOrderRef, Operation, Receipt, and Observation are parameterized only by P['identity']; the type does not bind P to ProviderId+ProjectionVersion or prevent cross-version association.",
      "sideA": "plans/uta-refactor/spec/00-decision-register.md:53-79; plans/uta-refactor/spec/04-provider-projections.md:18,60,126",
      "sideB": "plans/uta-refactor/spec/types/provider/indexed.ts:6-19; plans/uta-refactor/spec/types/provider/abi.ts:17-20",
      "resolution": "Parameterize/brand the local provider family by a projection identity and require registry re-association to validate both providerId and projectionVersion before constructing all P-indexed values."
    },
    {
      "id": "M12",
      "claim": "D11/§8.1 require distinct brands for source, provider identity, receipt identity, position, and payload; the domain types leave source, provider receipt requestId/code, correlation.providerRef, sourceDigest, and persistence metadata as raw strings/numbers.",
      "sideA": "plans/uta-refactor/design/01-detail-constraints.md:276-279,290-293; plans/uta-refactor/spec/05-protocol-and-replacement.md:93-96",
      "sideB": "plans/uta-refactor/spec/types/provider/indexed.ts:10-15; plans/uta-refactor/spec/types/ledger/entries.ts:21; plans/uta-refactor/spec/types/ledger/store.ts:6-11; plans/uta-refactor/spec/types/provider/declaration.ts:18",
      "resolution": "Add named brands/parsers for source, provider request/ref, digest, segment, byte offset, and opaque payload identity, and carry them through domain, persistence, and wire; reserve raw values for parser input only."
    },
    {
      "id": "M13",
      "claim": "The persistence type accepts LedgerPosition 0 for every EntryEnvelope and frame entry, while the persistence contract says a nonempty frame's final position is positive and the first entry is position 1; only the empty head may be 0.",
      "sideA": "plans/uta-refactor/spec/03-ledger-and-persistence.md:97-110,119-126",
      "sideB": "plans/uta-refactor/spec/types/ids.ts:48-65; plans/uta-refactor/spec/types/ledger/entries.ts:21-24; plans/uta-refactor/spec/types/ledger/store.ts:9-16",
      "resolution": "Use a positive-entry-position refinement for persisted entries/frame writes while retaining a separate zero-valued empty-head type/state; add a negative fixture for position-zero entries."
    },
    {
      "id": "M14",
      "claim": "Financial configuration uses a floating-point number for maxNotional, despite the ledger/security/protocol rules rejecting numeric JSON money and requiring canonical decimal strings.",
      "sideA": "plans/uta-refactor/spec/03-ledger-and-persistence.md:15-19; plans/uta-refactor/spec/05-protocol-and-replacement.md:90-96; plans/uta-refactor/spec/07-security-operations.md:337-345",
      "sideB": "plans/uta-refactor/spec/types/policy.ts:7-11",
      "resolution": "Parse maxNotional as DecimalString (with currency context) and use decimal arithmetic at the policy boundary; do not accept a positive finite number as a monetary domain value."
    },
    {
      "id": "M15",
      "claim": "The simulator contract preserves deltaPercent as a decimal-string financial value, but the wire simulator schema accepts an unconstrained numeric deltaPercent.",
      "sideA": "plans/uta-refactor/spec/05-protocol-and-replacement.md:90-96; plans/uta-refactor/spec/types/README.md:117-120",
      "sideB": "plans/uta-refactor/spec/types/wire/v2.ts:85-118",
      "resolution": "Make simulator deltaPercent use the canonical decimal parser/schema and reject numeric JSON values."
    },
    {
      "id": "M16",
      "claim": "Runtime config capacity validation accepts any positive maxProviderConnections, while operations/security require a lower bound of 100 and explicitly reject smaller values.",
      "sideA": "plans/uta-refactor/spec/07-security-operations.md:339-345,560-565",
      "sideB": "plans/uta-refactor/spec/types/policy.ts:30-40",
      "resolution": "Encode the declared minimum in the schema (or remove the normative lower-bound requirement); the parser must reject values below 100."
    },
    {
      "id": "M17",
      "claim": "Wire DTO schemas erase the domain brands and use plain numbers for positions/instants/asOf/durations, contrary to D11's requirement that brands survive the boundary and be parsed immediately into domain types.",
      "sideA": "plans/uta-refactor/spec/00-decision-register.md:169-177; plans/uta-refactor/spec/05-protocol-and-replacement.md:90-96",
      "sideB": "plans/uta-refactor/spec/types/wire/v2.ts:11-16,34-64,74-80,97-118",
      "resolution": "Keep encoded JSON strings/numbers at the external schema but expose parser outputs as named branded domain values (or provide explicit decode functions immediately after schema validation); do not treat inferred plain DTOs as domain types."
    },
    {
      "id": "M18",
      "claim": "Readiness is modeled as independently combinable dimensions and permits contradictory states such as process=draining with writable=ok or config-invalid with writable=ok; the contract requires draining to block writes and config-invalid to block account writability.",
      "sideA": "plans/uta-refactor/spec/07-security-operations.md:411-428; plans/uta-refactor/spec/01-architecture.md:177-179",
      "sideB": "plans/uta-refactor/spec/types/readiness.ts:4-7; plans/uta-refactor/spec/types/wire/v2.ts:53-56",
      "resolution": "Encode cross-field readiness invariants as a state machine/refined constructors, or define a single readiness variant whose writable state is derived and cannot contradict process/config/transport."
    },
    {
      "id": "M19",
      "claim": "The readiness capability table is an unconstrained record and may be empty, while readiness requires every account's full declared capability table and forbids an empty capability illusion.",
      "sideA": "plans/uta-refactor/spec/07-security-operations.md:406,414-418; plans/uta-refactor/spec/05-protocol-and-replacement.md:127-128",
      "sideB": "plans/uta-refactor/spec/types/readiness.ts:5; plans/uta-refactor/spec/types/wire/v2.ts:53-56",
      "resolution": "Validate the complete declaration-derived capability set, including non-empty/full coverage, before constructing AccountReadiness."
    },
    {
      "id": "M20",
      "claim": "Security requires config-invalid errors to expose structured path/code/safe-message records, but AccountReadiness and its wire schema carry only string[] errors.",
      "sideA": "plans/uta-refactor/spec/07-security-operations.md:414-418",
      "sideB": "plans/uta-refactor/spec/types/readiness.ts:5; plans/uta-refactor/spec/types/wire/v2.ts:54",
      "resolution": "Define a redacted ConfigIssue record with path/code/safe message and use it in both readiness domain and wire schemas."
    },
    {
      "id": "M21",
      "claim": "Issue desk settings/link types carry raw workspace/issue/comment strings, pollIntervalMs, and optional comment identity without boundary parsers or brands, despite the interaction/security contract requiring Alice-boundary parsing and stable identity/timeout semantics.",
      "sideA": "plans/uta-refactor/spec/06-interaction-and-issues.md:14,588-596,705-709; plans/uta-refactor/design/01-detail-constraints.md:276-279",
      "sideB": "plans/uta-refactor/spec/types/issues.ts:12-14",
      "resolution": "Add named parsed Issue/workspace/comment identifiers and a positive bounded duration type, with a structured parser at the Alice bridge boundary."
    },
    {
      "id": "M22",
      "claim": "WorkRequested lacks the provider/channel event cursor and source-event identity required for watch/news causality and replay, and it lacks explicit consumer/rule identity needed to audit the requested decision context.",
      "sideA": "plans/uta-refactor/spec/06-interaction-and-issues.md:347,448-452,506-510,839-845",
      "sideB": "plans/uta-refactor/spec/types/issues.ts:8-10",
      "resolution": "Add an optional provider Cursor/source-event identity and explicit emitter consumer/rule identifiers (or document a single canonical alternate field) to WorkRequested, then preserve them through ledger/events/Issue bridge."
    },
    {
      "id": "M23",
      "claim": "WorkRequested.admissibleDecisions is only readonly DecisionAction[], so an empty list and arbitrary duplicates/invalid combinations are constructible; the interaction contract requires a nonempty valid subset of admissible decisions.",
      "sideA": "plans/uta-refactor/spec/06-interaction-and-issues.md:619-622",
      "sideB": "plans/uta-refactor/spec/types/issues.ts:8-10",
      "resolution": "Use a nonempty tuple plus a validated set/subset refinement for admissible decisions."
    },
    {
      "id": "M24",
      "claim": "The positive crash walkthrough claims an unknown receipt before review work, but its history appends work.requested directly after attempt.started and never appends receipt.recorded{unknown}; this cannot prove the documented crash ordering.",
      "sideA": "plans/uta-refactor/spec/08-verification.md:152,187-193; plans/uta-refactor/spec/06-interaction-and-issues.md:959-961",
      "sideB": "plans/uta-refactor/spec/types/fixtures/positive/replay.ts:34-45",
      "resolution": "Add a durable unknown receipt entry before WorkRequested in the positive history and assert the ordering/unknown outcome explicitly."
    },
    {
      "id": "M25",
      "claim": "The composition acceptance claim says the fixture proves the Effect/static capability-handler composition, but the positive fixture only uses plain functions and Handlers; the only Effect import is a deliberately negative-only fixture.",
      "sideA": "plans/uta-refactor/spec/08-verification.md:75-78; plans/uta-refactor/spec/00-decision-register.md:39-45",
      "sideB": "plans/uta-refactor/spec/types/fixtures/positive/composition.ts:1-12; plans/uta-refactor/spec/types/fixtures/effect-boundary.ts:1-5",
      "resolution": "Either narrow the acceptance claim to the plain TypeScript handler proof or add a positive Effect composition fixture that exercises the declared residual-requirement guarantee."
    },
    {
      "id": "M26",
      "claim": "The public read catalogue requires generated, strict route response schemas, but ReadProjectionResponse<T> is only a structural type with no Zod schema/factory; several catalogue responses are unconstrained Serializable[]/maps.",
      "sideA": "plans/uta-refactor/spec/05-protocol-and-replacement.md:9,113,121-149,333-339",
      "sideB": "plans/uta-refactor/spec/types/wire/v2.ts:74-78,122-118",
      "resolution": "Define named route response schemas/decoders (including warning/error item shapes) and make OpenAPI generation consume those schemas; do not publish a generic raw Serializable wrapper as the route contract."
    },
    {
      "id": "M27",
      "claim": "D1's falsifier requires a reproducible representative replay at >=25k processed messages/s for 10 minutes with p99 <=5µs, but the verification runner only defines 5k/10k cases and gives no executable >=25k command or fixture.",
      "sideA": "plans/uta-refactor/spec/00-decision-register.md:25-35",
      "sideB": "plans/uta-refactor/spec/08-verification.md:239-294",
      "resolution": "Add the exact >=25k stress command/fixture and pass/fail output, or explicitly mark the falsifier as an unverified conditional rather than implying the current benchmark covers it."
    }
  ],
  "minors": [
    {
      "id": "m1",
      "claim": "The register describes negative fixtures using @ts-expect-error, while the type README and runner explicitly forbid it and require Expected TSxxxx comments plus diagnostic matching.",
      "sideA": "plans/uta-refactor/spec/00-decision-register.md:214",
      "sideB": "plans/uta-refactor/spec/types/README.md:173-184; plans/uta-refactor/spec/types/scripts/check-negatives.mjs:1-20",
      "resolution": "Adopt the implemented runner convention and replace the register wording with expected-diagnostic comments; keep one authoritative negative-fixture protocol."
    },
    {
      "id": "m2",
      "claim": "The README says the non-exhaustive fixture protects new capability/receipt/ledger variants, but the fixture only omits work.requested from the ledger switch.",
      "sideA": "plans/uta-refactor/spec/types/README.md:180-184",
      "sideB": "plans/uta-refactor/spec/types/fixtures/negative/non-exhaustive.ts:1-8",
      "resolution": "Narrow the claim to the actual work.requested omission or add separate negatives for capability and receipt variants."
    }
  ],
  "notes": [
    "The broad brand, envelope, and readiness findings are contract gaps in the canonical type package, not claims that the scoped package commands currently fail. Per the review contract, package typecheck/runtime pass does not discharge the spec-level contradictions.",
    "No files were edited and no formatters, linters, or project-wide tests were run."
  ]
}
```
