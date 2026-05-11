use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct UserEntry {
    pub count: u32,
    pub last_time: u64,
}

pub struct UserDict {
    entries: HashMap<String, UserEntry>,
    path: String,
}

impl UserDict {
    pub fn new(path: &str) -> Self {
        let mut entries = HashMap::new();

        if Path::new(path).exists() {
            if let Ok(content) = fs::read_to_string(path) {
                for line in content.lines() {
                    let parts: Vec<&str> = line.split('\t').collect();
                    if parts.len() >= 4 {
                        let text = parts[2].to_string();
                        let time: u64 = parts[3].parse().unwrap_or(0);

                        let entry = entries.entry(text).or_insert(UserEntry { count: 0, last_time: 0 });
                        if parts[0] == "^" {
                            entry.count += 1;
                        } else {
                            entry.count += 1;
                        }
                        entry.last_time = time;
                    }
                }
            }
        }

        UserDict { entries, path: path.to_string() }
    }

    pub fn record(&mut self, pinyin: &str, text: &str) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let entry = self.entries.entry(text.to_string()).or_insert(UserEntry { count: 0, last_time: 0 });
        entry.count += 1;
        entry.last_time = now;

        if let Ok(mut file) = fs::OpenOptions::new().append(true).create(true).open(&self.path) {
            let _ = writeln!(file, "^\t{}\t{}\t{}", pinyin, text, now);
        }
    }

    pub fn get_boost(&self, text: &str) -> f64 {
        if let Some(entry) = self.entries.get(text) {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let days = (now - entry.last_time) as f64 / 86400.0;
            let lambda = 0.01;
            entry.count as f64 * (-lambda * days).exp()
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_record_and_boost() {
        let tmp = "/tmp/test_user_dict.dict";
        let _ = fs::remove_file(tmp);

        let mut ud = UserDict::new(tmp);
        ud.record("zhong guo", "中国");
        ud.record("zhong guo", "中国");

        let boost = ud.get_boost("中国");
        assert!(boost > 0.0);

        assert_eq!(ud.get_boost("nonexistent"), 0.0);

        let _ = fs::remove_file(tmp);
    }

    #[test]
    fn test_load_existing() {
        let tmp = "/tmp/test_user_dict_load.dict";
        let _ = fs::remove_file(tmp);
        fs::write(tmp, "+\tzhong guo\t中国\t1700000000\n^\tzhong guo\t中国\t1700000100\n").unwrap();

        let ud = UserDict::new(tmp);
        let boost = ud.get_boost("中国");
        assert!(boost > 0.0);

        let _ = fs::remove_file(tmp);
    }
}
