use memmap2::Mmap;
use std::fs;

pub struct Dictionary {
    _mmap: Mmap,
    base: *const i64,
    check: *const i64,
    trie_size: usize,
    #[allow(dead_code)]
    num_entries: u32,
    payload_offsets: *const u32,
    payload_data: *const u8,
}

unsafe impl Send for Dictionary {}
unsafe impl Sync for Dictionary {}

#[derive(Debug, Clone)]
pub struct DictEntry {
    pub text: String,
    pub freq: u32,
    pub word_len: u8,
}

impl Dictionary {
    pub fn open(path: &str) -> Result<Self, String> {
        let file = fs::File::open(path).map_err(|e| format!("open: {}", e))?;
        let mmap = unsafe { Mmap::map(&file).map_err(|e| format!("mmap: {}", e))? };

        if mmap.len() < 64 {
            return Err("dict file too small".into());
        }
        if &mmap[0..7] != b"IBUSICE" {
            return Err("invalid magic".into());
        }

        let version = u32::from_le_bytes([mmap[7], mmap[8], mmap[9], mmap[10]]);
        if version != 1 {
            return Err(format!("unsupported version: {}", version));
        }

        let num_entries = u32::from_le_bytes([mmap[11], mmap[12], mmap[13], mmap[14]]);
        let trie_offset = u64::from_le_bytes(mmap[15..23].try_into().unwrap()) as usize;
        let payload_offset = u64::from_le_bytes(mmap[23..31].try_into().unwrap()) as usize;

        let trie_size = (payload_offset - trie_offset) / 16; // base + check, each 8 bytes

        let base = unsafe { mmap.as_ptr().add(trie_offset) as *const i64 };
        let check = unsafe { mmap.as_ptr().add(trie_offset + trie_size * 8) as *const i64 };
        let payload_offsets = unsafe { mmap.as_ptr().add(payload_offset) as *const u32 };
        let payload_data =
            unsafe { mmap.as_ptr().add(payload_offset + (num_entries as usize * 4)) };

        Ok(Dictionary {
            _mmap: mmap,
            base,
            check,
            trie_size,
            num_entries,
            payload_offsets,
            payload_data,
        })
    }

    pub fn lookup(&self, key: &str) -> Vec<DictEntry> {
        let mut results = Vec::new();
        let bytes = key.as_bytes();
        let mut s: usize = 0;

        for &byte in bytes.iter() {
            let base_s = unsafe { *self.base.add(s) as usize };
            let t = base_s + byte as usize;
            if t >= self.trie_size || unsafe { *self.check.add(t) as usize } != s {
                return results;
            }
            s = t;
        }

        let mut ids = Vec::new();
        self.collect_leaves(s, &mut ids);

        for id in ids {
            let offset = unsafe { std::ptr::read_unaligned(self.payload_offsets.add(id)) as usize };
            let ptr = unsafe { self.payload_data.add(offset) };
            let text_len = unsafe { std::ptr::read_unaligned(ptr as *const u16) } as usize;
            let text_bytes = unsafe { std::slice::from_raw_parts(ptr.add(2), text_len) };
            let text = String::from_utf8_lossy(text_bytes).into_owned();
            let freq = unsafe { std::ptr::read_unaligned(ptr.add(2 + text_len) as *const u32) };
            let word_len = unsafe { *ptr.add(2 + text_len + 4) };

            results.push(DictEntry { text, freq, word_len });
        }

        results
    }

    fn collect_leaves(&self, node: usize, ids: &mut Vec<usize>) {
        let base_val = unsafe { *self.base.add(node) };
        if base_val <= 0 {
            let id = (-base_val - 1) as usize;
            if !ids.contains(&id) {
                ids.push(id);
            }
            return;
        }
        for c in 0u8..=255u8 {
            let t = base_val as usize + c as usize;
            if t < self.trie_size && unsafe { *self.check.add(t) as usize } == node {
                self.collect_leaves(t, ids);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    fn dict_path() -> String {
        std::env::var("TEST_DICT").unwrap_or_else(|_| "/tmp/test.dict".into())
    }

    #[test]
    fn test_open_invalid_file() {
        let result = Dictionary::open("/nonexistent/path.dict");
        assert!(result.is_err());
    }
}
