use crate::DictEntry;

use crate::perf;

pub struct DoubleArrayTrie {
    pub base: Vec<i64>,
    pub check: Vec<i64>,
}

const ROOT: usize = 0;
const INITIAL_SIZE: usize = 524288;

impl DoubleArrayTrie {
    pub fn build(entries: &[DictEntry]) -> Self {
        let mut keys: Vec<(Vec<u8>, usize)> = entries
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let key = e.pinyin.join(" ");
                (key.into_bytes(), i)
            })
            .collect();

        keys.sort_by(|a, b| a.0.cmp(&b.0));

        let mut base = vec![0i64; INITIAL_SIZE];
        let mut check = vec![-1i64; INITIAL_SIZE];

        for (i, (key, payload_id)) in keys.iter().enumerate() {
            if perf::enabled() && (i % 1000 == 0 || i + 1 == keys.len()) {
                perf::progress(i, keys.len(), base.len());
                perf::memory_sample();
            }
            Self::insert(&mut base, &mut check, key, *payload_id as usize, &mut 1i64);
        }

        DoubleArrayTrie { base, check }
    }

    fn ensure_capacity(base: &mut Vec<i64>, check: &mut Vec<i64>, idx: usize) {
        if idx >= base.len() {
            base.resize(idx + 256, 0);
            check.resize(idx + 256, -1);
        }
    }

    fn find_base(base: &mut Vec<i64>, check: &mut Vec<i64>, children: &[u8], cursor: &mut i64) -> i64 {
        let mut s = *cursor;
        if s < 1 { s = 1; }
        let mut attempts: u64 = 0;
        loop {
            attempts += 1;
            let mut all_free = true;
            for &c in children {
                let t = s as usize + c as usize;
                Self::ensure_capacity(base, check, t);
                if check[t] != -1 {
                    all_free = false;
                    break;
                }
            }
            if all_free {
                *cursor = s + 1;
                if perf::enabled() {
                    perf::find_base(*cursor, base.len(), attempts, true);
                }
                return s;
            }
            s += 1;
        }
    }

    fn insert(base: &mut Vec<i64>, check: &mut Vec<i64>, key: &[u8], payload_id: usize, cursor: &mut i64) {
        let mut s = ROOT;
        let mut i = 0;

        while i < key.len() {
            let byte = key[i];

            if base[s] < 0 {
                let saved_payload = (-base[s] - 1) as usize;
                base[s] = 0;

                let children = if 0 == byte { vec![0u8] } else { vec![0u8, byte] };
                let new_base = Self::find_base(base, check, &children, cursor);
                base[s] = new_base;

                let term_t = new_base as usize;
                Self::ensure_capacity(base, check, term_t);
                base[term_t] = -(saved_payload as i64 + 1);
                check[term_t] = s as i64;

                continue;
            }

            let t = base[s] as usize + byte as usize;
            Self::ensure_capacity(base, check, t);

            if check[t] == -1 {
                check[t] = s as i64;
                base[t] = 1;
                s = t;
                i += 1;
            } else if check[t] == s as i64 {
                s = t;
                i += 1;
            } else {
                let old_base = base[s];
                let mut children: Vec<u8> = Vec::new();
                for c in 0u8..=255u8 {
                    let ct = old_base as usize + c as usize;
                    if ct < check.len() && check[ct] == s as i64 {
                        children.push(c);
                    }
                }
                if !children.contains(&byte) {
                    children.push(byte);
                }

                let new_base = Self::find_base(base, check, &children, cursor);

                for &c in &children {
                    if c == byte {
                        continue;
                    }
                    let old_t = old_base as usize + c as usize;
                    let new_t = new_base as usize + c as usize;
                    if base[old_t] > 0 {
                        for gk in 0u8..=255u8 {
                            let gk_old = base[old_t] as usize + gk as usize;
                            if gk_old < check.len() && check[gk_old] == old_t as i64 {
                                let gk_new = base[new_t] as usize + gk as usize;
                                Self::ensure_capacity(base, check, gk_new);
                                base[gk_new] = base[gk_old];
                                check[gk_new] = new_t as i64;
                                // Mark old grandchild position as free
                                check[gk_old] = -1;
                                base[gk_old] = 0;
                            }
                        }
                    }
                }

                for &c in &children {
                    if c == byte {
                        continue;
                    }
                    let old_t = old_base as usize + c as usize;
                    let new_t = new_base as usize + c as usize;
                    Self::ensure_capacity(base, check, new_t);
                    base[new_t] = base[old_t];
                    check[new_t] = s as i64;
                    // Mark old child position as free (after grandchildren moved)
                    check[old_t] = -1;
                    base[old_t] = 0;
                }

                base[s] = new_base;
            }
        }

        if base[s] < 0 {
            let old_payload = (-base[s] - 1) as usize;
            base[s] = 0;
            let new_base = Self::find_base(base, check, &[0u8, 1u8], cursor);
            base[s] = new_base;
            Self::ensure_capacity(base, check, new_base as usize + 1);
            base[new_base as usize] = -(old_payload as i64 + 1);
            check[new_base as usize] = s as i64;
            base[new_base as usize + 1] = -(payload_id as i64 + 1);
            check[new_base as usize + 1] = s as i64;
        } else if base[s] > 0 {
            let old_base = base[s];
            let mut slot = 0u32;
            loop {
                let ct = old_base as usize + slot as usize;
                if ct >= check.len() || check[ct] == -1 {
                    break;
                }
                slot += 1;
            }
            let children = if slot == 0 { vec![0u8] } else { vec![slot as u8] };
            let new_base = Self::find_base(base, check, &children, cursor);
            if new_base != old_base {
                let mut all_children: Vec<u8> = Vec::new();
                for c in 0u8..=255u8 {
                    let ct = old_base as usize + c as usize;
                    if ct < check.len() && check[ct] == s as i64 {
                        all_children.push(c);
                    }
                }
                for &c in &all_children {
                    let old_t = old_base as usize + c as usize;
                    let new_t = new_base as usize + c as usize;
                    if base[old_t] > 0 {
                        for gk in 0u8..=255u8 {
                            let gk_old = base[old_t] as usize + gk as usize;
                            if gk_old < check.len() && check[gk_old] == old_t as i64 {
                                let gk_new = base[new_t] as usize + gk as usize;
                                Self::ensure_capacity(base, check, gk_new);
                                base[gk_new] = base[gk_old];
                                check[gk_new] = new_t as i64;
                                check[gk_old] = -1;
                                base[gk_old] = 0;
                            }
                        }
                    }
                }
                for &c in &all_children {
                    let old_t = old_base as usize + c as usize;
                    let new_t = new_base as usize + c as usize;
                    Self::ensure_capacity(base, check, new_t);
                    base[new_t] = base[old_t];
                    check[new_t] = s as i64;
                    check[old_t] = -1;
                    base[old_t] = 0;
                }
                base[s] = new_base;
            }
            let term_t = base[s] as usize + slot as usize;
            Self::ensure_capacity(base, check, term_t);
            base[term_t] = -(payload_id as i64 + 1);
            check[term_t] = s as i64;
        } else {
            base[s] = -(payload_id as i64 + 1);
        }
    }

    #[allow(dead_code)]
    pub fn lookup(&self, key: &str) -> Vec<usize> {
        let mut results = Vec::new();
        let bytes = key.as_bytes();
        let mut s = ROOT;

        for &byte in bytes.iter() {
            if self.base[s] < 0 {
                return results;
            }
            let t = self.base[s] as usize + byte as usize;
            if t >= self.check.len() || self.check[t] != s as i64 {
                return results;
            }
            s = t;
        }

        self.collect_leaves(s, &mut results);
        results
    }

    pub fn len(&self) -> usize {
        self.base.len()
    }

    pub fn serialize<W: std::io::Write>(&self, writer: &mut W, entries: &[DictEntry]) -> std::io::Result<()> {
        let num_entries = entries.len() as u32;
        let trie_byte_size = (self.base.len() * 8 * 2) as u64;

        writer.write_all(b"IBUSICE")?;
        writer.write_all(&1u32.to_le_bytes())?;
        writer.write_all(&num_entries.to_le_bytes())?;
        writer.write_all(&64u64.to_le_bytes())?;
        writer.write_all(&(64 + trie_byte_size).to_le_bytes())?;
        let padding = [0u8; 33];
        writer.write_all(&padding)?;

        for &b in &self.base {
            writer.write_all(&b.to_le_bytes())?;
        }
        for &c in &self.check {
            writer.write_all(&c.to_le_bytes())?;
        }

        let _offset_table_size = (num_entries as usize) * 4;
        let mut payload_buf: Vec<u8> = Vec::new();
        let mut offsets: Vec<u32> = Vec::new();
        let mut current_offset: u32 = 0;

        for entry in entries {
            offsets.push(current_offset);
            let text_bytes = entry.text.as_bytes();
            let text_len = text_bytes.len() as u16;
            payload_buf.extend_from_slice(&text_len.to_le_bytes());
            payload_buf.extend_from_slice(text_bytes);
            payload_buf.extend_from_slice(&entry.freq.to_le_bytes());
            let word_len = entry.text.chars().count() as u8;
            payload_buf.push(word_len);
            current_offset += 2 + text_bytes.len() as u32 + 5;
        }

        for &off in &offsets {
            writer.write_all(&off.to_le_bytes())?;
        }
        writer.write_all(&payload_buf)?;

        Ok(())
    }

    #[allow(dead_code)]
    fn collect_leaves(&self, node: usize, results: &mut Vec<usize>) {
        if self.base[node] <= 0 {
            let payload_id = (-self.base[node] - 1) as usize;
            if !results.contains(&payload_id) {
                results.push(payload_id);
            }
        }
        if self.base[node] > 0 {
            let base_val = self.base[node];
            for c in 0u8..=255u8 {
                let t = base_val as usize + c as usize;
                if t < self.check.len() && self.check[t] == node as i64 {
                    self.collect_leaves(t, results);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DictEntry;

    fn make_entry(text: &str, pinyin: &str, freq: u32) -> DictEntry {
        DictEntry {
            text: text.to_string(),
            pinyin: pinyin.split_whitespace().map(|s| s.to_string()).collect(),
            freq,
        }
    }

    #[test]
    fn test_build_and_lookup() {
        let entries = vec![
            make_entry("中", "zhong", 100),
            make_entry("中国", "zhong guo", 200),
            make_entry("种", "zhong", 50),
        ];
        let trie = DoubleArrayTrie::build(&entries);

        let ids = trie.lookup("zhong");
        assert!(ids.contains(&0));
        assert!(ids.contains(&2));
    }

    #[test]
    fn test_exact_match_found() {
        let entries = vec![
            make_entry("中国", "zhong guo", 200),
        ];
        let trie = DoubleArrayTrie::build(&entries);
        let ids = trie.lookup("zhong guo");
        assert_eq!(ids, vec![0]);
    }

    #[test]
    fn test_lookup_miss() {
        let entries = vec![
            make_entry("中", "zhong", 100),
        ];
        let trie = DoubleArrayTrie::build(&entries);
        let ids = trie.lookup("abc");
        assert!(ids.is_empty());
    }

    #[test]
    fn test_roundtrip_serialize() {
        let entries = vec![
            make_entry("中国", "zhong guo", 200),
        ];
        let trie = DoubleArrayTrie::build(&entries);

        let mut buf = Vec::new();
        trie.serialize(&mut buf, &entries).unwrap();

        assert_eq!(&buf[0..7], b"IBUSICE");
    }
}
