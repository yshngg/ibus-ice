"""pytest fixtures for ibus-ice E2E tests."""

import os
import subprocess
import sys
import time
import xml.etree.ElementTree as ET

import pytest

sys.path.insert(0, os.path.dirname(__file__))

from test_helpers import TestClient, IBusTestClient
from trace import TraceEngine

PROJECT_DIR = os.path.realpath(os.path.join(os.path.dirname(__file__), ".."))
ENGINE_DIR = os.path.join(PROJECT_DIR, "engine")


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


# =============================================================================
# Session fixtures
# =============================================================================


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


# =============================================================================
# Component XML generation (for ibus-daemon discovery)
# =============================================================================


def _make_component_xml(component_dir, dict_dir, home, engine_wrapper):
    """Write ice.xml into component_dir for ibus-daemon discovery.

    engine_wrapper is a shell script path that sets env vars and launches the engine.
    """
    root = ET.Element("component")
    ET.SubElement(root, "name").text = "org.freedesktop.IBus.Ice"
    ET.SubElement(root, "description").text = "Ice Input Method (Test)"
    ET.SubElement(root, "exec").text = engine_wrapper
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


# =============================================================================
# Black-box IBus daemon fixture (dbus-run-session)
# =============================================================================


@pytest.fixture
def ibus_daemon(tmp_path, ice_dict, dict_dir):
    """Launch an isolated ibus-daemon inside dbus-run-session.

    Returns dict with 'bus_address' and 'proc' for teardown.
    """
    home_dir = os.path.join(tmp_path, "home")
    os.makedirs(os.path.join(home_dir, ".local", "share", "ibus-ice"), exist_ok=True)

    component_dir = os.path.join(tmp_path, "ibus-component")
    engine_wrapper = os.path.join(tmp_path, "ibus-engine-ice")
    engine_py = os.path.join(ENGINE_DIR, "main.py")
    wrapper_content = f"""#!/bin/sh
IBUS_ICE_DATA_DIR={dict_dir}
HOME={home_dir}
export IBUS_ICE_DATA_DIR HOME
exec /usr/bin/python3 {engine_py} --ibus
"""
    with open(engine_wrapper, "w") as f:
        f.write(wrapper_content)
    os.chmod(engine_wrapper, 0o755)

    _make_component_xml(component_dir, dict_dir, home_dir, engine_wrapper)

    # File for dbus-run-session child to write bus address into
    addr_file = os.path.join(tmp_path, "bus-address")

    # dbus-run-session starts a session bus, runs bash inside it.
    # bash writes DBUS_SESSION_BUS_ADDRESS to a file, then runs ibus-daemon.
    # ibus-daemon runs in foreground (no --daemonize) so dbus-run-session
    # stays alive until ibus-daemon exits.
    cmd = (
        f'echo "$DBUS_SESSION_BUS_ADDRESS" > "{addr_file}"; '
        f'export IBUS_COMPONENT_PATH="{component_dir}"; '
        f'export HOME="{home_dir}"; '
        f'ibus-daemon --replace --verbose; '
    )

    proc = subprocess.Popen(
        ["dbus-run-session", "--", "bash", "-c", cmd],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )

    # Wait for address file to appear and contain data
    actual_address = ""
    for _ in range(30):
        time.sleep(0.2)
        if os.path.exists(addr_file):
            with open(addr_file) as f:
                content = f.read().strip()
            if content:
                actual_address = content
                break

    if not actual_address:
        proc.terminate()
        proc.wait()
        raise RuntimeError("dbus-run-session did not write bus address")

    # Poll until ibus-daemon registers our engine
    ready = False
    for _ in range(20):
        time.sleep(0.5)
        result = subprocess.run(
            [
                "dbus-send",
                "--print-reply",
                f"--bus={actual_address}",
                "--dest=org.freedesktop.IBus",
                "/org/freedesktop/IBus",
                "org.freedesktop.IBus.ListEngines",
            ],
            capture_output=True,
        )
        if b"ice" in result.stdout:
            ready = True
            break

    if not ready:
        proc.terminate()
        proc.wait()
        raise RuntimeError("ibus-daemon did not register ice engine within 10s")

    yield {"bus_address": actual_address, "proc": proc}

    # Teardown: kill dbus-run-session (cleans bus + ibus-daemon)
    proc.terminate()
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait()


# =============================================================================
# Client fixture (picks mode based on env var)
# =============================================================================


def _is_blackbox():
    return os.environ.get("IBUS_E2E_MODE", "").lower() == "blackbox"


@pytest.fixture
def client(request, ice_dict, tmp_path):
    """Create a TestClient or IBusTestClient depending on IBUS_E2E_MODE."""
    trace_engine = TraceEngine(ice_dict)

    if _is_blackbox():
        ibus_daemon = request.getfixturevalue("ibus_daemon")
        bus_address = ibus_daemon["bus_address"]
        os.environ["DBUS_SESSION_BUS_ADDRESS"] = bus_address
        tc = IBusTestClient(ice_dict, trace_engine)
    else:
        user_dict_dir = os.path.join(tmp_path, "user-dict")
        os.makedirs(user_dict_dir, exist_ok=True)
        user_dict_path = os.path.join(user_dict_dir, "user.dict")
        tc = TestClient(ice_dict, user_dict_path, trace_engine)

    yield tc
    tc.close()
    trace_engine.close()
