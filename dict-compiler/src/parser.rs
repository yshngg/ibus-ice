#[derive(Debug, PartialEq, Clone)]
pub struct DictEntry {
    pub text: String,
    pub pinyin: Vec<String>,
    pub freq: u32,
}

pub struct Parser;

impl Parser {
    pub fn parse(input: &str) -> Result<Vec<DictEntry>, String> {
        let mut entries = Vec::new();
        let mut in_yaml_header = false;

        for line in input.lines() {
            let trimmed = line.trim();

            if trimmed.is_empty() {
                continue;
            }

            if trimmed == "---" {
                in_yaml_header = true;
                continue;
            }
            if trimmed == "..." {
                in_yaml_header = false;
                continue;
            }
            if in_yaml_header {
                continue;
            }
            if trimmed.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = trimmed.split('\t').collect();
            if parts.len() < 2 {
                continue;
            }

            let text = parts[0].to_string();
            let pinyin: Vec<String> = parts[1]
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();
            let freq = parts
                .get(2)
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);

            entries.push(DictEntry { text, pinyin, freq });
        }

        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_entry() {
        let input = "中国\tzhong guo\t100\n";
        let entries = Parser::parse(input).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].text, "中国");
        assert_eq!(entries[0].pinyin, vec!["zhong", "guo"]);
        assert_eq!(entries[0].freq, 100);
    }

    #[test]
    fn test_parse_without_freq() {
        let input = "你好\tnihao\n";
        let entries = Parser::parse(input).unwrap();
        assert_eq!(entries[0].text, "你好");
        assert_eq!(entries[0].pinyin, vec!["nihao"]);
        assert_eq!(entries[0].freq, 0);
    }

    #[test]
    fn test_skip_comments() {
        let input = "# comment\n中国\tzhong guo\t100\n";
        let entries = Parser::parse(input).unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_skip_yaml_frontmatter() {
        let input = "---\nname: test\n...\n中国\tzhong guo\t100\n";
        let entries = Parser::parse(input).unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_parse_en_dict() {
        let input = "hello\thello\t50\n";
        let entries = Parser::parse(input).unwrap();
        assert_eq!(entries[0].text, "hello");
        assert_eq!(entries[0].pinyin, vec!["hello"]);
        assert_eq!(entries[0].freq, 50);
    }
}
