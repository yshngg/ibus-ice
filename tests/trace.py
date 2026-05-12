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
