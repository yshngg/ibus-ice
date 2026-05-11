use crate::DictEntry;

pub struct DoubleArrayTrie {
    base: Vec<i32>,
    check: Vec<i32>,
}

const ROOT: usize = 0;
const INITIAL_SIZE: usize = 1024;

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

        let mut base = vec![0i32; INITIAL_SIZE];
        let mut check = vec![-1i32; INITIAL_SIZE];

        for (key, payload_id) in &keys {
            Self::insert(&mut base, &mut check, key, *payload_id as usize);
        }

        DoubleArrayTrie { base, check }
    }

    fn ensure_capacity(base: &mut Vec<i32>, check: &mut Vec<i32>, idx: usize) {
        if idx >= base.len() {
            let new_size = (idx + 256).next_power_of_two();
            base.resize(new_size, 0);
            check.resize(new_size, -1);
        }
    }

    fn insert(base: &mut Vec<i32>, check: &mut Vec<i32>, key: &[u8], payload_id: usize) {
        let mut s = ROOT;
        let mut i = 0;

        while i < key.len() {
            let byte = key[i];

            if base[s] < 0 {
                let saved_payload = (-base[s] - 1) as usize;
                base[s] = 0;

                let children = if 0 == byte { vec![0u8] } else { vec![0u8, byte] };
                let new_base = Self::find_base(base, check, &children);
                base[s] = new_base;

                let term_t = new_base as usize;
                Self::ensure_capacity(base, check, term_t);
                base[term_t] = -(saved_payload as i32 + 1);
                check[term_t] = s as i32;

                continue;
            }

            let t = base[s] as usize + byte as usize;
            Self::ensure_capacity(base, check, t);

            if check[t] == -1 {
                check[t] = s as i32;
                base[t] = 1;
                s = t;
                i += 1;
            } else if check[t] == s as i32 {
                s = t;
                i += 1;
            } else {
                let old_base = base[s];
                let mut children: Vec<u8> = Vec::new();
                for c in 0u8..=255u8 {
                    let ct = old_base as usize + c as usize;
                    if ct < check.len() && check[ct] == s as i32 {
                        children.push(c);
                    }
                }
                if !children.contains(&byte) {
                    children.push(byte);
                }

                let new_base = Self::find_base(base, check, &children);

                for &c in &children {
                    if c == byte {
                        continue;
                    }
                    let old_t = old_base as usize + c as usize;
                    let new_t = new_base as usize + c as usize;
                    Self::ensure_capacity(base, check, new_t);
                    base[new_t] = base[old_t];
                    check[new_t] = s as i32;
                }

                for &c in &children {
                    if c == byte {
                        continue;
                    }
                    let old_t = old_base as usize + c as usize;
                    let new_t = new_base as usize + c as usize;
                    if base[old_t] > 0 {
                        let mut grandkids: Vec<u8> = Vec::new();
                        for gk in 0u8..=255u8 {
                            let gk_old = base[old_t] as usize + gk as usize;
                            if gk_old < check.len() && check[gk_old] == old_t as i32 {
                                grandkids.push(gk);
                            }
                        }
                        for &gk in &grandkids {
                            let gk_old = base[old_t] as usize + gk as usize;
                            let gk_new = base[new_t] as usize + gk as usize;
                            Self::ensure_capacity(base, check, gk_new);
                            base[gk_new] = base[gk_old];
                            check[gk_new] = new_t as i32;
                        }
                    }
                }

                base[s] = new_base;
            }
        }

        // Store payload. Chain duplicates if a leaf already exists at this node.
        if base[s] < 0 {
            let old_payload = (-base[s] - 1) as usize;
            base[s] = 0;
            let new_base = Self::find_base(base, check, &[0u8, 1u8]);
            base[s] = new_base;
            Self::ensure_capacity(base, check, new_base as usize + 1);
            base[new_base as usize] = -(old_payload as i32 + 1);
            check[new_base as usize] = s as i32;
            base[new_base as usize + 1] = -(payload_id as i32 + 1);
            check[new_base as usize + 1] = s as i32;
        } else if base[s] > 0 {
            let old_base = base[s];
            let mut slot = 0u8;
            loop {
                let ct = old_base as usize + slot as usize;
                if ct >= check.len() || check[ct] == -1 {
                    break;
                }
                slot += 1;
            }
            let children = if slot == 0 { vec![0u8] } else { vec![slot] };
            let new_base = Self::find_base(base, check, &children);
            if new_base != old_base {
                let mut all_children: Vec<u8> = Vec::new();
                for c in 0u8..=255u8 {
                    let ct = old_base as usize + c as usize;
                    if ct < check.len() && check[ct] == s as i32 {
                        all_children.push(c);
                    }
                }
                for &c in &all_children {
                    let old_t = old_base as usize + c as usize;
                    let new_t = new_base as usize + c as usize;
                    Self::ensure_capacity(base, check, new_t);
                    base[new_t] = base[old_t];
                    check[new_t] = s as i32;
                    if base[old_t] > 0 {
                        let mut grandkids: Vec<u8> = Vec::new();
                        for gk in 0u8..=255u8 {
                            let gk_old = base[old_t] as usize + gk as usize;
                            if gk_old < check.len() && check[gk_old] == old_t as i32 {
                                grandkids.push(gk);
                            }
                        }
                        for &gk in &grandkids {
                            let gk_old = base[old_t] as usize + gk as usize;
                            let gk_new = base[new_t] as usize + gk as usize;
                            Self::ensure_capacity(base, check, gk_new);
                            base[gk_new] = base[gk_old];
                            check[gk_new] = new_t as i32;
                        }
                    }
                }
                base[s] = new_base;
            }
            let term_t = base[s] as usize + slot as usize;
            Self::ensure_capacity(base, check, term_t);
            base[term_t] = -(payload_id as i32 + 1);
            check[term_t] = s as i32;
        } else {
            base[s] = -(payload_id as i32 + 1);
        }
    }

    fn find_base(_base: &[i32], check: &[i32], children: &[u8]) -> i32 {
        let mut b = 1i32;
        loop {
            let mut ok = true;
            for &c in children {
                let t = b as usize + c as usize;
                if t < check.len() && check[t] != -1 {
                    ok = false;
                    break;
                }
            }
            if ok {
                return b;
            }
            b += 1;
        }
    }

    pub fn lookup(&self, key: &str) -> Vec<usize> {
        let mut results = Vec::new();
        let bytes = key.as_bytes();
        let mut s = ROOT;

        for &byte in bytes.iter() {
            if self.base[s] < 0 {
                return results;
            }
            let t = self.base[s] as usize + byte as usize;
            if t >= self.check.len() || self.check[t] != s as i32 {
                return results;
            }
            s = t;
        }

        self.collect_leaves(s, &mut results);
        results
    }

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
                if t < self.check.len() && self.check[t] == node as i32 {
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
        assert!(ids.contains(&0)); // "中"
        assert!(ids.contains(&2)); // "种"
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
}
