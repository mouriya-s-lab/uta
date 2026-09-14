import { z } from 'zod';
import {
  accountIdSchema,
  configRevisionSchema,
  entryPositionSchema,
  headPositionSchema,
  instantSchema,
  operationKindSchema,
  parseOperationKind,
  type AccountId,
  type ConfigRevision,
  type EntryPosition,
  type HeadPosition,
  type Instant,
  type OperationKind,
} from './ids.ts';
import {
  capabilityTableSchema,
  type CapabilityTable,
} from './provider/declaration.ts';
import {
  freshnessSchema,
  type Freshness,
} from './time.ts';

import { utaModeSchema } from './policy.ts';
export type ProcessState = 'running' | 'draining' | 'stopped';
export const MIN_PROVIDER_CONNECTIONS = 100;

export const processStateSchema = z.enum(['running', 'draining', 'stopped']);

export type ProcessModeValue = z.infer<typeof utaModeSchema>;
export const processModeValueSchema = utaModeSchema;

export const processModeSourceSchema = z.enum(['env', 'config', 'default']);
export type ProcessModeSource = z.infer<typeof processModeSourceSchema>;

export type ProcessMode = {
  readonly value: ProcessModeValue;
  readonly source: ProcessModeSource;
};

export const processModeSchema: z.ZodType<ProcessMode> = z.strictObject({
  value: processModeValueSchema,
  source: processModeSourceSchema,
});

export type ProcessSnapshot = {
  readonly state: ProcessState;
  readonly mode: ProcessMode;
};

export const processSnapshotSchema: z.ZodType<ProcessSnapshot> = z.strictObject({
  state: processStateSchema,
  mode: processModeSchema,
});

export type TransportError = {
  readonly code: string;
  readonly message: string;
};

export type TransportState =
  | { readonly kind: 'connected' }
  | { readonly kind: 'connecting' }
  | {
      readonly kind: 'disconnected';
      readonly since: Instant;
      readonly lastError?: TransportError | undefined;
    };

const transportErrorSchema = z.strictObject({
  code: z.string().min(1),
  message: z.string().min(1),
});

export const transportStateSchema: z.ZodType<TransportState> =
  z.discriminatedUnion('kind', [
    z.strictObject({ kind: z.literal('connected') }),
    z.strictObject({ kind: z.literal('connecting') }),
    z.strictObject({
      kind: z.literal('disconnected'),
      since: instantSchema,
      lastError: transportErrorSchema.optional(),
    }),
  ]);

export type ReadableState =
  | { readonly kind: 'ok'; readonly headPosition: HeadPosition }
  | {
      readonly kind: 'unavailable';
      readonly open: 'TornTail' | 'Corrupt' | 'DurabilityFailure';
      readonly at?: EntryPosition | undefined;
    };

export const readableStateSchema: z.ZodType<ReadableState> =
  z.discriminatedUnion('kind', [
    z.strictObject({
      kind: z.literal('ok'),
      headPosition: headPositionSchema,
    }),
    z.strictObject({
      kind: z.literal('unavailable'),
      open: z.enum(['TornTail', 'Corrupt', 'DurabilityFailure']),
      at: entryPositionSchema.optional(),
    }),
  ]);

export type ConfigError = {
  readonly path: string;
  readonly code: string;
  readonly message: string;
};

export const configErrorSchema: z.ZodType<ConfigError> = z.strictObject({
  path: z.string().min(1),
  code: z.string().min(1),
  message: z.string().min(1),
});

const configErrorsSchema = z.tuple([configErrorSchema]).rest(configErrorSchema);

export type ConfigState =
  | { readonly kind: 'valid'; readonly configRevision: ConfigRevision }
  | {
      readonly kind: 'config-invalid';
      readonly errors: readonly [ConfigError, ...ConfigError[]];
    };

export const configStateSchema: z.ZodType<ConfigState> = z.discriminatedUnion(
  'kind',
  [
    z.strictObject({
      kind: z.literal('valid'),
      configRevision: configRevisionSchema,
    }),
    z.strictObject({
      kind: z.literal('config-invalid'),
      errors: configErrorsSchema,
    }),
  ],
);

export const WRITABLE_BLOCK_PRECEDENCE = [
  'Draining',
  'ConfigInvalid',
  'LedgerUnreadable',
  'TransportDisconnected',
  'FirstObservationRequired',
  'QuarantineCapacityExceeded',
  'NoPlacementRecovery',
] as const;

export type WritableBlockedReason =
  | { readonly kind: 'NoPlacementRecovery'; readonly kinds: readonly [OperationKind, ...OperationKind[]] }
  | { readonly kind: 'ConfigInvalid'; readonly errors: readonly [ConfigError, ...ConfigError[]] }
  | { readonly kind: 'Draining'; readonly since: Instant }
  | {
      readonly kind: 'LedgerUnreadable';
      readonly open: 'TornTail' | 'Corrupt' | 'DurabilityFailure';
      readonly at?: EntryPosition | undefined;
    }
  | {
      readonly kind: 'TransportDisconnected';
      readonly since: Instant;
      readonly lastError?: TransportError | undefined;
    }
  | { readonly kind: 'FirstObservationRequired' }
  | {
      readonly kind: 'QuarantineCapacityExceeded';
      readonly quarantined: number;
      readonly capacity: number;
    };

const nonEmptyOperationKindsSchema: z.ZodType<[OperationKind, ...OperationKind[]]> = z
  .tuple([operationKindSchema])
  .rest(operationKindSchema);
const nonNegativeSafeIntegerSchema = z
  .number()
  .int()
  .nonnegative()
  .max(Number.MAX_SAFE_INTEGER);

export const writableBlockedReasonSchema: z.ZodType<WritableBlockedReason> =
  z.discriminatedUnion('kind', [
    z.strictObject({
      kind: z.literal('NoPlacementRecovery'),
      kinds: nonEmptyOperationKindsSchema,
    }),
    z.strictObject({
      kind: z.literal('ConfigInvalid'),
      errors: configErrorsSchema,
    }),
    z.strictObject({
      kind: z.literal('Draining'),
      since: instantSchema,
    }),
    z.strictObject({
      kind: z.literal('LedgerUnreadable'),
      open: z.enum(['TornTail', 'Corrupt', 'DurabilityFailure']),
      at: entryPositionSchema.optional(),
    }),
    z.strictObject({
      kind: z.literal('TransportDisconnected'),
      since: instantSchema,
      lastError: transportErrorSchema.optional(),
    }),
    z.strictObject({ kind: z.literal('FirstObservationRequired') }),
    z.strictObject({
      kind: z.literal('QuarantineCapacityExceeded'),
      quarantined: nonNegativeSafeIntegerSchema,
      capacity: nonNegativeSafeIntegerSchema,
    }),
  ]);

export type WritableState =
  | { readonly kind: 'ok' }
  | {
      readonly kind: 'blocked';
      readonly reason: WritableBlockedReason;
      readonly secondary: readonly WritableBlockedReason[];
    };

export const writableStateSchema: z.ZodType<WritableState> =
  z.discriminatedUnion('kind', [
    z.strictObject({ kind: z.literal('ok') }),
    z.strictObject({
      kind: z.literal('blocked'),
      reason: writableBlockedReasonSchema,
      secondary: z.array(writableBlockedReasonSchema),
    }),
  ]);

export type AccountReadiness = {
  readonly accountId: AccountId;
  readonly process: ProcessState;
  readonly transport: TransportState;
  readonly readable: ReadableState;
  readonly writable: WritableState;
  readonly capabilities: CapabilityTable;
  readonly observationFreshness: Freshness;
  readonly config: ConfigState;
};

export type ReadinessViolation =
  | { readonly kind: 'SchemaInvalid'; readonly path: string; readonly message: string }
  | { readonly kind: 'CapabilitiesEmpty'; readonly path: 'capabilities' }
  | { readonly kind: 'DeclaredKindsEmpty'; readonly path: 'declared' }
  | {
      readonly kind: 'CapabilityMissing';
      readonly path: 'capabilities';
      readonly operationKind: OperationKind;
    }
  | {
      readonly kind: 'CapabilityUnexpected';
      readonly path: 'capabilities';
      readonly operationKind: OperationKind;
    }
  | {
      readonly kind: 'CapabilityKeyInvalid';
      readonly path: string;
      readonly message: string;
    }
  | { readonly kind: 'ProcessDrainingButWritable'; readonly path: 'writable' }
  | { readonly kind: 'ConfigInvalidButWritable'; readonly path: 'writable' }
  | { readonly kind: 'UnreadableButWritable'; readonly path: 'writable' }
  | { readonly kind: 'DisconnectedButWritable'; readonly path: 'writable' }
  | { readonly kind: 'ConfigInvalidReasonMismatch'; readonly path: 'writable' }
  | { readonly kind: 'LedgerUnreadableReasonMismatch'; readonly path: 'writable' }
  | { readonly kind: 'TransportDisconnectedReasonMismatch'; readonly path: 'writable' }
  | {
      readonly kind: 'DuplicateBlockReason';
      readonly path: 'writable';
      readonly reason: WritableBlockedReason['kind'];
    }
  | {
      readonly kind: 'PrecedenceViolation';
      readonly path: 'writable';
      readonly expected: WritableBlockedReason['kind'];
      readonly actual: WritableBlockedReason['kind'];
    };

export type ReadinessResult =
  | { readonly kind: 'ok'; readonly value: AccountReadiness }
  | { readonly kind: 'inconsistent'; readonly violations: readonly ReadinessViolation[] };

const accountReadinessStructureSchema: z.ZodType<AccountReadiness> = z.strictObject({
  accountId: accountIdSchema,
  process: processStateSchema,
  transport: transportStateSchema,
  readable: readableStateSchema,
  writable: writableStateSchema,
  capabilities: capabilityTableSchema,
  observationFreshness: freshnessSchema,
  config: configStateSchema,
});

type DerivedBlockKind =
  | 'Draining'
  | 'ConfigInvalid'
  | 'LedgerUnreadable'
  | 'TransportDisconnected';

type ConfigInvalidReason = Extract<WritableBlockedReason, { readonly kind: 'ConfigInvalid' }>;
type LedgerUnreadableReason = Extract<WritableBlockedReason, { readonly kind: 'LedgerUnreadable' }>;
type TransportDisconnectedReason = Extract<
  WritableBlockedReason,
  { readonly kind: 'TransportDisconnected' }
>;

function sameConfigErrors(
  left: readonly ConfigError[],
  right: readonly ConfigError[],
): boolean {
  return (
    left.length === right.length &&
    left.every(
      (error, index) =>
        error.path === right[index]?.path &&
        error.code === right[index]?.code &&
        error.message === right[index]?.message,
    )
  );
}

function sameTransportError(
  left: TransportError | undefined,
  right: TransportError | undefined,
): boolean {
  if (left === undefined || right === undefined) return left === right;
  return left.code === right.code && left.message === right.message;
}


function findConfigInvalidReason(
  reasons: readonly WritableBlockedReason[],
): ConfigInvalidReason | undefined {
  for (const reason of reasons) {
    if (reason.kind === 'ConfigInvalid') return reason;
  }
  return undefined;
}

function findLedgerUnreadableReason(
  reasons: readonly WritableBlockedReason[],
): LedgerUnreadableReason | undefined {
  for (const reason of reasons) {
    if (reason.kind === 'LedgerUnreadable') return reason;
  }
  return undefined;
}

function findTransportDisconnectedReason(
  reasons: readonly WritableBlockedReason[],
): TransportDisconnectedReason | undefined {
  for (const reason of reasons) {
    if (reason.kind === 'TransportDisconnected') return reason;
  }
  return undefined;
}


function derivedBlockers(input: AccountReadiness): readonly DerivedBlockKind[] {
  const result: DerivedBlockKind[] = [];
  if (input.process === 'draining') result.push('Draining');
  if (input.config.kind === 'config-invalid') result.push('ConfigInvalid');
  if (input.readable.kind === 'unavailable') result.push('LedgerUnreadable');
  if (input.transport.kind === 'disconnected') result.push('TransportDisconnected');
  return result;
}

function missingBlockerViolation(required: DerivedBlockKind): ReadinessViolation {
  switch (required) {
    case 'Draining':
      return { kind: 'ProcessDrainingButWritable', path: 'writable' };
    case 'ConfigInvalid':
      return { kind: 'ConfigInvalidButWritable', path: 'writable' };
    case 'LedgerUnreadable':
      return { kind: 'UnreadableButWritable', path: 'writable' };
    case 'TransportDisconnected':
      return { kind: 'DisconnectedButWritable', path: 'writable' };
  }
}

function collectReadinessViolations(
  input: AccountReadiness,
  declared: ReadonlySet<OperationKind> | undefined,
): readonly ReadinessViolation[] {
  const violations: ReadinessViolation[] = [];
  const capabilityKeys = Object.keys(input.capabilities);

  if (capabilityKeys.length === 0) {
    violations.push({ kind: 'CapabilitiesEmpty', path: 'capabilities' });
  }

  if (declared !== undefined) {
    if (declared.size === 0) {
      violations.push({ kind: 'DeclaredKindsEmpty', path: 'declared' });
    }
    for (const operationKind of declared) {
      if (!Object.hasOwn(input.capabilities, operationKind)) {
        violations.push({
          kind: 'CapabilityMissing',
          path: 'capabilities',
          operationKind,
        });
      }
    }
  }

  for (const key of capabilityKeys) {
    const parsed = parseOperationKind(key);
    if (parsed.kind === 'error') {
      violations.push({
        kind: 'CapabilityKeyInvalid',
        path: `capabilities.${key}`,
        message: parsed.error.message,
      });
    } else if (declared !== undefined && !declared.has(parsed.value)) {
      violations.push({
        kind: 'CapabilityUnexpected',
        path: 'capabilities',
        operationKind: parsed.value,
      });
    }
  }

  const required = derivedBlockers(input);
  const reasons =
    input.writable.kind === 'blocked'
      ? [input.writable.reason, ...input.writable.secondary]
      : [];

  for (const blocker of required) {
    if (input.writable.kind === 'ok' || !reasons.some((reason) => reason.kind === blocker)) {
      violations.push(missingBlockerViolation(blocker));
    }
  }
  if (input.writable.kind === 'blocked' && required.length > 0 && reasons.length > 0) {
    const expected = required[0];
    const actual = reasons.at(0);
    if (
      expected !== undefined &&
      actual !== undefined &&
      WRITABLE_BLOCK_PRECEDENCE.indexOf(actual.kind) >
        WRITABLE_BLOCK_PRECEDENCE.indexOf(expected)
    ) {
      violations.push({
        kind: 'PrecedenceViolation',
        path: 'writable',
        expected,
        actual: actual.kind,
      });
    }
  }

  if (input.writable.kind === 'blocked') {
    const seen = new Set<WritableBlockedReason['kind']>();
    let previous = -1;
    for (const reason of reasons) {
      if (seen.has(reason.kind)) {
        violations.push({
          kind: 'DuplicateBlockReason',
          path: 'writable',
          reason: reason.kind,
        });
      }
      seen.add(reason.kind);
      const current = WRITABLE_BLOCK_PRECEDENCE.indexOf(reason.kind);
      if (current <= previous) {
        violations.push({
          kind: 'PrecedenceViolation',
          path: 'writable',
          expected: WRITABLE_BLOCK_PRECEDENCE[previous] ?? reason.kind,
          actual: reason.kind,
        });
      }
      previous = current;
    }

    if (input.config.kind === 'config-invalid') {
      const configReason = findConfigInvalidReason(reasons);
      if (configReason !== undefined && !sameConfigErrors(configReason.errors, input.config.errors)) {
        violations.push({ kind: 'ConfigInvalidReasonMismatch', path: 'writable' });
      }
    }

    if (input.readable.kind === 'unavailable') {
      const ledgerReason = findLedgerUnreadableReason(reasons);
      if (
        ledgerReason !== undefined &&
        (ledgerReason.open !== input.readable.open || ledgerReason.at !== input.readable.at)
      ) {
        violations.push({ kind: 'LedgerUnreadableReasonMismatch', path: 'writable' });
      }
    }

    if (input.transport.kind === 'disconnected') {
      const transportReason = findTransportDisconnectedReason(reasons);
      if (
        transportReason !== undefined &&
        (transportReason.since !== input.transport.since ||
          !sameTransportError(transportReason.lastError, input.transport.lastError))
      ) {
        violations.push({ kind: 'TransportDisconnectedReasonMismatch', path: 'writable' });
      }
    }
  }

  return violations;
}

// Exact capability-key coverage is constructor-level because the wire schema has no declaration context.
export const accountReadinessSchema = accountReadinessStructureSchema.superRefine(
  (input, context) => {
    for (const violation of collectReadinessViolations(input, undefined)) {
      context.addIssue({
        code: 'custom',
        path: [violation.path],
        message: violation.kind,
      });
    }
  },
);

function schemaViolations(error: z.ZodError): readonly ReadinessViolation[] {
  return error.issues.map((issue) => ({
    kind: 'SchemaInvalid',
    path: issue.path.map((segment) => String(segment)).join('.') || '$',
    message: issue.message,
  }));
}

export function makeAccountReadiness(
  input: unknown,
  declared: ReadonlySet<OperationKind>,
): ReadinessResult {
  const parsed = accountReadinessStructureSchema.safeParse(input);
  if (!parsed.success) {
    return { kind: 'inconsistent', violations: schemaViolations(parsed.error) };
  }
  const violations = collectReadinessViolations(parsed.data, declared);
  return violations.length === 0
    ? { kind: 'ok', value: parsed.data }
    : { kind: 'inconsistent', violations };
}

export type ReadinessResponse = {
  readonly process: ProcessSnapshot;
  readonly accounts: readonly AccountReadiness[];
};

export const readinessResponseSchema: z.ZodType<ReadinessResponse> = z
  .strictObject({
    process: processSnapshotSchema,
    accounts: z.array(accountReadinessSchema),
  })
  .superRefine((input, context) => {
    const seen = new Set<AccountId>();
    for (const [index, account] of input.accounts.entries()) {
      if (seen.has(account.accountId)) {
        context.addIssue({
          code: 'custom',
          path: ['accounts', index, 'accountId'],
          message: 'DuplicateAccountId',
        });
      }
      seen.add(account.accountId);
      if (
        input.process.state === 'draining' &&
        (account.writable.kind === 'ok' || account.writable.reason.kind !== 'Draining')
      ) {
        context.addIssue({
          code: 'custom',
          path: ['accounts', index, 'writable'],
          message: 'ProcessDrainingButWritable',
        });
      }
    }
  });

// Preserved /__uta/health uses ISO text (services/uta/src/main.ts:42,148-151), not v2 epoch milliseconds.
export type ProcessHealth = { readonly ok: true; readonly startedAt: string; readonly utas: number };
