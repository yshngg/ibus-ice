"""TestClient, dataclasses, and custom assertion helpers."""

from dataclasses import dataclass

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
    """Chainable E2E test client wrapping the Rust engine directly.

    Simulates user keystrokes by managing a pinyin buffer and calling
    the engine's process() and select() methods via FFI.
    """

    def __init__(self, dict_path: str, user_dict_path: str, trace_engine: TraceEngine | None = None):
        # Import late to keep ffi.py import independent
        import os
        import sys
        eng_dir = os.path.join(os.path.dirname(__file__), "..", "engine")
        sys.path.insert(0, eng_dir)
        from ffi import Engine
        sys.path.pop(0)

        self._engine = Engine(dict_path, user_dict_path)
        self._committed = ""
        self._preedit = ""
        self._candidates: list[Candidate] = []
        self._events: list[EngineEvent] = []
        self._pinyin_buffer = ""
        self._trace_engine = trace_engine

    # --- Input actions (chainable) ---

    def type_pinyin(self, text: str) -> "TestClient":
        for ch in text:
            self._pinyin_buffer += ch.lower()
        self._update_candidates()
        return self

    def press_space(self) -> "TestClient":
        if self._candidates:
            self._commit(0)
        return self

    def press_enter(self) -> "TestClient":
        if self._pinyin_buffer:
            self._commit_string(self._pinyin_buffer)
        return self

    def press_escape(self) -> "TestClient":
        self._reset()
        return self

    def press_backspace(self) -> "TestClient":
        if self._pinyin_buffer:
            self._pinyin_buffer = self._pinyin_buffer[:-1]
            self._update_candidates()
        return self

    def press_number(self, n: int) -> "TestClient":
        if 1 <= n <= 9 and n - 1 < len(self._candidates):
            self._commit(n - 1)
        return self

    def press_page_up(self) -> "TestClient":
        return self

    def press_page_down(self) -> "TestClient":
        return self

    def press_key(self, keyval: int, modifiers: int = 0) -> "TestClient":
        return self

    def press_apostrophe(self) -> "TestClient":
        self._pinyin_buffer += "'"
        self._update_candidates()
        return self

    # --- Inspection ---

    def get_candidates(self) -> list[Candidate]:
        return list(self._candidates)

    def get_preedit(self) -> str:
        return self._preedit

    def get_committed(self) -> str:
        return self._committed

    def get_aux_text(self) -> str:
        return ""

    def get_pinyin_buffer(self) -> str:
        return self._pinyin_buffer

    def events(self) -> list[EngineEvent]:
        return list(self._events)

    def clear_events(self) -> "TestClient":
        self._events.clear()
        return self

    # --- Tracing ---

    def get_trace(self) -> dict | None:
        if self._trace_engine and self._pinyin_buffer:
            return self._trace_engine.debug_process(self._pinyin_buffer)
        return None

    def close(self):
        if self._engine:
            self._engine.close()
            self._engine = None

    # --- Internal ---

    def _update_candidates(self):
        if not self._pinyin_buffer:
            self._candidates = []
            self._preedit = ""
            self._events.append(EngineEvent(type="hide-lookup-table"))
            return

        self._preedit = self._pinyin_buffer
        raw = self._engine.process(self._pinyin_buffer)
        self._candidates = []
        for i, c in enumerate(raw):
            self._candidates.append(Candidate(text=c["text"], label=str(i + 1)))
        self._events.append(EngineEvent(type="update-lookup-table", candidates=list(self._candidates)))

    def _commit(self, idx):
        if idx < len(self._candidates):
            text = self._candidates[idx].text
            self._engine.select(text)
            self._committed += text
            self._events.append(EngineEvent(type="commit-text", text=text))
        self._reset()

    def _commit_string(self, text):
        self._committed += text
        self._events.append(EngineEvent(type="commit-text", text=text))
        self._reset()

    def _reset(self):
        self._pinyin_buffer = ""
        self._candidates = []
        self._preedit = ""
        self._engine.reset()


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
