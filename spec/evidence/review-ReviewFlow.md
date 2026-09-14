# 验收评审最终产出：ReviewFlow

原始产物 `agent://ReviewFlow` 的逐字导出。

```
{
  "conclusion": "Do not accept the spec as implementation-ready. I found 32 cross-document/type contradictions: 8 blocker-level, 23 major-level, and 1 minor wording issue. Main has also received the complete findings by hub message.",
  "findings": [
    {
      "id": 1,
      "severity": "blocker",
      "claim": "Runtime-config ownership is internally contradictory even within the binding register.",
      "sideA": "plans/uta-refactor/spec/00-decision-register.md:133 says runtime config is UTA-owned and replaced atomically as one whole file.",
      "sideB": "plans/uta-refactor/spec/00-decision-register.md:139 keeps /api/trading/config/* Alice-owned and gives Alice GET/PUT /api/trading/config/runtime; the Alice-writer/UTA-reader rule is elaborated at 03-ledger-and-persistence.md:30,501 and 05-protocol-and-replacement.md:308,527, while 06-interaction-and-issues.md:528 and 07-security-operations.md:284,347 say UTA writes/owns it.",
      "resolution": "Choose Alice-owned writer and UTA reader, matching the route and elaborations; amend D5 and stale 06/07 text."
    },
    {
      "id": 2,
      "severity": "blocker",
      "claim": "IntentId allocation has two incompatible authorities.",
      "sideA": "plans/uta-refactor/spec/05-protocol-and-replacement.md:161,171,424 and types/wire/v2.ts:34-35 require a caller-supplied mandatory intentId.",
      "sideB": "plans/uta-refactor/spec/06-interaction-and-issues.md:58,125 says UTA allocates the immutable IntentId.",
      "resolution": "Producer/caller generates a stable IntentId, including for one-shot nested requests; UTA validates and records it. Update 06."
    },
    {
      "id": 3,
      "severity": "major",
      "claim": "ProposalId allocation is described as both client- and server-generated.",
      "sideA": "plans/uta-refactor/spec/05-protocol-and-replacement.md:165,425 and types/wire/v2.ts:95-98 make grouped proposalId a client-supplied stable identity.",
      "sideB": "plans/uta-refactor/spec/06-interaction-and-issues.md:125 says UTA allocates an optional ProposalId.",
      "resolution": "Caller generates proposalId for grouped proposals; UTA does not allocate it. Keep optional proposalId on individual intent membership."
    },
    {
      "id": 4,
      "severity": "major",
      "claim": "DecisionId allocation is contradictory.",
      "sideA": "plans/uta-refactor/spec/06-interaction-and-issues.md:61 calls DecisionId the caller idempotency key; 05-protocol-and-replacement.md:166,182,426 and types/issues.ts:6 and types/wire/v2.ts:38 require it.",
      "sideB": "plans/uta-refactor/spec/06-interaction-and-issues.md:125 says UTA allocates DecisionId.",
      "resolution": "Bridge/consumer generates and reuses DecisionId for retries; policy deterministic generation remains caller-side."
    },
    {
      "id": 5,
      "severity": "blocker",
      "claim": "Alice's decision DTO and UTA's decision DTO are conflated.",
      "sideA": "types/issues.ts:6 defines strict DecisionRequest as decisionId, action, scopeHash; 06-interaction-and-issues.md:768-778 says Alice sends that body and claims a shared strict three-field schema.",
      "sideB": "types/wire/v2.ts:38-39 defines strict AuthorizationDecisionRequest with an additional scope; 05-protocol-and-replacement.md:267-270 explicitly requires Alice to pass decisionId, action, scope, and scopeHash to UTA.",
      "resolution": "Keep the three-field Alice bridge DTO. Alice resolves authoritative scope and constructs the four-field UTA DTO; rewrite shared-schema wording and diagrams."
    },
    {
      "id": 6,
      "severity": "major",
      "claim": "IntentProposalResponse shape and status are described inconsistently.",
      "sideA": "types/wire/v2.ts:36-37 strictly permits only intentId, entryId, position, and status:'proposed'.",
      "sideB": "plans/uta-refactor/spec/06-interaction-and-issues.md:143 adds ProposalId?/scopeHash in the depicted response and 06:188,240 calls the response 'awaiting authorization', which is not a defined status.",
      "resolution": "Use the exact strict response/status proposed; obtain scopeHash and proposal membership through ledger/events. Treat awaiting authorization as UI/state prose only."
    },
    {
      "id": 7,
      "severity": "major",
      "claim": "Ordinary intent proposal HTTP status differs between protocol and verification.",
      "sideA": "plans/uta-refactor/spec/05-protocol-and-replacement.md:161 says new intent proposal is 201 and duplicate is 200.",
      "sideB": "plans/uta-refactor/spec/08-verification.md:145,163 says intent ingress is 202.",
      "resolution": "Standardize ordinary /intents on 201 for a new append and 200 for duplicate replay; reserve 202 for the explicitly asynchronous commands and update verification."
    },
    {
      "id": 8,
      "severity": "major",
      "claim": "Decision HTTP status differs for first and repeated requests.",
      "sideA": "plans/uta-refactor/spec/05-protocol-and-replacement.md:166 says first decision is 202 and duplicate is 200; 08-verification.md:146,157 also use 202.",
      "sideB": "plans/uta-refactor/spec/08-verification.md:155 says both first and repeated decisions are 200 (and 08:191 may be read as a blanket 200).",
      "resolution": "First decision request returns 202; same decisionId replay returns 200. Correct M-1 and any blanket statement."
    },
    {
      "id": 9,
      "severity": "blocker",
      "claim": "ProviderUnknown has incompatible HTTP mappings and an unclear error-body implication.",
      "sideA": "plans/uta-refactor/spec/01-architecture.md:119 maps ProviderUnknown to 503; 08-verification.md:185 P-3 expects 503; 05-protocol-and-replacement.md:83 maps ProviderTransportFailure to 503.",
      "sideB": "plans/uta-refactor/spec/05-protocol-and-replacement.md:85 maps ProviderUnknown to 202, while 05:68 says every non-2xx is ErrorEnvelope.",
      "resolution": "Map ProviderUnknown to 503, as architecture and verification require; 202 is an acknowledgement status, not an error-code mapping."
    },
    {
      "id": 10,
      "severity": "blocker",
      "claim": "x-uta-channel has a closed two-value vocabulary but generated route rows use two other values.",
      "sideA": "plans/uta-refactor/spec/04-provider-projections.md:232,366 permits only read-only and read-write, and 05-protocol-and-replacement.md:336 defines channel as that distinction.",
      "sideB": "plans/uta-refactor/spec/05-protocol-and-replacement.md:479-484 uses x-uta-channel:lifecycle and x-uta-channel:simulator.",
      "resolution": "Restrict x-uta-channel to read-only/read-write; classify lifecycle/simulator in x-uta-kind or operation metadata and fix generated rows."
    },
    {
      "id": 11,
      "severity": "blocker",
      "claim": "The canonical WorkRequested kind spelling differs.",
      "sideA": "types/issues.ts:9, 00-decision-register.md:162, 05-protocol-and-replacement.md:242, 06-interaction-and-issues.md:602,634, and 08-verification.md:152,353 use review.unknownOutcome.",
      "sideB": "plans/uta-refactor/spec/00-decision-register.md:115 says the bridge emits review.unknown-outcome.",
      "resolution": "Use review.unknownOutcome as the exact normative event/kind everywhere; allow hyphenated wording only in UI prose."
    },
    {
      "id": 12,
      "severity": "major",
      "claim": "Readiness writable-block reasons do not include the documented ReadinessUnavailable value.",
      "sideA": "types/readiness.ts:5 and types/wire/v2.ts:54 define a finite tagged blocked-reason set without ReadinessUnavailable; 05-protocol-and-replacement.md:128 and 07-security-operations.md:415 repeat it.",
      "sideB": "plans/uta-refactor/spec/06-interaction-and-issues.md:564 depicts writable=false {ReadinessUnavailable,freshness}.",
      "resolution": "Use a defined tagged reason such as FirstObservationRequired or TransportUnavailable for account-level blocking; reserve ReadinessUnavailable for global HTTP/error status."
    },
    {
      "id": 13,
      "severity": "minor",
      "claim": "Examples use boolean writable values while the type is a tagged union.",
      "sideA": "plans/uta-refactor/spec/00-decision-register.md:131, 01-architecture.md:179, 03-ledger-and-persistence.md:413, and 06-interaction-and-issues.md:553,561,564,939 show writable=true/false.",
      "sideB": "types/readiness.ts:5 models writable as {kind:'ok'} or {kind:'blocked',reason}.",
      "resolution": "Use the tagged representation in normative/JSON-like examples; retain boolean language only as informal prose."
    },
    {
      "id": 14,
      "severity": "blocker",
      "claim": "Shutdown drain semantics promise incompatible deadlines.",
      "sideA": "plans/uta-refactor/spec/00-decision-register.md:182 says an active attempt may reach provider deadline and then record receipt/unknown; 01-architecture.md:314-315 and 05-protocol-and-replacement.md:314 repeat provider-deadline behavior.",
      "sideB": "plans/uta-refactor/spec/07-security-operations.md:450,461-464 says draining waits only drainBudget (4s restart/7s shutdown), and 07:588 verifies that cap.",
      "resolution": "Define the effective shutdown deadline explicitly, preferably min(provider deadline, drainBudget), then classify unresolved attempts at the cap; amend D12 and all dependent text."
    },
    {
      "id": 15,
      "severity": "major",
      "claim": "The authentication exception for OpenAPI/readiness/health differs by document.",
      "sideA": "plans/uta-refactor/spec/05-protocol-and-replacement.md:57,119-121 permits bearer plus internal system without an explicit business Principal for readiness, OpenAPI, and health; 07-security-operations.md:180 says probes ignore Principal.",
      "sideB": "plans/uta-refactor/spec/07-security-operations.md:158 lists only health/readiness as grammar exceptions and otherwise requires bearer plus Principal, which includes OpenAPI.",
      "resolution": "Choose one exact exception set. Conservative resolution: health/readiness bearer-only, OpenAPI requires bearer plus Principal; remove OpenAPI from 05's exception."
    },
    {
      "id": 16,
      "severity": "major",
      "claim": "Readiness has a provider-read timeout status in one place but is defined as a local snapshot elsewhere.",
      "sideA": "plans/uta-refactor/spec/05-protocol-and-replacement.md:119 lists 504 Timeout(providerRead) for readiness.",
      "sideB": "plans/uta-refactor/spec/07-security-operations.md:420,425-426 defines a 1-second local readiness snapshot with 200/503 only and no provider read; 01-architecture.md describes current-state readiness.",
      "resolution": "Make readiness purely local: remove 504 providerRead and use 503 ReadinessUnavailable only when the local snapshot cannot be produced."
    },
    {
      "id": 17,
      "severity": "major",
      "claim": "Long-poll timeout behavior is contradictory.",
      "sideA": "plans/uta-refactor/spec/06-interaction-and-issues.md:79 says timeout may return Timeout/empty items.",
      "sideB": "plans/uta-refactor/spec/05-protocol-and-replacement.md:120,202 says exact deadline empty batch is legal 200 and Timeout means server exceeded the deadline; 01-architecture.md:222 says timeout returns readiness and cursor.",
      "resolution": "At the exact wait deadline return 200 EventsResponse with empty items, unchanged nextCursors, and readiness; reserve Timeout for server overrun and remove the alternative wording."
    },
    {
      "id": 18,
      "severity": "major",
      "claim": "Desk poll interval can exceed EventsQuery.wait's schema/route maximum.",
      "sideA": "plans/uta-refactor/spec/06-interaction-and-issues.md:588 allows pollIntervalMs up to 300000 and 06:706 says the bridge uses it as wait.",
      "sideB": "types/wire/v2.ts:59 caps wait at 25000; 05-protocol-and-replacement.md:106,120 repeats that cap.",
      "resolution": "Bridge sends min(pollIntervalMs,25000), then sleeps/continues the requested cadence after the long-poll; alternatively reduce the settings maximum. Preserve the larger cadence range unless requirement says otherwise."
    },
    {
      "id": 19,
      "severity": "major",
      "claim": "EventItem correlation is placed in different locations.",
      "sideA": "types/wire/v2.ts:61-64 strictly defines EventItem with only source, accountId, position, entry; types/README.md:118 confirms the shape.",
      "sideB": "plans/uta-refactor/spec/06-interaction-and-issues.md:76 directs consumers to EventItem.correlation, while 05-protocol-and-replacement.md:192-193 says correlation is inside entry payload.",
      "resolution": "Document correlation as EventItem.entry.payload.correlation (or parsed ledger envelope); do not add a top-level field to the strict wire type."
    },
    {
      "id": 20,
      "severity": "major",
      "claim": "WorkRequested observation asOf is modeled both nested and top-level.",
      "sideA": "types/issues.ts:8-10 contains freshness only, and types/time.ts nests asOf inside fresh/stale Freshness variants.",
      "sideB": "plans/uta-refactor/spec/00-decision-register.md:162, 06-interaction-and-issues.md:622, and 05-protocol-and-replacement.md:247 require/list separate asOf and freshness fields.",
      "resolution": "Use nested freshness.asOf as the wire/domain contract and rewrite docs; do not add a redundant top-level asOf."
    },
    {
      "id": 21,
      "severity": "major",
      "claim": "Empty admissibleDecisions is both valid for notification work and a parser error.",
      "sideA": "types/issues.ts:8-10 permits an unbounded readonly DecisionAction[] and optional intent/idempotency for watch/news; 06-interaction-and-issues.md:618 allows pure news with no intent.",
      "sideB": "plans/uta-refactor/spec/06-interaction-and-issues.md:620 says unknown or empty admissibleDecisions is a parser error for all kinds.",
      "resolution": "Make validation kind-specific: review/authorization/reconcile kinds require nonempty actions, while watch/news notification kinds may be empty."
    },
    {
      "id": 22,
      "severity": "blocker",
      "claim": "Decision failures have incompatible DTO contracts.",
      "sideA": "types/wire/v2.ts:40-41 defines AuthorizationDecisionResponse as success-only recorded; types/issues.ts:7 permits only a narrow rejected DecisionResponse; 05-protocol-and-replacement.md:68,177 says non-2xx errors are ErrorEnvelope.",
      "sideB": "plans/uta-refactor/spec/06-interaction-and-issues.md:111,789 claims AuthorizationDecisionResponse.rejected and broad codes including Unauthorized, CapabilityUnsupported, and ReadinessUnavailable.",
      "resolution": "Use ErrorEnvelope for every non-2xx decision failure and keep AuthorizationDecisionResponse success-only; remove rejected-response claims from 06."
    },
    {
      "id": 23,
      "severity": "major",
      "claim": "Persisted attempt key has ProviderKey versus ProviderEnvelope types.",
      "sideA": "plans/uta-refactor/spec/00-decision-register.md:94 says persisted attempt payload key is ProviderKey<P>; in-memory RecoveryAttempt uses ProviderKey<P> at 04-provider-projections.md:623,690.",
      "sideB": "types/ledger/entries.ts:13 declares persisted attempt.started.key as ProviderEnvelope; D3's storage/wire existential boundary is ProviderEnvelope at 00:79.",
      "resolution": "Persist/wire ProviderEnvelope and re-associate through ProjectionRegistry; retain generic ProviderKey<P> only in memory; amend 00:94."
    },
    {
      "id": 24,
      "severity": "blocker",
      "claim": "Simulator action response cannot satisfy the declared IntentProposalResponse contract and conflicts with no-ledger semantics.",
      "sideA": "plans/uta-refactor/spec/05-protocol-and-replacement.md:107,278,285 maps SimulatorActionRequest to IntentProposalResponse; that response requires intentId/entryId/position at types/wire/v2.ts:36-37, but SimulatorActionRequest variants at types/wire/v2.ts:109-118 have no intentId.",
      "sideB": "05-protocol-and-replacement.md:183 requires a durable ledger position for IntentProposalResponse, while 05:284 says price simulation MUST NOT mutate Mock state or ledger.",
      "resolution": "For state-only simulator actions return CommandResponse{kind:'completed',requestId} (or define a separate state-action result), with no ledger position; update status/catalogue. If IntentProposalResponse is desired, add explicit simulator intent/ledger semantics instead."
    },
    {
      "id": 25,
      "severity": "major",
      "claim": "Verification treats configRevision as an input field absent from the strict request DTO.",
      "sideA": "types/wire/v2.ts:34 omits configRevision; 05-protocol-and-replacement.md:171 omits it from mandatory request fields and says ledger configRevision is server/domain-filled.",
      "sideB": "plans/uta-refactor/spec/08-verification.md:127 calls configRevision legal HTTP input.",
      "resolution": "Describe configRevision in verification as an effective server-resolved snapshot/precondition, not an HTTP input; adding it to the DTO would require coordinated changes."
    },
    {
      "id": 26,
      "severity": "major",
      "claim": "The 49-registration count does not match the listed route families.",
      "sideA": "plans/uta-refactor/spec/00-decision-register.md:139 says there are 49 /api/trading/* and /api/simulator/* registrations; 05-protocol-and-replacement.md:348 says all 49.",
      "sideB": "05:352 lists 39 trading and 05:395 lists 9 simulator (48 total), while H01 health is separate at 05:409-415; 08-verification.md:9 calls the old system 49 UTA HTTP endpoints.",
      "resolution": "State 48 legacy /api/trading|simulator routes plus separate H01 /__uta/health equals 49 total mapping rows, and correct headings/counts."
    },
    {
      "id": 27,
      "severity": "major",
      "claim": "The legacy quote route is written with and without its mounted prefix.",
      "sideA": "plans/uta-refactor/spec/00-decision-register.md:139 and 08-verification.md:360 use GET /uta/:id/quote/:symbol.",
      "sideB": "plans/uta-refactor/spec/05-protocol-and-replacement.md:368 uses GET /api/trading/uta/:id/quote/:symbol; source mounts /api/trading at services/uta/src/main.ts:163-170 and handler is /uta/:id/quote/:symbol at services/uta/src/http/routes-trading.ts:303-307.",
      "resolution": "Use the externally mounted full path /api/trading/uta/:id/quote/:symbol consistently."
    },
    {
      "id": 28,
      "severity": "major",
      "claim": "Config change flow says both conditional hot reload and unconditional restart.",
      "sideA": "plans/uta-refactor/spec/03-ledger-and-persistence.md:535 and 05-protocol-and-replacement.md:317,527 allow hot reload for policy/rule/recovery/capacity/retention; only accounts/projection/Pack changes require restart flag.",
      "sideB": "plans/uta-refactor/spec/06-interaction-and-issues.md:524-532 is headed Config change -> restart and says durable config write is followed by restart flag; 06:561 describes a new process generally.",
      "resolution": "Make the sequence conditional: runtime policy/rule/recovery/capacity/retention replacement hot-reloads; account/projection/Pack changes request restart and change startedAt."
    },
    {
      "id": 29,
      "severity": "major",
      "claim": "Policy schema permits system self-approval while the role matrix forbids system business approval.",
      "sideA": "types/policy.ts:12 includes system in selfApprove's allowed principal-role values; 00-decision-register.md:111 and 05-protocol-and-replacement.md:167 define self-approval as automatic authorization control.",
      "sideB": "plans/uta-refactor/spec/06-interaction-and-issues.md:94 says system MUST NOT make human/agent/connector business approval.",
      "resolution": "Remove system from selfApprove configuration, or explicitly reject system in policy validation while retaining system for lifecycle operations."
    },
    {
      "id": 30,
      "severity": "major",
      "claim": "Reject reversal is described as unconditional despite the ledger legality rule.",
      "sideA": "plans/uta-refactor/spec/06-interaction-and-issues.md:829 says every consumer reject MUST append reversal.",
      "sideB": "plans/uta-refactor/spec/00-decision-register.md:99 and types/ledger/fold.ts:103-113 allow reversal only before attempt.started; 06:849 explicitly says post-attempt reject must not append reversal.",
      "resolution": "Require reversal only when the target has no attempt.started; after an attempt, use structured recovery/compensation and no reversal."
    },
    {
      "id": 31,
      "severity": "major",
      "claim": "Readiness config-invalid error shape differs between type and prose.",
      "sideA": "types/readiness.ts:5 and types/wire/v2.ts:54 define config-invalid.errors as string[].",
      "sideB": "plans/uta-refactor/spec/07-security-operations.md:357-359,418,426 and 06-interaction-and-issues.md:816 require structured path/code/safe-message leaves.",
      "resolution": "Either change the DTO to a typed error-leaf object, or keep string[] and rewrite the docs to require safe strings only. The minimal resolution is to remove the path/code decoding claim."
    },
    {
      "id": 32,
      "severity": "major",
      "claim": "The migration checklist promises a new ledger-entry detail route that is not mapped or typed.",
      "sideA": "plans/uta-refactor/spec/05-protocol-and-replacement.md:448 says tradingShow needs an archive selector endpoint plus new ledger entry detail.",
      "sideB": "The mapping has archive selector T25 only at 05:379 and ledger list T22 at 05:376; there is no entry-detail route or EntryDetailResponse in types/wire/v2.ts.",
      "resolution": "Remove 'new ledger entry detail' from the tradingShow checklist, or add an explicit route and DTO; remove it unless the detail requirement is intentional."
    }
  ],
  "consistentAreas": [
    "Event query/cursor fundamentals agree across 00-decision-register.md:155-157, 01-architecture.md:222-224, 05-protocol-and-replacement.md:120,191-208,568, types/wire/v2.ts:57-64, and types/README.md:117-118: per-account positions, omitted cursor=head, explicit 0 replay, wait/limit bounds, readiness in response, no SSE/outbox, and duplicate/cursor safety.",
    "Readiness dimensions, process states, finite writable reasons, and first-observation gate are aligned across types/readiness.ts:5, types/wire/v2.ts:54-55, 05-protocol-and-replacement.md:128, and 07-security-operations.md:390-418,425-428; findings 12, 13, 16, and 31 are the exceptions.",
    "Health shape is aligned across types/readiness.ts:7, 00-decision-register.md:137,181, 05-protocol-and-replacement.md:413-415, and 07-security-operations.md:370-378.",
    "Desk settings/link/assignee/cursor behavior is largely aligned across types/issues.ts:12-14, 05-protocol-and-replacement.md:256-258, and 06-interaction-and-issues.md:576-710,720+.",
    "Runtime config route and complete DTO are aligned at 03-ledger-and-persistence.md:501,533-535, 05-protocol-and-replacement.md:305-310,423,527, 07-security-operations.md:283-286,501, and 08-verification.md:351,566 once ownership/reload language is corrected.",
    "Principal header grammar, recovery result algebra, archive migration/no-authority behavior, config digest/schema constraints, D4 attempt/receipt/observation lifecycle, provider-overlay requirements, and simulator scope/no live provider are otherwise consistent; the listed contradictions are the exceptions."
  ]
}
```
