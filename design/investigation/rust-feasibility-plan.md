# Rust ecosystem feasibility investigation plan

Decisive question: can stable Rust crates satisfy B1-B7/D01-D08 core needs—parallel async distribution, bounded backpressure/fan-out, sharded state, deterministic isolated WASM programs, cross-process gRPC/protobuf over UDS, durable append-only effects, exact decimal/numeric calculations, process supervision, singleton/daemon lifecycle, and cross-platform IPC—or is any capability genuinely infeasible in Rust?

Steps:
1. Map each requested technical need to B1-B7/D01-D08 and classify it as a feasibility decision.
2. For each need, collect current official docs/release metadata, exact capability quote, version/date, maturity signal, limitation, concrete UTA risk, and fallback.
3. Resolve uncertainty using smallest authoritative observations; skip builds, tests, formatters, linters.
4. Synthesize an evidence-based infeasibility list and minimal crate set.
5. Write a report with exactly 15 summary lines, evidence sections, and explicit unknowns/could-not-verify.

Out of scope: repository changes; treating old UTA behavior as new design; venue-native idempotency/cursor decisions; implementation tutorial; validation suites.

Assumptions to verify: docs.rs/crates.io metadata are sufficient; Wasmtime fuel/memory/snapshot semantics can meet isolation/determinism; tonic/prost UDS works on target OSes or limitations are explicit; a durable log exists or can be safely wrapped around filesystem primitives; Tokio cancellation/channels preserve required ordering/backpressure.
