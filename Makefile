.PHONY: all build test install uninstall clean build-dict

PREFIX ?= /usr/local
LIBDIR ?= $(PREFIX)/lib/ibus-ice
DATADIR ?= $(PREFIX)/share/ibus-ice
ENGINEDIR ?= $(LIBDIR)/engine
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

test-e2e:
	cargo build --release -p core
	cargo build --release -p dict-compiler
	python -m pytest tests/ -v

install:
	install -d $(DESTDIR)$(LIBDIR)
	install -d $(DESTDIR)$(DATADIR)
	install -d $(DESTDIR)$(ENGINEDIR)
	install -d $(DESTDIR)$(LIBEXECDIR)
	install -d $(DESTDIR)$(IBUS_COMPONENT_DIR)
	install -m 755 target/release/libcore.so $(DESTDIR)$(LIBDIR)/libibus_ice_core.so
	install -m 644 build/ice.dict $(DESTDIR)$(DATADIR)/ice.dict
	install -m 644 engine/ffi.py $(DESTDIR)$(ENGINEDIR)/ffi.py
	install -m 644 engine/engine.py $(DESTDIR)$(ENGINEDIR)/engine.py
	install -m 644 engine/main.py $(DESTDIR)$(ENGINEDIR)/main.py
	sed 's|@libexecdir@|$(LIBEXECDIR)|g' engine/ice.xml.in > $(DESTDIR)$(IBUS_COMPONENT_DIR)/ice.xml
	sed -e 's|@datadir@|$(DATADIR)|g' -e 's|@enginedir@|$(ENGINEDIR)|g' engine/ibus-engine-ice.in > $(DESTDIR)$(LIBEXECDIR)/ibus-engine-ice
	chmod 755 $(DESTDIR)$(LIBEXECDIR)/ibus-engine-ice
	@echo "Installed to $(DESTDIR)$(PREFIX)"

uninstall:
	rm -f $(DESTDIR)$(LIBDIR)/libibus_ice_core.so
	rm -f $(DESTDIR)$(DATADIR)/ice.dict
	rm -rf $(DESTDIR)$(ENGINEDIR)
	rm -f $(DESTDIR)$(LIBEXECDIR)/ibus-engine-ice
	rm -f $(DESTDIR)$(IBUS_COMPONENT_DIR)/ice.xml
	-rmdir $(DESTDIR)$(LIBDIR) 2>/dev/null || true
	-rmdir $(DESTDIR)$(DATADIR) 2>/dev/null || true
	@echo "Uninstalled from $(DESTDIR)$(PREFIX)"

clean:
	cargo clean
	rm -rf build
