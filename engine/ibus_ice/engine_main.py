# ibus-ice - Ice Chinese Input Method engine for IBus
#
# Copyright (c) 2024 ibus-ice contributors
# License: GPLv3

import os
import sys
import getopt
import locale

import gi
gi.require_version("IBus", "1.0")
from gi.repository import IBus
from gi.repository import GLib
from gi.repository import GObject

# Ensure the engine package is importable
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import ibus_ice.engine


class IMApp:
    def __init__(self, exec_by_ibus):
        self._mainloop = GLib.MainLoop()
        self._bus = IBus.Bus()
        self._bus.connect("disconnected", self._bus_disconnected_cb)

        self._factory = IBus.Factory.new(self._bus.get_connection())
        self._factory.add_engine(
            "ice", GObject.type_from_name("IceIBusEngine")
        )

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
        engine = IBus.EngineDesc.new(
            "ice",
            "Ice",
            "Ice Chinese Input Method",
            "zh",
            "GPL",
            "ibus-ice contributors",
            "",
            "us",
        )
        component.add_engine(engine)
        return component

    def run(self):
        self._mainloop.run()

    def _bus_disconnected_cb(self, bus):
        self._mainloop.quit()


def launch_engine(exec_by_ibus):
    IBus.init()
    IMApp(exec_by_ibus).run()


def print_help(v=0):
    print("Usage: ibus-engine-ice [OPTIONS]")
    print("-i, --ibus     executed by IBus daemon")
    print("-h, --help     show this message")
    print("-d, --daemonize daemonize process")
    sys.exit(v)


def main():
    try:
        locale.setlocale(locale.LC_ALL, "")
    except Exception:
        pass

    exec_by_ibus = False
    daemonize = False

    shortopt = "ihd"
    longopt = ["ibus", "help", "daemonize"]

    try:
        opts, args = getopt.getopt(sys.argv[1:], shortopt, longopt)
    except getopt.GetoptError:
        print_help(1)

    for o, a in opts:
        if o in ("-h", "--help"):
            print_help()
        elif o in ("-d", "--daemonize"):
            daemonize = True
        elif o in ("-i", "--ibus"):
            exec_by_ibus = True

    if daemonize:
        if os.fork():
            sys.exit()

    launch_engine(exec_by_ibus)


if __name__ == "__main__":
    main()
