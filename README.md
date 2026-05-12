# ibus-ice

A Chinese input method engine for IBus, with a Rust-powered core and rime-ice dictionaries.

## Features

- **Full Pinyin** input with longest-match segmentation
- **English** word input (from rime-ice English dictionaries)
- **913K+ entries** compiled from rime-ice (雾凇拼音)
- **User frequency learning** with time decay
- **Pluggable ranking** via trait system (future AI/neural backends)

## Quick Start

```bash
make build          # compile Rust + build dictionary
sudo make install   # install to /usr/local
sudo make uninstall # remove installed files

# Restart IBus:
ibus-daemon -xrv
# Select "Ice" from IBus input method menu
```

## Architecture

See [docs/architecture.md](docs/architecture.md) for the full design.

## Project Layout

| Directory | Description |
|-----------|-------------|
| `core/` | Rust shared library (cdylib -> `libibus_ice_core.so`) |
| `dict-compiler/` | Build-time CLI to compile rime-ice YAML -> binary Trie |
| `python/` | Python IBus adapter (ctypes bindings + engine class) |
| `scripts/` | Build and install scripts |

## Requirements

- Rust stable toolchain
- Python 3.10+ with `uv`
- IBus >= 1.5

## License

[GPLv3](LICENSE)
