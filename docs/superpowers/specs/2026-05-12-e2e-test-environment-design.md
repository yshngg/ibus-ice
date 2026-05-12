# E2E Test Environment Design for ibus-ice

## Summary

A pytest-based E2E test environment for the ibus-ice Pinyin input method engine. Tests run against an isolated IBus session (dbus-run-session + ibus-daemon) with the full production dictionary. On failure, tests produce rich structured diffs of expected-vs-actual candidates along with full internal pipeline traces (segmentation, dictionary lookups, ranking scores).

## Architecture

Three layers:

```
pytest test files (tests/test_*.py)
    Chainable TestClient API, custom assert helpers, event inspection
        │
test fixtures (tests/conftest.py)
    Session-scoped: build dict (Makefile), find compiled .so, generate component XML
    Function-scoped: isolated dbus-run-session + ibus-daemon per test
    Clean teardown: kill daemon, fresh user dict per test
        │
instrumentation layer
    Normal path: engine.py (unchanged) → FFI process() → candidates
    Failure path: debug_process() FFI → internal pipeline state dump
```

Each `pytest` function gets a fresh, isolated IBus session. The engine code path under test is identical to production. Instrumentation (pipeline tracing) is called only on assertion failure, so passing tests pay zero overhead.

## Directory Structure

```
ibus-ice/
├── tests/
│   ├── conftest.py              # fixtures: dict, .so, component_xml, ibus_session, client
│   ├── test_helpers.py          # TestClient, Candidate, EngineEvent, assert_* helpers
│   ├── trace.py                 # debug_process() ctypes wrapper, TraceCapture
│   ├── test_basic_input.py      # basic pinyin typing, preedit display
│   ├── test_candidates.py       # candidate ordering, page navigation
│   ├── test_commit_select.py    # space commit, number select, explicit commit
│   ├── test_special_keys.py     # backspace, escape, enter, apostrophe separator
│   ├── test_english.py          # English word input path
│   ├── test_ranking.py          # frequency ordering, user boost after select
│   └── test_edge_cases.py       # empty input, rapid typing, max length
│
├── core/src/
│   ├── engine.rs                # unchanged
│   ├── ffi.rs                   # + debug_process(), DebugResult struct
│   └── debug_result.rs          # NEW: DebugResult type, DebugCandidate type
│
├── Makefile                     # + test-e2e target
└── scripts/
    └── build-dict.sh            # unchanged
```

## Test Fixtures

### Session-scoped (shared across all tests)

| Fixture | Purpose |
|----------|---------|
| `ice_dict` | Runs `make build-dict` (or `scripts/build-dict.sh`). Returns path to `build/ice.dict`. Built once. |
| `ice_engine_so` | Finds compiled `.so` from release or debug target directory. Returns path. |
| `component_xml` | Generates `ice.xml` component descriptor pointing at the `.so`, dict, and a tmp user-dict directory. |

### Function-scoped (fresh per test)

| Fixture | Purpose |
|----------|---------|
| `ibus_session` | Launches `dbus-run-session` with the generated `ice.xml` config. Inside it: `ibus-daemon --daemonize --replace`. Waits for daemon readiness (poll `ibus engine --list`). On teardown: kills daemon (dbus-run-session process exit cleans the D-Bus session). Uses `tmp_path` for user-dict isolation. |
| `client` | Creates a `TestClient` wrapping an `IBus.InputContext` created within the isolated session. Returns the client. |

### Makefile

```makefile
test-e2e:
	$(MAKE) build
	$(MAKE) build-dict
	python -m pytest tests/ -v
```

## TestClient API

Wraps `IBus.InputContext` with chainable methods that simulate real user key events:

```python
class TestClient:
    # Input actions (all return self for chaining)
    def type_pinyin(self, text: str) -> "TestClient": ...
    def press_space(self) -> "TestClient": ...
    def press_enter(self) -> "TestClient": ...
    def press_escape(self) -> "TestClient": ...
    def press_backspace(self) -> "TestClient": ...
    def press_number(self, n: int) -> "TestClient": ...
    def press_page_up(self) -> "TestClient": ...
    def press_page_down(self) -> "TestClient": ...
    def press_key(self, keyval: int, modifiers: int = 0) -> "TestClient": ...

    # Inspection
    def get_candidates(self) -> list[Candidate]: ...
    def get_preedit(self) -> str: ...
    def get_committed(self) -> str: ...        # accumulated committed text
    def get_aux_text(self) -> str: ...

    # Observable
    def events(self) -> list[EngineEvent]: ...  # all IBus signals received
    def clear_events(self) -> "TestClient": ...

@dataclass
class Candidate:
    text: str
    label: str        # "1", "2", etc.

@dataclass
class EngineEvent:
    type: str         # "commit-text", "update-preedit", "update-aux", "update-lookup-table"
    text: str | None
    candidates: list[Candidate] | None
```

### Usage Examples

```python
def test_basic_pinyin(client):
    client.type_pinyin("zhongguo")
    assert client.get_candidates()[0].text == "中国"
    assert client.get_preedit() == "zhongguo"

def test_select_and_commit(client):
    client.type_pinyin("nihao").press_number(2).press_space()
    assert "你好" in client.get_committed()

def test_backspace(client):
    client.type_pinyin("zhongguo").press_backspace()
    assert client.get_preedit() == "zhongg"
```

## Failure Diagnostics

### Rich Diffs

Custom assertion helpers produce position-aware diffs:

```python
def assert_candidates(actual: list[Candidate], expected: list[dict]):
    """Assert candidates match expected. On failure, show index-aligned diff."""

def assert_committed(client, expected_text: str): ...

def assert_preedit(client, expected_text: str): ...
```

Failure output format:
```
E   AssertionError: Candidate mismatch at index 1
E   Position:   0      1          2
E   Expected:   中国   中国人      中国画
E   Actual:     中国   中国话      中国人
```

### Pipeline Trace (called only on failure)

A new Rust FFI function mirrors `process()` but also returns internal state:

```rust
debug_process(pinyin: *const c_char) -> *mut DebugResult
```

```rust
struct DebugResult {
    pinyin: String,
    candidates: Vec<DebugCandidate>,
    segments: Vec<SegmentInfo>,
    total_dict_entries_searched: u32,
}

struct DebugCandidate {
    text: String,
    pinyin: String,
    freq: u32,
    total_score: f64,
}

struct SegmentInfo {
    segment: String,
    start: usize,
    end: usize,
}
```

On `AssertionError`, the assertion helper calls `debug_process()` with the same pinyin buffer, formats the trace, and appends it to the failure message:

```
E   AssertionError: Candidate mismatch at index 1
E   Position:   0      1          2
E   Expected:   中国   中国人      中国画
E   Actual:     中国   中国话      中国人
E
E   Pipeline trace for "zhongguo":
E   ── Segmentation ──
E     [0..4] "zhong"  [4..7] "guo"
E   ── Dict Lookups ──
E     "zhong" → 18 entries
E     "guo"   → 12 entries
E   ── Ranked Candidates ──
E     #1 "中国"   score=9.2  (base=7.8 user=0.0 exact=1.4)
E     #2 "中国话" score=6.1  (base=5.7 user=0.0 exact=0.4)
E     #3 "中国人" score=5.3  (base=5.1 user=0.0 exact=0.2)
```

## Rust Changes

Only one new file and one modified file:

### `core/src/debug_result.rs` (NEW)

`DebugResult`, `DebugCandidate`, `SegmentInfo` structs with `#[repr(C)]` for FFI.

`DebugCandidate` fields: `text` (CString ptr), `pinyin` (CString ptr), `freq` (u32), `total_score` (f64).

`SegmentInfo` fields: `segment` (CString ptr), `start` (usize), `end` (usize).

Memory owned by Rust; freed via `ice_debug_result_free()`.

### `core/src/ffi.rs` (MODIFIED)

Add functions:

```rust
pub extern "C" fn ice_debug_process(
    handle: *mut IceEngineHandle,
    pinyin: *const c_char,
) -> *mut DebugResult

pub extern "C" fn ice_debug_result_free(result: *mut DebugResult)
```

Corresponding Python ctypes bindings added in `tests/trace.py`.

## Python Changes

- `engine/engine.py` — **unchanged** (normal production path)
- `engine/ffi.py` — **unchanged** (test harness uses its own ctypes bindings)

## Test File Organization

| File | Tests |
|------|-------|
| `test_basic_input.py` | Typing pinyin produces correct preedit; candidates appear; multi-character input |
| `test_candidates.py` | Candidate ordering matches expected ranking; page up/down navigation |
| `test_commit_select.py` | Space commits first candidate; number key selects specific candidate; enter commits raw pinyin |
| `test_special_keys.py` | Backspace removes characters; escape resets state; apostrophe separator ("xi'an") |
| `test_english.py` | English words pass through; mixed pinyin/english fallback |
| `test_ranking.py` | Higher frequency ranks higher; selecting a candidate boosts it for the session; userdict persistence across engine restart |
| `test_edge_cases.py` | Empty input, very long input, rapid keystrokes, shift+letter (uppercase) |

## Dependencies

- **pytest** — test runner
- **pygobject / gi** — IBus Python bindings (already available at runtime)
- **dbus-run-session** — part of D-Bus, available on all Linux distributions with IBus
- **ibus-daemon** — part of IBus
- No new Rust dependencies, no new Python packages

## Error Handling & Edge Cases

| Scenario | Handling |
|----------|----------|
| dict-compiler or cargo build fails | Session-scoped fixture fails fast with clear error; tests don't run |
| ibus-daemon fails to start | `ibus_session` fixture retries up to 3 times with 1s delay; raises `RuntimeError` with bus logs |
| Engine crashes during test | daemon restart is automatic within the session; test gets a fresh engine state |
| Stale daemon from prior run | `ibus_session` fixture `pkill -f ibus-daemon` on the custom config path before starting |
| Concurrent test runs | `pytest-xdist` can be used; each worker gets its own tmp_path with unique D-Bus session address |
| `debug_process()` returns null or errors | Assertion helper catches and appends raw error to failure message instead of trace; test still shows the original diff |
| Test client cannot connect to isolated D-Bus session | `client` fixture retries with backoff up to 5s; raises `ConnectionError` with session status output |

## Success Criteria

1. `make test-e2e` passes with the full production dictionary
2. Test failures show index-aligned expected-vs-actual candidate diffs
3. Test failures display internal pipeline trace (segmentation, lookups, ranking scores)
4. No cross-test pollution (each test gets clean engine and user dict state)
5. Passing tests have zero overhead from the instrumentation layer
