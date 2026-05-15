//! Performance profiling for dict-compiler.
//!
//! Emits JSON Lines events to stderr.  Pipe them to scripts/profile-dict.py.
//! Set DICT_PROFILE=1 to enable (off by default for zero-overhead production).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::Instant;

static ENABLED: AtomicBool = AtomicBool::new(false);
static START: OnceLock<Instant> = OnceLock::new();

pub fn init() {
    let enabled = std::env::var("DICT_PROFILE").unwrap_or_default() == "1";
    if enabled {
        START.set(Instant::now()).ok();
    }
    ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

fn elapsed_ms() -> f64 {
    START.get().map(|s| s.elapsed().as_secs_f64() * 1000.0).unwrap_or(0.0)
}

fn emit(category: &str, data: &str) {
    eprintln!("{{\"ts\":{:.1},\"cat\":\"{}\",{}}}", elapsed_ms(), category, data);
}

pub fn phase(name: &str) {
    if !enabled() { return; }
    emit("phase", &format!("\"name\":\"{}\"", name));
}

pub fn progress(inserted: usize, total: usize, base_len: usize) {
    if !enabled() { return; }
    emit("progress", &format!("\"inserted\":{},\"total\":{},\"base_len\":{}",
        inserted, total, base_len));
}

pub fn memory_rss_mb() -> Option<u64> {
    // Read /proc/self/statm: resident set size (page count)
    std::fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|s| {
            let parts: Vec<&str> = s.split_whitespace().collect();
            parts.get(1).and_then(|v| v.parse::<u64>().ok())
        })
        .map(|pages| pages * 4 / 1024) // pages * 4KB / 1024 = MB
}

pub fn memory_sample() {
    if !enabled() { return; }
    if let Some(rss) = memory_rss_mb() {
        emit("memory", &format!("\"rss_mb\":{}", rss));
    }
}

pub fn finalize(trie_nodes: usize, entries: usize) {
    if !enabled() { return; }
    let rss = memory_rss_mb().unwrap_or(0);
    emit("final", &format!("\"trie_nodes\":{},\"entries\":{},\"rss_mb\":{},\"elapsed_ms\":{:.1}",
        trie_nodes, entries, rss, elapsed_ms()));
}
