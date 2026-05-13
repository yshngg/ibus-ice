"""pytest fixtures for ibus-ice E2E tests (direct engine mode)."""

import os
import subprocess
import sys
import time

import pytest

sys.path.insert(0, os.path.dirname(__file__))

from test_helpers import TestClient
from trace import TraceEngine

PROJECT_DIR = os.path.realpath(os.path.join(os.path.dirname(__file__), ".."))


TEST_DICT_YAML = """\
---
...
中国	zhong guo	10000
美国	mei guo	8000
中国话	zhong guo hua	5000
中国人	zhong guo ren	6000
中国画	zhong guo hua	4000
我	wo	10000
你	ni	9000
好	hao	5000
你好	ni hao	7000
西安	xi an	6000
人	ren	8000
和平	he ping	1000
苹果	ping guo	3000
"""


@pytest.fixture(scope="session")
def ice_dict(tmp_path_factory):
    """Build a test dictionary using dict-compiler in a session temp dir."""
    dict_dir = tmp_path_factory.mktemp("dict")
    path = os.path.join(dict_dir, "ice.dict")
    yaml_path = os.path.join(dict_dir, "test_dict.yaml")
    with open(yaml_path, "w") as f:
        f.write(TEST_DICT_YAML)
    dict_compiler = os.path.join(PROJECT_DIR, "target", "release", "dict-compiler")
    subprocess.run([dict_compiler, path, yaml_path], cwd=PROJECT_DIR, check=True)
    assert os.path.exists(path), f"Test dict not found at {path}"
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
def client(ice_dict, tmp_path):
    """Create a TestClient that directly wraps the Rust engine (no IBus needed).

    Uses the Engine class from ffi.py to directly call engine methods.
    """
    user_dict_dir = os.path.join(tmp_path, "user-dict")
    os.makedirs(user_dict_dir, exist_ok=True)
    user_dict_path = os.path.join(user_dict_dir, "user.dict")

    trace_engine = TraceEngine(ice_dict)
    tc = TestClient(ice_dict, user_dict_path, trace_engine)
    yield tc
    tc.close()
    trace_engine.close()
