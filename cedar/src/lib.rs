//! Double-array trie (DAT). Ported from cedarwood, optimized for
//! build-once / query-many. Zero dependencies.

use std::fmt;

// ---- data structures ----

#[derive(Debug, Default, Clone, Copy)]
struct Cell {
    base: i32,
    check: i32,
}

#[derive(Debug, Default, Clone, Copy)]
struct NInfo {
    sibling: u8,
    child: u8,
}

#[derive(Debug, Clone)]
struct Block {
    prev: i32,
    next: i32,
    num: i16,
    e_head: i32,
}

impl Block {
    fn new() -> Self {
        Block { prev: 0, next: 0, num: 256, e_head: 0 }
    }
}

impl Default for Block {
    fn default() -> Self { Self::new() }
}

enum BlockType { Open, Closed, Full }

#[derive(Clone)]
pub struct Cedar {
    array: Vec<Cell>,
    n_infos: Vec<NInfo>,
    blocks: Vec<Block>,
    blocks_head_full: i32,
    blocks_head_closed: i32,
    blocks_head_open: i32,
    capacity: usize,
    size: usize,
}

impl fmt::Debug for Cedar {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Cedar(size={})", self.size)
    }
}

impl Default for Cedar {
    fn default() -> Self { Self::new() }
}

// ---- public API ----

impl Cedar {
    pub fn new() -> Self {
        let mut array: Vec<Cell> = Vec::with_capacity(256);
        let n_infos: Vec<NInfo> = vec![NInfo::default(); 256];
        let mut blocks: Vec<Block> = vec![Block::new(); 1];

        array.push(Cell { base: 0, check: -1 }); // root (index 0)
        for i in 1..256 {
            array.push(Cell {
                base: -(i - 1),
                check: -(i + 1),
            });
        }
        // make them link as a cyclic doubly-linked list
        array[1].base = -255;
        array[255].check = -1;

        blocks[0].e_head = 1;

        Cedar {
            array,
            n_infos,
            blocks,
            blocks_head_full: 0,
            blocks_head_closed: 0,
            blocks_head_open: 0,
            capacity: 256,
            size: 256,
        }
    }

    pub fn build(&mut self, key_values: &[(&str, i32)]) {
        for (key, value) in key_values {
            self.update_(key.as_bytes(), *value, 0, 0);
        }
    }

    pub fn common_prefix_predict(&self, key: &str) -> Option<Vec<(i32, usize)>> {
        let key = key.as_bytes();
        let mut from: usize = 0;

        if self.find(key, &mut from) {
            let root = from;
            let mut results = Vec::new();

            let (first_val, from_pos, p) = self.begin(from, 0);
            if let Some(first) = first_val {
                results.push((first, p + key.len()));
                let mut f = from_pos;
                let mut pos = p;
                loop {
                    let (v2, f2, p2) = self.next(f, pos, root);
                    match v2 {
                        Some(val) => {
                            results.push((val, p2 + key.len()));
                            f = f2;
                            pos = p2;
                        }
                        None => break,
                    }
                }
            }

            if results.is_empty() { None } else { Some(results) }
        } else {
            None
        }
    }
}

// ---- insert engine ----

impl Cedar {
    fn update_(&mut self, key: &[u8], value: i32, mut from: usize, mut pos: usize) {
        if from == 0 && key.is_empty() {
            panic!("zero-length key");
        }

        while pos < key.len() {
            from = self.follow(from, key[pos]) as usize;
            pos += 1;
        }

        let to = self.follow(from, 0);

        self.array[to as usize].base = value;
    }

    #[inline]
    fn follow(&mut self, from: usize, label: u8) -> i32 {
        let base = self.array[from].base;

        let mut to;

        // the node is not there
        if base < 0 || self.array[(base ^ i32::from(label)) as usize].check < 0 {
            // allocate an e node
            to = self.pop_e_node(base, label, from as i32);
            let branch: i32 = to ^ i32::from(label);

            // maintain the info in ninfo
            self.push_sibling(from, branch, label, base >= 0);
        } else {
            // the node is already there and the ownership is not `from`, therefore a conflict.
            to = base ^ i32::from(label);
            if self.array[to as usize].check != (from as i32) {
                // call `resolve` to relocate.
                to = self.resolve(from, base, label);
            }
        }

        to
    }

    /// Walk the trie for `key`. Returns true if the prefix exists,
    /// placing the terminal position in `from`.
    #[inline]
    fn find(&self, key: &[u8], from: &mut usize) -> bool {
        let mut pos = 0;

        while pos < key.len() {
            let to = (self.array[*from].base ^ i32::from(key[pos])) as usize;
            if self.array[to].check != (*from as i32) {
                return false;
            }

            *from = to;
            pos += 1;
        }

        true
    }

    /// To get the cursor of the first leaf node starting by `from`
    #[inline]
    fn begin(&self, mut from: usize, mut p: usize) -> (Option<i32>, usize, usize) {
        let mut c = self.n_infos[from].child;

        if from == 0 {
            let base = self.array[0].base;
            c = self.n_infos[(base ^ i32::from(c)) as usize].sibling;

            if c == 0 {
                return (None, from, p);
            }
        }

        while c != 0 {
            from = (self.array[from].base ^ i32::from(c)) as usize;
            c = self.n_infos[from].child;
            p += 1;
        }

        let v = self.array[self.array[from].base as usize].base;
        (Some(v), from, p)
    }

    /// To move the cursor from one leaf to the next
    #[inline]
    fn next(&self, mut from: usize, mut p: usize, root: usize) -> (Option<i32>, usize, usize) {
        let mut c: u8 = {
            let base = self.array[from].base;
            self.n_infos[base as usize].sibling
        };

        while c == 0 && from != root {
            c = self.n_infos[from].sibling;
            from = self.array[from].check as usize;

            p -= 1;
        }

        if c != 0 {
            from = (self.array[from].base ^ i32::from(c)) as usize;
            self.begin(from, p + 1)
        } else {
            (None, from, p)
        }
    }

    fn resolve(&mut self, mut from_n: usize, base_n: i32, label_n: u8) -> i32 {
        let to_pn = base_n ^ i32::from(label_n);

        // the `base` and `from` for the conflicting one.
        let from_p = self.array[to_pn as usize].check;
        let base_p = self.array[from_p as usize].base;

        // whether to replace siblings of newly added
        let flag = self.consult(
            base_n,
            base_p,
            self.n_infos[from_n].child,
            self.n_infos[from_p as usize].child,
        );

        // collect the list of children for the block that we are going to relocate.
        let children = if flag {
            self.set_child(base_n, self.n_infos[from_n].child, label_n, true)
        } else {
            self.set_child(base_p, self.n_infos[from_p as usize].child, 255, false)
        };

        // decide which algorithm to allocate free block depending on the number of children
        let mut base = if children.len() == 1 {
            self.find_place()
        } else {
            self.find_places(&children)
        };

        base ^= i32::from(children[0]);

        let (from, base_) = if flag {
            (from_n as i32, base_n)
        } else {
            (from_p, base_p)
        };

        if flag && children[0] == label_n {
            self.n_infos[from as usize].child = label_n;
        }

        self.array[from as usize].base = base;

        // the actual work for relocating the children
        for i in 0..(children.len()) {
            let to = self.pop_e_node(base, children[i], from);
            let to_ = base_ ^ i32::from(children[i]);

            if i == children.len() - 1 {
                self.n_infos[to as usize].sibling = 0;
            } else {
                self.n_infos[to as usize].sibling = children[i + 1];
            }

            if flag && to_ == to_pn {
                continue;
            }

            self.array[to as usize].base = self.array[to_ as usize].base;

            if self.array[to as usize].base > 0 && children[i] != 0 {
                let mut c = self.n_infos[to_ as usize].child;

                self.n_infos[to as usize].child = c;

                loop {
                    let idx = (self.array[to as usize].base ^ i32::from(c)) as usize;
                    self.array[idx].check = to;
                    c = self.n_infos[idx].sibling;

                    if c == 0 {
                        break;
                    }
                }
            }

            if !flag && to_ == (from_n as i32) {
                from_n = to as usize;
            }

            // clean up the space that was moved away from.
            if !flag && to_ == to_pn {
                self.push_sibling(from_n, to_pn ^ i32::from(label_n), label_n, true);
                self.n_infos[to_ as usize].child = 0;

                if label_n != 0 {
                    self.array[to_ as usize].base = -1;
                } else {
                    self.array[to_ as usize].base = 0;
                }

                self.array[to_ as usize].check = from_n as i32;
            } else {
                self.push_e_node(to_);
            }
        }

        // return the position that is free now.
        if flag {
            base ^ i32::from(label_n)
        } else {
            to_pn
        }
    }

    /// Loop through the siblings to see which one reached the end first, which means it is the one
    /// with smaller children size, and we should try to relocate the smaller one.
    fn consult(&self, base_n: i32, base_p: i32, mut c_n: u8, mut c_p: u8) -> bool {
        loop {
            c_n = self.n_infos[(base_n ^ i32::from(c_n)) as usize].sibling;
            c_p = self.n_infos[(base_p ^ i32::from(c_p)) as usize].sibling;

            if !(c_n != 0 && c_p != 0) {
                break;
            }
        }

        c_p != 0
    }

    /// Collect the list of the children, and push the label as well if it is not terminal node.
    fn set_child(&self, base: i32, mut c: u8, label: u8, not_terminal: bool) -> Vec<u8> {
        let mut child: Vec<u8> = Vec::new();

        if c == 0 {
            child.push(c);
            c = self.n_infos[(base ^ i32::from(c)) as usize].sibling;
        }

        if not_terminal {
            child.push(label);
        }

        while c != 0 {
            child.push(c);
            c = self.n_infos[(base ^ i32::from(c)) as usize].sibling;
        }

        child
    }

    /// Push the `label` into the sibling chain
    fn push_sibling(&mut self, from: usize, base: i32, label: u8, has_child: bool) {
        // Unordered: keep_order is true only for the first child (child == 0)
        let keep_order: bool = self.n_infos[from].child == 0;

        let sibling: u8;
        {
            let mut c: &mut u8 = &mut self.n_infos[from].child;
            if has_child && keep_order {
                let code = i32::from(*c);
                c = &mut self.n_infos[(base ^ code) as usize].sibling;
            }
            sibling = *c;
            *c = label;
        }

        self.n_infos[(base ^ i32::from(label)) as usize].sibling = sibling;
    }
}

// ---- free-space management ----

impl Cedar {
    fn find_place(&mut self) -> i32 {
        if self.blocks_head_closed != 0 {
            return self.blocks[self.blocks_head_closed as usize].e_head;
        }

        if self.blocks_head_open != 0 {
            return self.blocks[self.blocks_head_open as usize].e_head;
        }

        self.add_block() << 8
    }

    fn find_places(&mut self, child: &[u8]) -> i32 {
        let mut idx = self.blocks_head_open;

        if idx != 0 {
            let bz = self.blocks[self.blocks_head_open as usize].prev;
            let nc = child.len() as i16;

            loop {
                if self.blocks[idx as usize].num >= nc {
                    let mut e = self.blocks[idx as usize].e_head;
                    loop {
                        let base = e ^ i32::from(child[0]);

                        let mut i = 1;
                        while self.array[(base ^ i32::from(child[i])) as usize].check < 0 {
                            if i == child.len() - 1 {
                                self.blocks[idx as usize].e_head = e;
                                return e;
                            }
                            i += 1;
                        }

                        e = -self.array[e as usize].check;
                        if e == self.blocks[idx as usize].e_head {
                            break;
                        }
                    }
                }

                if idx == bz {
                    break;
                }

                idx = self.blocks[idx as usize].next;
            }
        }

        self.add_block() << 8
    }

    fn add_block(&mut self) -> i32 {
        if self.size == self.capacity {
            self.capacity += self.capacity;

            self.array.resize(self.capacity, Cell::default());
            self.n_infos.resize(self.capacity, NInfo::default());
            self.blocks.resize(self.capacity >> 8, Block::new());
        }

        self.blocks[self.size >> 8].e_head = self.size as i32;

        // make it a doubly linked list
        self.array[self.size] = Cell {
            base: -((self.size as i32) + 255),
            check: -((self.size as i32) + 1),
        };

        for i in (self.size + 1)..(self.size + 255) {
            self.array[i] = Cell {
                base: -(i as i32 - 1),
                check: -(i as i32 + 1),
            };
        }

        self.array[self.size + 255] = Cell {
            base: -((self.size as i32) + 254),
            check: -(self.size as i32),
        };

        let is_empty = self.blocks_head_open == 0;
        let idx = (self.size >> 8) as i32;
        self.push_block(idx, BlockType::Open, is_empty);

        self.size += 256;

        ((self.size >> 8) - 1) as i32
    }

    fn pop_e_node(&mut self, base: i32, label: u8, from: i32) -> i32 {
        let e: i32 = if base < 0 {
            self.find_place()
        } else {
            base ^ i32::from(label)
        };

        let idx = e >> 8;
        let n_base = self.array[e as usize].base;
        let n_check = self.array[e as usize].check;

        self.blocks[idx as usize].num -= 1;
        // move the block at idx to the correct linked-list depending on free slots it still has.
        if self.blocks[idx as usize].num == 0 {
            if idx != 0 {
                self.transfer_block(
                    idx,
                    BlockType::Closed,
                    BlockType::Full,
                    self.blocks_head_full == 0,
                );
            }
        } else {
            self.array[(-n_base) as usize].check = n_check;
            self.array[(-n_check) as usize].base = n_base;

            if e == self.blocks[idx as usize].e_head {
                self.blocks[idx as usize].e_head = -n_check;
            }

            if idx != 0 && self.blocks[idx as usize].num == 1 {
                self.transfer_block(
                    idx,
                    BlockType::Open,
                    BlockType::Closed,
                    self.blocks_head_closed == 0,
                );
            }
        }

        if label != 0 {
            self.array[e as usize].base = -1;
        } else {
            self.array[e as usize].base = 0;
        }
        self.array[e as usize].check = from;
        if base < 0 {
            self.array[from as usize].base = e ^ i32::from(label);
        }

        e
    }

    fn push_e_node(&mut self, e: i32) {
        let idx = e >> 8;
        self.blocks[idx as usize].num += 1;

        if self.blocks[idx as usize].num == 1 {
            self.blocks[idx as usize].e_head = e;
            self.array[e as usize] = Cell { base: -e, check: -e };

            if idx != 0 {
                // Move the block from 'Full' to 'Closed' since it has one free slot now.
                self.transfer_block(
                    idx,
                    BlockType::Full,
                    BlockType::Closed,
                    self.blocks_head_closed == 0,
                );
            }
        } else {
            let prev = self.blocks[idx as usize].e_head;

            let next = -self.array[prev as usize].check;

            // Insert to the edge immediately after the e_head
            self.array[e as usize] = Cell {
                base: -prev,
                check: -next,
            };

            self.array[prev as usize].check = -e;
            self.array[next as usize].base = -e;

            // Move the block from 'Closed' to 'Open' since it has more than one free slot now.
            if self.blocks[idx as usize].num == 2 && idx != 0 {
                self.transfer_block(
                    idx,
                    BlockType::Closed,
                    BlockType::Open,
                    self.blocks_head_open == 0,
                );
            }
        }

        self.n_infos[e as usize] = NInfo::default();
    }

    fn pop_block(&mut self, idx: i32, from: BlockType, last: bool) {
        let head: &mut i32 = match from {
            BlockType::Open => &mut self.blocks_head_open,
            BlockType::Closed => &mut self.blocks_head_closed,
            BlockType::Full => &mut self.blocks_head_full,
        };

        if last {
            *head = 0;
        } else {
            let b_prev = self.blocks[idx as usize].prev;
            let b_next = self.blocks[idx as usize].next;
            self.blocks[b_prev as usize].next = b_next;
            self.blocks[b_next as usize].prev = b_prev;

            if idx == *head {
                *head = b_next;
            }
        }
    }

    fn push_block(&mut self, idx: i32, to: BlockType, empty: bool) {
        let head: &mut i32 = match to {
            BlockType::Open => &mut self.blocks_head_open,
            BlockType::Closed => &mut self.blocks_head_closed,
            BlockType::Full => &mut self.blocks_head_full,
        };

        if empty {
            self.blocks[idx as usize].next = idx;
            self.blocks[idx as usize].prev = idx;
            *head = idx;
        } else {
            self.blocks[idx as usize].prev = self.blocks[*head as usize].prev;
            self.blocks[idx as usize].next = *head;

            let t = self.blocks[*head as usize].prev;
            self.blocks[t as usize].next = idx;
            self.blocks[*head as usize].prev = idx;
            *head = idx;
        }
    }

    fn transfer_block(&mut self, idx: i32, from: BlockType, to: BlockType, to_empty: bool) {
        let is_last = idx == self.blocks[idx as usize].next;
        let is_empty = to_empty && (self.blocks[idx as usize].num != 0);

        self.pop_block(idx, from, is_last);
        self.push_block(idx, to, is_empty);
    }
}

// ---- tests ----

#[cfg(test)]
mod tests {
    use super::*;

    fn kv<'a>(keys: &'a [&'a str]) -> Vec<(&'a str, i32)> {
        keys.iter().enumerate().map(|(i, k)| (*k, i as i32)).collect()
    }

    // ---- smoke ----

    #[test]
    fn empty_dict_returns_none() {
        let cedar = Cedar::new();
        assert_eq!(cedar.common_prefix_predict("abc"), None);
    }

    #[test]
    fn single_key_exact_match() {
        let mut cedar = Cedar::new();
        cedar.build(&kv(&["hello"]));
        let r = cedar.common_prefix_predict("hello").unwrap();
        assert_eq!(r, vec![(0, 5)]);
    }

    #[test]
    fn multiple_keys_common_prefix() {
        let mut cedar = Cedar::new();
        cedar.build(&kv(&["a", "ab", "abc"]));
        let r = cedar.common_prefix_predict("a").unwrap();
        let vals: Vec<i32> = r.iter().map(|x| x.0).collect();
        assert_eq!(vals, vec![0, 1, 2]);
    }

    #[test]
    fn prefix_not_found() {
        let mut cedar = Cedar::new();
        cedar.build(&kv(&["abc"]));
        assert_eq!(cedar.common_prefix_predict("xyz"), None);
    }

    #[test]
    fn prefix_shorter_than_key_is_found() {
        let mut cedar = Cedar::new();
        cedar.build(&kv(&["hello"]));
        let r = cedar.common_prefix_predict("hel").unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].0, 0);
    }

    // ---- conflict / resolve ----

    #[test]
    fn conflicting_prefixes_resolve() {
        let mut cedar = Cedar::new();
        cedar.build(&kv(&["ab", "ac", "ad", "ae", "af"]));
        for key in &["ab", "ac", "ad", "ae", "af"] {
            let r = cedar.common_prefix_predict(key).unwrap();
            assert_eq!(r.len(), 1);
        }
    }

    #[test]
    fn deep_conflict_chain() {
        let mut cedar = Cedar::new();
        let keys: Vec<String> = (0..50)
            .map(|i| {
                let c = (b'a' + (i % 26) as u8) as char;
                format!("prefix_{i:02}{c}")
            })
            .collect();
        let key_strs: Vec<&str> = keys.iter().map(|s| s.as_str()).collect();
        cedar.build(&kv(&key_strs));
        for (i, key) in keys.iter().enumerate() {
            let r = cedar.common_prefix_predict(key).unwrap();
            assert_eq!(r.len(), 1, "expected exactly one result for key {key}, got {}", r.len());
            assert_eq!(r[0].0, i as i32);
        }
    }

    // ---- expansion ----

    #[test]
    fn array_expansion() {
        let keys: Vec<String> = (0..600).map(|i| format!("key_{i:04}")).collect();
        let key_strs: Vec<&str> = keys.iter().map(|s| s.as_str()).collect();
        let mut cedar = Cedar::new();
        cedar.build(&kv(&key_strs));
        for key in &keys {
            let r = cedar.common_prefix_predict(key).unwrap();
            assert_eq!(r.len(), 1);
        }
    }

    // ---- unicode ----

    #[test]
    fn cjk_keys() {
        let mut cedar = Cedar::new();
        cedar.build(&kv(&["中国", "中华人民共和国", "中华"]));
        let r = cedar.common_prefix_predict("中").unwrap();
        let mut vals: Vec<i32> = r.iter().map(|x| x.0).collect();
        vals.sort();
        assert_eq!(vals, vec![0, 1, 2]);
    }

    #[test]
    fn pinyin_style_keys() {
        let mut cedar = Cedar::new();
        cedar.build(&kv(&["zhong", "zhong guo", "zhong guo ren"]));
        let r = cedar.common_prefix_predict("zhong").unwrap();
        let vals: Vec<i32> = r.iter().map(|x| x.0).collect();
        assert_eq!(vals, vec![0, 1, 2]);
    }

    #[test]
    fn pinyin_prefix_with_space() {
        let mut cedar = Cedar::new();
        cedar.build(&kv(&["zhong", "zhong guo", "zhong guo ren"]));
        let r = cedar.common_prefix_predict("zhong ").unwrap();
        let vals: Vec<i32> = r.iter().map(|x| x.0).collect();
        assert_eq!(vals, vec![1, 2]);
    }

    // ---- integration ----

    #[test]
    fn x01_separator_pattern() {
        let keys: Vec<String> = vec![
            format!("zhong\x010"),
            format!("zhong guo\x011"),
            format!("zhong\x012"),
        ];
        let kv: Vec<(&str, i32)> = keys
            .iter()
            .enumerate()
            .map(|(i, k)| (k.as_str(), i as i32))
            .collect();
        let mut cedar = Cedar::new();
        cedar.build(&kv);
        let r = cedar.common_prefix_predict("zhong").unwrap();
        let ids: Vec<i32> = r.iter().map(|(v, _)| *v).collect();
        assert!(ids.contains(&0));
        assert!(ids.contains(&2));
    }

    #[test]
    fn duplicate_key_last_wins() {
        let mut cedar = Cedar::new();
        cedar.build(&kv(&["dup", "dup"]));
        let r = cedar.common_prefix_predict("dup").unwrap();
        assert_eq!(r[0].0, 1);
    }

    #[test]
    #[should_panic(expected = "zero-length key")]
    fn empty_key_panics() {
        let mut cedar = Cedar::new();
        cedar.build(&[("", 0)]);
    }

    #[test]
    fn rebuild_does_not_crash() {
        let mut cedar = Cedar::new();
        cedar.build(&kv(&["a", "b", "c"]));
        cedar.build(&kv(&["d", "e", "f"]));
        let r = cedar.common_prefix_predict("d").unwrap();
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn empty_string_query() {
        let mut cedar = Cedar::new();
        cedar.build(&kv(&["a", "ab", "abc"]));
        let r = cedar.common_prefix_predict("").unwrap();
        let vals: Vec<i32> = r.iter().map(|x| x.0).collect();
        assert_eq!(vals, vec![0, 1, 2]);
    }

    #[test]
    fn common_prefix_predict_empty_trie() {
        let cedar = Cedar::new();
        assert_eq!(cedar.common_prefix_predict(""), None);
    }

    #[test]
    fn many_single_char_keys() {
        // Insert keys a..z to fill many slots in the same block
        let chars: Vec<String> = (b'a'..=b'z').map(|c| String::from(c as char)).collect();
        let str_chars: Vec<&str> = chars.iter().map(|s| s.as_str()).collect();
        let mut cedar = Cedar::new();
        cedar.build(&kv(&str_chars));
        for (i, c) in chars.iter().enumerate() {
            let r = cedar.common_prefix_predict(c).unwrap();
            assert_eq!(r.len(), 1, "key {c}");
            assert_eq!(r[0].0, i as i32);
        }
    }

    #[test]
    fn two_level_trie_test() {
        // "aa", "ab", "ac" — share "a" prefix, diverge on second char
        let mut cedar = Cedar::new();
        cedar.build(&kv(&["aa", "ab", "ac"]));
        let r = cedar.common_prefix_predict("a").unwrap();
        let mut vals: Vec<i32> = r.iter().map(|x| x.0).collect();
        vals.sort();
        assert_eq!(vals, vec![0, 1, 2]);
    }

    #[test]
    fn deep_linear_chain() {
        // "a", "aa", "aaa", "aaaa", "aaaaa" — purely linear, no branches
        let mut cedar = Cedar::new();
        cedar.build(&kv(&["a", "aa", "aaa", "aaaa", "aaaaa"]));
        let r = cedar.common_prefix_predict("aa").unwrap();
        let vals: Vec<i32> = r.iter().map(|x| x.0).collect();
        assert_eq!(vals, vec![1, 2, 3, 4]);
    }

    #[test]
    fn single_char_not_found() {
        let mut cedar = Cedar::new();
        cedar.build(&kv(&["a"]));
        assert_eq!(cedar.common_prefix_predict("b"), None);
    }

    #[test]
    fn exact_non_terminal_prefix() {
        // "abc" and "abcd" share prefix "abc" which is also a complete key
        let mut cedar = Cedar::new();
        cedar.build(&kv(&["abc", "abcd"]));
        let r = cedar.common_prefix_predict("abc").unwrap();
        let vals: Vec<i32> = r.iter().map(|x| x.0).collect();
        assert_eq!(vals, vec![0, 1]);
    }
}
