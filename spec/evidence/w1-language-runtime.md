# Decision questions answered

1. **Can the current TypeScript runtime shape support the target of at least 100 concurrent upstream WebSockets with downstream fan-out?** **[observed]** In a two-process synthetic run on this M4, Node 26.8.1 and Bun 1.4.0 both kept 100 and 200 WebSockets open, decoded 200-byte JSON ticks, ran three consumers, and delivered the 5,000/10,000 requested-message-per-second cases with zero sequence gaps and zero parse errors. **[inferred]** This settles feasibility for the stated 5–10k target, not a product capacity SLO. The sweep found a useful boundary: approximately 18.3k actual processed messages/s remained bounded for five seconds under the declared harness heuristic; approximately 22.8k actual messages/s clearly grew the queue.
2. **Does the new OpenAPI-projection provider model make Rust materially cheaper?** **[observed]** The design requires provider capability declaration, translation, and transport execution to remain separate, and permits gateway/REST/OpenAPI/TCP/script transports without a universal provider object (`plans/uta-refactor/design/01-detail-constraints.md:220-233`). **[inferred]** A generated OpenAPI client can make the transport layer language-neutral, but it does not remove UTA domain work: append-only intent/attempt/receipt/observation handling, idempotency/unknown outcomes, reconciliation, guards, approval, and kind-routed consumers. Current UTA code has those concerns in addition to HTTP (`services/uta/src/main.ts:56-189`; `services/uta/src/domain/trading/order-sync-poller.ts:4-15`; `services/uta/src/domain/trading/git/TradingGit.ts:119-179,260-376`).
3. **What is the Rust networking/effects recommendation?** **[observed]** Pingora documents HTTP/1/2 proxying, WebSocket proxying, and upstream `Peer`/`Connector` concepts; its example opens an outbound stream and manually duplexes bytes (`/tmp/pingora-language-research/README.md:5-22`; `docs/user_guide/peer.md:1-48`; `pingora/examples/app/proxy.rs:22-103`). `tokio-tungstenite` exposes an outbound `connect_async` WebSocket client and `Stream`/`Sink` operations (official docs below). **[inferred]** Use Pingora only when UTA is also an inbound programmable proxy; use `tokio-tungstenite` for UTA-initiated provider subscriptions. Rust has no stable, mainstream general ZIO-like effect system: native `async fn` plus `Result`/typed errors is the boring choice; `effing-mad` is explicitly an experiment requiring nightly (`effing-mad-0.1.0/README.md:1-24,45-59,72-79`; `rust-toolchain.toml:1-2`).
4. **Which implementation language minimizes integration risk now?** **[inferred]** Keep the new core TypeScript first, with Node as the existing service default and Bun 1.4.0 as the already-supported standalone path. The measured target is not a reason by itself to port the service to Rust, while the existing JS Broker Pack loading, TS protocol package, Guardian lifecycle, Electron child spawn, and Bun role dispatch all require substantially more change for a Rust sidecar (`services/uta/src/domain/trading/brokers/registry.ts:44-136`; `packages/uta-protocol/src/client/UTAClient.ts:1-29,48-99`; `scripts/guardian/runtime-process-spec.mjs:1-24`; `apps/desktop/src/main.ts:786-813`; `packages/cli/bin/openalice-bun.ts:34-55`).

# Observed facts

## Benchmark method and scope

**[observed]** The exact throwaway harness was `/tmp/uta-language-bench/bench.mjs` (266 lines) plus `/tmp/uta-language-bench/run-one.sh` (39 lines). The harness runs a server process and a client process. Node uses the repository's `ws` package; Bun uses its native WebSocket implementation. The server keeps N sockets open and emits a JSON payload of exactly 200 bytes or more on one interval timer. The client parses each message, performs three synchronous consumer callbacks (shape validation, aggregate update, and bounded queue enqueue), and drains the queue in batches. The client measures processing time from entry to the message callback through parse, all three callbacks, and enqueue; it is not network RTT and not an end-to-end Alice callback latency.

**[observed]** The measurement window resets counters and queue state after warmup. The server continues to run through warmup, so server totals and client measurement totals are not expected to match exactly. The harness checks sequence gaps per connection, JSON parse errors, processed-versus-received equality, queue depth, and queue slope. `ps -o pcpu,rss` is sampled once per second for both processes; those values include startup/tail samples and are approximate host-process measurements.

**[inferred]** Before the sweep I used this explicit, harness-only boundedness heuristic: no sequence or parse errors; `processed == received`; queue maximum no more than one normal batch (2,000) for the 100-connection case or no unexplained sustained growth; and absolute queue slope below 500 messages/s over the five-second sweep. This is not a product requirement and must not be promoted to an SLO without a representative trace and a longer run.

### Exact setup and commands

**[observed]** The repository pins Bun 1.4.0 (`.bun-version:1`) and requires Node >=22.19.0 for the UTA package (`services/uta/package.json:32-34`). I installed and selected the pinned Bun executable with `mise install bun@1.4.0`; the observed tool versions were `node v26.8.1`, `bun 1.4.0`, and `ws 8.21.0` resolved from the repository dependency tree. The exact setup command was:

```sh
rm -f /tmp/uta-language-bench/node_modules && ln -s "$PWD/node_modules" /tmp/uta-language-bench/node_modules && node --version && bun --version && node -e "console.log(require.resolve('ws'))"
mise install bun@1.4.0
```

**[observed]** A one-case smoke run completed before the matrix:

```sh
bash /tmp/uta-language-bench/run-one.sh node 1 5 1000 100
```

**[observed]** Baseline commands (10,000 ms measurement, 2,000 ms warmup) were:

```sh
bash /tmp/uta-language-bench/run-one.sh node 100 50 10000 2000
bash /tmp/uta-language-bench/run-one.sh node 200 50 10000 2000
PATH="$HOME/.local/share/mise/installs/bun/1.4.0/bin:$PATH" bash /tmp/uta-language-bench/run-one.sh bun 100 50 10000 2000
PATH="$HOME/.local/share/mise/installs/bun/1.4.0/bin:$PATH" bash /tmp/uta-language-bench/run-one.sh bun 200 50 10000 2000
```

**[observed]** Sweep commands (5,000 ms measurement, 1,000 ms warmup) were:

```sh
bash /tmp/uta-language-bench/run-one.sh node 100 100 5000 1000
bash /tmp/uta-language-bench/run-one.sh node 100 200 5000 1000
bash /tmp/uta-language-bench/run-one.sh node 100 250 5000 1000
bash /tmp/uta-language-bench/run-one.sh node 100 300 5000 1000
bash /tmp/uta-language-bench/run-one.sh node 200 100 5000 1000
bash /tmp/uta-language-bench/run-one.sh node 200 200 5000 1000
PATH="$HOME/.local/share/mise/installs/bun/1.4.0/bin:$PATH" bash /tmp/uta-language-bench/run-one.sh bun 100 100 5000 1000
PATH="$HOME/.local/share/mise/installs/bun/1.4.0/bin:$PATH" bash /tmp/uta-language-bench/run-one.sh bun 100 200 5000 1000
PATH="$HOME/.local/share/mise/installs/bun/1.4.0/bin:$PATH" bash /tmp/uta-language-bench/run-one.sh bun 100 250 5000 1000
PATH="$HOME/.local/share/mise/installs/bun/1.4.0/bin:$PATH" bash /tmp/uta-language-bench/run-one.sh bun 100 300 5000 1000
PATH="$HOME/.local/share/mise/installs/bun/1.4.0/bin:$PATH" bash /tmp/uta-language-bench/run-one.sh bun 200 100 5000 1000
PATH="$HOME/.local/share/mise/installs/bun/1.4.0/bin:$PATH" bash /tmp/uta-language-bench/run-one.sh bun 200 200 5000 1000
```

### Exact throwaway benchmark scripts

**[observed]** These are the exact files executed by the commands above:

```js
#!/usr/bin/env node
// Throwaway UTA language benchmark. Run as `bun bench.mjs ...` or
// `node bench.mjs ...`; Node uses ws via the nearby node_modules symlink.
const isBun = typeof Bun !== 'undefined'
const runtime = isBun ? `bun-${Bun.version}` : `node-${process.versions.node}`
const [, , mode, portArg, connectionsArg, rateArg, durationArg, warmupArg, batchArg, intervalArg] = process.argv
const port = Number(portArg ?? 47681)
const connections = Number(connectionsArg ?? 100)
const rate = Number(rateArg ?? 50)
const durationMs = Number(durationArg ?? 10_000)
const warmupMs = Number(warmupArg ?? 2_000)
const queueBatch = Number(batchArg ?? 2_000)
const queueIntervalMs = Number(intervalArg ?? 100)
if (!['server', 'client'].includes(mode) || ![port, connections, rate, durationMs, warmupMs, queueBatch, queueIntervalMs].every(Number.isFinite)) {
  console.error('usage: <server|client> <port> <connections> <msg/s/connection> <duration-ms> [warmup-ms] [queue-batch] [queue-interval-ms]')
  process.exit(2)
}

function payloadFor(connection, sequence) {
  const body = {
    kind: 'tick', connection, sequence, symbol: 'BTC/USD',
    bid: 64123.12, ask: 64123.13, bidSize: 0.125, askSize: 0.250,
    source: 'synthetic', sentAtNs: Number(process.hrtime.bigint()), padding: '',
  }
  for (let n = 0; n < 512; n++) {
    body.padding = 'x'.repeat(n)
    const text = JSON.stringify(body)
    if (text.length >= 200) return text
  }
  throw new Error('could not construct 200-byte payload')
}

async function startServer() {
  const clients = new Map()
  let sendTimer = null
  let startedAt = null
  let firstSentAt = null
  let lastSentAt = null
  let sent = 0
  let timerTicks = 0
  let maxBufferedAmount = 0
  const payloadBytes = payloadFor(0, 0).length
  const started = () => {
    if (startedAt !== null || clients.size !== connections) return
    startedAt = performance.now()
    console.log(`READY runtime=${runtime} role=server pid=${process.pid} port=${port} connections=${connections} payloadBytes=${payloadBytes}`)
    const intervalMs = 1000 / rate
    sendTimer = setInterval(() => {
      timerTicks++
      for (const [socket, state] of clients) {
        if (socket.readyState !== 1) continue
        socket.send(payloadFor(state.id, state.sequence++))
        sent++
        const now = performance.now()
        firstSentAt ??= now
        lastSentAt = now
        maxBufferedAmount = Math.max(maxBufferedAmount, socket.bufferedAmount ?? 0)
      }
    }, intervalMs)
  }
  const stop = async (signal = 'SIGTERM') => {
    clearInterval(sendTimer)
    const runningMs = startedAt === null ? 0 : Math.max(0, performance.now() - startedAt)
    const sendElapsedMs = firstSentAt === null || lastSentAt === null ? 0 : Math.max(1, lastSentAt - firstSentAt + (1000 / rate))
    const summary = {
      runtime, role: 'server', pid: process.pid, signal, connections,
      requestedMsgPerSec: connections * rate,
      achievedMsgPerSec: sendElapsedMs ? sent / (sendElapsedMs / 1000) : 0,
      sent, timerTicks, runningMs, sendElapsedMs, payloadBytes, maxBufferedAmount,
      connectedAtStop: clients.size,
    }
    console.log(`SUMMARY ${JSON.stringify(summary)}`)
    for (const socket of clients.keys()) {
      try { socket.close() } catch {}
    }
    if (isBun) server.stop(true)
    else await new Promise((resolve) => wss.close(() => resolve()))
    process.exit(0)
  }
  process.once('SIGTERM', () => { void stop('SIGTERM') })
  process.once('SIGINT', () => { void stop('SIGINT') })

  let server
  let wss
  if (isBun) {
    server = Bun.serve({
      port,
      fetch(request, instance) {
        const id = Number(new URL(request.url).searchParams.get('id'))
        if (!Number.isInteger(id) || id < 0 || id >= connections || !instance.upgrade(request, { data: { id } })) {
          return new Response('websocket upgrade required', { status: 400 })
        }
        return undefined
      },
      websocket: {
        open(socket) {
          clients.set(socket, { id: socket.data.id, sequence: 0 })
          started()
        },
        close(socket) { clients.delete(socket) },
      },
    })
  } else {
    const ws = await import('ws')
    const WebSocketServer = ws.WebSocketServer ?? ws.default?.WebSocketServer
    if (!WebSocketServer) throw new Error('ws WebSocketServer export unavailable')
    wss = new WebSocketServer({ port })
    wss.on('listening', () => console.log(`LISTENING runtime=${runtime} role=server pid=${process.pid} port=${port}`))
    wss.on('connection', (socket, request) => {
      const id = Number(new URL(request.url ?? '/', `http://127.0.0.1:${port}`).searchParams.get('id'))
      clients.set(socket, { id, sequence: 0 })
      socket.on('close', () => clients.delete(socket))
      socket.on('error', () => clients.delete(socket))
      started()
    })
    server = wss
  }
  if (isBun) console.log(`LISTENING runtime=${runtime} role=server pid=${process.pid} port=${port}`)
}

async function startClient() {
  let WebSocketCtor
  if (isBun) WebSocketCtor = WebSocket
  else {
    const ws = await import('ws')
    WebSocketCtor = ws.WebSocket ?? ws.default
    if (!WebSocketCtor) throw new Error('ws WebSocket export unavailable')
  }

  const sockets = []
  let opened = 0
  let measuring = false
  let measureStartedAt = 0
  let processed = 0
  let received = 0
  let drained = 0
  let sequenceGaps = 0
  let parseErrors = 0
  let decoderCount = 0
  let aggregateCount = 0
  let queueCount = 0
  let aggregateValue = 0
  let maxQueue = 0
  const latenciesUs = []
  const queue = []
  const queueSamples = []
  let drainTimer = null
  let drainScheduled = false
  const lastSequence = new Map()
  const aggregates = new Map()

  function queueDrain() {
    drainScheduled = false
    const count = Math.min(queueBatch, queue.length)
    drained += count
    queue.splice(0, count)
    if (queue.length > 0) scheduleDrain()
  }
  function scheduleDrain() {
    if (drainScheduled) return
    drainScheduled = true
    drainTimer = setTimeout(queueDrain, queueIntervalMs)
  }
  function decoderSubscriber(value) {
    if (value.kind !== 'tick' || typeof value.symbol !== 'string' || typeof value.sequence !== 'number') throw new Error('bad tick')
    decoderCount++
  }
  function aggregatorSubscriber(value) {
    aggregateCount++
    aggregateValue += value.bid + value.ask
    aggregates.set(value.symbol, (aggregates.get(value.symbol) ?? 0) + 1)
  }
  function queueSubscriber(value) {
    queueCount++
    queue.push(value)
    if (queue.length > maxQueue) maxQueue = queue.length
    scheduleDrain()
  }
  function asText(data) {
    if (typeof data === 'string') return data
    if (typeof Buffer !== 'undefined' && Buffer.isBuffer(data)) return data.toString('utf8')
    return new TextDecoder().decode(data)
  }
  function onMessage(data) {
    const startedNs = process.hrtime.bigint()
    let value
    try { value = JSON.parse(asText(data)) } catch { if (measuring) parseErrors++; return }
    const previous = lastSequence.get(value.connection)
    if (measuring && previous !== undefined && value.sequence !== previous + 1) sequenceGaps += Math.max(1, value.sequence - previous - 1)
    lastSequence.set(value.connection, value.sequence)
    if (measuring) received++
    try {
      decoderSubscriber(value)
      aggregatorSubscriber(value)
      queueSubscriber(value)
    } catch { if (measuring) parseErrors++; return }
    if (measuring) {
      processed++
      latenciesUs.push(Number(process.hrtime.bigint() - startedNs) / 1_000)
    }
  }
  function attach(socket) {
    const onOpen = () => {
      opened++
      if (opened === connections) {
        console.log(`READY runtime=${runtime} role=client pid=${process.pid} connections=${connections} payloadTarget=200 warmupMs=${warmupMs} durationMs=${durationMs} queueBatch=${queueBatch} queueIntervalMs=${queueIntervalMs}`)
        setTimeout(() => {
          queue.length = 0
          maxQueue = 0
          latenciesUs.length = 0
          processed = received = drained = sequenceGaps = parseErrors = 0
          decoderCount = aggregateCount = queueCount = 0
          aggregateValue = 0
          aggregates.clear()
          lastSequence.clear()
          queueSamples.length = 0
          measureStartedAt = performance.now()
          measuring = true
          const sample = setInterval(() => queueSamples.push({ elapsedMs: performance.now() - measureStartedAt, depth: queue.length, processed }), 250)
          setTimeout(() => finish(sample), durationMs)
        }, warmupMs)
      }
    }
    const onMessageEvent = (event) => onMessage(event?.data ?? event)
    const onError = () => {}
    if (typeof socket.addEventListener === 'function') {
      socket.addEventListener('open', onOpen)
      socket.addEventListener('message', onMessageEvent)
      socket.addEventListener('error', onError)
    } else {
      socket.on('open', onOpen)
      socket.on('message', onMessage)
      socket.on('error', onError)
    }
  }
  function finish(sample) {
    clearInterval(sample)
    measuring = false
    clearTimeout(drainTimer)
    const elapsedMs = Math.max(1, performance.now() - measureStartedAt)
    const sorted = [...latenciesUs].sort((a, b) => a - b)
    const percentile = (p) => sorted.length ? sorted[Math.floor((sorted.length - 1) * p)] : null
    const first = queueSamples[0]?.depth ?? 0
    const last = queueSamples.at(-1)?.depth ?? queue.length
    const result = {
      runtime, role: 'client', pid: process.pid, connections, requestedMsgPerSec: connections * rate,
      received, processed, drained, achievedMsgPerSec: processed / (elapsedMs / 1000), parseErrors, sequenceGaps,
      decoderCount, aggregateCount, queueCount, aggregateValue, distinctSymbols: aggregates.size,
      elapsedMs, p50ProcessingUs: percentile(0.50), p99ProcessingUs: percentile(0.99),
      queueFinal: queue.length, queueMax: maxQueue, queueFirstSample: first, queueLastSample: last,
      queueSlopePerSec: queueSamples.length > 1 ? (last - first) / ((queueSamples.at(-1).elapsedMs - queueSamples[0].elapsedMs) / 1000) : null,
      queueSamples: queueSamples.slice(-5),
    }
    console.log(`SUMMARY ${JSON.stringify(result)}`)
    for (const socket of sockets) { try { socket.close() } catch {} }
    setTimeout(() => process.exit(0), Math.max(50, queueIntervalMs + 25))
  }
  for (let id = 0; id < connections; id++) {
    const socket = new WebSocketCtor(`ws://127.0.0.1:${port}/?id=${id}`)
    sockets.push(socket)
    attach(socket)
  }
}

if (mode === 'server') await startServer()
else await startClient()
```

```sh
#!/usr/bin/env bash
set -euo pipefail
runtime=${1:?runtime command (node or bun)}
connections=${2:?connections}
rate=${3:?messages per second per connection}
duration_ms=${4:-10000}
warmup_ms=${5:-2000}
port=$((47681 + (RANDOM % 500)))
root=$(cd "$(dirname "$0")" && pwd)
server_log="$root/${runtime// /-}-${connections}-${rate}-server.log"
client_log="$root/${runtime// /-}-${connections}-${rate}-client.log"
ps_log="$root/${runtime// /-}-${connections}-${rate}-ps.log"
: >"$server_log"; : >"$client_log"; : >"$ps_log"
"$runtime" "$root/bench.mjs" server "$port" "$connections" "$rate" "$duration_ms" "$warmup_ms" >"$server_log" 2>&1 &
server_pid=$!
trap 'kill "$server_pid" 2>/dev/null || true; wait "$server_pid" 2>/dev/null || true' EXIT
until grep -q '^LISTENING ' "$server_log"; do
  kill -0 "$server_pid" 2>/dev/null || { cat "$server_log"; exit 1; }
  sleep 0.02
done
"$runtime" "$root/bench.mjs" client "$port" "$connections" "$rate" "$duration_ms" "$warmup_ms" >"$client_log" 2>&1 &
client_pid=$!
printf 'timestamp,pid,cpu_percent,rss_kb\n' >"$ps_log"
until grep -q '^READY ' "$client_log"; do
  kill -0 "$client_pid" 2>/dev/null || { cat "$client_log"; exit 1; }
  sleep 0.02
done
while kill -0 "$client_pid" 2>/dev/null; do
  now=$(date +%s.%N)
  ps -p "$server_pid" -o pid=,pcpu=,rss= | awk -v t="$now" '{printf "%s,%s,%s,%s\n",t,$1,$2,$3}' >>"$ps_log" || true
  ps -p "$client_pid" -o pid=,pcpu=,rss= | awk -v t="$now" '{printf "%s,%s,%s,%s\n",t,$1,$2,$3}' >>"$ps_log" || true
  sleep 1
done
wait "$client_pid"
kill "$server_pid" 2>/dev/null || true
wait "$server_pid" 2>/dev/null || true
printf '\n=== server ===\n'; cat "$server_log"
printf '\n=== client ===\n'; cat "$client_log"
printf '\n=== ps samples ===\n'; cat "$ps_log"
```

## Results

**[observed]** Baseline summaries below are taken from the `SUMMARY` lines in `/tmp/uta-language-bench/*-{server,client}.log`. `actual` is producer/client achieved messages per second respectively. CPU is mean/max of one-second `ps` samples; RSS is maximum resident set size in kB. All baseline runs reported `maxBufferedAmount=0`, `sequenceGaps=0`, and `parseErrors=0`.

| Runtime | N | Requested msg/s | Producer / client actual msg/s | Client p50 / p99 processing (us) | Server CPU mean/max %; RSS max kB | Client CPU mean/max %; RSS max kB | Queue max / slope per s |
|---|---:|---:|---:|---:|---:|---:|---:|
| Node 26.8.1 | 100 | 5,000 | 4,800.886 / 4,809.847 | 0.583 / 12.500 | 6.942 / 18.3; 58,752 | 6.983 / 27.7; 64,336 | 500 / +21.1 |
| Node 26.8.1 | 200 | 10,000 | 9,602.168 / 9,599.610 | 0.542 / 10.542 | 10.333 / 14.1; 64,928 | 10.417 / 25.3; 74,384 | 1,000 / -41.9 |
| Bun 1.4.0 | 100 | 5,000 | 4,834.926 / 4,839.309 | 0.500 / 12.375 | 4.400 / 5.3; 57,584 | 3.708 / 4.2; 34,016 | 500 / +10.5 |
| Bun 1.4.0 | 200 | 10,000 | 9,645.742 / 9,638.670 | 0.459 / 10.292 | 7.517 / 9.9; 83,728 | 7.092 / 8.5; 43,472 | 1,000 / -21.0 |

**[observed]** Across the 5,000 ms sweep, Node's producer/client CPU means ranged from 12.571/9.029% at 100 connections and requested 10,000 msg/s to 31.357/26.414% at 200 connections and requested 40,000 msg/s; Bun ranged from 7.971/7.571% to 24.457/20.129%. Node client maximum RSS ranged from 73,392 to 148,784 kB; Bun client maximum RSS ranged from 41,936 to 70,256 kB. These are process measurements for this harness, not a language-wide memory comparison.

**[observed]** The sweep's queue outcomes were:

| Runtime | N | Requested msg/s | Producer / client actual msg/s | Queue max | Queue slope/s | Heuristic result |
|---|---:|---:|---:|---:|---:|---|
| Node | 100 | 10,000 | 9,290.504 / 9,260.601 | 1,000 | -177.0 | bounded |
| Node | 100 | 20,000 | 18,338.961 / 18,307.105 | 1,904 | +155.6 | bounded for 5 s |
| Node | 100 | 25,000 | 22,791.705 / 22,833.976 | 16,298 | +2,853.1 | growing |
| Node | 100 | 30,000 | 30,929.302 / 30,932.256 | 56,500 | +10,984.8 | growing |
| Node | 200 | 20,000 | 17,733.757 / 17,730.428 | 2,000 | +0.4 | bounded |
| Node | 200 | 40,000 | 35,433.285 / 35,350.601 | 78,577 | +15,411.9 | growing |
| Bun | 100 | 10,000 | 9,259.591 / 9,255.566 | 1,000 | 0.0 | bounded |
| Bun | 100 | 20,000 | 18,292.156 / 18,296.318 | 1,901 | +399.7 | bounded for 5 s; borderline |
| Bun | 100 | 25,000 | 22,914.285 / 22,897.495 | 16,500 | +2,911.6 | growing |
| Bun | 100 | 30,000 | 30,538.610 / 30,542.626 | 54,600 | +10,574.1 | growing |
| Bun | 200 | 20,000 | 17,737.741 / 17,733.010 | 2,000 | -19.9 | bounded |
| Bun | 200 | 40,000 | 34,854.458 / 34,882.658 | 76,028 | +14,911.7 | growing |

**[inferred]** The target of 100–200 sockets at 5–10k requested aggregate msg/s is comfortably below the first observed queue-growth region in this synthetic workload. **[inferred]** The data does not establish that Node or Bun is “faster” in general: the two runs use different WebSocket stacks (Node `ws` versus Bun native), a localhost producer, and lightweight callbacks rather than real provider decoding, ledger writes, or Alice Issue callbacks.

## Existing integration surfaces

**[observed]** UTA currently starts as a Node/Hono HTTP process, binds loopback, initializes accounts, starts snapshots and the order-sync poller, exposes `/__uta/health` and trading routes, and closes broker/event-log resources on SIGTERM (`services/uta/src/main.ts:41-193`). The package's normal start command is `node dist/uta.js` and development command is `tsx watch src/main.ts` (`services/uta/package.json:7-12`).

**[observed]** Alice-to-UTA transport is already a TypeScript HTTP client with fetch, timeout, JSON serialization, and structured `UTAHttpError` status/body fields (`packages/uta-protocol/src/client/UTAClient.ts:13-29,37-45,48-99`). The protocol package is TypeScript plus Zod and emits declaration/build artifacts (`packages/uta-protocol/package.json:1-39`; `packages/uta-protocol/tsconfig.json:1-16`).

**[observed]** The current UTA registry has five JS workspace Broker Pack entries (`ccxt`, `alpaca`, `ibkr`, `leverup`, `longbridge`) and dynamically imports the active installed pack through a file URL, validating the pack API/version/schema/factory (`services/uta/src/domain/trading/brokers/registry.ts:44-136`). The pack contract is deliberately JavaScript/Zod (`docs/broker-packs.md:34-61`), and release verification tests clean Node import plus compiled Bun external loading (`docs/broker-packs.md:244-286`).

**[observed]** Development Guardian launches UTA as `tsx` and waits on `/__uta/health` (`scripts/guardian/dev.ts:265-283`). Production Guardian resolves either the legacy Node entrypoint or the Bun self-role, injects UTA environment, treats unexpected UTA exit as trading-offline, and restarts with SIGTERM then SIGKILL fallback before a 15-second health wait (`scripts/guardian/prod.mjs:280-314,388-399,483-534`; `scripts/guardian/runtime-process-spec.mjs:1-24`). Electron similarly spawns `services/uta/dist/uta.js` through `process.execPath` with `ELECTRON_RUN_AS_NODE=1` (`apps/desktop/src/main.ts:786-813`). The standalone Bun binary dispatches an internal `uta` role by importing `services/uta/src/main.ts` (`packages/cli/bin/openalice-bun.ts:34-55`).

**[observed]** The current runtime has more than a provider transport surface. Scoped line counts were: `UnifiedTradingAccount.ts` 1,262; `TradingGit.ts` 978; trading HTTP routes 728; UTA main 200; order sync poller 105; order entry/history/guards and related files 1,270 combined; protocol client 103; broker registry 136. The count is only a size signal, not a claim that every line must be ported.

**[observed]** The current order state machine records a pushed order, polls only healthy accounts with pending orders, then syncs terminal status; the poller has a fast 10-second lane and a default 15-minute external-order observation lane (`services/uta/src/domain/trading/order-sync-poller.ts:4-15,20-29,39-105`). `TradingGit.push` executes operations, captures results, reads state after execution, appends a commit, invokes `onCommit`, and clears staging (`services/uta/src/domain/trading/git/TradingGit.ts:119-179`). This is domain consistency behavior that an OpenAPI generator cannot supply.

## Rust evidence and primary references

**[observed]** Local Pingora source describes Pingora as a Rust framework for programmable networked systems, with HTTP/1/2, TLS, gRPC, WebSocket proxying, graceful reload, and failover features; Linux is tier 1, macOS is Unix-like development support, and Windows support is preliminary community best effort (`/tmp/pingora-language-research/README.md:5-22,50-65`). Official sources: [Pingora repository](https://github.com/cloudflare/pingora), [Pingora Peer guide](https://github.com/cloudflare/pingora/blob/main/docs/user_guide/peer.md), [Pingora phase guide](https://github.com/cloudflare/pingora/blob/main/docs/user_guide/phase.md), [Pingora internals](https://github.com/cloudflare/pingora/blob/main/docs/user_guide/internals.md), and [outbound-duplex example](https://github.com/cloudflare/pingora/blob/main/pingora/examples/app/proxy.rs).

**[observed]** Pingora's `HttpPeer` and `PeerOptions` expose upstream address/SNI, connection/read/write timeouts, TLS verification, keepalive, and HTTP upgrade policy (`docs/user_guide/peer.md:5-48`). Its internals describe one task per downstream connection and connectors for establishing/reusing upstream connections (`docs/user_guide/internals.md:84-101,207-235`). The local example uses `TransportConnector::new_stream` and a `tokio::select!` read/write loop to proxy bytes (`pingora/examples/app/proxy.rs:22-103`). **[inferred]** This is useful evidence that Pingora can be composed into an outbound stream service, but it is not evidence of a high-level provider subscription client with provider-specific authentication, heartbeats, reconnect state, or message decoding.

**[observed]** The official [`tokio-tungstenite::connect_async`](https://docs.rs/tokio-tungstenite/latest/tokio_tungstenite/fn.connect_async.html) API returns a `WebSocketStream<MaybeTlsStream<TcpStream>>` and the HTTP upgrade response; the [`WebSocketStream`](https://docs.rs/tokio-tungstenite/latest/tokio_tungstenite/struct.WebSocketStream.html) implements asynchronous stream/sink behavior and supports split read/write ownership. **[inferred]** A Rust UTA should use one Tokio task per long-lived provider connection, split the stream, and make reconnect/heartbeat policy explicit rather than treating a proxy duplex loop as the provider abstraction.

**[observed]** Tokio's [`broadcast`](https://docs.rs/tokio/latest/tokio/sync/broadcast/) is bounded and reports `RecvError::Lagged(n)` when a slow receiver is overwritten; Tokio [`mpsc`](https://docs.rs/tokio/latest/tokio/sync/mpsc/) bounded sends apply backpressure and `try_send` reports full/closed. **[inferred]** Use `broadcast::Sender<Arc<MarketObservation>>` only for intentionally droppable market ticks, and use bounded `mpsc` or a durable outbox for order-modify intents and Alice Issue callbacks. A slow callback consumer must not silently lose a trading intent.

**[observed]** Rust's official [message-passing chapter](https://doc.rust-lang.org/book/ch16-02-message-passing.html), [shared-state chapter](https://doc.rust-lang.org/book/ch16-03-shared-state.html), and [Send/Sync chapter](https://doc.rust-lang.org/book/ch16-04-extensible-concurrency-sync-and-send.html) document ownership-moving channels, `Arc` shared ownership, `Mutex` exclusive guards, and `Send`/`Sync` bounds. **[inferred]** Prefer actor/task-local mutable aggregation and channels over a global `Arc<Mutex<...>>`; retain locks only for small metadata sections. Spawned tasks must own `Send + 'static` data and must not hold borrowed provider/ledger state across awaits.

**[observed]** Stable Rust supports `async fn` in traits, but trait methods with native return-position `impl Trait` are not dyn-compatible; [the Rust async-fn-in-traits announcement](https://blog.rust-lang.org/2023/12/21/async-fn-rpit-in-traits/) and the [`async-trait` crate documentation](https://docs.rs/async-trait/latest/src/async_trait/lib.rs.html) describe the static-dispatch versus boxed-future trade-off. The [`thiserror` documentation](https://docs.rs/thiserror/latest/thiserror/index.html) provides typed `Error` derives. **[inferred]** Define provider traits with static dispatch where providers are known at compile time, or use `async-trait`/boxed futures only at a genuinely dynamic plugin boundary; model expected provider errors as enums with `thiserror` and preserve raw responses separately.

**[observed]** `effing-mad` calls itself an experiment in algebraic effects, lists unstable generators and `unsafe`, and states that nightly is required (`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/effing-mad-0.1.0/README.md:1-24,45-59,72-79`; `rust-toolchain.toml:1-2`). The [Rust effects initiative](https://rust-lang.github.io/effects-initiative/) and [explainer](https://rust-lang.github.io/effects-initiative/explainer/index.html) are language-design material, not a stable application effect runtime. **[inferred]** Do not make a production UTA dependent on an algebraic-effects crate to obtain ZIO-like semantics; use explicit `async` boundaries, typed `Result`, cancellation, and channels.

# Recommendations

## Language and runtime choice

**[inferred]** Implement the new UTA core in TypeScript first. Keep the service's existing Node launch path as the default because dev Guardian, production Guardian, and Electron already launch Node-compatible UTA entries; retain Bun 1.4.0 compatibility for the existing standalone role and release path. Treat Bun-vs-Node selection as a deployment measurement, not as a source-language fork.

**[inferred]** Keep the new service's domain independent of `Hono`, `fetch`, and any one OpenAPI generator. Put provider integration behind three explicit seams matching the design: capability declaration, pure/local translation, and transport/session execution. Retain provider raw response/event bytes plus source/time/request identity at the translation boundary. This permits a later Rust transport or sidecar without forcing a second domain model.

**[inferred]** For market fan-out, decode once into an immutable observation and publish it to independent read consumers. For `watch -> order modify`, convert the observation through a pure function into a typed intent and enqueue it through the sole read-write/ledger entry point. For `news subscription -> Alice Issue callback`, use a typed callback event with stable request/idempotency identity and a bounded or durable delivery path; do not put callback delivery on the droppable market-tick channel.

**[inferred]** If Rust is selected after a representative workload proves the need, use Tokio plus `tokio-tungstenite` for outbound provider WebSockets. Use Pingora for an inbound HTTP/WebSocket gateway or programmable proxy only. Model the core with native `async` + `Result`/`thiserror`, and use `async-trait` only where dynamically selected providers require object-safe async dispatch.

## Integration-cost calculus

**[inferred]** The table treats the new implementation as a real replacement that must preserve current consumers and distribution surfaces; it does not count provider-domain feature work that both languages must perform.

| Component | Rust change needed | TS change needed |
|---|---|---|
| Wire protocol and Alice SDK | Implement the wire schema, JSON/OpenAPI serialization, typed error mapping, and an HTTP server compatible with the existing client; preserve raw provider payloads at the service boundary. | Reuse the existing TS protocol/Zod/client patterns; add new types/routes as the new contract requires, without changing the transport language. |
| OpenAPI provider projections | Generate or hand-maintain Rust clients for each non-identical projection; write per-provider translation and capability declarations; own auth, pagination, rate limits, reconnects, and raw-response retention. | Generate or hand-maintain TS clients in the existing package ecosystem; current Node/Bun HTTP/WebSocket libraries and type tooling are immediately available. |
| JS Broker Packs | Reimplement or replace all five current JS pack integrations, or add and secure a JS runtime bridge; preserve pack API/version/config-schema semantics if packs remain. | Reuse the current dynamic file-URL import and Zod pack contract; no language bridge is needed. |
| Ledger, approval, reconciliation, guards | Port the UTA domain state machine, append-only records, structured errors, idempotency/unknown handling, snapshots, poller, and approval transitions; establish Rust persistence compatibility. | Reuse the current TS domain patterns while replacing only the contracts that the new design deliberately changes. |
| Guardian/dev/prod lifecycle | Produce a standalone binary with `--internal-role`/health/shutdown behavior or change Guardian to launch a new command; preserve restart and offline semantics. | Existing `tsx`/Node/Bun role and health/restart path already exists; only new route/contract wiring is needed. |
| Electron desktop | Package a Rust sidecar, spawn it without `ELECTRON_RUN_AS_NODE`, terminate/restart it, and handle nested macOS signing/notarization plus Windows packaging/signing. | Existing Electron child path already packages `services/uta/dist/**` and launches the Node entry; no sidecar toolchain is added. |
| CLI/release matrix | Add Rust toolchains/targets, cross-build and artifact/checksum rules for macOS arm64/x64, Linux targets, and Windows x64/arm64; test each binary's health and self-role behavior. | Reuse the existing Bun 1.4.0 build/release matrix and package verification, subject to its current platform support. |
| CI/build/cache | Add Cargo lock/cache/audit, target images, native-linker/TLS choices, and Rust supply-chain policy. | Reuse pnpm/TypeScript/tsup/tsx CI and existing Node/Bun setup. |
| Runtime fan-out and callback delivery | Implement Tokio task ownership, split WebSocket streams, bounded channels, lag/backpressure policy, HTTP callback client, cancellation, and observability. | Implement equivalent worker/channel/backpressure policy with the existing event-loop and client libraries; the benchmark shows target feasibility but does not supply durability semantics. |

## Falsifier for this recommendation

**[inferred]** The TypeScript-first recommendation is falsified if a representative provider replay—not the synthetic localhost producer—requires all of the following for one UTA: 100–200 concurrently active WebSockets; at least 25,000 **processed** messages/s sustained for 10 minutes; p99 from message-callback entry through decode, all required fan-out handlers, and queue submission at or below 5 microseconds; zero sequence/parse loss; and a bounded non-growing intent/callback queue. The present harness already showed queue growth around 22.8k actual messages/s and p99 values of 7.9–11.5 microseconds near that region, so that requirement would justify a Rust prototype and a direct apples-to-apples replay. A second falsifier is a Rust prototype that meets the real SLO while the measured migration cost is lower than retaining the existing JS pack/distribution surface.

# Unavailable / contradictions

- **[observed]** No Rust UTA implementation or Rust benchmark was run. The Rust conclusions are API/source evidence plus engineering recommendations, not measured Rust throughput.
- **[observed]** No production provider WebSocket trace, provider OpenAPI projection, ledger persistence workload, or Alice Issue callback endpoint was exercised. The benchmark uses synthetic JSON and localhost only.
- **[observed]** No product CPU, RSS, p99, queue-loss, callback-latency, or sustained-duration SLO was present in the inspected design/runtime surfaces. The 500 msg/s slope and five-second sweep are declared harness criteria, not repository requirements.
- **[observed]** Node was tested at v26.8.1, which satisfies but does not exactly match the package minimum of >=22.19.0. Bun was selected at the repository-pinned 1.4.0; the initially installed system Bun was 1.4.1 and was not used for reported runs.
- **[observed]** This was one macOS 14-era M4 host, with no repeated-run confidence intervals, Linux/Windows runs, TLS, proxy, provider rate limits, reconnect storms, or bursty fan-out. CPU/RSS numbers should not be generalized across hosts.
- **[observed]** Pingora's local README states Linux is tier 1 and Windows support is preliminary community best effort (`README.md:50-59`), while this repository has Windows x64/ARM64 preview workflow targets (`.github/workflows/windows-cli-preview.yml:50-80`). A Rust desktop decision therefore has a platform-support/signing risk not settled by the WebSocket benchmark.
- **[inferred]** The OpenAPI projection input narrows the language gap for transport and serialization but does not, by itself, establish a reason to move the domain and lifecycle machinery out of TypeScript. A representative trace plus explicit SLO is the missing decisive observation.
