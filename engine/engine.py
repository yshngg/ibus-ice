import os
import sys

import gi

gi.require_version("IBus", "1.0")  # noqa: E402
from gi.repository import IBus

from ffi import Engine


class IceIBusEngine(IBus.Engine):
    __gtype_name__ = "IceIBusEngine"

    DICT_PATH = (
        os.environ.get("IBUS_ICE_DATA_DIR", "/usr/local/share/ibus-ice") + "/ice.dict"
    )
    USER_DICT_PATH = os.path.expanduser("~/.local/share/ibus-ice/user.dict")

    def __init__(self, bus, object_path, engine_name):
        # Initialize all attributes to safe defaults before super().__init__
        # in case super().__init__ fails (e.g., when instantiated via g_object_new).
        self._engine = None
        self._pinyin_buffer = ""
        self._candidates = []
        self._lookup_table = IBus.LookupTable.new(5, 0, True, True)

        try:
            super().__init__(
                engine_name=engine_name,
                object_path=object_path,
                connection=bus.get_connection(),
            )
        except Exception as e:
            sys.stderr.write(f"ibus-ice: super().__init__ failed: {e}\n")
            sys.stderr.flush()
            return

        os.makedirs(os.path.dirname(self.USER_DICT_PATH), exist_ok=True)

        try:
            self._engine = Engine(self.DICT_PATH, self.USER_DICT_PATH)
        except RuntimeError as e:
            print(f"ibus-ice: Failed to initialize engine: {e}", file=sys.stderr)
            self._engine = None

    def do_process_key_event(self, keyval, keycode, state):
        if self._engine is None:
            return False

        is_press = (state & IBus.ModifierType.RELEASE_MASK) == 0
        if not is_press:
            return False

        if state & (IBus.ModifierType.CONTROL_MASK | IBus.ModifierType.MOD1_MASK):
            return False

        if self._candidates:
            if IBus.KEY_1 <= keyval <= IBus.KEY_9:
                idx = keyval - IBus.KEY_1
                if idx < len(self._candidates):
                    self._commit(idx)
                    return True

            if keyval == IBus.KEY_space or keyval == IBus.KP_Space:
                self._commit(0)
                return True

            if keyval in (IBus.KEY_Page_Up, IBus.KEY_KP_Page_Up):
                if self._lookup_table.page_up():
                    self.page_up_lookup_table()
                return True

            if keyval in (IBus.KEY_Page_Down, IBus.KEY_KP_Page_Down):
                if self._lookup_table.page_down():
                    self.page_down_lookup_table()
                return True

            if keyval == IBus.KEY_Up or keyval == IBus.KEY_KP_Up:
                if self._lookup_table.cursor_up():
                    self.cursor_up_lookup_table()
                return True

            if keyval == IBus.KEY_Down or keyval == IBus.KEY_KP_Down:
                if self._lookup_table.cursor_down():
                    self.cursor_down_lookup_table()
                return True

        if keyval == IBus.KEY_BackSpace:
            if self._pinyin_buffer:
                self._pinyin_buffer = self._pinyin_buffer[:-1]
                self._update_candidates()
                return True
            return False

        if keyval == IBus.KEY_Escape:
            self._reset()
            return True

        if keyval == IBus.KEY_Return or keyval == IBus.KEY_KP_Enter:
            self._reset()
            return False

        if self._is_pinyin_char(keyval):
            char = chr(keyval)
            self._pinyin_buffer += char.lower()
            self._update_candidates()
            return True

        if keyval < 128 and self._pinyin_buffer:
            self._commit_string(self._pinyin_buffer + chr(keyval))
            return False

        return False

    def _is_pinyin_char(self, keyval):
        return (IBus.KEY_a <= keyval <= IBus.KEY_z) or keyval == IBus.KEY_apostrophe

    def _update_candidates(self):
        if not self._pinyin_buffer:
            self._hide_candidates()
            return

        self._candidates = self._engine.process(self._pinyin_buffer)

        self._lookup_table.clear()

        if not self._candidates:
            self._hide_candidates()
            return

        for c in self._candidates:
            text = IBus.Text.new_from_string(c["text"])
            self._lookup_table.append_candidate(text)

        self.update_preedit_text(
            IBus.Text.new_from_string(self._pinyin_buffer),
            0,
            True,
        )
        visible = self._lookup_table.get_number_of_candidates() > 0
        self.update_lookup_table(self._lookup_table, visible)

    def _hide_candidates(self):
        self.hide_lookup_table()
        if self._pinyin_buffer:
            self.update_preedit_text(
                IBus.Text.new_from_string(self._pinyin_buffer),
                0,
                True,
            )

    def _commit(self, idx):
        if idx < len(self._candidates):
            text = self._candidates[idx]["text"]
            self._engine.select(text)
            self.commit_text(IBus.Text.new_from_string(text))
        self._reset()

    def _commit_string(self, text):
        self.commit_text(IBus.Text.new_from_string(text))
        self._reset()

    def _reset(self):
        self._pinyin_buffer = ""
        self._candidates = []
        self._lookup_table.clear()
        if self._engine:
            self._engine.reset()
        self.hide_lookup_table()
        self.hide_preedit_text()

    def do_focus_out(self):
        self._reset()

    def do_destroy(self):
        if self._engine:
            self._engine.close()
