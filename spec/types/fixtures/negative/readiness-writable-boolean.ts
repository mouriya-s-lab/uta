// Expected TS2322: writable is a tagged readiness state, never a boolean.
import type { AccountReadiness } from '../../readiness.ts';

declare const readiness: AccountReadiness;
const invalid: AccountReadiness = { ...readiness, writable: false };
void invalid;
