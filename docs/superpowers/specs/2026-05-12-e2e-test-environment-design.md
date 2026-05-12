# Observable E2E Test Environment — Design Specification

## Overview

Build an observable end-to-end test environment for ibus-ice that exercises the full IBus integration path (key press → daemon → engine → Rust core → candidates → commit). Tests run in an isolated D-Bus session with a headless ibus-daemon, driven via pytest. The observability layer provides per-stage tracing (segmentation, dict lookup, ranking, display), structured JSON output, golden-file snapshots for regression detection, and a per-run aggregate trace file.

---

## 1. Architecture

```
┌─────────────────────────────────────────────────────┐
│                  Test Runner (pytest)                │
│  ┌───────────────────────────────────────────────┐  │
│  │            Observability Layer                 │  │
│  │  TracingEngine ──► StageTracer ──► JSON       │  │
│  │  SnapshotFixture ──► Golden file diff          │  │
│  │  RunAggregator ──► Per-run trace file          │  │
│  └───────────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────────┐  │
│  │              Test Fixtures (conftest.py)       │  │
│  │  dbus_session ──► isolated D-Bus session bus  │  │
│  │  ibus_daemon  ──► ibus-daemon -xrv            │  │
│  │  test_dict_path ──► compiled test.dict binary │  │
│  │  ice_engine   ──► TracingEngine + IceIBus     │  │
│  │  test_client  ──► InputContext helper          │  │
│  └───────────────────────────────────────────────┘  │
│                                                      │
│  ┌───────────────────────────────────────────────┐  │
│  │  IceIBusEngine ◄── D-Bus ◄── ibus-daemon      │  │
│  │       │                          ▲             │  │
│  │       ▼                          │             │  │
│  │  [Rust] ◄── ctypes               │             │  │
│  │  libibus_ice_core.so (tracing)   │             │  │
│  │       │                          │             │  │
│  │       ▼                          │             │  │
│  │  mmap(test.dict)                 │             │  │
│  └───────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────┘
```

**Key decisions:**
- Engine runs **in-process** with the test, registered via `IBus.Bus.request_name()`. D-Bus traffic exercises the full serialization path since the daemon dispatches key events through D-Bus.
- **`tests/test_dict.yaml`** (17 entries) is used instead of the full 913K-entry dict — fast startup, deterministic results.
- **Isolation:** Private D-Bus session bus + headless ibus-daemon per test run, no interference with user's desktop session.

---

## 2. Project Structure

```
ibus-ice/
├── tests/
│   ├── conftest.py              # pytest fixtures
│   ├── observability.py         # StageTracer, TracingEngine, StructuredLogger
│   ├── snapshot.py              # SnapshotFixture, golden file management
│   ├── test_client.py           # TestClient helper class
│   ├── test_e2e.py              # E2E test cases
│   ├── test_dict.yaml           # (existing) test dictionary — 17 entries
│   ├── snapshots/               # golden JSON files, one per test case
│   │   └── test_basic_pinyin.json
│   ├── output/                  # per-run aggregate trace files
│   │   └── run-2026-05-12T103000.json
│   └── dict/                    # compiled test dict output
│       └── test.dict
├── core/
│   ├── Cargo.toml               # + [features] tracing
│   └── src/
│       ├── engine.rs            # process() returns trace when tracing
│       ├── segmenter.rs         # tracing-gated span emission
│       ├── dictionary.rs        # tracing-gated span emission
│       ├── ranker.rs            # tracing-gated span emission
│       └── ffi.rs               # + ice_get_trace(), ice_trace_free()
├── Makefile                     # + test-e2e, build-test-dict targets
└── docs/
    └── superpowers/
        └── specs/
            └── 2026-05-12-e2e-test-environment-design.md
```

---

## 3. Test Fixtures

### 3a. Session Fixtures (`conftest.py`)

| Fixture | Scope | Responsibility |
|---------|-------|----------------|
| `dbus_session` | session | Spawns `dbus-run-session -- bash` with `DBUS_SESSION_BUS_ADDRESS` captured. Yields the bus address. Kills session on teardown. |
| `ibus_daemon` | session | Given `dbus_session`, launches `ibus-daemon -xrv --panel=disable` in background. Waits for readiness via `IBus.Bus()` connection. Stops on teardown. |
| `test_dict_path` | session | Compiles `tests/test_dict.yaml` → `tests/dict/test.dict` via `dict-compiler`. Idempotent — skips if binary is newer than YAML source. |
| `run_aggregator` | session | Collects per-test traces. On teardown, writes `tests/output/run-{timestamp}.json`. |

### 3b. Function Fixtures

| Fixture | Scope | Responsibility |
|---------|-------|----------------|
| `ice_engine` | function | Sets `IBUS_ICE_DATA_DIR` to `tests/dict/`, sets `USER_DICT_PATH` to a tempdir, creates `TracingEngine` wrapping `IceIBusEngine`, registers on the test bus. Resets state between tests. |
| `test_client` | function | Creates an `IBus.InputContext` connected to the test bus. Yields a `TestClient` wrapper that sends key events and captures signals. |
| `snapshot` | function | Provides `assert_match(data, name)` for golden-file comparison. Integrates with `run_aggregator`. |

---

## 4. TestClient API

Wraps `IBus.InputContext` for clean, chainable test ergonomics:

```python
class TestClient:
    def type_pinyin(self, text: str) -> "TestClient":
        """Type each char as a-z key press. Returns self for chaining."""

    def press_space(self) -> "TestClient":
        """Press space to commit first candidate."""

    def press_number(self, n: int) -> "TestClient":
        """Press 1-9 to commit nth candidate."""

    def press_escape(self) -> "TestClient":
        """Press Escape to reset."""

    def press_backspace(self, count: int = 1) -> "TestClient":
        """Press Backspace count times."""

    @property
    def candidates(self) -> list[str]:
        """Last observed candidate texts."""

    @property
    def preedit(self) -> str:
        """Current preedit text."""

    @property
    def committed(self) -> str:
        """Last committed text."""

    @property
    def trace(self) -> dict:
        """Full trace document from the last interaction sequence."""
```

---

## 5. Observability Layer

### 5a. StageTracer

Context manager with microsecond timing:

```python
with StageTracer("segment", input="zhongguo") as span:
    result = segment(pinyin)
# span: {stage, input, output, duration_us, timestamp}
```

### 5b. TracingEngine

Wraps `ffi.Engine` in the Python layer. On `process()`:
1. Calls Rust `ice_process()` (via ctypes)
2. Calls Rust `ice_get_trace()` to retrieve per-stage spans
3. Merges Python-side timing (ffi_total) with Rust-side spans
4. Accumulates spans for the current test case

Returns `(candidates, trace_spans)`.

### 5c. Capture Points

| Stage | Source | What's Logged |
|-------|--------|---------------|
| Key event | Python `do_process_key_event` wrapper | keyval, modifiers, timestamp |
| Segment | Rust `segmenter.rs` (tracing-gated) | raw pinyin → syllable list, duration |
| Dict lookup | Rust `dictionary.rs` (tracing-gated) | query string, match count, duration |
| Rank | Rust `ranker.rs` (tracing-gated) | candidates in/out count, duration |
| Display | Python `_update_candidates` post-call | preedit, visible candidates, count |
| Commit | Python `_commit` wrapper | selected text, trigger key, timestamp |
| Reset | Python `_reset` wrapper | buffer clear, lookup table hidden |

---

## 6. Rust Tracing Instrumentation

### 6a. Feature Gate

`core/Cargo.toml`:

```toml
[features]
tracing = []
```

Zero overhead when disabled (production). Test builds use `--features tracing`.

### 6b. Instrumented Functions

- **`segmenter::segment()`** — emits span with input syllables and duration
- **`dictionary::lookup()`** — emits span with query string, match count, duration
- **`ranker::rank()`** — emits span with candidate counts before/after, duration
- **`engine::process()`** — aggregates sub-spans, stores in engine state

### 6c. C ABI Extension (tracing-gated)

Two new `extern "C"` functions, only compiled under `#[cfg(feature = "tracing")]`:

```c
// Returns JSON string of trace spans for the last ice_process call.
// Caller must free with ice_trace_free().
char* ice_get_trace(IceEngine *engine);

void  ice_trace_free(char *trace);
```

The trace is a JSON array:

```json
[
  {"stage": "segment", "input": "zhongguo", "output": ["zhong", "guo"], "duration_us": 12},
  {"stage": "dict_lookup", "query": "zhong guo", "match_count": 3, "duration_us": 45},
  {"stage": "rank", "candidates_in": 3, "candidates_out": 3, "duration_us": 23}
]
```

### 6d. ffi.py Extension

The `Engine` wrapper gains a `get_trace()` method (returns parsed list of dicts, or empty list when tracing is disabled):

```python
def get_trace(self) -> list[dict]:
    trace_ptr = _lib.ice_get_trace(self._handle)
    if not trace_ptr:
        return []
    raw = ctypes.cast(trace_ptr, ctypes.c_char_p).value
    result = json.loads(raw) if raw else []
    _lib.ice_trace_free(trace_ptr)
    return result
```

---

## 7. Snapshot & Aggregate Output

### 7a. Per-Test Snapshots

Stored in `tests/snapshots/{test_name}.json`. Each file contains the trace document for that test with `input_sequence`, `stages`, `candidates`, `committed`, and `summary` fields. Compared against current run output via `SnapshotFixture.assert_match()`.

### 7b. Run Aggregate

Written by the `run_aggregator` fixture on session teardown to `tests/output/run-{timestamp}.json`:

```json
{
  "run_id": "20260512T103000-a1b2c3",
  "timestamp": "2026-05-12T10:30:00Z",
  "test_dict": "tests/test_dict.yaml",
  "test_dict_hash": "sha256:abc123...",
  "core_version": "0.1.0 (tracing enabled)",
  "pytest": {"total": 8, "passed": 7, "failed": 1, "duration_ms": 4500},
  "tests": [
    {
      "name": "test_basic_pinyin",
      "status": "passed",
      "duration_ms": 1200,
      "trace": { /* full per-test trace */ }
    }
  ],
  "summary": {
    "avg_lookup_us": 45,
    "p99_lookup_us": 120,
    "total_candidates_generated": 142
  }
}
```

### 7c. Snapshot Diff Format

On mismatch:
```
--- tests/snapshots/test_basic_pinyin.json  (stored)
+++ tests/snapshots/test_basic_pinyin.json  (actual)
@@ -10,7 +10,7 @@
     {
       "action": "process_key_event",
-      "candidates": [{"text": "中国", "freq": 10000}]
+      "candidates": [{"text": "中文", "freq": 8000}]
     }
```

### 7d. `--snapshot-update` Flag

When passed to pytest, overwrites golden files with current output instead of diffing. Used for first-time setup and intentional behavior changes.

---

## 8. Test Cases

| Test Name | Input Sequence | Assertions |
|-----------|---------------|------------|
| `test_basic_pinyin` | `z h o n g g u o SPACE` | `中国` at position 0, `中国` committed |
| `test_multi_candidates` | `z h o n g` | Multiple candidates, all start with zhong-prefix characters |
| `test_english` | `h e l l o` | `hello` in candidates |
| `test_backspace` | `z h o n g BS BS` | Candidates update after each backspace, buffer shows `zho` |
| `test_select_by_number` | `z h o n g g u o 2` | Second candidate committed |
| `test_escape_reset` | `z h o n g ESC` | Buffer cleared, candidates hidden |
| `test_ranking_deterministic` | `z h o n g` | Same input → same candidate order, snapshot matches |
| `test_apostrophe` | `x i ' a n` | `xi'an` segmented correctly, candidates for `西/西安` |

Each test also calls `snapshot.assert_match(client.trace, "test_name.json")` to verify full trace output.

---

## 9. Build Integration

### 9a. Makefile Targets

```makefile
# Full E2E test: build test dict + tracing core + run pytest
test-e2e: build-test-dict
	cargo build -p core --features tracing
	cd engine && uv run pytest ../tests/ -v

# Build test dictionary from test data
build-test-dict:
	cargo build --release -p dict-compiler
	mkdir -p tests/dict
	./target/release/dict-compiler tests/test_dict.yaml -o tests/dict/test.dict

# Update golden snapshots
test-e2e-update: build-test-dict
	cargo build -p core --features tracing
	cd engine && uv run pytest ../tests/ -v --snapshot-update

# Fast unit tests (unchanged)
test:
	cargo test -p core
	cargo test -p dict-compiler
```

### 9b. Dependencies

Python test deps (added to `engine/pyproject.toml`):
- `pytest>=8`
- `pytest-timeout`

IBus Python bindings (`gi`, `IBus`) are system packages — already required at runtime, no additional install needed.

### 9c. Session Fixture Lifecycle

```
Session start:
  dbus_session:  dbus-run-session -- bash -c 'echo $DBUS_SESSION_BUS_ADDRESS > /tmp/ibus-ice-test-addr && sleep infinity'
                 parse address from file, yield it
  ibus_daemon:   DBUS_SESSION_BUS_ADDRESS=$ADDR ibus-daemon -xrv --panel=disable &
                 wait until IBus.Bus() connects successfully (retry with timeout)
                 yield
  test_dict:     cargo run -p dict-compiler -- tests/test_dict.yaml -o tests/dict/test.dict
                 (idempotent — skip if binary newer than YAML source)

Per-test:
  ice_engine:    set IBUS_ICE_DATA_DIR=tests/dict, USER_DICT_PATH=<tempdir>/user.dict
                 create TracingEngine, register on test bus, reset state
  test_client:   create InputContext, wrap in TestClient, reset context

Session teardown:
  kill ibus-daemon, kill dbus-run-session, remove temp files
```

---

## 10. Error Handling & Edge Cases

- **ibus-daemon not installed:** Skip E2E tests with clear message ("ibus-daemon not found, install ibus to run E2E tests"), exit code 0
- **D-Bus session fails to start:** Fail with diagnostic showing dbus-run-session stderr
- **Daemon starts but engine registration fails:** Fail with daemon stderr output
- **Timeout:** Each test limited to 30s via pytest-timeout
- **Tracing C ABI functions not found:** `Engine.get_trace()` returns empty list gracefully (handles missing `tracing` feature at load time)
- **`ice_get_trace` returns NULL:** `get_trace()` returns `[]` rather than crashing
- **Snapshot directory missing:** Created automatically on first run
- **Concurrent test runs:** `dbus_session` uses a unique socket name per run, `tests/dict/` is idempotent

---

## 11. Out of Scope

- CI/CD integration (GitHub Actions) — local-only for this phase
- `pytest-snapshot` pip dependency — implement a minimal custom snapshot fixture inline to avoid adding pip deps beyond `pytest`
- Code coverage measurement
- Stress/performance benchmarks
- Tests against the full 913K-entry production dictionary
- Simulating Wayland-specific input behavior
- Non-ASCII keyboard layouts

---

## Decisions Record

| Decision | Option | Rationale |
|----------|--------|-----------|
| Test isolation | Private DBus session + headless ibus-daemon | True E2E without interfering with desktop |
| Dict for tests | `tests/test_dict.yaml` (17 entries) | Fast startup, deterministic, ships in repo |
| Tracing impl | `#[cfg(feature = "tracing")]` + FFI function | Zero production overhead, opt-in per build |
| Snapshot library | Custom inline fixture | Avoids adding pip deps beyond pytest |
| Engine process model | In-process (same Python process) | Simplifies debugging, D-Bus path still exercised |
| Test framework | pytest | Ubiquitous, fixture system maps well to our needs |
| Aggregate output | Single JSON per pytest invocation | Enables post-hoc analysis of full run |
