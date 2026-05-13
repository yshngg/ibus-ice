import getopt
import locale
import os
import sys

import gi

import engine

gi.require_version("IBus", "1.0")  # noqa: E402
from gi.repository import GLib, GObject, IBus


class IMApp:
    def __init__(self, exec_by_ibus):
        self._mainloop = GLib.MainLoop()
        self._bus = IBus.Bus()
        self._bus.connect("disconnected", self._bus_disconnected_cb)

        self._factory = IBus.Factory.new(self._bus.get_connection())
        self._factory.add_engine("ice", GObject.type_from_name("IceIBusEngine"))

        if exec_by_ibus:
            self._bus.request_name("org.freedesktop.IBus.Ice", 0)
        else:
            self._bus.register_component(self._make_component())
            self._bus.set_global_engine_async("ice", -1, None, None, None)

    def _make_component(self):
        component = IBus.Component.new(
            "org.freedesktop.IBus.Ice",
            "Ice Chinese Input Method",
            "0.1.0",
            "GPL",
            "ibus-ice contributors",
            "https://github.com/yshngg/ibus-ice",
            "/usr/local/libexec/ibus-engine-ice",
            "ibus-ice",
        )
        eng = IBus.EngineDesc.new(
            "ice",
            "Ice",
            "Ice Chinese Input Method",
            "zh",
            "GPL",
            "ibus-ice contributors",
            "",
            "us",
        )
        component.add_engine(eng)
        return component

    def run(self):
        self._mainloop.run()

    def _bus_disconnected_cb(self, bus):
        self._mainloop.quit()


def launch_engine(exec_by_ibus):
    IBus.init()
    IMApp(exec_by_ibus).run()


def cli_test():
    """Terminal test: type pinyin, see candidates."""
    from ffi import Engine

    # Try env var, then project build dir, then installed path
    data_dir = os.environ.get("IBUS_ICE_DATA_DIR", "")
    if not data_dir:
        candidates = [
            os.path.join(os.path.dirname(__file__), "..", "build"),
            os.path.join(os.path.dirname(__file__), "..", "target"),
            "/usr/local/share/ibus-ice",
        ]
        for d in candidates:
            if os.path.exists(os.path.join(d, "ice.dict")):
                data_dir = d
                break
        if not data_dir:
            print("Cannot find ice.dict. Set IBUS_ICE_DATA_DIR or run `make build-dict`.")
            sys.exit(1)

    dict_path = os.environ.get("IBUS_ICE_DICT", os.path.join(data_dir, "ice.dict"))

    user_dir = os.path.expanduser("~/.local/share/ibus-ice/user.dict")

    e = Engine(dict_path, user_dir)
    print("===== ibus-ice CLI =====")
    print("Type pinyin (e.g. zhongguo), Ctrl-C to quit.\n")
    try:
        while True:
            try:
                pinyin = input("> ").strip()
            except EOFError:
                break
            if not pinyin:
                continue
            results = e.process(pinyin)
            if not results:
                print("  (no candidates)")
            else:
                for idx, c in enumerate(results[:9]):
                    print(f"  {idx + 1}. {c['text']}")
            print()
    except KeyboardInterrupt:
        pass
    print("bye.")


def print_help(v=0):
    print("Usage: ibus-engine-ice [OPTIONS]")
    print("-i, --ibus      executed by IBus daemon")
    print("-h, --help      show this message")
    print("-d, --daemonize daemonize process")
    print("-t, --test      terminal CLI test mode")
    print("-x, --xml       output engine XML description")
    sys.exit(v)


def print_engine_xml():
    """Print engine description XML for ibus-daemon discovery."""
    print("""<?xml version='1.0' encoding='utf-8'?>
<engines>
  <engine>
    <name>ice</name>
    <language>zh</language>
    <license>GPLv3</license>
    <author>ibus-ice contributors</author>
    <layout>us</layout>
    <longname>Ice</longname>
    <description>Ice Chinese Input Method</description>
    <rank>50</rank>
  </engine>
</engines>""")
    sys.exit(0)


def main():
    try:
        locale.setlocale(locale.LC_ALL, "")
    except Exception:
        pass

    exec_by_ibus = False
    daemonize = False
    test_mode = False
    xml_mode = False

    shortopt = "ihdtx"
    longopt = ["ibus", "help", "daemonize", "test", "xml"]

    try:
        opts, args = getopt.getopt(sys.argv[1:], shortopt, longopt)
    except getopt.GetoptError:
        print_help(1)

    for o, _a in opts:
        if o in ("-h", "--help"):
            print_help()
        elif o in ("-d", "--daemonize"):
            daemonize = True
        elif o in ("-i", "--ibus"):
            exec_by_ibus = True
        elif o in ("-t", "--test"):
            test_mode = True
        elif o in ("-x", "--xml"):
            xml_mode = True

    if xml_mode:
        print_engine_xml()

    if daemonize:
        if os.fork():
            sys.exit()

    if test_mode:
        cli_test()
    else:
        launch_engine(exec_by_ibus)


if __name__ == "__main__":
    main()
