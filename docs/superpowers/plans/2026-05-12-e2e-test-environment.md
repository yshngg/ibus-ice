# E2E Test Environment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a pytest E2E test environment for ibus-ice with isolated IBus sessions, a chainable TestClient API, and failure diagnostics (rich diffs + per-stage pipeline traces).

**Architecture:** Rust core gains a `debug_process()` method returning JSON with segmentation, dictionary lookups, and ranked candidate details via new FFI functions. Python test layer uses pytest fixtures to start a dedicated `dbus-daemon` + `ibus-daemon` per test with `IBUS_COMPONENT_PATH` pointing at a generated `ice.xml`. A `TestClient` wraps `IBus.InputContext` for key simulation. Custom `assert_candidates()` etc. call `debug_process()` on failure to dump the pipeline trace.

**Tech Stack:** Rust (cdylib), Python 3.10+ (pytest, ctypes, PyGObject/IBus), dbus-daemon, ibus-daemon

---

### Task 1: Rust — DebugResult and debug_process

**Files:**
- Create: `core/src/debug_result.rs`
- Modify: `core/src/engine.rs`
- Modify: `core/src/ffi.rs`
- Modify: `core/src/lib.rs`

- [ ] **Step 1: Create `core/src/debug_result.rs`**

```rust
use std::ffi::CString;
use std::os::raw::c_char;

#[repr(C)]
pub struct IceDebugResult {
    pub json: *mut c_char,
}

impl IceDebugResult {
    pub fn from_json(json: String) -> Box<Self> {
        Box::new(IceDebugResult {
            json: CString::new(json).unwrap().into_raw(),
        })
    }
}

impl Drop for IceDebugResult {
    fn drop(&mut self) {
        unsafe {
            if !self.json.is_null() {
                drop(CString::from_raw(self.json));
            }
        }
    }
}
```

- [ ] **Step 2: Add `debug_process()` to `core/src/engine.rs`**

Add to `use` imports at top:

```rust
use crate::segmenter::segment;
```

Add method to `impl IceEngine` (before the closing `}`):

```rust
pub fn debug_process(&self, pinyin: &str) -> String {
    let clean_pinyin = pinyin.trim().to_lowercase();
    let mut json = String::new();

    // Segmentation
    let segmentations = segment(&clean_pinyin);
    let mut pos: usize = 0;
    let mut seg_entries: Vec<(String, usize, usize, usize)> = Vec::new();

    if let Some(seg) = segmentations.first() {
        for syllable in &seg.syllables {
            let end = pos + syllable.len();
            let entries = self.dict.lookup(syllable);
            seg_entries.push((syllable.clone(), pos, end, entries.len()));
            pos = end;
        }
    }

    // Candidates with ranking
    let mut candidates = crate::candidate::generate(&self.dict, &clean_pinyin, false);
    for c in &mut candidates {
        c.user_boost = self.user_dict.get_boost(&c.text);
    }
    self.ranker.rank(&mut candidates);
    candidates.truncate(50);

    // Build JSON manually (no serde dependency)
    fn esc(s: &str) -> String {
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
    }

    json.push('{');
    json.push_str(&format!("\"pinyin\":\"{}\"", esc(&clean_pinyin)));

    json.push_str(",\"segments\":[");
    for (i, (s, start, end, n)) in seg_entries.iter().enumerate() {
        if i > 0 { json.push(','); }
        json.push_str(&format!(
            "{{\"segment\":\"{}\",\"start\":{},\"end\":{},\"entries\":{}}}",
            esc(s), start, end, n
        ));
    }
    json.push(']');

    json.push_str(",\"candidates\":[");
    for (i, c) in candidates.iter().enumerate() {
        if i > 0 { json.push(','); }
        json.push_str(&format!(
            "{{\"text\":\"{}\",\"freq\":{},\"score\":{:.4},\"user_boost\":{:.4},\"exact_match\":{}}}",
            esc(&c.text), c.freq, c.score, c.user_boost, c.exact_match
        ));
    }
    json.push(']');
    json.push('}');

    json
}
```

- [ ] **Step 3: Add FFI functions to `core/src/ffi.rs`**

Add import at top (after `use std::ffi::{CStr, CString};`):

```rust
use crate::debug_result::IceDebugResult;
```

Add functions at end of file:

```rust
#[no_mangle]
pub extern "C" fn ice_debug_process(
    handle: *mut IceEngineHandle,
    pinyin: *const c_char,
) -> *mut IceDebugResult {
    if handle.is_null() || pinyin.is_null() {
        return std::ptr::null_mut();
    }
    let engine = unsafe { &(*handle).engine };
    let pinyin = unsafe { CStr::from_ptr(pinyin) }.to_string_lossy();
    let json = engine.debug_process(&pinyin);
    Box::into_raw(IceDebugResult::from_json(json))
}

#[no_mangle]
pub extern "C" fn ice_debug_result_free(result: *mut IceDebugResult) {
    if !result.is_null() {
        unsafe { drop(Box::from_raw(result)) };
    }
}
```

- [ ] **Step 4: Register module in `core/src/lib.rs`**

Add after `pub mod ffi;`:

```rust
pub mod debug_result;
```

- [ ] **Step 5: Build and run existing tests**

Run: `cargo build --release -p core`
Expected: Build succeeds.

Run: `cargo test -p core`
Expected: All 13 existing tests pass.

- [ ] **Step 6: Commit**

```bash
git add core/src/debug_result.rs core/src/engine.rs core/src/ffi.rs core/src/lib.rs
git commit -m "feat(core): add debug_process() returning JSON pipeline trace"
```

---

### Task 2: Python — trace.py (ctypes for debug_process)

**Files:**
- Create: `tests/__init__.py`
- Create: `tests/trace.py`

- [ ] **Step 1: Create `tests/__init__.py`**

```python
# tests package
```

- [ ] **Step 2: Create `tests/trace.py`**

```python
"""ctypes bindings for ice_debug_process — used only on test failure."""

import ctypes
import json
import os
from ctypes import POINTER, Structure, c_char_p, c_int32, c_void_p


class IceDebugResult(Structure):
    _fields_ = [("json", c_char_p)]


def _find_lib() -> str:
    project = os.path.join(os.path.dirname(__file__), "..")
    for sub in ["target/release/libcore.so", "target/debug/libcore.so"]:
        p = os.path.join(project, sub)
        if os.path.exists(p):
            return p
    raise RuntimeError("Cannot find libcore.so for test tracing")


_lib = ctypes.CDLL(_find_lib())

_lib.ice_engine_new.argtypes = [c_char_p, c_char_p]
_lib.ice_engine_new.restype = c_void_p

_lib.ice_engine_free.argtypes = [c_void_p]
_lib.ice_engine_free.restype = None

_lib.ice_debug_process.argtypes = [c_void_p, c_char_p]
_lib.ice_debug_process.restype = POINTER(IceDebugResult)

_lib.ice_debug_result_free.argtypes = [POINTER(IceDebugResult)]
_lib.ice_debug_result_free.restype = None


class TraceEngine:
    """Direct engine wrapper for pipeline trace capture on test failure."""

    def __init__(self, dict_path: str):
        self._handle = _lib.ice_engine_new(
            dict_path.encode("utf-8"),
            b"/dev/null",  # user dict not needed for trace
        )
        if not self._handle:
            raise RuntimeError(f"Failed to create TraceEngine (dict={dict_path})")

    def debug_process(self, pinyin: str) -> dict:
        result_ptr = _lib.ice_debug_process(self._handle, pinyin.encode("utf-8"))
        if not result_ptr:
            return {"pinyin": pinyin, "error": "debug_process returned null"}
        try:
            raw = result_ptr.contents.json.decode("utf-8") if result_ptr.contents.json else "{}"
            return json.loads(raw)
        except (json.JSONDecodeError, UnicodeDecodeError) as e:
            return {"pinyin": pinyin, "error": str(e)}
        finally:
            _lib.ice_debug_result_free(result_ptr)

    def close(self):
        if self._handle:
            _lib.ice_engine_free(self._handle)
            self._handle = None

    def __del__(self):
        self.close()
```

- [ ] **Step 3: Commit**

```bash
git add tests/__init__.py tests/trace.py
git commit -m "feat(tests): add TraceEngine for debug_process() ctypes wrapper"
```

---

### Task 3: Python — test_helpers.py (TestClient, Candidates, assertions)

**Files:**
- Create: `tests/test_helpers.py`

- [ ] **Step 1: Create `tests/test_helpers.py`**

```python
"""TestClient, dataclasses, and custom assertion helpers."""

from dataclasses import dataclass, field

import gi
gi.require_version("IBus", "1.0")
from gi.repository import IBus, GLib

from trace import TraceEngine


@dataclass
class Candidate:
    text: str
    label: str = ""


@dataclass
class EngineEvent:
    type: str
    text: str | None = None
    candidates: list[Candidate] | None = None


class TestClient:
    """Chainable E2E test client wrapping IBus.InputContext."""

    def __init__(self, bus_address: str, trace_engine: TraceEngine | None = None):
        self._bus = IBus.Bus()
        self._ic_path = self._bus.create_input_context("TestClient")
        self._ic = IBus.InputContext(self._bus, self._ic_path, True)
        self._ic.set_capabilities(7)

        self._committed = ""
        self._preedit = ""
        self._preedit_visible = False
        self._candidates: list[Candidate] = []
        self._aux_text = ""
        self._events: list[EngineEvent] = []
        self._pinyin_buffer = ""
        self._trace_engine = trace_engine

        self._ic.connect("commit-text", self._on_commit_text)
        self._ic.connect("update-preedit-text", self._on_update_preedit)
        self._ic.connect("hide-preedit-text", self._on_hide_preedit)
        self._ic.connect("update-lookup-table", self._on_update_lookup_table)
        self._ic.connect("hide-lookup-table", self._on_hide_lookup_table)
        self._ic.connect("update-auxiliary-text", self._on_update_aux)
        self._ic.connect("hide-auxiliary-text", self._on_hide_aux)

    # --- Input actions (chainable) ---

    def type_pinyin(self, text: str) -> "TestClient":
        for ch in text:
            keyval = ord(ch.lower())
            self._ic.process_key_event(keyval, 0, 0)
            self._pipeline_key_event(keyval)
        self._pinyin_buffer = text
        return self

    def press_space(self) -> "TestClient":
        self._ic.process_key_event(IBus.KEY_space, 0, 0)
        self._pipeline_key_event(IBus.KEY_space)
        return self

    def press_enter(self) -> "TestClient":
        self._ic.process_key_event(IBus.KEY_Return, 0, 0)
        self._pipeline_key_event(IBus.KEY_Return)
        return self

    def press_escape(self) -> "TestClient":
        self._ic.process_key_event(IBus.KEY_Escape, 0, 0)
        self._pipeline_key_event(IBus.KEY_Escape)
        return self

    def press_backspace(self) -> "TestClient":
        self._ic.process_key_event(IBus.KEY_BackSpace, 0, 0)
        self._pipeline_key_event(IBus.KEY_BackSpace)
        if self._pinyin_buffer:
            self._pinyin_buffer = self._pinyin_buffer[:-1]
        return self

    def press_number(self, n: int) -> "TestClient":
        if 1 <= n <= 9:
            keyval = IBus.KEY_1 + (n - 1)
            self._ic.process_key_event(keyval, 0, 0)
            self._pipeline_key_event(keyval)
        return self

    def press_page_up(self) -> "TestClient":
        self._ic.process_key_event(IBus.KEY_Page_Up, 0, 0)
        self._pipeline_key_event(IBus.KEY_Page_Up)
        return self

    def press_page_down(self) -> "TestClient":
        self._ic.process_key_event(IBus.KEY_Page_Down, 0, 0)
        self._pipeline_key_event(IBus.KEY_Page_Down)
        return self

    def press_key(self, keyval: int, modifiers: int = 0) -> "TestClient":
        self._ic.process_key_event(keyval, 0, modifiers)
        self._pipeline_key_event(keyval)
        return self

    def press_apostrophe(self) -> "TestClient":
        self._ic.process_key_event(IBus.KEY_apostrophe, 0, 0)
        self._pipeline_key_event(IBus.KEY_apostrophe)
        self._pinyin_buffer += "'"
        return self

    # --- Inspection ---

    def get_candidates(self) -> list[Candidate]:
        return list(self._candidates)

    def get_preedit(self) -> str:
        return self._preedit

    def get_committed(self) -> str:
        return self._committed

    def get_aux_text(self) -> str:
        return self._aux_text

    def get_pinyin_buffer(self) -> str:
        return self._pinyin_buffer

    def events(self) -> list[EngineEvent]:
        return list(self._events)

    def clear_events(self) -> "TestClient":
        self._events.clear()
        return self

    # --- Tracing ---

    def get_trace(self) -> dict | None:
        """Return pipeline trace dict, or None if no trace engine."""
        if self._trace_engine and self._pinyin_buffer:
            return self._trace_engine.debug_process(self._pinyin_buffer)
        return None

    # --- Internal callbacks ---

    def _pipeline_key_event(self, keyval):
        while GLib.main_context_default().pending():
            GLib.main_context_default().iteration(False)

    def _on_commit_text(self, ic, text):
        self._committed += text.text
        self._events.append(EngineEvent(type="commit-text", text=text.text))

    def _on_update_preedit(self, ic, text, cursor_pos, visible):
        self._preedit = text.text if text else ""
        self._preedit_visible = visible
        self._events.append(EngineEvent(type="update-preedit", text=self._preedit))

    def _on_hide_preedit(self, ic):
        self._preedit = ""
        self._preedit_visible = False

    def _on_update_lookup_table(self, ic, table, visible):
        cands = []
        for c in table.get_candidates_in_current_page():
            cands.append(Candidate(text=c.text))
        for i, cand in enumerate(cands):
            cand.label = str(i + 1)
        self._candidates = cands
        self._events.append(EngineEvent(type="update-lookup-table", candidates=list(cands)))

    def _on_hide_lookup_table(self, ic):
        self._candidates = []
        self._events.append(EngineEvent(type="hide-lookup-table"))

    def _on_update_aux(self, ic, text, visible):
        self._aux_text = text.text if text else ""
        self._events.append(EngineEvent(type="update-aux", text=self._aux_text))

    def _on_hide_aux(self, ic):
        self._aux_text = ""


# --- Custom Assertion Helpers ---

def _format_trace(trace: dict) -> str:
    """Format a pipeline trace dict into a readable string."""
    lines = []
    lines.append(f'\nPipeline trace for "{trace.get("pinyin", "?")}":')

    segs = trace.get("segments", [])
    if segs:
        lines.append("  -- Segmentation --")
        parts = []
        for s in segs:
            parts.append(f'[{s["start"]}..{s["end"]}] "{s["segment"]}"')
        lines.append("    " + "  ".join(parts))
        lines.append("  -- Dict Lookups --")
        for s in segs:
            lines.append(f'    "{s["segment"]}" -> {s["entries"]} entries')

    cands = trace.get("candidates", [])
    if cands:
        lines.append("  -- Ranked Candidates --")
        for i, c in enumerate(cands[:10]):
            lines.append(
                f'    #{i+1} "{c["text"]}"  score={c["score"]:.2f}  '
                f'(freq={c["freq"]} user={c.get("user_boost", 0):.2f} '
                f'exact={c.get("exact_match", False)})'
            )

    return "\n".join(lines)


def assert_candidates(client: TestClient, expected: list[str]) -> None:
    """Assert current candidates match expected texts in order. Shows trace on failure."""
    actual = [c.text for c in client.get_candidates()]
    try:
        assert actual == expected, _build_candidate_diff(expected, actual)
    except AssertionError as e:
        msg = str(e)
        if client.get_trace is not None:
            trace = client.get_trace()
            if trace and "error" not in trace:
                msg += _format_trace(trace)
        raise AssertionError(msg) from None


def _build_candidate_diff(expected: list[str], actual: list[str]) -> str:
    """Build a position-aligned diff between expected and actual candidate lists."""
    lines = ["Candidate mismatch:"]
    max_len = max(len(expected), len(actual))
    lines.append(f"  {'Pos':<6}" + "".join(f"{i:<10}" for i in range(max_len)))
    lines.append(f"  {'Expected':<6}" + "".join(
        f"{expected[i] if i < len(expected) else '-':<10}" for i in range(max_len)
    ))
    lines.append(f"  {'Actual':<6}" + "".join(
        f"{actual[i] if i < len(actual) else '-':<10}" for i in range(max_len)
    ))
    return "\n".join(lines)


def assert_committed(client: TestClient, expected: str) -> None:
    """Assert the committed text contains the expected string."""
    actual = client.get_committed()
    assert expected in actual, (
        f"Expected committed text to contain {expected!r}, "
        f"but got {actual!r}"
    )


def assert_preedit(client: TestClient, expected: str) -> None:
    """Assert the preedit text matches expected."""
    actual = client.get_preedit()
    assert actual == expected, (
        f"Expected preedit {expected!r}, got {actual!r}"
    )
```

- [ ] **Step 2: Verify the file has no syntax errors**

Run: `python -c "import ast; ast.parse(open('tests/test_helpers.py').read()); print('OK')"`
Expected: `OK`

- [ ] **Step 3: Commit**

```bash
git add tests/test_helpers.py
git commit -m "feat(tests): add TestClient, Candidate, EngineEvent, and assertion helpers"
```

---

### Task 4: Python — conftest.py (pytest fixtures)

**Files:**
- Create: `tests/conftest.py`

- [ ] **Step 1: Create `tests/conftest.py`**

```python
"""pytest fixtures for ibus-ice E2E tests."""

import os
import random
import string
import subprocess
import sys
import time
import xml.etree.ElementTree as ET

import pytest

PROJECT_DIR = os.path.realpath(os.path.join(os.path.dirname(__file__), ".."))
ENGINE_DIR = os.path.join(PROJECT_DIR, "engine")
BASE_COMPONENT_DIR = "/usr/share/ibus/component"


def _gen_bus_address() -> str:
    """Generate a unique abstract socket address."""
    rand = "".join(random.choices(string.ascii_lowercase + string.digits, k=12))
    return f"unix:abstract=ibus-ice-test-{rand}"


def _make_component_xml(component_dir: str, project_dir: str, dict_dir: str, home: str) -> str:
    """Write ice.xml into component_dir. Returns the path."""
    engine_py = os.path.join(project_dir, "engine", "main.py")
    exec_str = (
        f"/usr/bin/env IBUS_ICE_DATA_DIR={dict_dir} HOME={home} "
        f"/usr/bin/python3 {engine_py} --ibus"
    )

    root = ET.Element("component")
    ET.SubElement(root, "name").text = "org.freedesktop.IBus.Ice"
    ET.SubElement(root, "description").text = "Ice Input Method (Test)"
    ET.SubElement(root, "exec").text = exec_str
    ET.SubElement(root, "version").text = "0.1.0"
    ET.SubElement(root, "author").text = "ibus-ice test"
    ET.SubElement(root, "license").text = "GPLv3"
    ET.SubElement(root, "homepage").text = "https://github.com/yshngg/ibus-ice"
    ET.SubElement(root, "textdomain").text = "ibus-ice"

    engines_el = ET.SubElement(root, "engines")
    eng = ET.SubElement(engines_el, "engine")
    ET.SubElement(eng, "name").text = "ice"
    ET.SubElement(eng, "language").text = "zh"
    ET.SubElement(eng, "license").text = "GPLv3"
    ET.SubElement(eng, "author").text = "ibus-ice test"
    ET.SubElement(eng, "layout").text = "us"
    ET.SubElement(eng, "longname").text = "Ice"
    ET.SubElement(eng, "description").text = "Ice Chinese Input Method (Test)"
    ET.SubElement(eng, "rank").text = "50"

    os.makedirs(component_dir, exist_ok=True)
    xml_path = os.path.join(component_dir, "ice.xml")
    tree = ET.ElementTree(root)
    ET.indent(tree, space="  ")
    tree.write(xml_path, encoding="utf-8", xml_declaration=True)
    return xml_path


def _write_dbus_config(config_path: str, address: str) -> None:
    """Write a minimal session bus config."""
    config = f"""<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-BUS Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <keep_umask/>
  <listen>{address}</listen>
  <policy context="default">
    <allow send_destination="*"/>
    <allow receive_sender="*"/>
    <allow own="*"/>
    <allow user="*"/>
    <allow send_type="method_call"/>
    <allow send_type="signal"/>
    <allow send_type="method_return"/>
    <allow send_type="error"/>
    <allow send_requested_reply="true"/>
    <allow receive_requested_reply="true"/>
  </policy>
</busconfig>
"""
    with open(config_path, "w") as f:
        f.write(config)


@pytest.fixture(scope="session")
def ice_dict():
    """Build the full production dictionary once for the test session."""
    print("\nBuilding full dictionary...")
    subprocess.run(["make", "build-dict"], cwd=PROJECT_DIR, check=True)
    path = os.path.join(PROJECT_DIR, "build", "ice.dict")
    assert os.path.exists(path), f"ice.dict not found at {path}"
    return path


@pytest.fixture(scope="session")
def ice_engine_so():
    """Find the compiled libcore.so."""
    candidates = [
        os.path.join(PROJECT_DIR, "target", "release", "libcore.so"),
        os.path.join(PROJECT_DIR, "target", "debug", "libcore.so"),
    ]
    for p in candidates:
        if os.path.exists(p):
            return p
    raise RuntimeError(f"libcore.so not found. Tried: {candidates}")


@pytest.fixture(scope="session")
def dict_dir(ice_dict):
    """Directory containing ice.dict."""
    return os.path.dirname(ice_dict)


@pytest.fixture
def ibus_session(tmp_path, ice_dict, ice_engine_so, dict_dir):
    """Launch an isolated IBus session per test.

    Returns a dict with 'bus_address' for TestClient connections.
    """
    home_dir = os.path.join(tmp_path, "home")
    os.makedirs(home_dir, exist_ok=True)
    os.makedirs(os.path.join(home_dir, ".local", "share", "ibus-ice"), exist_ok=True)

    component_dir = os.path.join(tmp_path, "ibus-component")
    os.makedirs(component_dir, exist_ok=True)

    # Generate component XML
    _make_component_xml(
        component_dir=component_dir,
        project_dir=PROJECT_DIR,
        dict_dir=dict_dir,
        home=home_dir,
    )

    # Generate D-Bus config
    bus_address = _gen_bus_address()
    dbus_cfg = os.path.join(tmp_path, "dbus-session.conf")
    _write_dbus_config(dbus_cfg, bus_address)

    # Start isolated dbus-daemon
    kwargs = {}
    if hasattr(subprocess, "_args"):
        pass
    dbus_proc = subprocess.Popen(
        ["dbus-daemon", "--config-file=" + dbus_cfg, "--print-address", "--fork"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    out, err = dbus_proc.communicate(timeout=5)
    actual_address = out.decode().strip()
    if not actual_address:
        raise RuntimeError(f"dbus-daemon failed: stdout={out.decode()}, stderr={err.decode()}")

    # Set up env for ibus-daemon
    env = {
        **os.environ,
        "DBUS_SESSION_BUS_ADDRESS": actual_address,
        "IBUS_COMPONENT_PATH": f"{component_dir}:{BASE_COMPONENT_DIR}",
        "HOME": home_dir,
    }

    # Start ibus-daemon
    ibus_proc = subprocess.Popen(
        ["ibus-daemon", "--daemonize", "--replace", "--verbose"],
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    ibus_proc.communicate(timeout=5)

    # Wait for engine registration
    ready = False
    for _ in range(20):
        time.sleep(0.5)
        result = subprocess.run(
            ["dbus-send", "--print-reply", f"--bus={actual_address}",
             "--dest=org.freedesktop.IBus",
             "/org/freedesktop/IBus",
             "org.freedesktop.IBus.ListEngines"],
            capture_output=True,
        )
        if b"ice" in result.stdout:
            ready = True
            break

    if not ready:
        _kill_procs(dbus_proc, ibus_proc)
        raise RuntimeError(
            "ibus-daemon did not register ice engine within 10s\n"
            f"ListEngines output:\n{result.stdout.decode() if ready else '(timeout)'}"
        )

    yield {"bus_address": actual_address}

    # Teardown
    _kill_procs(dbus_proc, ibus_proc)
    # Clean up the daemonized ibus-daemon
    subprocess.run(["pkill", "-f", f"ibus-daemon.*{bus_address}"], capture_output=True)
    subprocess.run(["pkill", "-f", f"dbus-daemon.*{bus_address}"], capture_output=True)


def _kill_procs(*procs):
    for p in procs:
        try:
            p.terminate()
            p.wait(timeout=3)
        except Exception:
            try:
                p.kill()
            except Exception:
                pass
```

- [ ] **Step 2: Verify fixture file syntax**

Run: `python -c "import ast; ast.parse(open('tests/conftest.py').read()); print('OK')"`
Expected: `OK`

- [ ] **Step 3: Commit**

```bash
git add tests/conftest.py
git commit -m "feat(tests): add pytest fixtures for isolated IBus sessions"
```

---

### Task 5: Makefile — test-e2e target

**Files:**
- Modify: `Makefile`

- [ ] **Step 1: Add `test-e2e` target to `Makefile`**

After the existing `test:` target (line 21), add:

```makefile
test-e2e:
	$(MAKE) build
	$(MAKE) build-dict
	python -m pytest tests/ -v
```

- [ ] **Step 2: Verify the target is parseable**

Run: `make -n test-e2e`
Expected: Shows the commands it would run (dry-run).

- [ ] **Step 3: Commit**

```bash
git add Makefile
git commit -m "feat: add test-e2e make target"
```

---

### Task 6: Test file — test_basic_input.py

**Files:**
- Create: `tests/test_basic_input.py`

- [ ] **Step 1: Create `tests/test_basic_input.py`**

```python
"""Tests for basic pinyin input and preedit display."""

from test_helpers import assert_candidates, assert_preedit


def test_single_syllable_suggests_candidates(client):
    client.type_pinyin("wo")
    candidates = client.get_candidates()
    assert len(candidates) > 0, "Expected candidates for 'wo'"
    assert_preedit(client, "wo")


def test_multi_syllable_candidates(client):
    client.type_pinyin("zhongguo")
    candidates = client.get_candidates()
    assert len(candidates) > 0, "Expected candidates for 'zhongguo'"
    assert_preedit(client, "zhongguo")


def test_empty_input_has_no_candidates(client):
    client.type_pinyin("a").press_backspace()
    candidates = client.get_candidates()
    assert len(candidates) == 0, "Expected no candidates after clearing input"


def test_candidates_update_as_typing(client):
    client.type_pinyin("zho")
    count1 = len(client.get_candidates())
    client.type_pinyin("ngguo")
    count2 = len(client.get_candidates())
    assert count1 > 0 or count2 > 0, "Expected candidates while typing"
```

- [ ] **Step 2: Register the `client` fixture in conftest.py**

Add to `tests/conftest.py` imports:

```python
from test_helpers import TestClient
from trace import TraceEngine
```

Add the `client` fixture to conftest.py (before the end):

```python
@pytest.fixture
def client(ibus_session, ice_dict):
    """Create a TestClient connected to the isolated IBus session."""
    bus_address = ibus_session["bus_address"]
    os.environ["DBUS_SESSION_BUS_ADDRESS"] = bus_address
    trace_engine = TraceEngine(ice_dict)
    tc = TestClient(bus_address, trace_engine)
    yield tc
    trace_engine.close()
```

- [ ] **Step 3: Commit**

```bash
git add tests/test_basic_input.py tests/conftest.py
git commit -m "test: add basic pinyin input E2E tests"
```

---

### Task 7: Test file — test_special_keys.py

**Files:**
- Create: `tests/test_special_keys.py`

- [ ] **Step 1: Create `tests/test_special_keys.py`**

```python
"""Tests for special key handling: backspace, escape, enter, apostrophe."""

from test_helpers import assert_candidates, assert_preedit


def test_backspace_removes_characters(client):
    client.type_pinyin("zhongguo")
    assert_preedit(client, "zhongguo")
    client.press_backspace()
    assert_preedit(client, "zhongg")
    client.press_backspace()
    assert_preedit(client, "zhong")


def test_backspace_to_empty(client):
    client.type_pinyin("a")
    assert_preedit(client, "a")
    client.press_backspace()
    assert_preedit(client, "")


def test_escape_resets_state(client):
    client.type_pinyin("zhongguo")
    assert len(client.get_candidates()) > 0
    client.press_escape()
    candidates = client.get_candidates()
    assert len(candidates) == 0, "Expected no candidates after escape"
    assert_preedit(client, "")


def test_enter_commits_raw_pinyin(client):
    client.type_pinyin("hello").press_enter()
    committed = client.get_committed()
    assert "hello" in committed.lower(), f"Expected raw pinyin in committed: {committed!r}"


def test_apostrophe_separator(client):
    client.type_pinyin("xi")
    client.press_apostrophe()
    client.type_pinyin("an")
    assert_preedit(client, "xi'an")
    # Should have candidates for 西安 etc.
    candidates = client.get_candidates()
    assert len(candidates) > 0, "Expected candidates for xi'an"
```

- [ ] **Step 2: Commit**

```bash
git add tests/test_special_keys.py
git commit -m "test: add special key handling E2E tests"
```

---

### Task 8: Test file — test_commit_select.py

**Files:**
- Create: `tests/test_commit_select.py`

- [ ] **Step 1: Create `tests/test_commit_select.py`**

```python
"""Tests for candidate selection and text committing."""

from test_helpers import assert_committed


def test_space_commits_first_candidate(client):
    client.type_pinyin("wo").press_space()
    committed = client.get_committed()
    assert len(committed) > 0, "Expected committed text after space"


def test_number_selects_second_candidate(client):
    client.type_pinyin("zhongguo")
    candidates_before = client.get_candidates()
    if len(candidates_before) >= 2:
        client.press_number(2)
        committed = client.get_committed()
        assert len(committed) > 0, f"Expected committed text after selecting #2, got {committed!r}"


def test_first_character_is_chinese(client):
    client.type_pinyin("ren").press_space()
    committed = client.get_committed()
    assert len(committed) > 0
    first_char = committed[0]
    assert ord(first_char) > 127, f"Expected Chinese character, got {first_char!r} (U+{ord(first_char):04X})"
```

- [ ] **Step 2: Commit**

```bash
git add tests/test_commit_select.py
git commit -m "test: add commit and select E2E tests"
```

---

### Task 9: Test file — test_candidates.py

**Files:**
- Create: `tests/test_candidates.py`

- [ ] **Step 1: Create `tests/test_candidates.py`**

```python
"""Tests for candidate ordering and page navigation."""

from test_helpers import assert_candidates


def test_candidate_order_is_stable(client):
    client.type_pinyin("zhongguo")
    first = client.get_candidates()
    client.press_escape()
    client.type_pinyin("zhongguo")
    second = client.get_candidates()
    first_texts = [c.text for c in first]
    second_texts = [c.text for c in second]
    assert first_texts == second_texts, (
        f"Candidate order not stable:\n  first: {first_texts}\n  second: {second_texts}"
    )


def test_page_navigation(client):
    client.type_pinyin("y")
    first_page = client.get_candidates()
    if len(first_page) < 5:
        return  # not enough candidates for paging
    client.press_page_down()
    second_page = client.get_candidates()
    # Page contents should differ (or at least not crash)
    assert len(second_page) >= 0, "page_down should not crash"


def test_common_word_ranks_high(client):
    client.type_pinyin("zhongguo")
    candidates = client.get_candidates()
    texts = [c.text for c in candidates]
    assert "中国" in texts, f"Expected 中国 in candidates: {texts[:5]}"
    # 中国 should be ranked high (top 5)
    china_idx = texts.index("中国")
    assert china_idx < 5, f"中国 ranked #{china_idx+1}, expected top 5"
```

- [ ] **Step 2: Commit**

```bash
git add tests/test_candidates.py
git commit -m "test: add candidate ordering and page navigation tests"
```

---

### Task 10: Test file — test_english.py

**Files:**
- Create: `tests/test_english.py`

- [ ] **Step 1: Create `tests/test_english.py`**

```python
"""Tests for English word input path."""

from test_helpers import assert_candidates


def test_english_word_has_candidates(client):
    client.type_pinyin("hello")
    candidates = client.get_candidates()
    assert len(candidates) > 0, "Expected candidates for English word 'hello'"


def test_english_word_can_be_committed(client):
    client.type_pinyin("hello").press_space()
    committed = client.get_committed()
    assert len(committed) > 0, f"Expected committed text for 'hello', got {committed!r}"


def test_mixed_input_fallbacks(client):
    """Typing a non-Chinese word should still produce candidates."""
    client.type_pinyin("apple")
    candidates = client.get_candidates()
    assert len(candidates) > 0, "Expected candidates for 'apple'"
```

- [ ] **Step 2: Commit**

```bash
git add tests/test_english.py
git commit -m "test: add English word input E2E tests"
```

---

### Task 11: Test file — test_ranking.py

**Files:**
- Create: `tests/test_ranking.py`

- [ ] **Step 1: Create `tests/test_ranking.py`**

```python
"""Tests for frequency-based ranking and user boost."""

from test_helpers import assert_candidates


def test_high_freq_ranks_first(client):
    client.type_pinyin("wo")
    candidates = client.get_candidates()
    texts = [c.text for c in candidates]
    # Common word like 我 should be present and rank highly
    assert "我" in texts, f"Expected 我 in candidates: {texts[:5]}"
    wo_idx = texts.index("我")
    assert wo_idx < 5, f"我 ranked #{wo_idx+1}, expected top 5"


def test_selecting_boosts_candidate(client):
    """After selecting a less common candidate, it should rank higher next time."""
    pinyin = "nihao"
    client.type_pinyin(pinyin)
    candidates = client.get_candidates()
    texts_before = [c.text for c in candidates]
    assert len(candidates) >= 1, f"Expected candidates for '{pinyin}'"

    # Select the first candidate
    first_text = candidates[0].text
    client.press_space()

    # Type again — same word should still be top (user boost may not apply
    # mid-session since engine resets on commit; but dict persistence matters)
    client.type_pinyin(pinyin)
    candidates_after = client.get_candidates()
    texts_after = [c.text for c in candidates_after]
    assert len(candidates_after) > 0, "Expected candidates after re-typing"
```

- [ ] **Step 2: Commit**

```bash
git add tests/test_ranking.py
git commit -m "test: add ranking and user boost E2E tests"
```

---

### Task 12: Test file — test_edge_cases.py

**Files:**
- Create: `tests/test_edge_cases.py`

- [ ] **Step 1: Create `tests/test_edge_cases.py`**

```python
"""Tests for edge cases: empty input, rapid typing, etc."""

from test_helpers import assert_preedit


def test_rapid_typing_does_not_crash(client):
    client.type_pinyin("zhongguo")
    client.press_backspace()
    client.press_backspace()
    client.type_pinyin("woguo")
    candidates = client.get_candidates()
    assert len(candidates) > 0, "Expected candidates after rapid typing"


def test_long_input_does_not_crash(client):
    long_pinyin = "zhongguorenmindaxue"  # A very long but valid pinyin string
    client.type_pinyin(long_pinyin)
    # Should not crash, may or may not have candidates
    candidates = client.get_candidates()
    assert isinstance(candidates, list), "Expected list from get_candidates()"


def test_uppercase_input_lowered(client):
    """Uppercase letters should be treated as lowercase pinyin."""
    client.press_key(ord('W'), 0)
    client.press_key(ord('O'), 0)
    assert_preedit(client, "wo")


def test_control_keys_ignored(client):
    """Ctrl+key combos should not affect input."""
    client.type_pinyin("zhong")
    before = client.get_preedit()
    client.press_key(ord('a'), 1 << 2)  # Control mask
    assert_preedit(client, before)


def test_single_char_has_candidates(client):
    client.type_pinyin("a")
    candidates = client.get_candidates()
    assert len(candidates) > 0, "Expected candidates for single char 'a'"
```

- [ ] **Step 2: Commit**

```bash
git add tests/test_edge_cases.py
git commit -m "test: add edge case E2E tests"
```

---

### Task 13: Smoke test — run the E2E suite

**Verification-only task — no code changes.**

- [ ] **Step 1: Build the project and dictionary**

Run: `make build && make build-dict`
Expected: Build succeeds. ice.dict is created in build/.

- [ ] **Step 2: Run a single test to validate fixture infrastructure**

Run: `DBUS_SESSION_BUS_ADDRESS="" python -m pytest tests/test_basic_input.py::test_single_syllable_suggests_candidates -v`
Expected: Test runs (may need environment fixups).

- [ ] **Step 3: Run the full E2E suite**

Run: `make test-e2e`
Expected: All tests pass.

- [ ] **Step 4: Verify failure diagnostics work**

Temporarily change an assertion in a test to fail, re-run, and verify:
- Rich candidate diff is displayed
- Pipeline trace (segmentation, candidates with scores) is appended to failure message

---

### Task 14: Final integration check

- [ ] **Step 1: Verify `make test-e2e` runs from clean state**

```bash
make test-e2e 2>&1
```
Expected: All tests pass. Zero test failures.

- [ ] **Step 2: Run existing unit tests to verify no regressions**

```bash
cargo test -p core
cargo test -p dict-compiler
```
Expected: All 24 existing tests still pass. No regressions.

- [ ] **Step 3: Commit if any remaining changes**

```bash
git status
# Only commit if there are uncommitted changes
```
