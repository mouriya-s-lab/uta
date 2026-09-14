// Expected TS2741: decisions require caller-owned idempotency identity.
import type { AuthorizationDecisionRequest } from '../../wire/v2.ts';
declare const scope:AuthorizationDecisionRequest['scope'];
declare const scopeHash:AuthorizationDecisionRequest['scopeHash'];
const forbidden:AuthorizationDecisionRequest = {action:'approve',scope,scopeHash};
