use crate::candidate::{self, Candidate};
use crate::dictionary::Dictionary;
use crate::ranker::{Ranker, WeightedRanker};
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

    pub fn select(&mut self, text: &str) {
        let pinyin = self.current_pinyin.clone();
        self.user_dict.record(&pinyin, text);
    }

    pub fn reset(&mut self) {
        self.current_pinyin.clear();
    }
}
