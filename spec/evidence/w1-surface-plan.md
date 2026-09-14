# Surface-map plan

## Decisive question
Can the existing 49 UTA HTTP routes be mapped to concrete Alice/UI callers and exact old semantics (read/write/config/process/simulator), including dead routes and reverse imports, well enough to define a compatibility facade without inventing behavior?

## Steps
1. Reconcile the reported 49-route list with `services/uta/src/http/routes-trading.ts`, `routes-simulator.ts`, and `main.ts`, recording registration lines, request/response/error behavior, mode gates, and side effects.
2. Trace each route into SDK, tools, BFF/config, bars, connector, CLI, and UI callers; mark live, indirect, and zero-call routes.
3. Inspect `packages/uta-protocol/src/index.ts`, `types/**`, `schemas/index.ts`, UI API type mirrors, and all `services/uta/src` `@/*` imports to answer type ownership and reverse-import questions.
4. Write one endpoint table (49 rows), consumer-dependency table, reverse-import list, and minimal compatibility requirements for release-preserved paths versus atomic migration.
5. Perform a scoped evidence check: route registrations/count, reverse-import search, protocol exports/schema emptiness, and caller searches. No formatter/linter/project-wide tests.

## Cheapest decisive observations
- Route registrations and handlers settle the endpoint count and semantics.
- Exact string/path searches in SDK/UI/tools/BFF settle live callers and dead routes.
- `grep` of `from '@/` plus `src/index.ts`/`schemas/index.ts` settles the 21-import and protocol-export claims.

## Out of scope
No production code changes; no redesign of internal broker, ledger, snapshot, FX, or persistence implementations; no exhaustive code tour; no project-wide validation.

## Assumptions to verify
- The requested 49 rows are UTA service routes (39 trading + 9 simulator + health), while Alice-owned `/api/trading/config/*` routes belong in the consumer/dependency section rather than the 49-row UTA table.
- Existing reports are maps, not proof; every decisive claim will carry source path:line evidence or be explicitly marked inferred.
- Endpoint idempotency describes repeated requests against the same current state, not merely whether a GET is safe in HTTP terminology.
