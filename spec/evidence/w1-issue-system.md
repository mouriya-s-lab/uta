# Workspace Issue system evidence for UTA ↔ Alice

## Decision questions answered

1. **Persisted contract:** Issues are workspace-local Markdown files at `.alice/issues/<id>.md`; the filename stem is the Issue id, the frontmatter is a closed validated set (no labels), and canonical What is the body. Comments are a version-1 JSON sidecar with mutable delivery/progress projections. [observed: `src/workspaces/issues/declaration.ts:66-74,156-200,276-283`; `src/workspaces/issues/comments.ts:1-15,98-120`]
2. **Execution:** The scanner is a first-after-interval, 60-second polling loop. It reads live files, calculates due-ness from the declaration plus a launcher marker, dispatches one headless task, then advances the marker only after a durable run is accepted. [observed: `src/workspaces/schedule/scanner.ts:59,149-169,231-269,275-343`; `src/workspaces/schedule/scanner.ts:345-387`]
3. **Ingress:** HTTP Issue writes, loopback MCP/CLI tools, and direct file writes exist. There is no UTA-specific Issue endpoint or Issue-capable Electron IPC surface in the inspected code. The inspected Connector/Telegram path is the established external-process precedent with a durable queue, claims, leases, and event-derived deduplication. [observed: `src/webui/routes/issues.ts:90-161,164-437`; `src/server/mcp.ts:215-270`; `src/webui/workspaces-ipc.ts:1-9,17-35,83-156`; `services/connector/src/main.ts:75-132`; `services/connector/src/core/work-queue.ts:46-109`]
4. **Approval:** Current trading proposals are UTA staged/committed Git state; the commit hash is the durable decision identity. `tradingPush` refuses venue side effects by default, while Web/Telegram approval sends an expected pending hash to UTA. Current push/reject calls do not carry an actor or decision correlation into UTA. [observed: `src/tool/trading.ts:185-212,671-718,779-870`; `services/uta/src/http/routes-trading.ts:474-527`; `packages/connector-protocol/src/types.ts:287-310`; `docs/conversation-provenance.md:467-506`]
5. **Failure/duplicates:** Same-process Issue comment mutations are serialized, and deterministic ids make Connector/reply paths replay-safe; Issue creation, human HTTP comments, and cross-process file/sidecar writes lack a request-level idempotency contract. There are crash windows around dispatch-marker ordering and swallowed marker/registry flush errors. [observed: `src/workspaces/issues/comments.ts:143-180,199-239`; `src/workspaces/issues/mutate.ts:340-422`; `src/workspaces/schedule/scanner.ts:359-387`; `src/workspaces/schedule/marker-store.ts:73-106`; `src/workspaces/headless-task-registry.ts:178-200,203-245,358-373`; inference where stated below]

## Observed facts

### 1. Persisted Issue schema and identity

#### Canonical Issue file

| Field/layer | Exact contract and behavior |
|---|---|
| Path and id | `<workspace>/.alice/issues/<id>.md`; `id` is the filename stem, accepted by `^[a-zA-Z0-9][a-zA-Z0-9_-]*$`. A legacy `.alice/issue.json` is reported as retired rather than silently read. [observed: `src/workspaces/issues/declaration.ts:66-74,341-379`; `src/workspaces/issues/mutate.ts:122-135`] |
| `title` | Required non-empty string. [observed: `src/workspaces/issues/declaration.ts:160-163`] |
| `status` | `backlog / todo / in_progress / done / canceled`; default `todo`. `done` and `canceled` are terminal for scheduling. [observed: `src/workspaces/issues/declaration.ts:76-107,297-300`] |
| `priority` | `urgent / high / medium / low / none`; default `none`. [observed: `src/workspaces/issues/declaration.ts:76-82,160-165`] |
| `assignee` | `@new-each-run`, `@new-then-resume`, `@human`, `@unassigned`, or exact `@resume-...`; scheduled Issues cannot use `@human`/`@unassigned`, and fresh-owner modes require `when`. Omission defaults to `@new-then-resume` for scheduled work and `@unassigned` otherwise. [observed: `src/workspaces/issues/declaration.ts:127-153,203-251`; `src/tool/issue-tools.ts:151-177`] |
| `when` | Optional discriminated schedule: `{kind:'at',at}`, `{kind:'every',every}`, or `{kind:'cron',cron,timezone?,catchUp?}`. `timezone` is `local` or a valid IANA zone; omitted `catchUp` means catch up. [observed: `src/workspaces/issues/declaration.ts:110-125`; docs `docs/workspace-issues-and-scheduling.md:78-81,194-228`] |
| Runtime tuple | Optional `agent`, secret-free `credential` slug, mutually exclusive `credentialSource:'native'`, `model`, `effort`, and optional `timeout` (`15m|30m|45m|60m`). An exact Session owner cannot carry the runtime tuple; `timeout` is still allowed. [observed: `src/workspaces/issues/declaration.ts:172-192,227-261`; `src/workspaces/issues/mutate.ts:57-68,212-219`] |
| `commentPrompt` | Optional template; it must include `{comment}`. Supported tokens are `{comment,title,id,workspaceId,author,what}`; null/removal restores the default. [observed: `src/workspaces/issues/declaration.ts:190-192,262-271`; docs `docs/workspace-issues-and-scheduling.md:100-111`] |
| Connector fields | `connectorDesk` is a constrained adapter id and is settings-only; `telegramConnector:true` is a read-compatibility alias transformed to `connectorDesk:'telegram'`. `execution` is explicitly forbidden. [observed: `src/workspaces/issues/declaration.ts:193-225`; `src/workspaces/issues/mutate.ts:286-305,396-406`; `src/webui/routes/connectors.ts:111-181`] |
| Canonical What | Markdown below the closing frontmatter fence. `parseIssueContent` merges legacy frontmatter `what` only for compatibility; new mutations remove that key. Scheduled dispatch sends `issue.what` unchanged. [observed: `src/workspaces/issues/declaration.ts:333-337,422-451`; `src/workspaces/issues/mutate.ts:321-337`; docs `docs/workspace-issues-and-scheduling.md:152-162`] |
| Labels | No `labels`/label field exists in the Issue schema, mutation patch, or board/detail projection. `priority` is the only priority-like classification. [observed: `src/workspaces/issues/declaration.ts:160-200`; `src/workspaces/issues/mutate.ts:49-81`; `src/workspaces/issues/board.ts:38-71,242-269`] |
| Derived, not file state | `lastFiredAtMs`, `nextDueAtMs`, and `automationHealth` are projections from the schedule marker, latest run, owner, and runtime. They are not frontmatter. [observed: `src/workspaces/issues/board.ts:54-65`; docs `docs/workspace-issues-and-scheduling.md:319-339`] |

The parser validates recognized fields with a Zod object; the mutation helper reads raw frontmatter and intentionally preserves author-written keys before reserializing, while the parsed `IssueRecord` exposes only the recognized schema. Therefore an arbitrary `uta:*` YAML key is not a typed extension point: it can be invisible to readers and is not safe linkage storage. [observed: `src/workspaces/issues/declaration.ts:160-203,273-283`; `src/workspaces/issues/mutate.ts:169-177,327-337`]

#### Comments and identity

- Sidecar path is `.alice/issues/<id>.comments.json`; persisted root is `{version:1, issueId, comments:[]}`. Each comment is `{id,author,at,markdown,replyTo?,delivery?,via?}`. [observed: `src/workspaces/issues/comments.ts:23-34,98-120`]
- `delivery` is a tagged union: `pending {targetResumeId,taskId,progress?}`, `replied {targetResumeId,taskId,replyCommentId}`, or `failed {targetResumeId?,taskId?,error}`. Progress is a bounded structured snapshot of text/tool/error blocks and counts; it is removed from the task record when terminal. [observed: `src/workspaces/issues/comments.ts:36-95`; `src/workspaces/service.ts:2050-2073`; `src/workspaces/headless-task-registry.ts:265-279`]
- Append accepts an optional deterministic `id`; a duplicate id returns the existing comment with `created:false`. Without an id, the helper generates `comment-<randomUUID>`. Reply ids are `comment-reply-<taskId>`; Connector inbound ids are derived from connector event ids when available. [observed: `src/workspaces/issues/comments.ts:138-149,182-219`; `src/workspaces/issues/comment-delivery.ts:144-169`; `src/workspaces/issues/telegram-desk-chat.ts:123-134`]
- The in-process mutation chain serializes complete read-modify-write operations per `(workspaceDir,issueId)`, covering comments, delivery, and progress. There is no cross-process lock. [observed: `src/workspaces/issues/comments.ts:157-180`]

#### Creation, writers, and readers

- `createIssue` derives a kebab slug from title when `id` is omitted, refuses an existing id, validates the assembled frontmatter, and defaults What to `what || body || title`. It has no idempotency-key parameter; a retried successful request returns `conflict`, not the original result. [observed: `src/workspaces/issues/mutate.ts:83-120,128-146,340-422`]
- The existence check and write are separate, and `writeWorkspaceFile` calls plain `writeFile` after a symlink check rather than a temporary-file rename. Concurrent create races and a crash during an Issue/sidecar write can therefore lose or truncate data. This is an inference from the implementation; it contradicts the `writeWorkspaceFile` “Atomically write” comment. [observed: `src/workspaces/issues/mutate.ts:359-421`; `src/workspaces/file-service.ts:117-142`; **inferred:** race/torn-write consequences]
- Agent-facing `issue_create`, `issue_update`, and `issue_comment` write only the current Workspace’s files; their comments sign the authoritative Session as `@resumeId` when available. Global list/show reads scan all workspaces, while writes remain local. [observed: `src/server/cli-commands.ts:175-188`; `src/tool/issue-tools.ts:163-209,308-397,401-479,484-569,599-649`]
- Human Web writes use `PATCH /api/issues/:wsId/:id` and `POST /api/issues/:wsId/:id/comments`; the route passes through the shared mutation seam, fixes the comment author to `human`, and records `{kind:'human'}` provenance. It has no request-id field. [observed: `src/webui/routes/issues.ts:164-191,348-370,374-437`]
- Direct native file edits are valid. The 60-second scanner observes live files and `IssueChangeTracker` compares a launcher-owned compact snapshot; if the source cannot be uniquely attributed, provenance is recorded as unknown/direct-file-edit rather than guessed. [observed: `src/workspaces/issues/declaration.ts:341-412`; `src/workspaces/issues/change-tracker.ts:1-9,129-217`; `src/workspaces/service.ts:812-875`]
- Issue identity is `{workspaceId, issueId}`; `issueId` is not globally unique. `taskId`/`runId` identifies one headless turn, `resumeId` identifies a durable product Session, and native `agentSessionId` is backend-only. [observed: `docs/conversation-provenance.md:142-190,192-233`; `src/workspaces/headless-task-registry.ts:89-143`]

### 2. Agent execution, comments, Inbox, and provenance

The scanner starts only after one interval, then polls every 60 seconds. Every scan reads each registered Workspace’s live `.alice/issues` directory, isolates malformed files, ignores unscheduled board items, and uses `when + marker` due math. [observed: `src/workspaces/schedule/scanner.ts:59,149-169,231-335`; `src/workspaces/issues/declaration.ts:346-396`]

```mermaid
sequenceDiagram
    participant S as ScheduleScanner
    participant M as MarkerStore
    participant D as dispatchIssue
    participant C as SessionCoordinator/ResumeRegistry
    participant R as HeadlessTaskRegistry
    participant P as Native Agent CLI
    participant I as Issue comments sidecar
    participant N as InboxStore

    S->>M: Read last fired/held cursor
    S->>D: Dispatch exact canonical What
    D->>C: Resolve exact owner or create fresh Session
    C-->>D: Product resumeId and roster identity
    D->>R: Persist running task with issue trigger
    R-->>D: Durable taskId
    D->>P: Start fire-and-forget native headless process
    D-->>S: Accepted taskId/resumeId
    S->>M: Persist marker after acceptance
    P-->>R: Progress, native id, terminal status/output
    P-->>I: Pending progress; final reply gets deterministic comment id
    P-->>N: Only when the agent explicitly calls inbox_push
```

The dispatch path resolves exact `@resumeId` ownership to its Session Workspace; fresh recruits execute in the Issue’s Workspace. It records a `HeadlessTaskRecord` before launching the native process, uses a global maximum of eight running headless tasks, rejects a running task for the same Issue, and has a short per-Issue dispatch-start guard rather than a Workspace-wide lock. [observed: `src/workspaces/service.ts:1849-1940,1941-2020`; `src/workspaces/schedule/scanner.ts:412-485`; docs `docs/workspace-issues-and-scheduling.md:369-376`]

The durable task record carries `taskId`, stable `resumeId`, optional `parentTaskId`, execution `wsId`, Issue trigger `{kind:'issue',workspaceId,issueId,retryOfTaskId?,metadata?}`, prompt, runtime selection, lifecycle status, process diagnostics, output summary, and optional inquiry. Task logs are under `<launcherRoot>/state/headless-logs`; the registry is `<launcherRoot>/state/headless-tasks.json`; resume and provenance registries are separate launcher state files. [observed: `src/workspaces/headless-task-registry.ts:41-143,146-155`; `src/workspaces/service.ts:725-760`]

- `@new-then-resume` claims the first fresh Session by rewriting the Issue to an exact `@resumeId`; a claim-write failure is logged after the worker is already accepted and does not cancel that worker. `@new-each-run` recruits every fire; exact owners continue one Session. [observed: `src/workspaces/schedule/scanner.ts:486-572`; `src/workspaces/service.ts:2316-2364`; docs `docs/workspace-issues-and-scheduling.md:477-483`]
- A comment from another fixed owner is dispatched to that exact Session; fresh-owner modes use the shared Issue dispatcher; an agent-authored comment without a fixed owner remains a note. A successful reply appends `comment-reply-<taskId>` with `replyTo` and marks the source delivery `replied`; failure marks it `failed`. [observed: `src/workspaces/issues/comment-delivery.ts:41-142,144-226`]
- `inbox_push` is explicit. The tool description says Inbox is a human notification/report channel, not an Agent-to-Agent channel; the server stamps `runId`, `issueId`, `resumeId`, and agent from authoritative out-of-band headers, and the Issue detail joins reports by stamped Issue origin. Inbox entries use random UUID ids and do not deduplicate body/request retries. [observed: `src/tool/inbox-push.ts:15-55`; `src/core/inbox-store.ts:18-70,189-200`; `src/workspaces/issues/board.ts:286-307,438-451`]
- Provenance separates creation/edit/comment occurrence from execution and keeps typed artifact edges. Session origin is `{workspaceId,resumeId,agent,execution}`; native ids never enter the envelope. [observed: `docs/conversation-provenance.md:192-233,324-403`; `src/workspaces/issues/change-tracker.ts:179-217`]

### 3. Current ingress and external-process options

| Mechanism | Principal/authentication | Idempotency and concurrency | Evidence |
|---|---|---|---|
| Direct `.alice/issues/<id>.md` or sidecar file write | Any process with Workspace filesystem access; no application principal. Scanner sees it on the next poll. [observed] | Filename conflict is not enforced for a raw write; no request id. Issue reads isolate malformed files. Concurrent writes are not locked; torn-write risk is inferred from plain `writeFile`. [observed/inferred] | `src/workspaces/schedule/scanner.ts:275-335`; `src/workspaces/file-service.ts:117-142`; `src/workspaces/issues/declaration.ts:403-412` |
| Web `GET/PATCH/POST /api/issues` | Auth middleware requires `alice_session` except true loopback with no trusted proxy and no foreign Origin; mutating requests with no Origin are allowed. This identifies a human/session boundary, not a UTA service identity. [observed] | `PATCH` has no request id; human comments use random ids. Create is not a Web route. [observed] | `src/webui/middleware/auth.ts:75-113`; `src/webui/routes/issues.ts:90-161,164-437`; `src/webui/plugin.ts:212-240,308-313` |
| Loopback MCP `/mcp/:wsId` and CLI `/cli/:wsId/:export/invoke` | No auth; listener is pinned to `127.0.0.1`. URL chooses Workspace. Optional `x-openalice-run`/`x-openalice-session` is resolved against OpenAlice registries; unknown/forged values produce no authoritative origin. [observed] | Issue create conflicts on filename; the Issue comment tool does not expose an external id; no machine idempotency. [observed] | `src/server/mcp.ts:215-270`; `src/server/cli.ts:285-375`; `src/tool/issue-tools.ts:418-426,544-565` |
| `/api/inquiries/issues/:wsId/:id` | Same Web auth; human caller asks creator/owner/run. It resolves an existing Issue and dispatches a follow-up; it cannot create an Issue or append a decision comment. [observed] | No request-id/CAS field; task gets an inquiry subject and durable task reverse link. [observed] | `src/webui/routes/inquiries.ts:80-188` |
| Connector/Telegram owner desk | Connector adapter checks configured linked owner; inbound messages travel through Connector’s loopback HTTP queue. External provenance is `{kind:'external',system:connectorId}`. [observed] | Queue mutation is serialized, sealed, atomic temp→rename, duplicate queue ids are ignored, claims lease for 30s, and event-id-bearing inbound batches derive deterministic comment ids. [observed] | `services/connector/src/main.ts:75-132,153-170`; `services/connector/src/core/work-queue.ts:46-109,112-164`; `src/workspaces/issues/telegram-desk-chat.ts:102-173`; `src/services/connector-client/action-bridge.ts:96-145` |
| Electron child-process IPC | Parent/child IPC exists only for PTY attach/client messages; the message union has no Issue or UTA command. [observed] | PTY attachment validates a SessionRecord id; not an Issue ingress. [observed] | `src/webui/workspaces-ipc.ts:1-9,17-35,83-156` |
| UTA HTTP today | UTA is trading-only, loopback-bound, and exposes `/api/trading` routes. Alice’s BFF forwards trading requests without an internal actor identity. [observed] | Existing wallet push/reject takes only `expectedPendingHash` (reject may include `reason`); no Issue request or decision endpoint was observed in the inspected UTA route surface. [observed] | `services/uta/src/main.ts:1-10,58-65,159-177`; `src/webui/routes/trading-proxy.ts:1-12,115-187`; `services/uta/src/http/routes-trading.ts:447-527` |

The Connector bridge is polled by Alice every 1.5 seconds, claims/fulfills UTA review actions, and retries by releasing uncompleted queue claims. Connector UTA action records have a random request id, connector id, action, optional `utaId`, optional pending hash, and a 60-second action TTL; they are not Issue records. [observed: `src/services/connector-client/action-bridge.ts:46-92,96-145`; `src/services/connector-client/uta-review.ts:33-46,129-218`; `packages/connector-protocol/src/types.ts:231-233,282-310`]

### 4. Approval/rejection flows and immutable trade identity

| Flow | Proposal and decision | Actor/correlation and final side effect |
|---|---|---|
| Agent trading tools | `placeOrder`, `modifyOrder`, `closePosition`, and `cancelOrder` stage operations; optional `commitMessage` also commits. `tradingCommit` commits staged operations and does not execute. [observed: `src/tool/trading.ts:671-718,720-776,779-795`] | UTA commit hash is the durable decision identity. OpenAlice records `decided` provenance only for commits made through an authoritative run/session header; push/fill ids are execution evidence. [observed: `src/server/trade-provenance.ts:41-63`; `src/server/cli.ts:341-370`; `docs/conversation-provenance.md:475-506`] |
| `tradingPush` default | With `allowAiTrading:false` (default), the tool returns pending operations and instructs human Web approval; it does not call UTA push. With it enabled, it calls `uta.push(pendingHash)` and per-account failures are returned. [observed: `src/tool/trading.ts:202-212,798-846`; `src/core/config.ts:242-249`] | This is a global policy switch, not a typed Issue decision or actor record. Per-account readonly still blocks venue writes. [observed: `src/core/config.ts:242-249`; `src/services/connector-client/uta-review.ts:159-194`] |
| Web Trading as Git | UI polls status, requires the current pending hash, asks for a second confirmation click, then posts `/api/trading/uta/:id/wallet/push` or `reject` with `expectedPendingHash`. [observed: `ui/src/components/PushApprovalPanel.tsx:397-450,926-964`; `ui/src/api/trading.ts:130-167`] | Alice Web auth is the outer human boundary, but the BFF forwards only a whitelist including `x-request-id`; current UTA push/reject endpoints receive no actor or decision id. UTA rejects stale hashes with `409 PENDING_HASH_CONFLICT`. [observed: `src/webui/routes/trading-proxy.ts:27-34,138-187`; `services/uta/src/http/routes-trading.ts:474-527`] |
| Telegram `uta-review` | `/uta` is owner-gated; review is queued, account selection requires a pending hash, and push/reject requires a confirmation transition. [observed: `services/connector/src/adapters/telegram.ts:514-537,623-666,697-699`; `services/connector/src/adapters/telegram-uta.ts:177-281,294-308`] | Connector request id and pending hash correlate queue work and prevent stale execution, with a 60-second TTL and 30-second queue lease. The request schema carries `connectorId`, but not Telegram user id; UTA receives only account/hash (and reject reason). [observed: `packages/connector-protocol/src/types.ts:282-310`; `services/connector/src/core/delivery-manager.ts:247-274`; `src/services/connector-client/uta-review.ts:150-217`; **inferred:** human actor is not persisted into UTA’s current push/reject call]
| Issue comments today | A comment is a collaboration input. It can dispatch an owner/fresh Agent reply, remain a note, or fail delivery; Issue status/comment text is not interpreted as an approval command. [observed: `src/workspaces/issues/comment-delivery.ts:41-142`; `src/webui/routes/issues.ts:374-437`; `src/tool/issue-tools.ts:401-479`] | Existing provenance can record human, external connector, or Session origin, but the generic comment shape has no typed approval payload. Therefore a freeform `approve` comment would not be a safe UTA command. [observed: `src/workspaces/issues/comments.ts:23-34`; `docs/conversation-provenance.md:219-233`]

### 5. Failure, duplicate, ordering, reopen, and expiry semantics

- **Accepted-dispatch crash window:** the scanner calls dispatch, then writes the last-fired marker. A process crash after the durable task record is created/returned but before marker persistence can leave the occurrence due on restart. This is an inference from the ordering, not an explicitly tested guarantee. [observed: `src/workspaces/schedule/scanner.ts:345-387`; `src/workspaces/service.ts:1975-2020`; **inferred:** duplicate after crash before `markers.set`]
- **Marker durability:** `ScheduleMarkerStore.set` updates memory and `flush` catches/logs write errors. If flush fails, the process can retain an advanced in-memory marker while a restart reloads the old marker and repeats a due occurrence. [observed: `src/workspaces/schedule/marker-store.ts:65-106`; **inferred:** restart replay after swallowed flush failure]
- **Task restart/crash:** registry load converts persisted `running` tasks to `interrupted`, sets a terminal time, removes progress, and flushes. There is no automatic retry; Retry now is restricted to the latest failed/interrupted Issue run and creates a distinct retry lineage. [observed: `src/workspaces/headless-task-registry.ts:178-200`; `src/workspaces/schedule/scanner.ts:438-443`; docs `docs/workspace-issues-and-scheduling.md:226-228,356-367`]
- **Duplicate/racing dispatch:** one in-process Issue dispatch-start guard plus `isIssueRunning` rejects concurrent schedule/manual/retry starts for the same Issue; other Issues may run concurrently, up to global capacity eight. [observed: `src/workspaces/schedule/scanner.ts:426-444`; `src/workspaces/service.ts:1855-1857`; docs `docs/workspace-issues-and-scheduling.md:369-376`]
- **Comment duplicates:** deterministic Connector/reply ids are replay-safe in one process; human Web comments and the generic Issue tool omit an id and get random ids, so HTTP/network retries append separate comments. [observed: `src/webui/routes/issues.ts:387-408`; `src/tool/issue-tools.ts:418-426`; `src/workspaces/issues/comments.ts:203-219`]
- **Out-of-order delivery updates:** delivery update replaces the comment’s `delivery` field without an expected prior state, task/version, or monotonic transition check. The in-process chain serializes writes but does not reject a stale update; an older completion could overwrite a later state if the same comment were ever dispatched more than once. The last clause is an inference; normal comment flow creates one task per comment. [observed: `src/workspaces/issues/comments.ts:222-238`; **inferred:** stale-update overwrite]
- **File races/torn sidecars:** Issue and sidecar writers use `writeWorkspaceFile`, whose implementation is direct `writeFile`; unlike Marker/Registry/Connector queue, it does not use temp+rename. Readers isolate invalid Issue files; sidecar parse failure returns an error. [observed: `src/workspaces/file-service.ts:117-142`; `src/workspaces/issues/declaration.ts:403-412`; `src/workspaces/issues/comments.ts:122-135`; **inferred:** crash can produce invalid/truncated content]
- **Reopen/terminal:** `isFireable` checks `when` and non-terminal status. Setting `done`/`canceled` stops firing; setting a status back to a non-terminal value makes the declaration eligible again, with due-ness still based on the retained marker. The latter re-fire timing is an inference from the predicate and due calculation. [observed: `src/workspaces/issues/declaration.ts:102-107,297-300`; `src/workspaces/schedule/scanner.ts:338-343`; **inferred:** reopen resumes normal due evaluation]
- **One-shot/expiry:** successful `when.kind:'at'` runs are auto-marked `done`; failures/interruption leave the Issue open. There is no Issue-level expiry field. Connector UTA actions expire after 60 seconds; stale UTA pending hashes produce conflict rather than execution. [observed: `src/workspaces/issues/auto-complete.ts:1-9,44-75`; `packages/connector-protocol/src/types.ts:231-233,282-285`; `services/uta/src/http/routes-trading.ts:478-496,508-525`]

## Recommendations (inferred design, not current behavior)

### A. Preferred: authenticated UTA work queue + normal Issue + typed UTA linkage/decision sidecar

Preserve the Issue file as the exact human-visible/Agent prompt, but add a dedicated UTA bridge rather than teaching UTA to impersonate a local CLI or parse freeform comments.

```ts
interface UtaWorkRequest {
  requestId: string                 // caller idempotency key
  accountId: string
  intentId: string                  // UTA-owned immutable intent
  sourceEventId?: string            // watch/news causal input
  kind: 'research' | 'watch' | 'trade-review'
  what: string                      // canonical Markdown What
  targetWorkspaceId: string
  expiresAt?: string
}

interface UtaIssueLink {
  version: 1
  requestId: string
  accountId: string
  intentId: string
  issueId: string
  expectedVersion?: string         // pending hash / UTA CAS token
  state: 'open' | 'decided' | 'expired' | 'failed'
}

interface UtaIssueDecision {
  decisionId: string
  requestId: string
  intentId: string
  action: 'approve' | 'reject'
  expectedVersion?: string
  actor: { kind: 'human' | 'session'; id: string }
  correlationId: string
  at: string
  reason?: string
}
```

1. UTA submits the request to a new authenticated Alice bridge with `requestId` as an idempotency key. Authentication must identify the UTA service (loopback/Unix-socket deployment or IaC-managed mTLS/HMAC for a remote UTA); it must not rely on Web localhost bypass or unauthenticated MCP/CLI.
2. Alice atomically records the request-to-Issue mapping and creates an Issue using a deterministic id derived from `accountId + requestId`; duplicate `requestId` returns the existing mapping. The Issue’s `what` remains canonical Markdown. `UtaIssueLink` belongs in a typed adjacent sidecar or launcher-owned UTA request store, not an unknown YAML field.
3. The Agent may write a typed proposal/report comment and stage work. It cannot turn Issue status or freeform Markdown into venue authority. A human/session decision is accepted only through a typed decision endpoint or a versioned `decision` field in the comment sidecar; the actor is server-stamped.
4. Alice submits `UtaIssueDecision` to UTA with `intentId`, expected version/hash, decision id, and actor/correlation. UTA validates idempotency, expiry, and compare-and-set, remains the authority for push/reject/fills, and returns a durable receipt. Alice appends a receipt comment and may close the Issue only as work lifecycle; `done` must not mean “broker executed.”

**Trade-offs:** This reuses the existing scanner/Issue UX and preserves the UTA append-only intent/execution boundary, while making retries, actor identity, and causal links explicit. It requires one new bridge, a typed linkage store, and a typed decision projection; that is more durable than overloading `status`, Markdown, or current unauthenticated wallet routes.

### B. Lower-change alternative: UTA-owned intent/approval ledger with an Issue projection

Keep all proposal, approval, actor, expiry, and execution state in UTA. Alice receives a signed `UtaWorkRequest`, creates a normal Issue whose What contains the user-facing task, and exposes a dedicated `POST /api/uta/decisions`-style bridge carrying `{requestId,intentId,issueId,action,expectedVersion,actor}`. The bridge appends a human-readable Issue comment as a projection and calls UTA; generic Issue comments remain non-authoritative.

**Trade-offs:** UTA remains the sole state machine and avoids extending the existing comment schema, which is safer for execution. It adds a second UI/API projection and means an Issue detail is not itself sufficient to reconstruct the typed decision without querying UTA. This is appropriate when UTA already owns the authoritative intent ledger and Alice is only the AI-work/human-attention surface.

Both shapes should model `requestId`, `intentId`, `expectedVersion/pendingHash`, `decisionId`, actor, and correlation explicitly; neither should treat Issue id, status, comment text, or `allowAiTrading` as a substitute for UTA intent/approval authority. [inferred]

## Unavailable / contradictions

- No current UTA→Issue request/decision API, callback, or machine-principal contract was found in the inspected UTA, Issue-route, MCP, CLI, and IPC surfaces. A full-repository absence claim is not made beyond those inspected paths. [observed scoped inspection; unavailable]
- The exact UTA new-service intent/approval schema, provider-side event identity, and whether UTA will be same-host or remote are not established by current code. Recommendations therefore use abstract `intentId`/`expectedVersion` rather than assuming a Git hash, broker order id, or transport.
- `writeWorkspaceFile` is documented as atomic but directly calls `writeFile`; MarkerStore, HeadlessTaskRegistry, and ConnectorWorkQueue do use temp+rename. This implementation/documentation mismatch is load-bearing for Issue and comment durability. [observed: `src/workspaces/file-service.ts:117-142`; `src/workspaces/schedule/marker-store.ts:96-106`; `src/workspaces/headless-task-registry.ts:364-373`; `services/connector/src/core/work-queue.ts:153-164`]
- Existing human/Telegram approval provenance is not a complete actor record at the UTA boundary: current UTA wallet endpoints receive expected hash (and optional reject reason), while Connector’s request schema carries connector id but no owner user id. [observed: `services/uta/src/http/routes-trading.ts:474-527`; `packages/connector-protocol/src/types.ts:294-310`]
