.PHONY: all build test install uninstall clean build-dict

PREFIX ?= /usr/local
LIBDIR ?= $(PREFIX)/lib/ibus-ice
DATADIR ?= $(PREFIX)/share/ibus-ice
PYDIR ?= $(LIBDIR)/python/ibus_ice
LIBEXECDIR ?= $(PREFIX)/libexec
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
	install -d $(DESTDIR)$(LIBDIR)
	install -d $(DESTDIR)$(DATADIR)
	install -d $(DESTDIR)$(PYDIR)
	install -d $(DESTDIR)$(LIBEXECDIR)
	install -d $(DESTDIR)$(IBUS_COMPONENT_DIR)
	install -m 755 target/release/libcore.so $(DESTDIR)$(LIBDIR)/libibus_ice_core.so
	install -m 644 build/ice.dict $(DESTDIR)$(DATADIR)/ice.dict
	install -m 644 engine/ibus_ice/__init__.py $(DESTDIR)$(PYDIR)/__init__.py
	install -m 644 engine/ibus_ice/ffi.py $(DESTDIR)$(PYDIR)/ffi.py
	install -m 644 engine/ibus_ice/engine.py $(DESTDIR)$(PYDIR)/engine.py
	install -m 644 engine/ibus_ice/engine_main.py $(DESTDIR)$(PYDIR)/engine_main.py
	sed 's|@libexecdir@|$(LIBEXECDIR)|g' engine/ibus_ice/ice.xml.in > $(DESTDIR)$(IBUS_COMPONENT_DIR)/ice.xml
	sed 's|@datadir@|$(DATADIR)|g' engine/ibus-engine-ice.in > $(DESTDIR)$(LIBEXECDIR)/ibus-engine-ice
	chmod 755 $(DESTDIR)$(LIBEXECDIR)/ibus-engine-ice
	@echo "Installed to $(DESTDIR)$(PREFIX)"

uninstall:
	rm -f $(DESTDIR)$(LIBDIR)/libibus_ice_core.so
	rm -f $(DESTDIR)$(DATADIR)/ice.dict
	rm -rf $(DESTDIR)$(PYDIR)
	rm -f $(DESTDIR)$(LIBEXECDIR)/ibus-engine-ice
	rm -f $(DESTDIR)$(IBUS_COMPONENT_DIR)/ice.xml
	-rmdir $(DESTDIR)$(LIBDIR) 2>/dev/null || true
	-rmdir $(DESTDIR)$(DATADIR) 2>/dev/null || true
	@echo "Uninstalled from $(DESTDIR)$(PREFIX)"

clean:
	cargo clean
	rm -rf build
