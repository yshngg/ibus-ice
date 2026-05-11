.PHONY: all build test install clean build-dict

PREFIX ?= /usr/local
LIB_DIR ?= $(PREFIX)/lib/ibus-ice
DATA_DIR ?= $(PREFIX)/share/ibus-ice

all: build

build: build-dict
	cargo build --release -p core

build-dict:
	bash scripts/build-dict.sh

test:
	cargo test -p core
	cargo test -p dict-compiler

install: build
	install -d $(DESTDIR)$(LIB_DIR)
	install -d $(DESTDIR)$(DATA_DIR)
	install -m 755 target/release/libcore.so $(DESTDIR)$(LIB_DIR)/libibus_ice_core.so
	install -m 644 build/ice.dict $(DESTDIR)$(DATA_DIR)/ice.dict
	@echo "Installed to $(DESTDIR)$(PREFIX)"

clean:
	cargo clean
	rm -rf build
