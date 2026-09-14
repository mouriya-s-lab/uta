import { deriveConfigRevision, parseAccountId, parseAsOf, parseAttemptNo, parseConfigRevision, parseDecisionId, parseEntryId, parseEntryPosition, parseIdempotencyKey, parseInstant, parseIntentHash, parseIntentId, parseOperationKind, parseProviderErrorCode, parseProviderId, parseProjectionVersion, parseRequestId, parseScopeHash, parseSha256, parseSource, sha256, coreOperationKind } from '../../ids.ts';
import { parseDuration } from '../../time.ts';
import { parseDecimalString } from '../../money.ts';
import { unwrap } from '../../result.ts';
import type { LedgerPayloads, EntryOf, LedgerEntry, ReasonTree } from '../../ledger/entries.ts';
import { parseLedgerEntry } from '../../ledger/entries.ts';
import { intentOutcome, orderProjection, validateReversal } from '../../ledger/fold.ts';
import { parseProviderKey, parseProviderOrderRef } from '../../provider/indexed.ts';
import type { OperationEnvelope, KeyEnvelope, ReceiptEnvelope, ObservationEnvelope, Observation, ProviderKey, ProviderOrderRef, ProviderTypes } from '../../provider/indexed.ts';
import { nextAction, verifyReadByKey, recoveryProbe } from '../../provider/recovery.ts';
import type { RecoveryAttempt, RecoveryResult } from '../../provider/recovery.ts';
import type { WriteContract } from '../../provider/declaration.ts';
import { ledgerFrameSchema } from '../../ledger/store.ts';
import { parseRuleConfig, parseUtaRuntimeConfig, deriveRuntimeConfigDigest } from '../../policy.ts';
import { authorizationDecisionRequestSchema, eventsQuerySchema, executeSimulatorActionRequestSchema, reasonTreeSchema, serializableSchema } from '../../wire/v2.ts';
import { alpaca, longbridge, okx, ibkrCP, bybit } from './providers.ts';
import { Decimal } from 'decimal.js';
import { coreConsumerRule } from './composition.ts';

function check(condition: boolean, label: string): void {
  if (!condition) throw new Error(label);
}

const now = (n: number) => unwrap(parseInstant(n));
const ms = (n: number) => unwrap(parseDuration(n));
const dec = (s: string) => unwrap(parseDecimalString(s));
const accountId = unwrap(parseAccountId('paper'));
const intentId = unwrap(parseIntentId('intent-1'));
const providerId = unwrap(parseProviderId('model'));
const projectionVersion = unwrap(parseProjectionVersion('2'));
const source = unwrap(parseSource('model'));
const operationKind = unwrap(parseOperationKind('order.place'));
const scopeHash = unwrap(parseScopeHash('b'.repeat(64)));
const intentHash = unwrap(parseIntentHash('c'.repeat(64)));
const replacementIntentHash = unwrap(parseIntentHash('d'.repeat(64)));
const replacementScopeHash = unwrap(parseScopeHash('e'.repeat(64)));
const runtimeConfig = {
  schemaVersion: 1,
  fx: {maxAge: 60000},
  policies: [],
  rules: [],
  projections: [],
  recovery: [],
  capacity: {
    maxProviderConnections: 100,
    maxConcurrentReads: 4,
    perAccountWriteQueueCapacity: 16,
    fanoutQueueCapacity: 512,
    ledgerSegmentMaxBytes: 16777216,
  },
  retention: {
    snapshotMaxAge: 1000,
    snapshotMaxBytes: 1000,
    diagnosticLogMaxBytes: 1000,
    quarantineMaxBytes: 1000,
  },
  updatedAt: 1,
};
const runtimeDigest = await deriveRuntimeConfigDigest(unwrap(parseUtaRuntimeConfig(runtimeConfig)));
const accountDigest = await sha256(JSON.stringify({accountId,providerId,projectionVersion}));
const configRevision = await deriveConfigRevision({
  runtimeConfigDigest: runtimeDigest,
  accountConfigDigest: accountDigest,
  projectionVersion,
});
check(await deriveRuntimeConfigDigest(unwrap(parseUtaRuntimeConfig({...runtimeConfig,updatedAt:2})))===runtimeDigest,'updatedAt does not change semantic config digest');
check(await deriveRuntimeConfigDigest(unwrap(parseUtaRuntimeConfig({...runtimeConfig,capacity:{...runtimeConfig.capacity,maxConcurrentReads:5}})))!==runtimeDigest,'Semantic config change changes runtime digest');
check(parseConfigRevision('revision-1').kind==='error','Revision parser rejects non-hash values');
check(parseUtaRuntimeConfig({...runtimeConfig,mode:'readonly'}).kind==='ok','Configured readonly mode is accepted');
check(parseUtaRuntimeConfig({...runtimeConfig,mode:'invalid'}).kind==='error','Unknown configured mode is rejected');
check(await deriveRuntimeConfigDigest(unwrap(parseUtaRuntimeConfig({...runtimeConfig,mode:'pro'})))!==runtimeDigest,'Mode change changes semantic runtime config digest');
check(parseUtaRuntimeConfig({...runtimeConfig,fx:{maxAge:0}}).kind==='error','FX freshness age must be a positive duration');
check(await deriveRuntimeConfigDigest(unwrap(parseUtaRuntimeConfig({...runtimeConfig,fx:{maxAge:120000}})))!==runtimeDigest,'FX freshness policy changes semantic digest');
const why: ReasonTree = { kind: 'Because', code: 'Fixture', message: 'Explicit model scenario' };
const provider: ProviderTypes = {
  identity: providerId,
  projectionVersion,
  operation: { kind: 'order.place', quantity: '100' },
  receipt: { kind: 'accepted', providerRef: 'order-1' },
  observation: { kind: 'order', filledQty: '0' },
};
const key = unwrap(parseProviderKey('same-key', provider));
const providerRef = unwrap(parseProviderOrderRef('order-1', provider));
const operationEnvelope: OperationEnvelope = {
  role: 'operation',
  providerId,
  projectionVersion,
  kind: operationKind,
  payload: { kind: 'order.place', quantity: '100' },
};
const keyEnvelope: KeyEnvelope = {
  role: 'key',
  providerId,
  projectionVersion,
  kind: operationKind,
  payload: key,
};
const acceptedReceiptEnvelope: ReceiptEnvelope = {
  role: 'receipt',
  providerId,
  projectionVersion,
  kind: operationKind,
  payload: {
    kind: 'accepted',
    providerRef,
    provenance: { kind: 'original' },
    raw: { requestId: 'r-1' },
  },
};
const rejectedReceiptEnvelope: ReceiptEnvelope = {
  role: 'receipt',
  providerId,
  projectionVersion,
  kind: operationKind,
  payload: {
    kind: 'rejected',
    code: unwrap(parseProviderErrorCode('NoFunds')),
    requestId: unwrap(parseRequestId('r-2')),
    raw: { code: 'NoFunds' },
  },
};
const processRestartReceiptEnvelope: ReceiptEnvelope = {
  role: 'receipt',
  providerId,
  projectionVersion,
  kind: operationKind,
  payload: { kind: 'unknown', cause: 'processRestart' },
};
const attemptNo = unwrap(parseAttemptNo(1));

function providerRefForPayload<K extends keyof LedgerPayloads>(
  payload: LedgerPayloads[K],
): ProviderOrderRef<ProviderTypes> | undefined {
  if ('receipt' in payload && payload.receipt.role === 'receipt' && payload.receipt.payload.kind === 'accepted') {
    return payload.receipt.payload.providerRef;
  }
  if ('observation' in payload && payload.observation.role === 'observation') {
    return payload.observation.payload.providerRef;
  }
  return undefined;
}

function entry<K extends keyof LedgerPayloads>(
  kind: K,
  payload: LedgerPayloads[K],
  position: number,
): EntryOf<K> {
  const payloadIntentId =
    'intentId' in payload
      ? payload.intentId
      : kind === 'observation.recorded'
        ? intentId
        : undefined;
  const payloadAttemptNo =
    'attemptNo' in payload
      ? payload.attemptNo
      : kind === 'observation.recorded'
        ? attemptNo
        : undefined;
  return {
    schemaVersion: 2,
    accountId,
    entryId: unwrap(parseEntryId(`e-${position}`)),
    position: unwrap(parseEntryPosition(position)),
    occurredAt: now(position),
    recordedAt: now(position),
    source,
    idempotencyKey: unwrap(parseIdempotencyKey(`event-${position}`)),
    why,
    correlation: {
      intentId: payloadIntentId,
      attemptNo: payloadAttemptNo,
      providerRef: providerRefForPayload(payload),
    },
    kind,
    payload,
  };
}

function observation(
  asOf: number,
  filledQty: string,
  position: number,
  stateKind: 'open' | 'filled' | 'partiallyFilled' = 'filled',
): EntryOf<'observation.recorded'> {
  const observationEnvelope: ObservationEnvelope = {
    role: 'observation',
    providerId,
    projectionVersion,
    kind: operationKind,
    payload: {
      asOf: unwrap(parseAsOf(asOf)),
      source: providerId,
      providerRef,
      state: { kind: stateKind, filledQty: dec(filledQty) },
      raw: { asOf, filledQty },
    },
  };
  return entry('observation.recorded', { observation: observationEnvelope }, position);
}

const proposed = entry(
  'intent.proposed',
  {
    intentId,
    intentHash,
    principal: { kind: 'human', sessionId: 'session' },
    origin: { kind: 'direct' },
    scope: { accountId, operationKind, operation: operationEnvelope },
    expiresAt: now(1000),
    configRevision,
  },
  1,
);
const approved = entry(
  'authorization.decided',
  {
    decisionId: unwrap(parseDecisionId('approve-1')),
    scope: { kind: 'intents', intentIds: [intentId] },
    scopeHash,
    action: 'approve',
    principal: { kind: 'human', sessionId: 'session' },
    policyResult: { kind: 'binding' },
  },
  2,
);
const started = entry(
  'attempt.started',
  { intentId, attemptNo, key: keyEnvelope, configRevision, deadline: now(20) },
  3,
);
const attempt: RecoveryAttempt<ProviderTypes> = {
  intentId,
  attemptNo,
  key,
  startedAt: now(3),
  unknownSince: now(20),
  maxUnknownDuration: ms(100),
  payloadHash: intentHash,
  terminal: 'unverified',
};
const contract: WriteContract = {
  idempotency: { kind: 'none' },
  readByKey: {
    kind: 'byClientKey',
    operation: 'lookup',
    coverage: 'unverified',
    historyWindow: 'unverified',
    ambiguity: 'unique',
  },
};
const unknown: RecoveryResult<ProviderTypes> = {
  kind: 'stillUnknown',
  nextCheckAfter: now(120),
  reason: 'ProviderUnavailable',
};
const processRestartReceipt = entry(
  'receipt.recorded',
  { intentId, attemptNo, receipt: processRestartReceiptEnvelope },
  4,
);
const recoveryStillUnknown = entry(
  'recovery.resolved',
  { intentId, attemptNo, result: unknown },
  5,
);
const historyA: LedgerEntry[] = [proposed, approved, started, processRestartReceipt, recoveryStillUnknown];
check(
  processRestartReceipt.payload.receipt.payload.kind === 'unknown' &&
    processRestartReceipt.payload.receipt.payload.cause === 'processRestart' &&
    processRestartReceipt.position < recoveryStillUnknown.position,
  'A restart must durably record processRestart unknown before recovery',
);
check(intentOutcome(intentId, historyA) === 'unknown', 'A restart without receipt stays unknown');
check(
  nextAction(contract, unknown, attempt, now(119)).kind === 'scheduleRecheck',
  'A before budget reschedules',
);
check(
  nextAction(contract, unknown, attempt, now(120)).kind === 'awaitingReview',
  'A budget boundary goes to review',
);
historyA.push(
  entry(
    'work.requested',
    {
      kind: 'review.unknownOutcome',
      requestId: unwrap(parseRequestId('review-1')),
      accountId,
      causal: { entryId: started.entryId, position: started.position, why },
      authority: 'binding',
      admissibleDecisions: ['reject'],
      scopeHash,
      intentHash,
      freshness: { kind: 'missing', reason: 'Crash recovery' },
      providerId,
      projectionVersion,
      operation: operationEnvelope,
      what: 'Unknown outcome — review',
      expiresAt: now(200),
      intentId,
      idempotencyKey: unwrap(parseIdempotencyKey('same-key')),
    },
    6,
  ),
);
check(
  intentOutcome(intentId, historyA) === 'awaitingReview',
  'A bridge input produces awaitingReview',
);
console.log('PASS A: crash, processRestart unknown, bounded stillUnknown, review.unknownOutcome, awaitingReview');

const accepted = entry(
  'receipt.recorded',
  { intentId, attemptNo, receipt: acceptedReceiptEnvelope },
  4,
);
const historyB: LedgerEntry[] = [
  proposed,
  approved,
  started,
  accepted,
  observation(10, '0', 5, 'open'),
  observation(30, '100', 6),
  observation(20, '40', 7, 'partiallyFilled'),
  observation(30, '100', 8),
];
const order = orderProjection(providerRef, historyB);
check(
  order.kind === 'observed' &&
    order.state.filledQty === '100' &&
    order.position === 6 &&
    !order.observationConflict,
  'B late/duplicate observations cannot regress or duplicate effect',
);
check(intentOutcome(intentId, historyB) === 'filled', 'B intent filled');
const conflict = orderProjection(providerRef, [
  ...historyB,
  observation(30, '90', 9, 'partiallyFilled'),
]);
check(
  conflict.kind === 'observed' &&
    conflict.observationConflict &&
    conflict.position === 9,
  'Equal-asOf conflicting observation keeps later position with flag',
);
const sameAsOf = observation(30, '100', 9);
const reordered = orderProjection(providerRef, [
  ...historyB,
  {
    ...sameAsOf,
    payload: {
      ...sameAsOf.payload,
      observation: {
        ...sameAsOf.payload.observation,
        payload: {
          ...sameAsOf.payload.observation.payload,
          state: { filledQty: '100', kind: 'filled' },
        },
      },
    },
  },
]);
check(
  reordered.kind === 'observed' &&
    reordered.position === 6 &&
    !reordered.observationConflict,
  'Object field order is not an observation conflict',
);
const providerDetail = orderProjection(providerRef, [
  ...historyB,
  {
    ...sameAsOf,
    payload: {
      ...sameAsOf.payload,
      observation: {
        ...sameAsOf.payload.observation,
        payload: {
          ...sameAsOf.payload.observation.payload,
          state: { kind: 'filled', filledQty: '100', venueDetail: 'corrected' },
        },
      },
    },
  },
]);
check(
  providerDetail.kind === 'observed' &&
    providerDetail.observationConflict &&
    providerDetail.position === 9,
  'Provider-local state detail is preserved and participates in conflict detection',
);
console.log('PASS B: t1/t3/t2/t3 replay = filled 100; equal-asOf conflict flagged');

const rejected = entry(
  'receipt.recorded',
  { intentId, attemptNo, receipt: rejectedReceiptEnvelope },
  4,
);
const compensationId = unwrap(parseIntentId('compensation'));
const compensation = entry(
  'intent.proposed',
  { ...proposed.payload, intentId: compensationId, intentHash: replacementIntentHash },
  5,
);
const compensationApproval = entry(
  'authorization.decided',
  {
    ...approved.payload,
    decisionId: unwrap(parseDecisionId('approve-2')),
    scope: { kind: 'intents', intentIds: [compensationId] },
    scopeHash: replacementScopeHash,
  },
  6,
);
const compensationAttempt = entry(
  'attempt.started',
  { ...started.payload, intentId: compensationId },
  7,
);
const compensationReceipt = entry(
  'receipt.recorded',
  { ...accepted.payload, intentId: compensationId },
  8,
);
const historyC: LedgerEntry[] = [
  proposed,
  approved,
  started,
  rejected,
  compensation,
  compensationApproval,
  compensationAttempt,
  compensationReceipt,
];
check(intentOutcome(intentId, historyC) === 'rejectedByProvider', 'C original rejection terminal');
check(
  intentOutcome(compensationId, historyC) === 'accepted',
  'C compensation is separate accepted intent',
);
const refusal = validateReversal(compensation.entryId, historyC);
check(
  refusal.kind === 'rejected' && refusal.code === 'ReversalTargetAlreadyExecuted',
  'C late risk reversal refused',
);
console.log('PASS C: rejectedByProvider; compensation accepted; late reversal refused');

const verified = unwrap(
  verifyReadByKey({
    kind: 'byClientKey',
    operation: 'lookup',
    coverage: 'openAndHistory',
    historyWindow: ms(1000),
    ambiguity: 'unique',
  }),
);
const absent: RecoveryResult<ProviderTypes> = {
  kind: 'confirmedAbsent',
  proof: {
    kind: 'keyedLookupMiss',
    contract: verified,
    coverage: 'openAndHistory',
    window: verified.historyWindow,
    attemptTime: attempt.startedAt,
    checkedAt: now(30),
  },
  validUntil: now(100),
};
const safe: WriteContract = {
  idempotency: {
    kind: 'cachedReplay',
    keyField: 'key',
    scope: 'account',
    retention: ms(600000),
  },
  readByKey: verified,
};
const retry = nextAction(safe, absent, attempt, now(40));
check(retry.kind === 'retrySameKey' && retry.key === key, 'Verified absence allows same-key only');
check(
  nextAction(safe, absent, attempt, now(100)).kind === 'awaitingReview',
  'Expired proof forbids retry',
);
check(
  verifyReadByKey({
    kind: 'byClientKey',
    operation: 'lookup',
    coverage: 'openOnly',
    historyWindow: ms(1000),
    ambiguity: 'unique',
  }).kind === 'error',
  'Open-only miss cannot prove terminal absence',
);
for (const declaration of [alpaca, longbridge, okx, ibkrCP, bybit]) {
  for (const venue of Object.values(declaration.venues)) {
    for (const capability of Object.values(venue)) {
      if (capability.direction === 'write') {
        check(
          verifyReadByKey(capability.contract.readByKey).kind === 'error',
          'Evidence rows cannot claim verified absence',
        );
      }
    }
  }
}
check(parseRuleConfig({ kind: 'maxNotional', limit: NaN, currency: 'USD' }).kind === 'error', 'NaN config rejected');
check(parseRuleConfig({ kind: 'allowedInstruments', instruments: [] }).kind === 'error', 'Empty set rejected');
check(parseRuleConfig({ kind: 'allowedInstruments', instruments: [' '] }).kind === 'error', 'Blank set rejected');
check(parseRuleConfig({ kind: 'unknown' }).kind === 'error', 'Unknown rule rejected');
check(
  parseDecimalString(1).kind === 'error' &&
    parseDecimalString('1.00').kind === 'error' &&
    parseDecimalString('NaN').kind === 'error',
  'Wire money rejects number/noncanonical/nonfinite',
);
check(parseAccountId('../escape').kind === 'error', 'Traversal rejected');
check(
  !authorizationDecisionRequestSchema.safeParse({
    action: 'approve',
    scope: { kind: 'intents', intentIds: ['intent-1'] },
    scopeHash,
  }).success,
  'Decision parser requires id',
);
check(
  !authorizationDecisionRequestSchema.safeParse({
    decisionId: 'decision-1',
    action: 'approve',
    scope: { kind: 'intents', intentIds: ['intent-1'] },
    scopeHash,
    principal: { kind: 'system' },
  }).success,
  'Body cannot forge stamped principal',
);
check(
  eventsQuerySchema.parse({}).limit === 500 &&
    eventsQuerySchema.parse({}).cursors === undefined,
  'Event omission preserves from-head distinction',
);
check(
  !eventsQuerySchema.safeParse({ wait: 25001 }).success,
  'Long poll bounded',
);
check(
  executeSimulatorActionRequestSchema.safeParse({
    action: 'tick',
    requestId: 'r',
    nativeKey: 'BTC',
    deltaPercent: '1',
  }).success,
  'Simulator relative tick preserved',
);
check(
  !executeSimulatorActionRequestSchema.safeParse({
    action: 'mark',
    requestId: 'r',
    nativeKey: 'BTC',
    price: 1,
  }).success,
  'Simulator decimal money only',
);
check(
  reasonTreeSchema.safeParse({
    kind: 'RuleRejected',
    ruleId: 'parent',
    children: [{ kind: 'UnknownKind', operationKind: 'custom.new' }],
  }).success,
  'Recursive constructors preserved',
);
check(!serializableSchema.safeParse(Infinity).success, 'ABI rejects nonfinite values');
check(!serializableSchema.safeParse(new Decimal('1')).success, 'ABI rejects real Decimal class instance');
check(!serializableSchema.safeParse({ nested: new Date() }).success, 'ABI rejects nested class instance');

const latest: WriteContract = {
  idempotency: {
    kind: 'uniqueKey',
    keyField: 'key',
    scope: 'account',
    window: 'untilTerminal',
    reuseAfterTerminal: true,
    duplicateResponse: 'rejectsWithCode',
  },
  readByKey: {
    kind: 'byClientKey',
    operation: 'lookup',
    coverage: 'openAndHistory',
    historyWindow: 'unverified',
    ambiguity: 'latestMatch',
  },
};
const foundObservation: Observation<ProviderTypes> = {
  asOf: unwrap(parseAsOf(40)),
  source: providerId,
  providerRef,
  state: { kind: 'open', filledQty: '0' },
  raw: { providerRef },
};
const found: Extract<RecoveryResult<ProviderTypes>, { kind: 'found' }> = {
  kind: 'found',
  receipt: {
    kind: 'accepted',
    providerRef,
    provenance: { kind: 'recovered', via: 'keyedRead' },
    raw: { providerRef },
  },
  observation: foundObservation,
  correlation: { kind: 'unverified' },
};
check(
  nextAction(latest, found, attempt, now(40)).kind === 'awaitingReview',
  'Latest-match uncorrelated result never authorizes continuation',
);
check(
  nextAction(
    latest,
    {
      ...found,
      correlation: { kind: 'payloadMatch', echoedKey: key, payloadHash: intentHash },
    },
    attempt,
    now(40),
  ).kind === 'recordFound',
  'Matching echo plus payload hash upgrades keyed recovery',
);
check(
  nextAction(contract, absent, attempt, now(40)).kind === 'awaitingReview',
  'Absence proof cannot be transplanted onto unknown declaration',
);
const replayOnly: WriteContract = {
  idempotency: {
    kind: 'cachedReplay',
    keyField: 'client_request_id',
    scope: 'account',
    retention: ms(600000),
  },
  readByKey: { kind: 'byProviderRef', coverage: 'unverified', historyWindow: 'unverified' },
};
check(
  recoveryProbe(replayOnly, attempt, now(40)).kind === 'replaySameKey',
  'Longbridge replay is recovery, not new attempt',
);
check(
  recoveryProbe(
    {
      ...replayOnly,
      idempotency: {
        kind: 'uniqueKey',
        keyField: 'key',
        scope: 'account',
        window: 'unverified',
        reuseAfterTerminal: 'unverified',
        duplicateResponse: 'rejectsWithCode',
      },
    },
    attempt,
    now(40),
  ).kind === 'awaitingReview',
  'Duplicate rejection contract does not permit blind duplicate probe',
);

const recoveredReceiptEnvelope: ReceiptEnvelope = {
  role: 'receipt',
  providerId,
  projectionVersion,
  kind: operationKind,
  payload: {
    kind: 'accepted',
    providerRef,
    provenance: { kind: 'recovered', via: 'keyedRead' },
    raw: { recovered: true },
  },
};
const recoveredReceipt = entry(
  'receipt.recorded',
  { intentId, attemptNo, receipt: recoveredReceiptEnvelope },
  10,
);
const recoveredObservation = observation(40, '100', 11);
const recoveredResolution = entry(
  'recovery.resolved',
  {
    intentId,
    attemptNo,
    result: {
      kind: 'found',
      receiptEntryId: recoveredReceipt.entryId,
      observationEntryId: recoveredObservation.entryId,
    },
  },
  12,
);
const recoveryFrame = ledgerFrameSchema.safeParse({
  schemaVersion: 2,
  accountId,
  position: recoveredResolution.position,
  entries: [recoveredReceipt, recoveredObservation, recoveredResolution],
  prevHash: unwrap(parseSha256('0'.repeat(64))),
});
check(
  recoveryFrame.success,
  'Found recovery must place recovered receipt and observation in one valid frame',
);
check(!ledgerFrameSchema.safeParse({schemaVersion:2,accountId,position:recoveredResolution.position,entries:[recoveredReceipt,recoveredResolution],prevHash:'0'.repeat(64)}).success,'Found references cannot escape their append frame');
check(intentOutcome(intentId,[proposed,approved,started,recoveredResolution])==='attempting','A recovery found reference is not an accepted receipt');
check(intentOutcome(intentId,[proposed,approved,started,recoveredReceipt,recoveredObservation,recoveredResolution])==='filled','Recovered receipt and observation are sole sources of recovered order facts');
const invalidState:EntryOf<'observation.recorded'>={...recoveredObservation,payload:{observation:{...recoveredObservation.payload.observation,payload:{...recoveredObservation.payload.observation.payload,state:{kind:'untranslated'}}}}};
check(orderProjection(providerRef,[invalidState]).kind==='unconsumed','Relevant untranslatable observation is visible to consumers');
const invalidKnown=parseLedgerEntry({...accepted,payload:{intentId,attemptNo,receipt:operationEnvelope}});
check(invalidKnown.kind==='error'&&invalidKnown.error.kind==='Corrupt','Known malformed receipt role cannot downgrade to unknown');
check(parseLedgerEntry({...proposed,position:0}).kind==='error','A ledger entry cannot use the genesis head position');
check(parseLedgerEntry({...started,correlation:{intentId,attemptNo:2}}).kind==='error','Kind-specific correlation mismatch is corrupt');
const unknownEntry=parseLedgerEntry({...proposed,kind:'extension.persisted',payload:{value:'preserve'},consumption:'unconsumed'});
check(unknownEntry.kind==='ok'&&unknownEntry.value.kind==='extension.persisted','Unknown kind preserves validated common envelope and raw payload');
const pastRecheck=nextAction(contract,{...unknown,nextCheckAfter:now(1)},attempt,now(40));
const farRecheck=nextAction(contract,{...unknown,nextCheckAfter:now(999)},attempt,now(40));
check(pastRecheck.kind==='scheduleRecheck'&&pastRecheck.at===41,'Past recovery hint cannot create a busy loop');
check(farRecheck.kind==='scheduleRecheck'&&farRecheck.at===120,'Provider hint cannot extend unknown budget');

check(parseUtaRuntimeConfig(runtimeConfig).kind === 'ok', 'Complete zero-account runtime config is valid');
check(
  parseUtaRuntimeConfig({
    ...runtimeConfig,
    capacity: { ...runtimeConfig.capacity, ledgerSegmentMaxBytes: 1024 },
  }).kind === 'error',
  'Schema v1 fixes segment size at 16MiB',
);
check(
  parseUtaRuntimeConfig({
    ...runtimeConfig,
    recovery: [{ accountId: 'unconfigured', maxUnknownDuration: 10 }],
  }).kind === 'error',
  'Account binding cannot drift between config tables',
);
check(
  parseUtaRuntimeConfig({ ...runtimeConfig, configRevision: 'caller-supplied' }).kind === 'error',
  'Persisted config cannot inject derived revision',
);
check(parseOperationKind('place').kind === 'error', 'Operation kinds require a namespace');
check(
  coreOperationKind(unwrap(parseOperationKind('custom.transfer'))).kind === 'error',
  'An open extension does not silently enter core consumers',
);
check(
  coreOperationKind(unwrap(parseOperationKind('order.place'))).kind === 'ok',
  'Core literal subset remains recognized',
);
const unknownRule = coreConsumerRule(unwrap(parseOperationKind('custom.transfer')));
check(
  unknownRule.kind === 'reject' && unknownRule.reason.kind === 'UnknownKind',
  'Core consumer explicitly rejects unknown extensions',
);
console.log('PASS boundaries: recovery permissions, provider evidence, strict config, decimals, IDs, wire, reason trees');
