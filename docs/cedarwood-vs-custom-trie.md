# Why cedarwood works and the custom trie didn't

The `dict-compiler` originally used a hand-rolled Double-Array Trie with three
compounding bugs. Replacing it with [cedarwood](https://crates.io/crates/cedarwood)
(a Rust port of the C++ cedar library) eliminated all three.

---

## Bug 1: Memory explosion (26 GB RSS → OOM kill)

### How it happened

```rust
// Original (broken) find_base:
fn find_base(base: &mut Vec<i64>, check: &mut Vec<i64>, children: &[u8]) -> i64 {
    let max_c = children.iter().copied().max().unwrap_or(0) as usize;
    let new_len = check.len() + max_c + 1;
    base.resize(new_len, 0);
    check.resize(new_len, -1);
    check.len() as i64    // ← RETURNS THE LENGTH, NOT A VALID INDEX
}
```

After `resize`, `check.len()` returns `new_len`. This is **one past the last
valid index** (indices are `0..len`, so the last valid index is `len - 1`).

This out-of-bounds value became `base[s]`. When the engine accessed
`t = base[s] + byte`, it got another OOB index. Each access triggered
`ensure_capacity`:

```rust
fn ensure_capacity(base: &mut Vec<i64>, check: &mut Vec<i64>, idx: usize) {
    if idx >= base.len() {
        let new_size = (idx + 256).next_power_of_two();
        base.resize(new_size, 0);   // Doubles memory
        check.resize(new_size, -1);  // Doubles memory
    }
}
```

`next_power_of_two` doubled the array size each time. For 913K entries,
each insertion triggered at least one doubling, consuming **26 GB RSS**
before the kernel OOM-killed the process:

```
Out of memory: Killed process 391666 (dict-compiler)
total-vm:33986616kB anon-rss:26197640kB
```

### How cedarwood avoids it

cedarwood maintains a proper `base`/`check` invariant where every index is
valid. No OOB values are ever written or read.

---

## Bug 2: O(n²) performance (82+ minutes at 100% CPU)

### How it happened

The first attempted fix replaced the memory explosion with a linear scan
starting from `s = 1`:

```rust
// First fix (correct but O(n²)):
fn find_base(..., cursor: &mut i64) -> i64 {
    let mut s = *cursor;
    if s < 1 { s = 1; }
    loop {
        let mut all_free = true;
        for &c in children {
            if check[s + c] != -1 { all_free = false; break; }
        }
        if all_free { return s; }
        s += 1;  // ← walks one-by-one through millions of occupied slots
    }
}
```

For a dense trie with 500K+ occupied positions, each `find_base` call scanned
millions of positions. With 913K entries × ~3 `find_base` calls each × millions
of checks, the total was **billions of comparisons**. The process ran at 100%
CPU for 82+ minutes without completing.

### How cedarwood avoids it

cedarwood uses the proven cedar algorithm — amortized O(1) per insertion
regardless of trie density. 913K entries build in seconds.

---

## Bug 3: Space leak after collision relocation

### How it happened

The allocate-at-end approach was O(1) but never freed old positions after
collision relocation. When two keys shared a prefix and then diverged, the
trie moved children to new positions but left `check[old_t]` pointing to
the old parent. No future insertion would reuse that space because the old
parent's `base` had been changed to the new position — the old slots became
**permanently unreachable garbage**.

For 913K entries, this produced a trie with **billions of abandoned slots**.

### How cedarwood avoids it

cedarwood's relocation correctly marks old slots as free, allowing them to be
reused by future insertions. Memory usage stays proportional to the number
of unique keys, not the number of insertions.

---

## Summary

| Property | Custom trie | cedarwood |
|----------|-------------|-----------|
| Memory (913K entries) | 26 GB (OOM) | ~50 MB |
| Build time (913K entries) | never completes | seconds |
| Collision handling | abandoned slots | freed + reused |
| Prefix search | manual `collect_leaves` | `common_prefix_predict` |
| Serialization | manual mmap format | compact entry format |
| Safety | `unsafe` pointer arithmetic | pure safe Rust |
