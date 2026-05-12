use crate::candidate::{self, Candidate};
use crate::dictionary::Dictionary;
use crate::ranker::{Ranker, WeightedRanker};
use crate::segmenter::segment;
use crate::userdict::UserDict;

pub struct IceEngine {
    dict: Dictionary,
    user_dict: UserDict,
    ranker: Box<dyn Ranker>,
    current_pinyin: String,
}

impl IceEngine {
    pub fn new(dict_path: &str, user_dict_path: &str) -> Result<Self, String> {
        let dict = Dictionary::open(dict_path)?;
        let user_dict = UserDict::new(user_dict_path);
        Ok(IceEngine {
            dict,
            user_dict,
            ranker: Box::new(WeightedRanker::default()),
            current_pinyin: String::new(),
        })
    }

    pub fn process(&mut self, pinyin: &str) -> Vec<Candidate> {
        self.current_pinyin = pinyin.to_string();
        let input_complete = pinyin.ends_with(' ');

        let clean_pinyin = pinyin.trim().to_lowercase();
        if clean_pinyin.is_empty() {
            return Vec::new();
        }

        let mut candidates = candidate::generate(&self.dict, &clean_pinyin, input_complete);

        for c in &mut candidates {
            c.user_boost = self.user_dict.get_boost(&c.text);
        }

        self.ranker.rank(&mut candidates);

        candidates.truncate(50);
        candidates
    }

    pub fn debug_process(&self, pinyin: &str) -> String {
        let clean_pinyin = pinyin.trim().to_lowercase();
        let mut json = String::new();

        // Segmentation
        let segmentations = segment(&clean_pinyin);
        let mut pos: usize = 0;
        let mut seg_entries: Vec<(String, usize, usize, usize)> = Vec::new();

        if let Some(seg) = segmentations.first() {
            for syllable in &seg.syllables {
                let end = pos + syllable.len();
                let entries = self.dict.lookup(syllable);
                seg_entries.push((syllable.clone(), pos, end, entries.len()));
                pos = end;
            }
        }

        // Candidates with ranking
        let mut candidates = crate::candidate::generate(&self.dict, &clean_pinyin, false);
        for c in &mut candidates {
            c.user_boost = self.user_dict.get_boost(&c.text);
        }
        self.ranker.rank(&mut candidates);
        candidates.truncate(50);

        // Build JSON manually
        fn esc(s: &str) -> String {
            s.replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
        }

        json.push('{');
        json.push_str(&format!("\"pinyin\":\"{}\"", esc(&clean_pinyin)));

        json.push_str(",\"segments\":[");
        for (i, (s, start, end, n)) in seg_entries.iter().enumerate() {
            if i > 0 { json.push(','); }
            json.push_str(&format!(
                "{{\"segment\":\"{}\",\"start\":{},\"end\":{},\"entries\":{}}}",
                esc(s), start, end, n
            ));
        }
        json.push(']');

        json.push_str(",\"candidates\":[");
        for (i, c) in candidates.iter().enumerate() {
            if i > 0 { json.push(','); }
            json.push_str(&format!(
                "{{\"text\":\"{}\",\"freq\":{},\"score\":{:.4},\"user_boost\":{:.4},\"exact_match\":{}}}",
                esc(&c.text), c.freq, c.score, c.user_boost, c.exact_match
            ));
        }
        json.push(']');
        json.push('}');

        json
    }

    pub fn select(&mut self, text: &str) {
        let pinyin = self.current_pinyin.clone();
        self.user_dict.record(&pinyin, text);
    }

    pub fn reset(&mut self) {
        self.current_pinyin.clear();
    }
}
