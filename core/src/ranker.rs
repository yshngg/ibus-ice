use crate::candidate::Candidate;

pub trait Ranker: Send + Sync {
    fn rank(&self, candidates: &mut [Candidate]);
}

pub struct WeightedRanker {
    pub w_freq: f64,
    pub w_user: f64,
    pub w_exact: f64,
}

impl Default for WeightedRanker {
    fn default() -> Self {
        WeightedRanker {
            w_freq: 1.0,
            w_user: 2.0,
            w_exact: 3.0,
        }
    }
}

impl Ranker for WeightedRanker {
    fn rank(&self, candidates: &mut [Candidate]) {
        for c in candidates.iter_mut() {
            let freq_log = if c.freq > 0 {
                (c.freq as f64).ln()
            } else {
                0.0
            };
            let exact_bonus = if c.exact_match { self.w_exact } else { 0.0 };
            c.score = freq_log * self.w_freq
                + c.user_boost * self.w_user
                + exact_bonus;
        }
        // Sort descending by score
        candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_candidate(text: &str, freq: u32, exact: bool) -> Candidate {
        Candidate {
            text: text.to_string(),
            freq,
            word_len: text.chars().count() as u8,
            exact_match: exact,
            user_boost: 0.0,
            score: 0.0,
        }
    }

    #[test]
    fn test_higher_freq_ranks_first() {
        let mut candidates = vec![
            make_candidate("中国", 10000, true),
            make_candidate("中过", 10, true),
        ];
        let ranker = WeightedRanker::default();
        ranker.rank(&mut candidates);
        assert_eq!(candidates[0].text, "中国");
    }

    #[test]
    fn test_exact_match_beats_partial() {
        let mut candidates = vec![
            make_candidate("中", 5000, false),    // partial match
            make_candidate("中国", 5000, true),    // exact match
        ];
        let ranker = WeightedRanker::default();
        ranker.rank(&mut candidates);
        assert_eq!(candidates[0].text, "中国");
    }

    #[test]
    fn test_user_boost_ranks_higher() {
        let mut candidates = vec![
            make_candidate("中国", 10000, true),
            make_candidate("中过", 10, true),
        ];
        candidates[1].user_boost = 10.0; // heavy user boost
        let ranker = WeightedRanker::default();
        ranker.rank(&mut candidates);
        assert_eq!(candidates[0].text, "中过");
    }
}
