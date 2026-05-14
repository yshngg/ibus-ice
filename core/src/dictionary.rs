use cedar::Cedar;
use std::fs;
use std::io::Read;

#[derive(Debug, Clone)]
pub struct DictEntry {
    pub text: String,
    pub freq: u32,
    pub word_len: u8,
}

pub struct Dictionary {
    cedar: Cedar,
    texts: Vec<String>,
    freqs: Vec<u32>,
    word_lens: Vec<u8>,
}

impl Dictionary {
    pub fn open(path: &str) -> Result<Self, String> {
        let mut file = fs::File::open(path).map_err(|e| format!("open: {}", e))?;

        // Magic
        let mut magic = [0u8; 8];
        file.read_exact(&mut magic).map_err(|e| format!("read magic: {}", e))?;
        if &magic != b"IBUSIC03" {
            return Err("invalid magic".into());
        }

        // Num entries
        let mut num_buf = [0u8; 4];
        file.read_exact(&mut num_buf).map_err(|e| format!("read num entries: {}", e))?;
        let num_entries = u32::from_le_bytes(num_buf) as usize;

        // Read entries, building key-values for cedar
        let mut keys: Vec<(String, i32)> = Vec::with_capacity(num_entries);
        let mut texts: Vec<String> = Vec::with_capacity(num_entries);
        let mut freqs: Vec<u32> = Vec::with_capacity(num_entries);
        let mut word_lens: Vec<u8> = Vec::with_capacity(num_entries);

        let mut len_buf = [0u8; 2];
        for i in 0..num_entries {
            // Pinyin
            file.read_exact(&mut len_buf).map_err(|e| format!("read pinyin len: {}", e))?;
            let pinyin_len = u16::from_le_bytes(len_buf) as usize;
            let mut pinyin_bytes = vec![0u8; pinyin_len];
            file.read_exact(&mut pinyin_bytes).map_err(|e| format!("read pinyin: {}", e))?;
            let pinyin = String::from_utf8(pinyin_bytes).map_err(|e| format!("utf8: {}", e))?;

            // Append separator for unique keys (matching dict-compiler: \x01 + index)
            let key = format!("{}\x01{}", pinyin, i);

            // Text
            file.read_exact(&mut len_buf).map_err(|e| format!("read text len: {}", e))?;
            let text_len = u16::from_le_bytes(len_buf) as usize;
            let mut text_bytes = vec![0u8; text_len];
            file.read_exact(&mut text_bytes).map_err(|e| format!("read text: {}", e))?;
            let text = String::from_utf8(text_bytes).map_err(|e| format!("utf8: {}", e))?;

            // Freq
            let mut freq_buf = [0u8; 4];
            file.read_exact(&mut freq_buf).map_err(|e| format!("read freq: {}", e))?;
            let freq = u32::from_le_bytes(freq_buf);

            // Word len
            let mut wl_buf = [0u8; 1];
            file.read_exact(&mut wl_buf).map_err(|e| format!("read word len: {}", e))?;

            keys.push((key, i as i32));
            texts.push(text);
            freqs.push(freq);
            word_lens.push(wl_buf[0]);
        }

        // Build cedar trie
        let key_slices: Vec<(&str, i32)> = keys.iter().map(|(k, v)| (k.as_str(), *v)).collect();
        let mut cedar = Cedar::new();
        cedar.build(&key_slices);

        Ok(Dictionary { cedar, texts, freqs, word_lens })
    }

    pub fn lookup(&self, key: &str) -> Vec<DictEntry> {
        let mut results = Vec::new();

        // common_prefix_predict returns all entries whose key
        // starts with `key`.  Since our keys are "pinyin\x00_index", this
        // correctly returns all entries with the given pinyin prefix.
        if let Some(matches) = self.cedar.common_prefix_predict(key) {
            for (value, _len) in &matches {
                let idx = *value as usize;
                if idx < self.texts.len() {
                    results.push(DictEntry {
                        text: self.texts[idx].clone(),
                        freq: self.freqs[idx],
                        word_len: self.word_lens[idx],
                    });
                }
            }
        }

        results
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_invalid_file() {
        let result = Dictionary::open("/nonexistent/path.dict");
        assert!(result.is_err());
    }
}
