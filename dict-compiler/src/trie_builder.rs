use cedar::Cedar;
use std::io::{self, Write};

use crate::perf;
use crate::parser::DictEntry;

/// Build a double-array trie from DictEntry records.
/// Keys are space-joined pinyin strings; values are entry indices.
pub fn build_trie(entries: &[DictEntry]) -> Cedar {
    let total = entries.len();

    // The DAT overwrites duplicate keys, so append \x01 + entry
    // index to make every key unique.  common_prefix_predict still
    // works because all entries sharing the same pinyin start with
    // the same prefix (up to the first \x01).
    let keys: Vec<String> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let base = e.pinyin.join(" ");
            format!("{}\x01{}", base, i)
        })
        .collect();

    let mut cedar = Cedar::new();

    let key_slices: Vec<(&str, i32)> = keys
        .iter()
        .enumerate()
        .map(|(i, k)| (k.as_str(), i as i32))
        .collect();

    perf::phase("cedar_build_start");
    if perf::enabled() {
        perf::progress(0, total, 0);
        perf::memory_sample();
    }

    cedar.build(&key_slices);

    if perf::enabled() {
        perf::progress(total, total, 0);
        perf::memory_sample();
    }
    perf::phase("cedar_build_end");

    cedar
}

/// Binary dictionary format:
///
///   Magic:     "IBUSICE03"  (8 bytes)
///   u32:       num_entries  (little-endian)
///   For each entry i in 0..num_entries:
///     u16       pinyin_byte_len
///     [bytes]   pinyin (UTF-8, the trie key)
///     u16       text_byte_len
///     [bytes]   text (UTF-8)
///     u32       freq
///     u8        word_len   (char count)
///
/// At load time, we reconstruct the DictEntry list and call
/// build() — O(n) and fast.
pub fn serialize_trie<W: Write>(_cedar: &Cedar, writer: &mut W, entries: &[DictEntry]) -> io::Result<()> {
    // Magic
    writer.write_all(b"IBUSIC03")?;

    let num_entries = entries.len() as u32;
    writer.write_all(&num_entries.to_le_bytes())?;

    for entry in entries {
        // Pinyin key
        let pinyin = entry.pinyin.join(" ");
        let pinyin_bytes = pinyin.as_bytes();
        let pinyin_len = pinyin_bytes.len() as u16;
        writer.write_all(&pinyin_len.to_le_bytes())?;
        writer.write_all(pinyin_bytes)?;

        // Text
        let text_bytes = entry.text.as_bytes();
        let text_len = text_bytes.len() as u16;
        writer.write_all(&text_len.to_le_bytes())?;
        writer.write_all(text_bytes)?;

        // Freq + word_len
        writer.write_all(&entry.freq.to_le_bytes())?;
        let word_len = entry.text.chars().count() as u8;
        writer.write_all(&word_len.to_le_bytes())?;
    }

    Ok(())
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Seek};

    struct CompiledDict {
        pub cedar: Cedar,
        pub entries: Vec<DictEntry>,
    }

    fn load_dict<R: Read + Seek>(reader: &mut R) -> io::Result<CompiledDict> {
        // Magic
        let mut magic = [0u8; 8];
        reader.read_exact(&mut magic)?;
        if &magic != b"IBUSIC03" {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid magic"));
        }

        // Num entries
        let mut num_buf = [0u8; 4];
        reader.read_exact(&mut num_buf)?;
        let num_entries = u32::from_le_bytes(num_buf) as usize;

        // Read all entries
        let mut entries: Vec<DictEntry> = Vec::with_capacity(num_entries);
        for _ in 0..num_entries {
            // Pinyin
            let mut len_buf = [0u8; 2];
            reader.read_exact(&mut len_buf)?;
            let pinyin_len = u16::from_le_bytes(len_buf) as usize;
            let mut pinyin_bytes = vec![0u8; pinyin_len];
            reader.read_exact(&mut pinyin_bytes)?;
            let pinyin_str = String::from_utf8(pinyin_bytes)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            let pinyin: Vec<String> = pinyin_str.split_whitespace().map(|s| s.to_string()).collect();

            // Text
            reader.read_exact(&mut len_buf)?;
            let text_len = u16::from_le_bytes(len_buf) as usize;
            let mut text_bytes = vec![0u8; text_len];
            reader.read_exact(&mut text_bytes)?;
            let text = String::from_utf8(text_bytes)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

            // Freq
            let mut freq_buf = [0u8; 4];
            reader.read_exact(&mut freq_buf)?;
            let freq = u32::from_le_bytes(freq_buf);

            // Word len
            let mut wl_buf = [0u8; 1];
            reader.read_exact(&mut wl_buf)?;
            let _word_len = wl_buf[0];

            entries.push(DictEntry { text, pinyin, freq });
        }

        // Rebuild cedar trie
        let cedar = build_trie(&entries);

        Ok(CompiledDict { cedar, entries })
    }

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
        let cedar = build_trie(&entries);

        let results = cedar.common_prefix_predict("zhong").unwrap();
        let ids: Vec<i32> = results.iter().map(|(v, _)| *v).collect();
        assert!(ids.contains(&0));
        assert!(ids.contains(&2));

        let results = cedar.common_prefix_predict("zhong guo").unwrap();
        let ids: Vec<i32> = results.iter().map(|(v, _)| *v).collect();
        assert!(ids.contains(&1));
    }

    #[test]
    fn test_lookup_miss() {
        let entries = vec![make_entry("中", "zhong", 100)];
        let cedar = build_trie(&entries);
        assert!(cedar.common_prefix_predict("abc").is_none());
    }

    #[test]
    fn test_roundtrip_serialize() {
        let entries = vec![
            make_entry("中国", "zhong guo", 200),
            make_entry("中国人", "zhong guo ren", 150),
        ];
        let cedar = build_trie(&entries);

        let mut buf = Vec::new();
        serialize_trie(&cedar, &mut buf, &entries).unwrap();

        let mut cursor = std::io::Cursor::new(&buf);
        let loaded = load_dict(&mut cursor).unwrap();

        assert_eq!(loaded.entries.len(), 2);
        assert_eq!(loaded.entries[0].text, "中国");
        assert_eq!(loaded.entries[0].freq, 200);
        assert_eq!(loaded.entries[1].text, "中国人");

        let results = loaded.cedar.common_prefix_predict("zhong guo").unwrap();
        let ids: Vec<i32> = results.iter().map(|(v, _)| *v).collect();
        assert!(ids.contains(&0));
        assert!(ids.contains(&1));
    }

    #[test]
    fn test_exact_match_found() {
        let entries = vec![
            make_entry("中国", "zhong guo", 200),
        ];
        let cedar = build_trie(&entries);
        // Keys have \x01 + index suffix, so exact match won't work.
        // Use common_prefix_predict instead (what the engine uses).
        let results = cedar.common_prefix_predict("zhong guo").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 0);
    }
}
