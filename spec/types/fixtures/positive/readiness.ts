import { parseAccountId, parseAsOf, parseConfigRevision, parseEntryPosition, parseHeadPosition, parseInstant, parseOperationKind } from '../../ids.ts';
import { capabilityTableSchema } from '../../provider/declaration.ts';
import { parseDuration } from '../../time.ts';
import { unwrap } from '../../result.ts';
import {
  accountReadinessSchema,
  makeAccountReadiness,
  readinessResponseSchema,
  type AccountReadiness,
  type ReadinessResponse,
  type ReadinessResult,
} from '../../readiness.ts';
function check(condition: boolean, label: string): void {
  if (!condition) throw new Error(label);
}

function expectViolation(result: ReadinessResult, kind: string, label: string): void {
  check(result.kind === 'inconsistent', `${label}: expected inconsistent result`);
  if (result.kind === 'inconsistent') {
    check(result.violations.some((violation) => violation.kind === kind), `${label}: missing ${kind}`);
  }
}

const accountId = unwrap(parseAccountId('readiness'));
const operationKind = unwrap(parseOperationKind('order.place'));
const configRevision = unwrap(parseConfigRevision('a'.repeat(64)));
const headPosition = unwrap(parseHeadPosition(0));
const asOf = unwrap(parseAsOf(1000));
const disconnectSince = unwrap(parseInstant(2000));
const unreadableAt = unwrap(parseEntryPosition(1));
const freshnessMaxAge = unwrap(parseDuration(60000));
const configErrors = [{ path: 'capacity.maxProviderConnections', code: 'TooSmall', message: 'At least 100' }] as const;
const capabilities = capabilityTableSchema.parse({
  'order.place': {
    direction: 'read',
    status: { kind: 'supported' },
    cursor: { kind: 'snapshotOnly' },
  },
});
const declared = new Set([operationKind]);

const base: AccountReadiness = {
  accountId,
  process: 'running',
  transport: { kind: 'connected' },
  readable: { kind: 'ok', headPosition },
  writable: { kind: 'blocked', reason: { kind: 'FirstObservationRequired' }, secondary: [] },
  capabilities,
  observationFreshness: { kind: 'fresh', asOf, maxAge: freshnessMaxAge },
  config: { kind: 'valid', configRevision },
};

const consistent = makeAccountReadiness(base, declared);
check(consistent.kind === 'ok', 'consistent readiness must be accepted');
check(accountReadinessSchema.safeParse(base).success, 'consistent readiness schema must parse');

const readinessResponse: ReadinessResponse = {
  process: { state: 'running', mode: { value: 'readonly', source: 'config' } },
  accounts: [base],
};
check(
  readinessResponseSchema.safeParse(readinessResponse).success,
  'readiness response accepts process state and R2 mode source',
);
for (const mode of [
  { value: 'lite', source: 'env' },
  { value: 'readonly', source: 'config' },
  { value: 'pro', source: 'default' },
]) {
  check(
    readinessResponseSchema.safeParse({
      ...readinessResponse,
      process: { state: 'running', mode },
    }).success,
    'readiness response accepts every effective mode source',
  );
}
check(
  !readinessResponseSchema.safeParse({ ...readinessResponse, process: 'running' }).success,
  'readiness response rejects the legacy scalar process shape',
);
check(
  !readinessResponseSchema.safeParse({
    ...readinessResponse,
    process: {
      ...readinessResponse.process,
      mode: { ...readinessResponse.process.mode, value: 'desktop' },
    },
  }).success,
  'readiness response rejects unknown process modes',
);

const draining = { ...base, process: 'draining', writable: { kind: 'ok' } };
expectViolation(
  makeAccountReadiness(draining, declared),
  'ProcessDrainingButWritable',
  'draining readiness',
);
check(!accountReadinessSchema.safeParse(draining).success, 'schema must reject draining+writable');

const configInvalid = {
  ...base,
  writable: { kind: 'ok' },
  config: {
    kind: 'config-invalid',
    errors: configErrors,
  },
};
expectViolation(
  makeAccountReadiness(configInvalid, declared),
  'ConfigInvalidButWritable',
  'config-invalid readiness',
);
check(!accountReadinessSchema.safeParse(configInvalid).success, 'schema must reject config-invalid+writable');

const emptyCapabilities = { ...base, capabilities: {} };
expectViolation(
  makeAccountReadiness(emptyCapabilities, declared),
  'CapabilitiesEmpty',
  'empty capabilities readiness',
);
check(!accountReadinessSchema.safeParse(emptyCapabilities).success, 'schema must reject empty capabilities');
const unreadable = {
  ...base,
  writable: { kind: 'ok' },
  readable: { kind: 'unavailable', open: 'Corrupt', at: unreadableAt },
};
expectViolation(
  makeAccountReadiness(unreadable, declared),
  'UnreadableButWritable',
  'unreadable readiness',
);
check(!accountReadinessSchema.safeParse(unreadable).success, 'schema must reject unreadable+writable');

const disconnected = {
  ...base,
  writable: { kind: 'ok' },
  transport: {
    kind: 'disconnected',
    since: disconnectSince,
    lastError: { code: 'ECONNRESET', message: 'connection reset' },
  },
};
expectViolation(
  makeAccountReadiness(disconnected, declared),
  'DisconnectedButWritable',
  'disconnected readiness',
);
check(!accountReadinessSchema.safeParse(disconnected).success, 'schema must reject disconnected+writable');

console.log('PASS readiness: constructor and schema enforce draining, config, consistency, and capability invariants');
