use crate::dictionary::Dictionary;
use crate::segmenter::segment;

#[derive(Debug, Clone)]
pub struct Candidate {
    pub text: String,
    pub freq: u32,
    pub word_len: u8,
    pub exact_match: bool,
    pub user_boost: f64,
    pub score: f64,
}

pub fn generate(dict: &Dictionary, pinyin: &str, input_complete: bool) -> Vec<Candidate> {
    let segmentations = segment(pinyin);
    let mut candidates = Vec::new();

    for seg in &segmentations {
        let joined = seg.syllables.join(" ");
        let entries = dict.lookup(&joined);

        for entry in &entries {
            candidates.push(Candidate {
                text: entry.text.clone(),
                freq: entry.freq,
                word_len: entry.word_len,
                exact_match: input_complete,
                user_boost: 0.0,
                score: 0.0,
            });
        }
    }

    // Deduplicate by text
    let mut seen = std::collections::HashSet::new();
    candidates.retain(|c| seen.insert(c.text.clone()));

    candidates
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_generate_empty_for_no_dict() {
        let candidates: Vec<crate::candidate::Candidate> = Vec::new();
        assert!(candidates.is_empty());
    }
}
