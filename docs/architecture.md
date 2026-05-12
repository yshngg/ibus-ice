# ibus-ice Architecture

## Overview

ibus-ice is a Chinese input method engine built on the [IBus](https://github.com/ibus/ibus) framework. The core logic (pinyin parsing, dictionary lookup, candidate ranking, user frequency learning) is implemented in **Rust** and compiled to a shared library. A thin **Python** adapter connects it to IBus via ctypes. The dictionary is sourced from [rime-ice](https://github.com/iDvel/rime-ice) and compiled at build time into a binary Double-Array Trie.

```
+------------------------------------------+
|              IBus Daemon                 |
|  +------------------------------------+  |
|  |  ibus-ice (Python IBusEngine)      |  |
|  |  - Key event handling              |  |
|  |  - LookupTable (candidate window)  |  |
|  |  - Text committing                 |  |
|  +--------------+---------------------+  |
|                 | ctypes (C ABI)         |
|  +--------------v---------------------+  |
|  |  libibus_ice_core.so (Rust)        |  |
|  |  +----------------------------+    |  |
|  |  |  IceEngine                 |    |  |
|  |  |  - Segmenter               |    |  |
|  |  |  - Dictionary (mmap Trie)  |    |  |
|  |  |  - Candidate Generator     |    |  |
|  |  |  - Ranker (pluggable)      |    |  |
|  |  |  - User Dictionary         |    |  |
|  |  +----------------------------+    |  |
|  +------------------------------------+  |
+------------------------------------------+

Build-time:
+----------------------------+
|  dict-compiler (Rust CLI)  |
|  - Parse rime-ice YAML     |
|  - Build Double-Array Trie |
|  - Output .dict binary     |
+----------------------------+
```

## Project Structure

```
ibus-ice/
+-- Cargo.toml                # Rust workspace
+-- Makefile                  # Build system
|
+-- core/                     # Rust shared library -> libibus_ice_core.so
|   +-- src/
|       +-- ffi.rs            # extern "C" ABI (6 functions)
|       +-- engine.rs         # IceEngine orchestrator
|       +-- segmenter.rs      # Pinyin segmentation algorithm
|       +-- syllable.rs       # Valid pinyin syllable table (~410)
|       +-- dictionary.rs     # mmap-based Trie reader
|       +-- candidate.rs      # Candidate generation
|       +-- ranker.rs         # Pluggable ranking (WeightedRanker)
|       +-- userdict.rs       # Append-only user frequency log
|
+-- dict-compiler/            # Build-time CLI tool
|   +-- src/
|       +-- parser.rs         # rime-ice YAML parser
|       +-- trie_builder.rs   # Double-Array Trie construction
|
+-- engine/                    # IBus engine adapter
|   +-- ibus-engine-ice.in     # Shell wrapper script
|   +-- ibus_ice/
|       +-- engine.py          # IceIBusEngine (IBus.Engine subclass)
|       +-- engine_main.py     # IMApp entry point
|       +-- ffi.py             # ctypes bindings
|       +-- ice.xml            # IBus component descriptor
|
+-- scripts/
|   +-- build-dict.sh         # Dictionary compilation script
|
+-- docs/
    +-- architecture.md
```

## Data Flow

```
User types "zhongguo"
  |
  v
ibus-daemon -> Python process_key_event()
  |
  v
ice_process("zhongguo") via ctypes
  |
  v
Rust: segmenter.segment("zhongguo") -> ["zhong", "guo"]
  |
  v
Rust: dictionary.lookup("zhong guo") -> Trie prefix match
  |
  v
Rust: candidate.generate() -> [{text:"...", freq:200}, ...]
  |
  v
Rust: ranker.rank() -> apply weights, sort by score
  |
  v
Python: populate IBus LookupTable with results
  |
  v
IBus Panel displays candidate window
```

## Dictionary Format

### Source (rime-ice YAML)

```
zhong guo  100
zhong guo ren  200
```

Tab-separated: `text\tpinyin(space-separated)\tweight(optional)`.

### Compiled Binary (.dict)

```
Header (64B) | "IBUSICE" | version | num_entries | trie_offset | payload_offset
Trie         | base[0..n] as i64 LE, then check[0..n] as i64 LE
Payload      | offset_table u32[num_entries], then entries for each:
             |   text_len:u16 LE, text:UTF-8, freq:u32 LE, word_len:u8
```

The Trie is mmap'd at runtime for zero-copy loading. Lookups are O(n) in query length.

## C ABI Interface

Six `extern "C"` functions exposed by `libibus_ice_core.so`:

| Function | Description |
|----------|-------------|
| `ice_engine_new(dict_path, user_dict_path)` | Create engine instance |
| `ice_engine_free(handle)` | Destroy engine |
| `ice_process(handle, pinyin)` | Query candidates for pinyin input |
| `ice_select(handle, text)` | Record user selection for frequency learning |
| `ice_candidates_free(list)` | Free candidate list memory |
| `ice_reset(handle)` | Clear input state |

## Candidate Ranking

Multi-factor weighted scoring (v1):

```
score = ln(freq) * 1.0      // system dictionary frequency
      + user_boost * 2.0    // user selection history with time decay
      + exact_match * 3.0   // exact vs prefix match bonus
```

Ranking is pluggable via the `Ranker` trait. Future backends: N-gram language model, neural ranking.

## User Dictionary

Append-only log at `~/.local/share/ibus-ice/user.dict`:

```
+   zhong guo  ... 1700000000   # new entry
^   zhong guo  ... 1700000100   # frequency increment
```

Frequency decays exponentially with time (lambda = 0.01, halves in ~70 days).

## Build & Install

```bash
make build          # compile Rust + compile dictionaries
make test           # run all tests
sudo make install   # install to /usr/local
```

Requires: Rust toolchain, Python 3.10+ with `uv`, `ibus` >= 1.5.
