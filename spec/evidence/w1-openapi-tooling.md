# OpenAPI tooling research for provider spec-in / typed client / parser-out

## Method and version context

- [Observed] Evidence below comes from canonical vendor docs plus local shallow checkouts/package manifests; no OpenAlice files were edited and no formatter, linter, or project-wide check was run.
- [Observed] `openapi-typescript` 7.13.0 and `openapi-fetch` 0.17.0 are the versions in the official package manifests/registry latest metadata: https://registry.npmjs.org/openapi-typescript/latest and https://registry.npmjs.org/openapi-fetch/latest.
- [Observed] Orval is 8.33.0: https://registry.npmjs.org/orval/latest and official manifest `packages/orval/package.json:1-4` (repo: https://github.com/orval-labs/orval).
- [Observed] `@hey-api/openapi-ts` is 0.99.0: https://registry.npmjs.org/@hey-api%2fopenapi-ts/latest and official manifest `packages/openapi-ts/package.json:1-4` (repo: https://github.com/hey-api/hey-api). The old `https://heyapi.dev/openapi-ts` aliases redirect to canonical `https://heyapi.dev/docs/openapi/typescript/get-started`.
- [Observed] `@effect/platform` is 0.97.2 with peer `effect: ^3.22.2`: official package manifest `package.json:1-19`, registry metadata https://registry.npmjs.org/@effect%2fplatform/latest. Effect main currently also exposes `effect/unstable/httpapi` (`@since 4.0.0` in https://github.com/Effect-TS/effect/tree/main/packages/effect/src/unstable/httpapi); do not mix v3 and v4 imports.
- [Observed] `typed-openapi` is 4.0.1 in registry metadata https://registry.npmjs.org/typed-openapi/latest and official package manifest `packages/typed-openapi/package.json:1-4` (repo: https://github.com/astahmer/typed-openapi). Its README help block still prints `typed-openapi/3.0.0` (`README.md:139-189`), so use the installed CLI version rather than that embedded banner.
- [Inference] No common fixture was run through every generator; preservation of arbitrary vendor extensions and cross-tool output equivalence remain unproven. The fit labels below are documented-fit judgments, not a winner claim.

## Findings by tool

### `openapi-typescript` + `openapi-fetch`

- [Observed] Direction is OpenAPI 3.0/3.1 YAML or JSON (local or remote) -> runtime-free TypeScript declarations; the official README states this explicitly (`packages/openapi-typescript/README.md:3,12-17,53-65`): https://openapi-ts.dev/introduction and https://openapi-ts.dev/cli.
- [Observed] `openapi-fetch` consumes the generated `paths` type; its docs show `createClient<paths>`, typed path/params/body, and GET/PUT/POST wrappers over native `fetch`: https://openapi-ts.dev/openapi-fetch/.
- [Observed] Runtime implementation calls the configured/global Fetch, parses JSON/text/blob/arrayBuffer, or exposes a raw stream; successful calls return `{ data, response }`, documented HTTP failures return `{ error, response }` (`packages/openapi-fetch/src/index.js:168-279`): https://github.com/openapi-ts/openapi-typescript/tree/main/packages/openapi-fetch.
- [Observed] The response source path uses native `JSON.parse`/`response[parseAs]()` and has no generated schema/value-validator hook (`src/index.js:248-279`); the docs describe the client as having “virtually zero runtime”: https://openapi-ts.dev/openapi-fetch/api.
- [Inference] This is a strong provider-spec-in + compile-time typed-client base, but parser-out must be a separate boundary (Zod, Effect Schema, etc.). Do not treat TypeScript response types as validation.
- [Observed] Middleware has `onRequest`, `onResponse`, and `onError`, so auth/header injection and centralized parsing can be layered without changing generated declarations: https://openapi-ts.dev/openapi-fetch/middleware-auth.
- [Observed] `parseAs: "stream"` skips parsing; HTTP streaming is represented, but no reviewed OpenAPI TypeScript page documents AsyncAPI or a WebSocket client. `openapi-fetch` source accepts `parseAs` but does not itself define a WS protocol (`src/index.d.ts:73-85`; `src/index.js:248-264`).
- [Observed] Its generated helper types deliberately use `Writable` for request bodies and `Readable` for responses, with comments that read-only fields are excluded from request bodies and write-only fields from responses (`src/index.d.ts:101-122`).
- [Observed] Only selected extensions are consumed by the generator: the documented `x-enum-varnames`/`x-enum-descriptions` become enum names/comments: https://openapi-ts.dev/advanced. [Inference] Arbitrary `x-uta` metadata is not established as preserved in generated declarations; extract it separately or prove it with a fixture.

### Orval

- [Observed] Direction is OpenAPI v3 or Swagger v2 (YAML/JSON) -> generated TypeScript models/request functions/mocks; overview and basics docs state the input/output contract: https://orval.dev/docs/ and https://orval.dev/docs/guides/basics/.
- [Observed] Output chooses generated client style (`fetch`, axios, React Query, SWR, `effect`, etc.) and HTTP transport; Fetch output emits a response type, URL generator, and Fetch function (`https://orval.dev/docs/guides/fetch/`). Orval v8 changed the default `httpClient` to Fetch: https://orval.dev/docs/versions/v8/.
- [Observed] Zod schemas are generated from OpenAPI (`client: 'zod'` or `schemas: { type: 'zod' }`) and are usable with `.parse()`/`.safeParse()`: https://orval.dev/docs/guides/zod/.
- [Observed] Built-in Fetch response validation is opt-in via `override.fetch.runtimeValidation`; generator source only enables it when a non-primitive schema exists, schemas are Zod, and the response is not NDJSON (`packages/fetch/src/index.ts:448-466`): https://github.com/orval-labs/orval/tree/main/packages/fetch.
- [Observed] The generated Fetch path parses JSON then applies the emitted response-validation expression; with `strategy: 'both'` it logs before rethrowing. A custom mutator owns the request and bypasses the generated parse; `includeZodSchemaInArguments` can pass the Zod schema to that mutator (`packages/fetch/src/index.ts:653-680,758-794`; configuration docs: https://orval.dev/docs/reference/configuration/output/).
- [Observed] Validation is skipped for primitive/void, non-JSON/NDJSON, and custom-mutator paths in the documented configuration/source. [Inference] Orval + Zod Fetch is a good parser-out path only when those exclusions are acceptable; custom transports must explicitly parse.
- [Observed] `client: 'effect'` generates Effect Schema modules from the same OpenAPI input: https://orval.dev/docs/guides/effect/. The combined-client guide generates the HTTP client and Effect schemas as separate outputs and demonstrates manual `Schema.decodeUnknownSync` before sending: https://orval.dev/docs/guides/client-with-effect/.
- [Inference] Treat Orval Effect output as generated boundary schemas, not an automatically response-validating Effect transport. Verify the exact response parser wiring in a fixture before relying on it.
- [Observed] HTTP streaming documented by Orval is NDJSON, Fetch-only; the guide gives a typed stream response and asks the consumer to implement line parsing: https://orval.dev/docs/guides/stream-ndjson/. No reviewed Orval page documents AsyncAPI/WS generation.
- [Inference] Orval is a strong multi-file provider-spec-in/client choice, with an opt-in Zod parser-out for built-in Fetch; it is less direct for a single canonical boundary module or Effect-native transport.

### `@hey-api/openapi-ts`

- [Observed] Direction is OpenAPI input -> plugin-selected TypeScript output; input can be a path, URL, registry reference, inline object, or OpenAPI object, and docs say all valid OpenAPI versions/formats are accepted: https://heyapi.dev/docs/openapi/typescript/get-started and https://heyapi.dev/docs/openapi/typescript/configuration/input.
- [Observed] The SDK plugin emits typed functions/methods per operation, with built-in auth handling and configurable request/response validation: https://heyapi.dev/docs/openapi/typescript/plugins/sdk.
- [Observed] Fetch is a thin wrapper around Fetch API with configuration, custom fetch, auth, request/response interceptors, and original response access: https://heyapi.dev/docs/openapi/typescript/clients/fetch.
- [Observed] Zod is an optional plugin that generates request-layer, response, and reusable-definition schemas; multiple endpoint responses become a Zod union: https://heyapi.dev/docs/openapi/typescript/plugins/zod.
- [Observed] SDK validation is not enabled by default due to runtime cost; `validator` can select a validator and can target request and response independently (SDK docs, Validators section). The native validator list is Valibot, Zod, Ajv, ArkType, Joi, TypeBox, and Yup: https://heyapi.dev/docs/openapi/typescript/validators.
- [Observed] The current Effect client page says “Planned / Vote to prioritize,” not an available Effect client: https://heyapi.dev/docs/openapi/typescript/clients/effect. [Inference] Do not assume Hey API emits Effect Schema or an Effect-native client; the documented parser-out route here is Zod (or one of its listed validators).
- [Observed] Parser patches run before parsing/plugins and can add or repair input fields; parser validation is marked experimental/limited: https://heyapi.dev/docs/openapi/typescript/configuration/parser. [Inference] This is an extension hook, not proof that arbitrary provider `x-*` metadata is preserved in generated artifacts.
- [Observed] Reviewed canonical Hey API pages are HTTP-client/plugin pages; no AsyncAPI or WebSocket generator is documented in those pages. [Inference] Keep WS/message parsing outside this OpenAPI client until a separate official protocol path is selected.
- [Inference] Hey API is a strong provider-spec-in + generated SDK + optional Zod parser-out choice, but pin the exact version (the get-started page explicitly says the package is in initial development and recommends pinning) and verify extension/read-write semantics with a fixture.

### Effect `HttpApi` / `@effect/platform`

- [Observed] This is code-first/reverse direction: the official `@effect/platform@0.97.2` declaration says `OpenApi.fromApi(api)` converts a declarative `HttpApi` into an OpenAPI 3.1.0 object with paths, operations, security, components, annotations, and deduplication (`dist/dts/OpenApi.d.ts:120-172`; package URL: https://registry.npmjs.org/@effect/platform/-/platform-0.97.2.tgz).
- [Observed] `HttpApiEndpoint` stores schemas for path, URL params, payload, headers, success, and errors; endpoint declarations use those schemas to validate request bodies (`dist/dts/HttpApiEndpoint.d.ts:49-100`).
- [Observed] `HttpApiClient` derives a typed method per endpoint; its return type includes declared errors, `HttpClientError`, and `ParseResult.ParseError` (`dist/dts/HttpApiClient.d.ts:15-70`).
- [Observed] The v3 client implementation encodes request schemas, executes the Effect `HttpClient`, then decodes the response with `Schema.decode`/content-type-specific codecs (`dist/esm/HttpApiClient.js:23-91,190-252`). Thus runtime parsing is first-class when the API is declared in Effect schemas.
- [Observed] The official Effect HTTP API guide describes one definition powering server, documentation, and typed client, and shows `HttpApiScalar`/`HttpApiSwagger` plus `HttpApiClient`: https://github.com/Effect-TS/effect/blob/main/packages/effect/HTTPAPI.md.
- [Inference] `@effect/platform` is not a provider OpenAPI-spec-in generator; it is a good code-first internal API contract and runtime parser/client, but unsuitable as the primary UTA provider projection unless another importer converts provider OpenAPI into `HttpApi` (not documented here).
- [Observed] Client types include SSE/Uint8Array stream success forms (`HttpApiClient` source/dts), while OpenAPI generation remains HTTP-path based. No AsyncAPI generation was found in the reviewed package/docs. [Inference] Treat WebSocket as a separate protocol/transport boundary, not an `OpenApi.fromApi` result.

### `typed-openapi`

- [Observed] Direction is OpenAPI path/URL -> one generated TypeScript client; default runtime `none` emits types, endpoint definitions, and a headless client, while `--default-fetcher` emits a Fetch transport: https://typed-openapi-docs.vercel.app/getting-started/ and https://typed-openapi-docs.vercel.app/clients/promise-client/.
- [Observed] Runtime adapters include Zod v4/v3, Effect Schema v4/v3, Valibot, ArkType, TypeBox, and Typia; `none` is types-only. All adapters share the same input/client API: https://typed-openapi-docs.vercel.app/validation/runtimes/.
- [Observed] `--validate-side` is `none|input|output|both`, defaulting to both with a runtime; output validates successful response data and input validates path/query/cookie/header/body parameters: https://typed-openapi-docs.vercel.app/validation/input-output/.
- [Observed] Generated client source runs input validation before URL construction/fetch, parses response data, then runs output validation selected by status; `responseFormat: 'sse'` bypasses output validation (`packages/typed-openapi/src/generator.ts:1234-1355`; `src/effect-api-client.ts:74-101`).
- [Observed] `--default-fetcher` handles URL/query/cookie/body encoding and OpenAPI `securitySchemes`; the generated API client parses responses. The docs also expose a `withResponse` status/data mode: https://typed-openapi-docs.vercel.app/clients/requests-auth-and-bodies/ and https://typed-openapi-docs.vercel.app/clients/promise-client/.
- [Observed] Effect-native mode (`--client effect`) returns an Effect; status errors are `TypedStatusError`, while transport/decode/validation/parse failures become `HttpClientError` with the original cause: https://typed-openapi-docs.vercel.app/clients/effect-client/ and `packages/typed-openapi/API_CLIENT_EXAMPLES.md:159-168`.
- [Observed] README documents readOnly/writeOnly stripping, date/bigint transforms, SSE as `ReadableStream`, and a runtime type sidecar; named `$ref` components remain shared for read/write stripping (`README.md:33-61`).
- [Inference] This is a direct documented provider-spec-in + typed client + boundary parser-out fit when a single generated module and multi-runtime choice matter. A headless custom fetcher remains possible, while `--default-fetcher` covers ordinary HTTP/auth.
- [Observed] SSE output is explicitly supported, but the reviewed docs/source do not document AsyncAPI or WebSocket input/client generation. [Inference] WS/message validation still needs a separate protocol path.

## Cross-tool decisions for UTA

- [Inference] Types-only path: `openapi-typescript` + `openapi-fetch` keeps upstream OpenAPI shapes closest to the generated TypeScript surface and gives typed read/write request/response views, but requires a separately owned parser-out boundary.
- [Inference] Validator-backed path: `typed-openapi` has an explicit input/output validation switch and Zod/Effect choices in one generated client; Orval and Hey API are viable when their generated Fetch/SDK integration and multi-file layout are preferred.
- [Inference] Effect distinction: Effect `HttpApi` itself is code-first; use `@effect/platform` for APIs authored in Effect, not as a reverse provider-spec importer. `typed-openapi --runtime effect` is the OpenAPI-in alternative.
- [Inference] No candidate advertises UTA domain semantics such as provider-indexed partial capability, idempotent placement, or mandatory read-by-key declarations. Preserve those as explicit domain types/metadata around generated projections; do not infer them from endpoint generation.
- [Inference] Arbitrary extension preservation, read-only/read-write behavior for every tool, and status/union edge cases require one shared fixture before committing to a generator. Include `$ref`, `readOnly`/`writeOnly`, unions, multiple statuses/content types, security, and an `x-uta` marker.

## Streaming and boundary caveats

- [Observed] `openapi-fetch` exposes raw HTTP streams via `parseAs: "stream"`; it does not run a schema parser on those bytes (`https://openapi-ts.dev/openapi-fetch/api`; `packages/openapi-fetch/src/index.js:248-264`).
- [Observed] Orval documents Fetch-only NDJSON and consumer-owned line parsing (`https://orval.dev/docs/guides/stream-ndjson/`).
- [Observed] typed-openapi maps SSE to `ReadableStream` and skips output validation for SSE (`https://typed-openapi-docs.vercel.app/clients/promise-client/`; `packages/typed-openapi/src/generator.ts:1314-1343`).
- [Observed] Effect HttpApi/Client has typed HTTP stream forms and schema decoding, but `OpenApi.fromApi` is still an HTTP OpenAPI document generator (`@effect/platform` dts cited above).
- [Inference] Treat OpenAPI HTTP response parsing and WebSocket/AsyncAPI message parsing as separate boundaries. The reviewed official pages did not establish an AsyncAPI integration for any candidate.

## Primary URL index

- openapi-typescript: https://openapi-ts.dev/introduction · https://openapi-ts.dev/cli · https://openapi-ts.dev/openapi-fetch/ · https://openapi-ts.dev/openapi-fetch/api · https://openapi-ts.dev/openapi-fetch/middleware-auth
- Orval: https://orval.dev/docs/ · https://orval.dev/docs/guides/fetch/ · https://orval.dev/docs/guides/zod/ · https://orval.dev/docs/reference/configuration/output/ · https://orval.dev/docs/guides/effect/ · https://orval.dev/docs/guides/stream-ndjson/
- Hey API: https://heyapi.dev/docs/openapi/typescript/get-started · https://heyapi.dev/docs/openapi/typescript/configuration/input · https://heyapi.dev/docs/openapi/typescript/plugins/sdk · https://heyapi.dev/docs/openapi/typescript/plugins/zod · https://heyapi.dev/docs/openapi/typescript/clients/fetch
- Effect: https://github.com/Effect-TS/effect/blob/main/packages/effect/HTTPAPI.md · https://registry.npmjs.org/@effect/platform/-/platform-0.97.2.tgz
- typed-openapi: https://typed-openapi-docs.vercel.app/getting-started/ · https://typed-openapi-docs.vercel.app/validation/input-output/ · https://typed-openapi-docs.vercel.app/validation/runtimes/ · https://typed-openapi-docs.vercel.app/clients/effect-client/ · https://github.com/astahmer/typed-openapi
