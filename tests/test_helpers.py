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
