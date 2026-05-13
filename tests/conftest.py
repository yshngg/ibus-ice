"""pytest fixtures for ibus-ice E2E tests."""

import os
import random
import string
import subprocess
import sys
import time
import xml.etree.ElementTree as ET

import pytest

from test_helpers import TestClient
from trace import TraceEngine

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


@pytest.fixture
def client(ibus_session, ice_dict):
    """Create a TestClient connected to the isolated IBus session."""
    bus_address = ibus_session["bus_address"]
    os.environ["DBUS_SESSION_BUS_ADDRESS"] = bus_address
    trace_engine = TraceEngine(ice_dict)
    tc = TestClient(bus_address, trace_engine)
    yield tc
    trace_engine.close()


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

    # Start isolated dbus-daemon (--nofork keeps it as a child process)
    dbus_proc = subprocess.Popen(
        ["dbus-daemon", "--config-file=" + dbus_cfg, "--print-address", "--nofork"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    actual_address = dbus_proc.stdout.readline().strip()
    if not actual_address:
        dbus_proc.kill()
        dbus_proc.wait()
        raise RuntimeError("dbus-daemon did not print an address")

    # Set up env for ibus-daemon
    env = {
        **os.environ,
        "DBUS_SESSION_BUS_ADDRESS": actual_address,
        "IBUS_COMPONENT_PATH": f"{component_dir}:{BASE_COMPONENT_DIR}",
        "HOME": home_dir,
    }

    # Start ibus-daemon (--nodaemon keeps it as a child process)
    ibus_proc = subprocess.Popen(
        ["ibus-daemon", "--nodaemon", "--replace", "--verbose"],
        env=env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )

    # Wait for engine registration via D-Bus ping
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
        _kill_procs(dbus_proc, ibus_proc)
        raise RuntimeError("ibus-daemon did not register ice engine within 10s")

    yield {"bus_address": actual_address}

    # Teardown: kill child processes cleanly
    _kill_procs(ibus_proc, dbus_proc)
