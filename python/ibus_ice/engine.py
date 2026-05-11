"""IBus engine adapter for ibus-ice."""
import os
from gi.repository import IBus

from .ffi import Engine


DATA_DIR = "/usr/share/ibus-ice"


class IceIBusEngine(IBus.Engine):
    """IBus engine that delegates to the Rust Ice core."""

    def __init__(self, bus, object_path, engine_name):
        super().__init__(
            engine_name=engine_name,
            object_path=object_path,
            connection=bus.get_connection(),
        )

        dict_path = os.path.join(DATA_DIR, "ice.dict")
        user_dict_path = os.path.expanduser("~/.local/share/ibus-ice/user.dict")

        os.makedirs(os.path.dirname(user_dict_path), exist_ok=True)

        try:
            self._engine = Engine(dict_path, user_dict_path)
        except RuntimeError as e:
            print(f"ibus-ice: Failed to initialize engine: {e}")
            self._engine = None

        self._pinyin_buffer = ""
        self._candidates = []

    def do_process_key_event(self, keyval, keycode, state):
        if self._engine is None:
            return False

        if state & (IBus.ModifierType.CONTROL_MASK | IBus.ModifierType.MOD1_MASK):
            return False

        # Handle candidate selection
        if self._candidates and state == 0:
            if IBus.KEY_1 <= keyval <= IBus.KEY_9:
                idx = keyval - IBus.KEY_1
                if idx < len(self._candidates):
                    self._commit(idx)
                    return True

            if keyval == IBus.KEY_space:
                self._commit(0)
                return True

        # Handle backspace
        if keyval == IBus.KEY_BackSpace:
            if self._pinyin_buffer:
                self._pinyin_buffer = self._pinyin_buffer[:-1]
                self._update_candidates()
                return True
            return False

        # Handle escape — reset
        if keyval == IBus.KEY_Escape:
            self._reset()
            return True

        # Handle enter — let IBus handle
        if keyval == IBus.KEY_Return:
            self._reset()
            return False

        # Accumulate pinyin input
        if self._is_pinyin_char(keyval):
            char = chr(keyval)
            self._pinyin_buffer += char.lower()
            self._update_candidates()
            return True

        return False

    def _is_pinyin_char(self, keyval: int) -> bool:
        return (
            (IBus.KEY_a <= keyval <= IBus.KEY_z)
            or keyval == IBus.KEY_apostrophe
        )

    def _update_candidates(self):
        if not self._pinyin_buffer:
            self._hide_candidates()
            return

        self._candidates = self._engine.process(self._pinyin_buffer)

        if not self._candidates:
            self._hide_candidates()
            return

        table = IBus.LookupTable.new(5, 0, True, True)
        for c in self._candidates:
            text = IBus.Text.new_from_string(c["text"])
            table.append_candidate(text)

        self.update_lookup_table(table, True)
        self.update_preedit_text(
            IBus.Text.new_from_string(self._pinyin_buffer),
            0,
            True,
        )

    def _hide_candidates(self):
        self.hide_lookup_table()
        if self._pinyin_buffer:
            self.update_preedit_text(
                IBus.Text.new_from_string(self._pinyin_buffer),
                0,
                True,
            )

    def _commit(self, idx: int):
        if idx < len(self._candidates):
            text = self._candidates[idx]["text"]
            self._engine.select(text)
            self.commit_text(IBus.Text.new_from_string(text))
        self._reset()

    def _reset(self):
        self._pinyin_buffer = ""
        self._candidates = []
        if self._engine:
            self._engine.reset()
        self.hide_lookup_table()
        self.hide_preedit_text()

    def do_focus_out(self):
        self._reset()
        super().do_focus_out()

    def do_destroy(self):
        if self._engine:
            self._engine.close()
        super().do_destroy()
