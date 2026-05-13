"""pytest fixtures for ibus-ice E2E tests."""

import os
import subprocess
import sys
import time

import pytest

sys.path.insert(0, os.path.dirname(__file__))

from test_helpers import TestClient
from trace import TraceEngine

PROJECT_DIR = os.path.realpath(os.path.join(os.path.dirname(__file__), ".."))


@pytest.fixture
def client(ibus_session, ice_dict):
    """Create a TestClient connected to the isolated IBus session."""
    bus_address = ibus_session["bus_address"]
    os.environ["DBUS_SESSION_BUS_ADDRESS"] = bus_address
    trace_engine = TraceEngine(ice_dict)
    tc = TestClient(bus_address, trace_engine)
    yield tc
    trace_engine.close()


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
def ibus_session(tmp_path, ice_dict, ice_engine_so, dict_dir):
    """Register ice engine with the running ibus-daemon and set up a factory.

    Creates an IBus.Factory that handles key events for the ice engine,
    connecting to the running ibus-daemon via D-Bus.
    """
    import gi
    gi.require_version("IBus", "1.0")
    from gi.repository import IBus, GLib, GObject

    home_dir = os.path.join(tmp_path, "home")
    os.makedirs(os.path.join(home_dir, ".local", "share", "ibus-ice"), exist_ok=True)

    # Set env vars for engine.py to find the dict and user dict
    os.environ["IBUS_ICE_DATA_DIR"] = dict_dir
    os.environ["HOME"] = home_dir

    bus = IBus.Bus()
    if bus is None or not bus.is_connected():
        raise RuntimeError("Cannot connect to IBus daemon. Is ibus-daemon running?")

    # Import engine module to register the GObject type
    sys.path.insert(0, os.path.join(PROJECT_DIR, "engine"))
    import engine  # noqa: F811 - registers IceIBusEngine type
    sys.path.pop(0)

    # Build an EngineDesc for our engine
    engine_desc = IBus.EngineDesc.new(
        "ice",
        "Ice",
        "Ice Chinese Input Method (Test)",
        "zh",
        "GPLv3",
        "ibus-ice test",
        "",
        "us",
    )

    # Build and register component
    component = IBus.Component.new(
        "org.freedesktop.IBus.Ice",
        "Ice Input Method (Test)",
        "0.1.0",
        "GPLv3",
        "ibus-ice test",
        "https://github.com/yshngg/ibus-ice",
        "",
        "ibus-ice",
    )
    component.add_engine(engine_desc)
    bus.register_component(component)

    # Create engine factory to handle key events
    factory = IBus.Factory.new(bus.get_connection())
    factory.add_engine("ice", GObject.type_from_name("IceIBusEngine"))

    # Set the engine as current
    bus.set_global_engine_async("ice", -1, None, None, None)

    # Process pending events
    context = GLib.main_context_default()
    for _ in range(10):
        while context.pending():
            context.iteration(False)
        time.sleep(0.1)

    yield {"bus_address": os.environ.get("DBUS_SESSION_BUS_ADDRESS", "")}

    # Teardown
    factory.destroy()
    bus.set_global_engine_async("xkb:us::eng", -1, None, None, None)
