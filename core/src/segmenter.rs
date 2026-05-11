use crate::syllable;

#[derive(Debug, PartialEq, Clone)]
pub struct Segmentation {
    pub syllables: Vec<String>,
}

pub fn segment(input: &str) -> Vec<Segmentation> {
    let input = input.to_lowercase();
    let bytes = input.as_bytes();
    let mut results = Vec::new();

    // Try manual separator first
    if input.contains('\'') {
        let parts: Vec<&str> = input.split('\'').collect();
        let syllables: Vec<String> = parts.iter().map(|s| s.to_string()).collect();
        results.push(Segmentation { syllables });
        return results;
    }

    // Greedy longest-match segmentation
    let syllables = greedy_segment(bytes);
    if !syllables.is_empty() {
        results.push(Segmentation { syllables });
    }

    // If no valid pinyin segmentation found, treat whole input as one unit (English)
    if results.is_empty() {
        results.push(Segmentation {
            syllables: vec![input.clone()],
        });
    }

    results
}

fn greedy_segment(bytes: &[u8]) -> Vec<String> {
    let mut syllables = Vec::new();
    let mut pos = 0;
    let max_len = syllable::max_syllable_length();

    while pos < bytes.len() {
        let mut matched = false;
        let remaining = bytes.len() - pos;
        let look = std::cmp::min(max_len, remaining);

        for len in (1..=look).rev() {
            let candidate = std::str::from_utf8(&bytes[pos..pos + len]).unwrap();
            if syllable::is_valid_syllable(candidate) {
                syllables.push(candidate.to_string());
                pos += len;
                matched = true;
                break;
            }
        }

        if !matched {
            // Cannot segment as pinyin — fail so English fallback kicks in
            return Vec::new();
        }
    }

    syllables
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_segmentation() {
        let results = segment("zhongguo");
        assert!(!results.is_empty());
        let expected: Vec<String> = vec!["zhong".into(), "guo".into()];
        assert!(results.iter().any(|s| s.syllables == expected));
    }

    #[test]
    fn test_single_syllable() {
        let results = segment("wo");
        assert!(!results.is_empty());
        let expected: Vec<String> = vec!["wo".into()];
        assert!(results.iter().any(|s| s.syllables == expected));
    }

    #[test]
    fn test_with_separator() {
        let results = segment("xi'an");
        assert!(!results.is_empty());
        let expected: Vec<String> = vec!["xi".into(), "an".into()];
        assert!(results.iter().any(|s| s.syllables == expected));
    }

    #[test]
    fn test_english_no_segmentation_needed() {
        let results = segment("hello");
        assert!(!results.is_empty());
        let expected: Vec<String> = vec!["hello".into()];
        assert!(results.iter().any(|s| s.syllables == expected));
    }
}
