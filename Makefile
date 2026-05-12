.PHONY: all build test install clean build-dict

PREFIX ?= /usr/local
LIB_DIR ?= $(PREFIX)/lib/ibus-ice
DATA_DIR ?= $(PREFIX)/share/ibus-ice
PY_DIR ?= $(LIB_DIR)/python/ibus_ice
IBUS_COMPONENT_DIR ?= /usr/share/ibus/component

all: build

build: build-dict
	cargo build --release -p core

build-dict:
	bash scripts/build-dict.sh

test:
	cargo test -p core
	cargo test -p dict-compiler

install:
	install -d $(DESTDIR)$(LIB_DIR)
	install -d $(DESTDIR)$(DATA_DIR)
	install -d $(DESTDIR)$(PY_DIR)
	install -d $(DESTDIR)$(IBUS_COMPONENT_DIR)
	install -m 755 target/release/libcore.so $(DESTDIR)$(LIB_DIR)/libibus_ice_core.so
	install -m 644 build/ice.dict $(DESTDIR)$(DATA_DIR)/ice.dict
	install -m 644 python/ibus_ice/__init__.py $(DESTDIR)$(PY_DIR)/__init__.py
	install -m 644 python/ibus_ice/ffi.py $(DESTDIR)$(PY_DIR)/ffi.py
	install -m 644 python/ibus_ice/engine.py $(DESTDIR)$(PY_DIR)/engine.py
	install -m 644 python/ibus_ice/engine_main.py $(DESTDIR)$(PY_DIR)/engine_main.py
	install -m 644 python/ibus_ice/ice.xml $(DESTDIR)$(IBUS_COMPONENT_DIR)/ice.xml
	@echo "Installed to $(DESTDIR)$(PREFIX)"

uninstall:
	rm -f $(DESTDIR)$(LIB_DIR)/libibus_ice_core.so
	rm -f $(DESTDIR)$(DATA_DIR)/ice.dict
	rm -rf $(DESTDIR)$(PY_DIR)
	rm -f $(DESTDIR)$(IBUS_COMPONENT_DIR)/ice.xml
	-rmdir $(DESTDIR)$(LIB_DIR) 2>/dev/null || true
	-rmdir $(DESTDIR)$(DATA_DIR) 2>/dev/null || true
	@echo "Uninstalled from $(DESTDIR)$(PREFIX)"

clean:
	cargo clean
	rm -rf build
