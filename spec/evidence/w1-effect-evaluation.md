# W1 Effect-ts v3 adoption evidence

## Decision questions answered

1. **Should the new UTA core adopt Effect? — Yes, choose Option B (Effect inside UTA core only).** [inferred] Effect materially improves the exact hard parts in the design—mandatory capability composition, scoped WebSocket lifetimes, cancellation, bounded concurrency, serial writes, typed expected errors, retry schedules, and deterministic clocks—without forcing the public protocol or dynamically loaded Broker Packs to share Effect runtime identity. [observed; Effect docs and probes cited below]
2. **Should domain and persistence types become Effect types? — No.** [inferred] Keep ledger entries, idempotency keys, receipts, observations, provider-indexed ADTs, and config/wire types as library-neutral records/sum types. Put `Effect<A, E, R>` at orchestration and IO edges, and convert to/from `Promise` at the pack/HTTP boundary. [inferred]
3. **Should the shared protocol move from Zod to Effect Schema now? — No.** [inferred] Effect Schema can technically validate/encode and emit JSON Schema/OpenAPI 3.1 fragments, but the current protocol and Pack API explicitly use Zod. A wholesale switch would widen the migration and couple Alice, UTA, and Packs without solving provider-specific OpenAPI projection semantics. [observed; `AGENTS.md:182-187`, `packages/uta-protocol/src/schemas/index.ts:1-10`, `services/uta/src/domain/trading/brokers/registry.ts:23-32`]
4. **Can Effect Schema consume arbitrary provider OpenAPI projections? — Not from the installed public v3 APIs.** [observed] `effect/JSONSchema` and `@effect/platform/OpenApi` expose generation (`JSONSchema.make`, `OpenApi.fromApi`), while a scoped declaration search found no `fromOpenAPI`, `fromOpenApi`, or `fromJsonSchema` importer. [inferred] Provider projections therefore still need an OpenAPI-generated client/adapter or hand-authored translator; Option C does not remove that work.
5. **What would falsify Option B? —** [inferred] A released active-pack probe would falsify it if a Promise-only Pack cannot be adapted without sharing Effect types or if the pinned Bun runtime cannot compile/load the core with Effect. A full UTA benchmark would also falsify the recommendation if the added dependency/closure materially violates the product's measured startup/memory budget. These are open acceptance checks, not current failures.

Evidence tags used below: `[observed; ...]` is directly grounded in a repository path/line, official documentation URL/line range, installed declaration, or command output. `[inferred]` is a design judgment from those facts and is not presented as an existing implementation behavior.

## Observed facts

### Existing UTA and Pack boundaries

- The repository has no `effect` dependency in the root or UTA service manifests. [observed; `package.json:101-131`, `services/uta/package.json:14-30`]
- The repository convention is ESM with `.js` TypeScript imports, strict ES2023, Zod for config schemas, TypeBox for tool parameters, and `decimal.js` for financial arithmetic. [observed; `AGENTS.md:182-187`]
- UTA is built as ESM/ES2023 with `splitting: false`; `tsup` bundles local source but leaves `node_modules` external. [observed; `services/uta/tsup.config.ts:3-20`]
- The current Pack module contract is `configSchema: z.ZodType` plus `createBroker(config): IBroker`; the registry dynamically imports a selected Pack by file URL, caches its Promise, validates API/version/engine/function shape, and translates failures to `BrokerPackUnavailableError`. [observed; `services/uta/src/domain/trading/brokers/registry.ts:23-32,52-66,84-128`]
- Pack-local dependency copies intentionally cross a structural boundary; the repository says core code must not depend on Pack class identity and should use structural checks/stable error codes. [observed; `docs/broker-packs.md:67-70`]
- The current SDK is Promise-facing: e.g. `UTAManagerSDK.listUTAs`, `resolve`, and `getAggregatedEquity` are `async`/`Promise` methods, while broker methods such as Alpaca `init`, `placeOrder`, and `getAccount` are async. [observed; `src/services/uta-client/UTAManagerSDK.ts:81-162`, `services/uta/src/domain/trading/brokers/alpaca/AlpacaBroker.ts:163-180,290-301,451-456`]
- The current Hono route parses input with Zod and maps validation failures or thrown errors directly to JSON responses with HTTP 400; the shared wire error has `code`, `message`, `transient`, and optional `hint`. [observed; `services/uta/src/http/routes-trading.ts:594-606,609-636`, `packages/uta-protocol/src/types/errors.ts:1-19`]
- The installed Pack artifacts are not all inside the core process: the documented split keeps live SDKs out of UTA Core and installs versioned Packs only when selected; Mock is the sole built-in engine and missing Packs do not silently fall back. [observed; `docs/broker-packs.md:8-30`]

### What Effect v3 actually provides

- `Effect<Success, Error, Requirements>` is a lazy, immutable workflow description. The runtime executes it at an entry point; `Error` is the expected failure channel and `Requirements` is the required Context. [observed; <https://effect.website/docs/v3/getting-started/the-effect-type>, lines 68-131]
- `Context.Tag` places a service in the Requirements type, and `Layer` constructs/provides services. The official example says running without a required service is a type-checking error; `serviceOption` deliberately erases that requirement and is therefore inappropriate for mandatory capabilities. [observed; <https://effect.website/docs/v3/requirements-management/services>, lines 609-653, 887-899, 1581-1663]
- `Layer` keeps construction dependencies separate from the service interface and composes output/requirements as types. [observed; <https://effect.website/docs/v3/requirements-management/layers>, lines 556-564, 674-675, 869-911, 1796-1829]
- `Data.TaggedError` supplies a `_tag` for `catchTag`/`catchTags`; it is an expected-error discriminator, not an HTTP status or wire error by itself. [observed; <https://effect.website/docs/v3/data-types/data>, lines 2213-2223; inferred for the latter clause]
- `Effect.acquireRelease` registers cleanup in a Scope after successful acquisition; `Effect.scoped` closes that Scope, and interruption runs finalizers. [observed; <https://effect.website/docs/v3/resource-management/scope>, lines 556-567, 862-868, 1734-1748; <https://effect.website/docs/v3/concurrency/fibers>, lines 850-898]
- `Stream<A, E, R>` models zero or more values over time with errors and requirements; resourceful stream constructors acquire before use and close after use. [observed; <https://effect.website/docs/v3/stream/introduction>, lines 556-566, 620-686; <https://effect.website/docs/v3/stream/resourceful-streams>, lines 556-560]
- Effect concurrency accepts a numeric bound, `"unbounded"`, or `"inherit"`; the default for the cited `Effect.all` API is sequential. [observed; <https://effect.website/docs/v3/concurrency/basic-concurrency>, lines 556-589, 703-705, 863-865]
- A bounded `Queue` applies backpressure when full; `offer`/`take` are suspending operations, and `shutdown` interrupts fibers waiting on the queue. [observed; <https://effect.website/docs/v3/concurrency/queue>, lines 556-568, 630-636, 778-836, 916-920, 1461-1465]
- A semaphore is a generalized mutex: `withPermits` waits for permits and releases them when the effect completes; the declaration documents FIFO processing for concurrent pending `take` calls. [observed; `/tmp/w1-effect-eval/node_modules/effect/dist/dts/Effect.d.ts:24072-24118`]
- `Schedule` is an immutable recurrence/retry policy that consumes inputs (including errors) and controls continuation; `Effect.retry` propagates the final failure when its schedule is exhausted. [observed; <https://effect.website/docs/v3/scheduling/introduction>, lines 556-590; <https://effect.website/docs/v3/error-management/retrying>, lines 556-574]
- `TestClock` does not advance on its own; `TestClock.adjust` runs effects due at or before the adjusted time, making timer tests deterministic. [observed; <https://effect.website/docs/v3/testing/testclock>, lines 556-564, 676-822]

### Runtime, type-system, and footprint probes

All probes below ran outside the repository in `/tmp/w1-effect-eval`; no production file or lockfile was changed. [observed; scoped command setup]

- Installed versions were `effect@3.22.2`, `@effect/platform@0.97.2`, `@effect/platform-bun@0.91.2`, `@effect/platform-node@0.108.2`, and `typescript@5.9.3`. `@effect/platform` declares peer `effect: ^3.22.2`; platform-bun declares peer `effect: ^3.22.1` and platform `^0.97.1`. [observed; `/tmp/w1-effect-eval/node_modules/*/package.json:1-24`]
- `npm install --no-package-lock --ignore-scripts --no-audit --no-fund` reported `added 32 packages in 6s`; `du -sh` reported approximately `33M` for installed `effect`, `21M` for `@effect/platform`, `920K` for platform-bun, and `1.1M` for platform-node. [observed; scoped command output]
- Bun in this shell is `1.4.1`, but the repository pin is `1.4.0`; the feasibility script rejects a mismatch before doing its probe. [observed; `.bun-version:1`, `scripts/build-bun-runtime-feasibility.ts:10-14`, `bun --version` scoped command output]
- A Bun runtime smoke importing `Data` and `Effect` printed `{"effect":true,"bun":"1.4.1"}`. Compiling the same file with `bun build ... --compile` printed the same output, and Bun reported `bundle 153 modules`. [observed; `/tmp/w1-effect-eval/bun-smoke.ts:1-6`, scoped Bun command output]
- A minimal `Effect` minified bundle was `121,350` bytes versus `56` bytes for a bare Bun module, a measured delta of `121,294` bytes (`118.5 KiB`). The compiled executable sizes were `62,573,666` bytes with Effect versus `62,309,474` bare, a delta of `264,192` bytes (`0.252 MiB`); the executable is dominated by the Bun runtime. [observed; scoped `bun build`/`stat` command output]
- A local Bun server smoke opened 100 WebSockets through `Effect.async`, wrapped each socket in `Effect.acquireRelease`, ran all 100 with `{ concurrency: 100 }`, and printed `{"result":"opened-and-released-100","released":100}`. [observed; `/tmp/w1-effect-eval/ws-100-smoke.ts:1-42`, `bun /tmp/w1-effect-eval/ws-100-smoke.ts` output]
- A semaphore smoke issued ten concurrent writes through `Effect.makeSemaphore(1)` and printed `{"maximum":1,"orderLength":10,"distinct":10}`. [observed; `/tmp/w1-effect-eval/serial-smoke.ts:1-21`, Bun output]
- A TestClock smoke showed no queued value before 100 ms and one after `TestClock.adjust('100 millis')`, printing `{"before":true,"after":false}`. [observed; `/tmp/w1-effect-eval/testclock-smoke.ts:1-16`, Bun output]

### Typecheck evidence

The positive sketch is intentionally small and uses Effect only as the orchestration layer. It models `ReadOnly<Heartbeat>`, a single-entry `ReadWrite<Ledger>`, a provider indexed by read-only/read-write mode, and a semaphore around the write handler. [observed; `/tmp/w1-effect-eval/sketch.ts:1-74`]

```ts
import { Context, Data, Effect, Layer } from "effect"

type Heartbeat = { readonly asOf: number }
type Ledger = { readonly entryId: string; readonly kind: string }
type Receipt = { readonly accepted: boolean }

class ReadFailure extends Data.TaggedError("ReadFailure")<{
  readonly reason: string
}> {}
class WriteFailure extends Data.TaggedError("WriteFailure")<{
  readonly reason: string
}> {}

type ReadOnly<T> = {
  readonly read: Effect.Effect<T, ReadFailure>
}
type ReadWrite<T> = {
  readonly append: (entry: T) => Effect.Effect<Receipt, WriteFailure>
}

class HeartbeatChannel extends Context.Tag("UTA/Heartbeat")<
  HeartbeatChannel,
  ReadOnly<Heartbeat>
>() {}
class LedgerChannel extends Context.Tag("UTA/Ledger")<
  LedgerChannel,
  ReadWrite<Ledger>
>() {}

const program = Effect.gen(function* () {
  const heartbeat = yield* HeartbeatChannel
  const ledger = yield* LedgerChannel
  const current = yield* heartbeat.read
  yield* ledger.append({ entryId: String(current.asOf), kind: "observation" })
})

type ProviderMode = "read-only" | "read-write"
type ProviderBase = { readonly heartbeat: ReadOnly<Heartbeat> }
type WritableProvider = ProviderBase & { readonly ledger: ReadWrite<Ledger> }
type Provider<P extends ProviderMode> =
  P extends "read-write" ? WritableProvider : ProviderBase
type WriteHandler<P extends ProviderMode> =
  P extends "read-write" ? ReadWrite<Ledger> : never

const readOnlyProvider: Provider<"read-only"> = {
  heartbeat: { read: Effect.succeed({ asOf: Date.now() }) },
}
// @ts-expect-error A read-only provider has no constructible write handler.
const impossibleWrite: WriteHandler<"read-only"> = {
  append: (_entry: Ledger) => Effect.fail(new WriteFailure({ reason: "not writable" })),
}

const writableProvider: Provider<"read-write"> = {
  ...readOnlyProvider,
  ledger: {
    append: (_entry) => Effect.succeed({ accepted: true }),
  },
}

const HeartbeatLive = Layer.succeed(HeartbeatChannel, readOnlyProvider.heartbeat)
const LedgerLive = Layer.effect(
  LedgerChannel,
  Effect.gen(function* () {
    const mutex = yield* Effect.makeSemaphore(1)
    return {
      append: (entry: Ledger) =>
        mutex.withPermits(1)(writableProvider.ledger.append(entry)),
    }
  }),
)

const UtaLive = Layer.merge(HeartbeatLive, LedgerLive)
const runnable = program.pipe(Effect.provide(UtaLive))
void Effect.runPromise(runnable)
```

The positive compiler proof used the sketch's strict ES2023/Bundler/no-emit config. [observed; `/tmp/w1-effect-eval/tsconfig.json:1-10`]

```text
$ cd /tmp/w1-effect-eval && npx tsc --noEmit
# no compiler output
exit=0
```

A second file intentionally omits `LedgerLive` at the composition root. [observed; `/tmp/w1-effect-eval/missing-layer.ts:17-28`]

```text
$ set +e; out=$(npx tsc --noEmit -p /tmp/w1-effect-eval/tsconfig-missing.json 2>&1); status=$?; printf '%s\nexit=%s\n' "$out" "$status"; exit 0
../../../tmp/w1-effect-eval/missing-layer.ts(28,24): error TS2345: Argument of type 'Effect<void, never, LedgerChannel>' is not assignable to parameter of type 'Effect<void, never, never>'.
Type 'LedgerChannel' is not assignable to type 'never'.
exit=2
```

Removing the `@ts-expect-error` from `impossibleWrite` produced the independent read-only-provider proof. [observed; `/tmp/w1-effect-eval/readonly-provider.ts:45-51`]

```text
../../../tmp/w1-effect-eval/readonly-provider.ts(48,7): error TS2322: Type '{ append: (_entry: Ledger) => Effect.Effect<never, WriteFailure, never>; }' is not assignable to type 'never'.
exit=2
```

The important boundary detail is that a partial `Effect.provide` can still carry an unsatisfied requirement; `Effect.runPromise` is the composition-root boundary that requires `R = never`. [observed; installed declaration `/tmp/w1-effect-eval/node_modules/effect/dist/dts/Effect.d.ts:13852-13913`; negative compiler output above]

### Schema, HTTP, and OpenAPI facts

- Effect Schema v3 describes `Schema<Type, Encoded, Requirements>` and supports decoding, encoding, asserting, Standard Schema V1, arbitraries, JSON Schema, and equivalence; the v3 docs require TypeScript 5.4+ and `strict`. [observed; <https://effect.website/docs/v3/schema/introduction>, lines 557-585; <https://effect.website/docs/v3/schema/introduction>, lines 774-806]
- `JSONSchema.make` supports targets `jsonSchema7`, `jsonSchema2019-09`, `jsonSchema2020-12`, and `openApi3.1`. [observed; <https://effect.website/docs/v3/schema/json-schema>, lines 558-559, 682-687, 837-849; installed `/tmp/w1-effect-eval/node_modules/effect/dist/dts/JSONSchema.d.ts:212-229`]
- JSON Schema generation is not a lossless provider-contract importer: the docs state it walks the decoding side and stops at the first transformation, so a transformation may not appear in the generated schema. [observed; <https://effect.website/docs/v3/schema/json-schema>, lines 682-687]
- In the installed `@effect/platform@0.97.2`, `HttpApiEndpoint` carries schemas for path, URL params, payload, headers, success, and error; `addError` supports endpoint error schemas/status annotations. [observed; `/tmp/w1-effect-eval/node_modules/@effect/platform/dist/dts/HttpApiEndpoint.d.ts:49-100`]
- `@effect/platform/OpenApi.fromApi` converts an Effect `HttpApi` into an OpenAPI 3.1.0 specification. [observed; `/tmp/w1-effect-eval/node_modules/@effect/platform/dist/dts/OpenApi.d.ts:124-188`]
- The package README documents `HttpApiBuilder` handlers, Swagger, and a derived `HttpApiClient`; these are useful for a first-party UTA API declared in Effect, but they do not import a provider's arbitrary existing OpenAPI document. [observed; <https://www.npmjs.com/package/@effect/platform>, lines 48-56, 108-137, 152-186, 198-249]
- A scoped search of the installed v3 declarations for `fromJsonSchema|fromOpenAPI|fromOpenApi|OpenAPI.*import|OpenApi.*import|import.*OpenAPI` returned only `OpenApi.d.ts` references/imports and no matching importer declaration. [observed; `functions.grep` scoped command output on `/tmp/w1-effect-eval/node_modules/effect/dist/dts;/tmp/w1-effect-eval/node_modules/@effect/platform/dist/dts`]
- The Schema smoke decoded `{ asOf: 123 }` and emitted a JSON Schema fragment with `target: 'openApi3.1'`; the output included `$schema: "https://json-schema.org/draft/2020-12/schema"`, `type: "object"`, and `required: ["asOf"]`. [observed; `/tmp/w1-effect-eval/schema-smoke.ts:1-5`, Bun output]
- The HttpApi smoke generated a complete spec whose `openapi` value was `3.1.0`; its minified Bun bundle was printed as `199.48 KB` across `545 modules`, while the Schema smoke bundle was `173.92 KB` across `417 modules`. [observed; `/tmp/w1-effect-eval/httpapi-smoke.ts:1-8`, `/tmp/w1-effect-eval/schema-smoke.ts:1-5`, scoped Bun build output]
- The current v3 docs have no retrievable `platform/http-api` page at the attempted v3 URL, while the current platform introduction is v4-oriented. The npm README and installed declarations above were used for the platform evidence instead. [observed; scoped URL read failures and `/tmp/effect-docs/platform-intro.txt:1-10`, `/tmp/effect-docs/platform-npm.txt:37-56`]

## Scenario mapping: Effect versus plain TypeScript

The table intentionally describes contracts rather than classes. Caller/trust/request identity/retry/error/side effect/persistence entries are the proposed UTA semantics and are marked as inference; current code facts are cited separately above.

| Concrete UTA scenario | Semantic behavior to preserve | Effect v3 fit | Plain TypeScript alternative and cost | Decision |
|---|---|---|---|---|
| **Capability composition: heartbeat read plus ledger write (A3/A4)** | Caller: UTA orchestration. [inferred] Trust principal: composition root chooses handlers; a provider Pack is an implementation boundary. [inferred] Request identity: heartbeat observation and ledger entry IDs stay domain values. [inferred] Concurrency: reads may be concurrent; writes must use one logical entry. [inferred] Retry: only policy-approved failures. [inferred] Error: typed read/write variants. [inferred] Side effect: read observation may feed a ledger append. [inferred] Persistence: ledger repository owns durability, not the effect runtime. [inferred] | `Context.Tag` makes each capability a separate requirement; `Layer.merge` composes them; `Effect.provide` makes the root runnable only after requirements are removed. [observed; Effect docs cited above and `/tmp/w1-effect-eval/missing-layer.ts` diagnostic] | Use explicit `Capabilities` records and constructor parameters. [inferred] This is straightforward, but every nested function must thread records manually and a broad `Partial`/optional map would erase the missing-handler guarantee. [inferred] | **B for core orchestration; library-neutral records remain the domain contract.** [inferred] |
| **Provider-indexed read-only versus read-write (A7)** | Caller: capability resolver or market/data consumer. [inferred] Trust principal: provider declaration plus runtime adapter; compile-time provider index cannot attest a downloaded module. [inferred] Request identity: provider/account ID and operation key. [inferred] Concurrency/retry/error: provider-specific and declared per capability. [inferred] Side effect: a read-only provider may observe but cannot place/modify/cancel. [inferred] Persistence: capability declaration/config is separate from ledger. [inferred] | Conditional `Provider<P>` and `WriteHandler<P>` make `WriteHandler<"read-only">` equal `never`; the negative proof rejects an attempted handler. [observed; `/tmp/w1-effect-eval/readonly-provider.ts` output] Effect does not replace runtime module validation. [inferred] | Use a discriminated union `{ mode: 'read-only'; heartbeat: ... } | { mode: 'read-write'; heartbeat: ...; ledger: ... }` and exhaustive narrowing. [inferred] It gives the same static result with less runtime/library surface, but does not itself solve service assembly. [inferred] | **Either A or B for this type model; B only if the core already uses Effect.** [inferred] |
| **Market watching: 100+ upstream WebSockets** | Caller: market watcher. [inferred] Trust principal: UTA process and provider connection credentials. [inferred] Request identity: `(provider, account, symbol, subscription cursor)`; reconnect must not duplicate downstream identity. [inferred] Concurrency: at least 100 connections, bounded per provider/runtime. [inferred] Retry: reconnect only on classified close/network errors, never replay a write blindly. [inferred] Error: typed connect/close/decode/provider errors. [inferred] Side effect: ticks fan out to watch/order-modify/news consumers. [inferred] Persistence: cursor/observation policy is explicit; socket lifetime is not ledger durability. [inferred] | `acquireRelease`/Scope provides cleanup; `Effect.all(..., { concurrency: 100 })` expresses the bound; `Stream` models the event sequence; fibers provide cancellation/supervision. [observed; docs above; local smoke opened/released 100] It still needs a WebSocket adapter and explicit fan-out/backpressure policy. [inferred] | Use `WebSocket`/`ws`, `AbortController`, `AsyncGenerator`, and `try/finally`; manage a task registry and cancellation manually. [inferred] This can reach 100 sockets, but leak prevention, child lifetime, queue pressure, and retry state become conventions to enforce in review. [inferred] | **B if the 100-connection watch path is first-class; A remains viable for a small finite watcher.** [inferred] |
| **Single-entry ledger placement / order modification (A4/A5)** | Caller: approval connector, Alice tool, or watch-triggered modifier. [inferred] Trust principal: approved UTA command path. [inferred] Request identity: idempotency key/intent ID; receipt and observation IDs must remain distinct. [inferred] Concurrency: one external mutation per account/provider stream. [inferred] Retry: only after a keyed readback or provider-specific safe condition. [inferred] Error: declaration/transport/unknown-outcome/reversal variants, not a string. [inferred] Side effect: exactly one provider mutation attempt followed by receipt/observation append. [inferred] Persistence: append intent/attempt/receipt/observation in the ledger. [inferred] | A one-permit semaphore serializes the effect; bounded `Queue` adds backpressure and shutdown cancellation; `Schedule` can encode retry once the error/receipt class is known. [observed; semaphore/queue/schedule docs and serial smoke] Effect does not provide idempotency or persistence. [inferred] | Use a per-account Promise chain or an async worker over an `AsyncQueue`; add `finally`, abort wiring, and explicit queue close/error propagation. [inferred] It is smaller, but those invariants are handwritten. [inferred] | **B for orchestration, with idempotency/ledger rules kept in domain code.** [inferred] |
| **Watch event → order modify / news subscription → Alice Issue callback** | Caller: watcher or news connector. [inferred] Trust principal: only authorized downstream adapter may submit a write or callback. [inferred] Request identity: event ID plus source cursor; callback deduplicates on that identity. [inferred] Concurrency: bounded fan-out; writes for one account serialized, unrelated consumers may run concurrently. [inferred] Retry: event delivery may retry; order modification must consult receipt/readback policy. [inferred] Error: preserve source failure and nested consumer failure separately. [inferred] Side effect: provider mutation or Issue callback. [inferred] Persistence: cursor and delivery attempt policy are explicit. [inferred] | Fibers, queues, typed error channels, and structured interruption express the topology, but the domain must still choose fan-out isolation and durable cursor behavior. [observed; Fiber/Queue docs above] | Use `Promise.allSettled`, per-consumer queues, abort signals, and explicit result unions. [inferred] This is viable but needs a house convention for cancellation and nested failures. [inferred] | **B gives useful mechanics; do not mistake it for delivery semantics.** [inferred] |
| **Reconciliation and health polling** | Caller: supervisor/reconciliation job. [inferred] Trust principal: UTA scheduler. [inferred] Request identity: account plus reconciliation epoch/cursor. [inferred] Concurrency: bounded accounts; no reentrant run for one account. [inferred] Retry: exponential/capped for transient reads; stop for auth/config. [inferred] Error: health state versus fatal process error. [inferred] Side effect: refresh observations and possibly append a correction/reversal. [inferred] Persistence: snapshots/ledger are explicit sinks. [inferred] | `Schedule`, `Effect.retry`, scoped fibers, and TestClock cover policy, lifetime, and deterministic tests. [observed; docs above and TestClock smoke] | Use `setInterval`/`setTimeout` plus an in-flight flag and fake clock; current UTA code already follows this style for recovery and pollers. [observed; `services/uta/src/domain/trading/UnifiedTradingAccount.ts:467-504`, `services/uta/src/domain/trading/order-sync-poller.ts:1-105`] The implementation is familiar but policy/error composition is manual. [inferred] | **B for new long-lived jobs; migrate only the changed path, not every timer by association.** [inferred] |
| **HTTP/API edge** | Caller: Alice SDK, UI, AI tool, or CLI. [observed; `packages/uta-protocol/src/index.ts:3-12` and current SDK paths] Trust principal: authenticated HTTP caller plus UTA route authorization. [inferred] Request identity: HTTP request/correlation ID and domain idempotency key. [inferred] Concurrency/retry: client retries only declared transient wire errors; server maps provider unknown outcomes explicitly. [inferred] Error: `WireBrokerError` with stable code/transient/hint. [observed; `packages/uta-protocol/src/types/errors.ts:10-18`] Side effect: response plus any already-committed ledger entries. [inferred] Persistence: HTTP layer does not substitute for ledger/config persistence. [inferred] | Keep Effect errors internal, then one route adapter maps typed failures to current wire errors/statuses. [inferred] `HttpApiEndpoint.addError` and `HttpApiBuilder` can type first-party endpoints and generated docs. [observed; installed declarations/npm README] | Keep Hono/Zod and write a small `Result`/error-to-status mapper. [inferred] This preserves the current protocol and avoids bringing `@effect/platform` into Alice/Packs. [inferred] | **B + current Hono/Zod at first; C only if generated UTA API/docs justify shared migration.** [inferred] |

## Options A/B/C comparison

| Dimension | A — no Effect | B — Effect inside UTA core only | C — Effect + Schema/HttpApi throughout |
|---|---|---|---|
| **Boundary and dependency policy** | Existing Promise/AbortController/timers/queues; no new runtime dependency in current manifests. [observed; `package.json:101-131`, `services/uta/package.json:14-30`; inferred mechanics] | Add `effect` to UTA Core only. Keep Pack API, protocol, Alice SDK, and generated/provider adapters Promise/Zod/library-neutral; wrap Promise calls at the core edge. [inferred] This fits the existing dynamic file-URL Pack boundary. [observed; registry/docs above] | Add `effect`, `@effect/platform`, and likely runtime adapter package(s) to every layer that shares Schema/HttpApi. [inferred] This couples protocol/client/Packs to Effect types and increases migration surface. |
| **Measured package/bundle numbers** | No added package bytes; no new bundle measurement was taken. [observed; current manifests] | `effect@3.22.2` unpacked metadata: `27,163,958` bytes; installed directory: ~`33M`. Basic minified microbundle: `121,350` bytes versus bare `56` (`+118.5 KiB`); compiled executable delta: `+264,192` bytes. [observed; scoped npm/stat/Bun outputs] | `@effect/platform@0.97.2` unpacked metadata: `19,891,956` bytes; installed directory: ~`21M`; platform-bun `0.91.2` unpacked `286,573`; platform-node `0.108.2` unpacked `403,892`. HttpApi microbundle: `199.48 KB`, `545` modules; Schema microbundle: `173.92 KB`, `417` modules. [observed; npm metadata and scoped Bun outputs] |
| **Learning curve** | Lowest. [inferred] Team uses existing Promise/Zod/Hono patterns; lifecycle and retry correctness must be learned as local conventions. [observed current conventions; inferred rating] | High but contained. [inferred] Contributors must learn `Effect<A,E,R>`, Context tags, Layers, fibers/scopes, schedules, and error handling; the boundary keeps the rest of the monorepo familiar. | Highest. [inferred] Adds B plus Schema's encoded/decoded types, HttpApi endpoint/group/client/builders, generated OpenAPI conventions, and cross-package Effect ownership. |
| **Bun compatibility** | Uses the existing product toolchain. [observed; `.bun-version:1`, root Bun feasibility script] | Effect core ran under Bun `1.4.1` and compiled with `bun build --compile`; the 100-WebSocket scope smoke passed. [observed; outputs above] Exact repository-pinned `1.4.0` acceptance was unavailable because this shell is `1.4.1` and the feasibility script rejects that mismatch. [observed; `scripts/build-bun-runtime-feasibility.ts:10-14`] | Effect Schema and `@effect/platform` HttpApi ran under Bun `1.4.1`, generated `3.1.0`, and compiled in the microprobe. [observed; outputs above] The same pinned-version limitation remains, and full UTA/Pack compiled loading is untested. [observed/unavailable] |
| **Testability** | Custom dependency injection, fake timers, and test queues are possible. [inferred] Current timer code uses native `setInterval`/`setTimeout`. [observed; paths above] | Layers can swap live/test services; TestClock controls time without waiting; scopes/fibers make cleanup testable. [observed; Effect layer/TestClock docs and output] This is the strongest fit for reconnect, poll, and shutdown behavior. [inferred] | Same B benefits plus Schema decode/encode and HttpApi error/status declarations can test wire boundaries. [observed; Schema/HttpApi declarations] It also requires testing a larger type/runtime boundary. [inferred] |
| **Errors and HTTP** | Current Hono/Zod route maps parse/throw failures manually; wire shape already has stable code/transient/hint. [observed; route/protocol paths above] Plain discriminated unions can preserve it. [inferred] | Use typed internal error variants (`Data.TaggedError` or domain ADTs), then one explicit adapter to `WireBrokerError`/HTTP status. [inferred] Effect does not automatically select HTTP status or persist a ledger. [inferred] | `HttpApiEndpoint.addError` attaches error schemas/status metadata and `HttpApiBuilder` connects handlers; `OpenApi.fromApi` emits a first-party OpenAPI 3.1 spec. [observed; declarations/npm README] Provider error translation and unknown-write policy remain domain work. [inferred] |
| **Provider OpenAPI projections** | External generated clients/adapters can target each provider's OpenAPI projection; UTA translates into provider-indexed domain types. [inferred] | Same, with Promise→Effect adapters inside UTA. [inferred] No Effect importer is needed. | Still needs external generation/adapter: observed public declarations expose generation, not arbitrary OpenAPI import. [observed; declaration search above] |
| **Fit to this UTA** | Safest dependency-wise; higher handwritten lifecycle/concurrency policy. [inferred] | **Recommended:** buys safety where UTA is genuinely concurrent/resourceful while preserving stable wire/Pack seams. [inferred] | Defer: choose only if UTA's own API must be declared once and generated OpenAPI/client/Swagger value outweighs migration and coupling. [inferred] |

## Schema/Zod and provider OpenAPI decision

### Can Effect Schema replace Zod?

- **Technical answer: yes for a new internal schema surface.** [observed] Effect Schema v3 supports decode/encode/assert operations and has `Schema<Type, Encoded, Requirements>`; it requires TS 5.4+ and strict mode. [observed; v3 Schema docs cited above]
- **Repository answer: do not replace the current Zod boundary in this phase.** [inferred] Existing conventions use Zod for config, the Pack API exports `configSchema: ZodType`, the protocol package describes shared Zod schemas, and current UTA routes call Zod parse/`zValidator`. [observed; `AGENTS.md:182-187`, `docs/broker-packs.md:34-41`, `packages/uta-protocol/src/schemas/index.ts:1-10`, `services/uta/src/http/routes-trading.ts:594-601`]
- **OpenAPI answer: partial, not a full contract migration.** [observed] `JSONSchema.make(..., { target: 'openApi3.1' })` can emit a schema fragment; transformations may be omitted at the first transformation. `@effect/platform` can emit a complete OpenAPI 3.1 document from an Effect `HttpApi`. [observed; docs/declarations/probes above] It does not make current provider-specific schemas identical, nor does it preserve every transformation automatically. [inferred]
- **Recommended migration rule:** keep Zod at Pack/config and existing HTTP/wire boundaries; optionally use Effect Schema for a new UTA-internal boundary only when the owning API explicitly needs its encoded/decoded model or generated OpenAPI. Do not expose `Schema` values through `@traderalice/uta-protocol` until Alice, UTA, and Packs have an intentional coordinated contract migration. [inferred]

### Can Effect consume provider OpenAPI projections?

- **Observed direction:** Effect Schema emits JSON Schema; `OpenApi.fromApi` emits OpenAPI from an Effect-declared API. [observed; installed declarations]
- **Observed absence:** the scoped declaration search found no importer for an arbitrary OpenAPI/JSON Schema document in the installed v3 packages. [observed; command output above]
- **Design consequence:** provider projections can remain intentionally non-identical. [inferred] For each provider, generated or hand-authored code should parse the provider document, map provider-specific fields/errors/cursors into provider-indexed UTA types, and expose Promise functions that UTA wraps. Effect can orchestrate that adapter; it cannot infer upstream semantics or idempotency from an OpenAPI document. [inferred]

## Recommendations (separate from observed facts)

1. **Adopt Option B in the UTA service only.** [inferred] Treat `Effect` as the internal interpreter for IO orchestration, not as the persisted/domain/wire model.
2. **Keep a Promise-facing Pack boundary for now.** [inferred] Define a small adapter at the UTA composition root (`tryPromise`/equivalent) that translates unknown thrown values into named domain error variants; do not make dynamically loaded Packs import `Effect` merely to satisfy the core.
3. **Use mandatory Context requirements for mandatory capabilities.** [inferred] Never use `serviceOption` for a capability required by the selected provider/account; that would recreate the forbidden optional-handler matrix even though the rest of the program typechecks.
4. **Use `Effect.scoped`/`acquireRelease` for every long-lived upstream connection.** [inferred] Use resourceful `Stream` for market/news sequences, a bounded queue for fan-out pressure, and scoped fibers for account/watch lifetimes. The local 100-socket probe establishes feasibility, not provider throughput. [observed probe; inferred production rule]
5. **Serialize only the external mutation path.** [inferred] A per-account/provider semaphore or worker queue can enforce one logical ledger entry at a time, while unrelated reads/market streams remain concurrent. Idempotency keys, keyed readback, receipt classification, unknown outcomes, reversal entries, and persistence stay in explicit domain handlers. [inferred]
6. **Use typed retry schedules only after error classification.** [inferred] `Effect.retry` should receive a schedule selected from declaration/transport/receipt semantics; never apply a generic retry to an unknown placement outcome. [inferred from A5/A7 and observed Schedule behavior]
7. **Keep HTTP translation explicit.** [inferred] Have one adapter map internal expected errors to `WireBrokerError` and HTTP status; do not leak `Cause`, `FiberFailure`, or `Schema` values into the protocol. [inferred]
8. **Defer Option C.** [inferred] Reconsider `effect/Schema` + `@effect/platform` only when there is a concrete requirement to generate UTA's own OpenAPI/client/Swagger surface and a measured decision to migrate the shared wire contract. It is not the mechanism for importing arbitrary provider projections. [inferred]
9. **Acceptance gate before calling B complete:** run the full UTA service under the pinned Bun version, load an actual compiled active Pack through the documented file-URL path, exercise a 100+ connection provider-like stream, stop/restart the scope, and observe downstream event delivery plus ledger/readback behavior. [inferred; current probes cover only local sockets/type/runtime]

## Unavailable / contradictions

- The repository pins Bun `1.4.0`, while this shell has Bun `1.4.1`; the exact pinned compile/Pack acceptance therefore remains unavailable. [observed; `.bun-version:1`, feasibility guard `scripts/build-bun-runtime-feasibility.ts:10-14`]
- The 100-WebSocket result uses a local Bun server and no broker/OpenAPI provider, so it proves resource cleanup and concurrency mechanics only—not provider rate limits, latency, reconnect correctness, or downstream fan-out durability. [observed; `/tmp/w1-effect-eval/ws-100-smoke.ts:6-41`; inferred limits]
- No full UTA bundle/startup/RSS benchmark was run; microbundle and package sizes are directional, not a production budget result. [observed; scoped probe scope]
- The Effect v3 website has core/Schema/concurrency/resource pages, but the attempted v3 platform HttpApi URL was unavailable and the current platform introduction is v4-oriented. Platform claims here use the installed v3-compatible declarations and npm README; version drift should be rechecked before implementation. [observed; URL read outcomes and package metadata]
- The installed declaration search did not find an OpenAPI importer; this is evidence about the selected package versions/public declarations, not proof that no third-party generator exists. [observed; scoped grep output; inferred limitation]
- Current repository guidance mentions TypeBox for tool schemas, but the scoped manifests/source search found no `typebox`, `@sinclair`, or `Type.Object` match. [observed; scoped grep output] This report does not resolve that unrelated convention discrepancy.
- Duplicate-Effect probes imported two physical copies and showed same-key Context/Layer and tagged-error handling working for simple values, while constructors were not identical (`sameTaggedErrorConstructor:false`). [observed; scoped duplicate-effect outputs] This is not a released Pack compatibility proof; the recommendation remains to keep Effect out of the dynamic Pack API unless dependency pinning/peer ownership is deliberate.
- No production code, package manifest, lockfile, test, formatter, linter, or project-wide build was changed or run for this evidence wave. [observed; worktree/tool scope]
