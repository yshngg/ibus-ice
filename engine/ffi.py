"""ctypes bindings for libibus_ice_core.so."""

import ctypes
import os
from ctypes import POINTER, Structure, c_char_p, c_int32, c_void_p


class IceCandidate(Structure):
    _fields_ = [
        ("text", c_char_p),
        ("freq", c_int32),
        ("word_len", c_int32),
    ]


class IceCandidateList(Structure):
    _fields_ = [
        ("candidates", POINTER(IceCandidate)),
        ("count", c_int32),
    ]


def _find_lib() -> str:
    """Find the libibus_ice_core.so shared library."""
    paths = [
        os.path.join(
            os.path.dirname(__file__), "..", "..", "..", "target", "debug", "libcore.so"
        ),
        os.path.join(
            os.path.dirname(__file__),
            "..",
            "..",
            "..",
            "target",
            "release",
            "libcore.so",
        ),
        "/usr/lib/ibus-ice/libibus_ice_core.so",
        "/usr/local/lib/ibus-ice/libibus_ice_core.so",
    ]
    for p in paths:
        if os.path.exists(p):
            return p
    raise RuntimeError("Cannot find libibus_ice_core.so")


_lib = ctypes.CDLL(_find_lib())

_lib.ice_engine_new.argtypes = [c_char_p, c_char_p]
_lib.ice_engine_new.restype = c_void_p

_lib.ice_engine_free.argtypes = [c_void_p]
_lib.ice_engine_free.restype = None

_lib.ice_process.argtypes = [c_void_p, c_char_p]
_lib.ice_process.restype = POINTER(IceCandidateList)

_lib.ice_select.argtypes = [c_void_p, c_char_p]
_lib.ice_select.restype = None

_lib.ice_candidates_free.argtypes = [POINTER(IceCandidateList)]
_lib.ice_candidates_free.restype = None

_lib.ice_reset.argtypes = [c_void_p]
_lib.ice_reset.restype = None


class Engine:
    """Python wrapper around the Rust IceEngine."""

    def __init__(self, dict_path: str, user_dict_path: str):
        self._handle = _lib.ice_engine_new(
            dict_path.encode("utf-8"),
            user_dict_path.encode("utf-8"),
        )
        if not self._handle:
            raise RuntimeError(f"Failed to create engine (dict_path={dict_path})")

    def process(self, pinyin: str) -> list[dict]:
        """Process pinyin input, return list of candidate dicts."""
        result = _lib.ice_process(self._handle, pinyin.encode("utf-8"))
        if not result:
            return []

        candidates = []
        clist = result.contents
        for i in range(clist.count):
            c = clist.candidates[i]
            candidates.append(
                {
                    "text": c.text.decode("utf-8") if c.text else "",
                    "freq": c.freq,
                    "word_len": c.word_len,
                }
            )

        _lib.ice_candidates_free(result)
        return candidates

    def select(self, text: str) -> None:
        """Record user selection."""
        _lib.ice_select(self._handle, text.encode("utf-8"))

    def reset(self) -> None:
        """Reset engine state."""
        _lib.ice_reset(self._handle)

    def close(self) -> None:
        """Free the engine."""
        if self._handle:
            _lib.ice_engine_free(self._handle)
            self._handle = None

    def __del__(self):
        self.close()
