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
| `engine/` | IBus engine adapter (Python, shell wrapper, component XML) |
| `scripts/` | Build and install scripts |

## Testing

```bash
make test           # Rust unit tests (24 tests)
make test-e2e       # E2E test environment (25 tests)
```

The E2E test environment uses pytest with a chainable `TestClient` API that
wraps the engine directly:

```python
client.type_pinyin("zhongguo")
assert client.get_candidates()[0].text == "中国"
client.press_space()
assert "中国" in client.get_committed()
```

On failure, tests produce rich position-aligned candidate diffs and full
pipeline traces (segmentation, dictionary lookups, ranking scores) via
`debug_process()` instrumentation in the Rust core.

| File | Tests |
|------|-------|
| `tests/test_basic_input.py` | Pinyin typing, preedit display, candidate updates |
| `tests/test_candidates.py` | Candidate ordering, page navigation, common word ranking |
| `tests/test_commit_select.py` | Space commit, number select, Chinese text committing |
| `tests/test_special_keys.py` | Backspace, escape, enter, apostrophe separator |
| `tests/test_english.py` | English word input, non-pinyin fallback |
| `tests/test_ranking.py` | Frequency ranking, user boost after selection |
| `tests/test_edge_cases.py` | Rapid typing, long input, uppercase, control keys, single char |

## Requirements

- Rust stable toolchain
- Python 3.10+ with `uv`
- IBus >= 1.5

## License

[GPLv3](LICENSE)
