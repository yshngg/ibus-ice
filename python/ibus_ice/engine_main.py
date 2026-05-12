"""Entry point for IBus to launch the ibus-ice engine."""
import os
import sys

# Ensure the ibus_ice package and its Python deps are importable
sys.path.insert(0, "/usr/lib/ibus-ice/python")

from gi.repository import IBus

from ibus_ice.engine import IceIBusEngine


class EngineFactory(IBus.Factory):
    def __init__(self, bus):
        super().__init__(bus)
        self._engine_id = 0

    def create_engine(self, engine_name):
        if engine_name == "ice":
            self._engine_id += 1
            return IceIBusEngine(
                self.get_bus(),
                f"/org/freedesktop/IBus/Engine/Ice/{self._engine_id}",
                engine_name,
            )
        return None


def main():
    IBus.init()
    bus = IBus.Bus()
    factory = EngineFactory(bus)
    bus.connect("destroy", lambda *args: IBus.quit())
    bus.request_name("org.freedesktop.IBus.Ice", 0)
    factory.register()
    IBus.main()


if __name__ == "__main__":
    main()
