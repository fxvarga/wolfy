//! FuzzyMatcher - fuzzy string matching algorithm
//!
//! Implements fuzzy matching for search functionality.

/// Result of a fuzzy match
#[derive(Clone, Debug)]
pub struct MatchResult {
    /// Whether the pattern matched
    pub matched: bool,
    /// Match score (higher is better)
    pub score: i32,
    /// Indices of matched characters in the target
    pub matched_indices: Vec<usize>,
}

impl MatchResult {
    /// Create a non-matching result
    pub fn no_match() -> Self {
        Self {
            matched: false,
            score: 0,
            matched_indices: Vec::new(),
        }
    }

    /// Create a matching result
    pub fn new(score: i32, matched_indices: Vec<usize>) -> Self {
        Self {
            matched: true,
            score,
            matched_indices,
        }
    }
}

/// Fuzzy string matcher
#[derive(Clone, Debug, Default)]
pub struct FuzzyMatcher {
    /// Bonus for consecutive character matches
    pub consecutive_bonus: i32,
    /// Bonus for matching at word start
    pub word_start_bonus: i32,
    /// Bonus for matching at string start
    pub first_char_bonus: i32,
    /// Penalty per unmatched character
    pub unmatched_penalty: i32,
}

impl FuzzyMatcher {
    /// Create a new fuzzy matcher with default settings
    pub fn new() -> Self {
        Self {
            consecutive_bonus: 15,
            word_start_bonus: 10,
            first_char_bonus: 20,
            unmatched_penalty: 1,
        }
    }

    /// Match a pattern against a target string
    pub fn fuzzy_match(&self, pattern: &str, target: &str) -> MatchResult {
        if pattern.is_empty() {
            return MatchResult::new(0, Vec::new());
        }

        let pattern_lower = pattern.to_lowercase();
        let target_lower = target.to_lowercase();

        let pattern_chars: Vec<char> = pattern_lower.chars().collect();
        let target_chars: Vec<char> = target_lower.chars().collect();

        // Try to find all pattern characters in order
        let mut matched_indices = Vec::new();
        let mut pattern_idx = 0;
        let mut last_match_idx: Option<usize> = None;
        let mut score = 0;

        for (target_idx, &target_char) in target_chars.iter().enumerate() {
            if pattern_idx < pattern_chars.len() && target_char == pattern_chars[pattern_idx] {
                matched_indices.push(target_idx);

                // Bonus for consecutive matches
                if let Some(last) = last_match_idx {
                    if target_idx == last + 1 {
                        score += self.consecutive_bonus;
                    }
                }

                // Bonus for first character match
                if target_idx == 0 {
                    score += self.first_char_bonus;
                }

                // Bonus for word boundary match
                if target_idx > 0 {
                    let prev_char = target.chars().nth(target_idx - 1).unwrap_or(' ');
                    if prev_char == ' '
                        || prev_char == '_'
                        || prev_char == '-'
                        || prev_char == '/'
                        || prev_char == '\\'
                    {
                        score += self.word_start_bonus;
                    }
                }

                last_match_idx = Some(target_idx);
                pattern_idx += 1;
            }
        }

        // Check if all pattern characters were matched
        if pattern_idx == pattern_chars.len() {
            // Base score for matching
            score += 100;

            // Penalty for target length (prefer shorter matches)
            score -= (target_chars.len() - pattern_chars.len()) as i32 * self.unmatched_penalty;

            MatchResult::new(score, matched_indices)
        } else {
            MatchResult::no_match()
        }
    }

    /// Check if a pattern matches a target (simple contains check)
    pub fn contains_match(&self, pattern: &str, target: &str) -> bool {
        if pattern.is_empty() {
            return true;
        }

        let pattern_lower = pattern.to_lowercase();
        let target_lower = target.to_lowercase();

        target_lower.contains(&pattern_lower)
    }

    /// Score a contains match (for substring matching)
    pub fn score_contains(&self, pattern: &str, target: &str) -> i32 {
        if pattern.is_empty() {
            return 0;
        }

        let pattern_lower = pattern.to_lowercase();
        let target_lower = target.to_lowercase();

        if let Some(pos) = target_lower.find(&pattern_lower) {
            let mut score = 100;

            // Bonus for early match
            score -= pos as i32 * 2;

            // Bonus for matching at word boundary
            if pos == 0 {
                score += self.first_char_bonus;
            } else {
                let prev_char = target.chars().nth(pos - 1).unwrap_or(' ');
                if prev_char == ' ' || prev_char == '_' || prev_char == '-' {
                    score += self.word_start_bonus;
                }
            }

            // Bonus for shorter targets
            score -= (target.len() - pattern.len()) as i32;

            score.max(1)
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        let matcher = FuzzyMatcher::new();
        let result = matcher.fuzzy_match("chrome", "Chrome");

        assert!(result.matched);
        assert!(result.score > 100);
    }

    #[test]
    fn test_fuzzy_match() {
        let matcher = FuzzyMatcher::new();
        let result = matcher.fuzzy_match("vsc", "Visual Studio Code");

        assert!(result.matched);
        assert!(result.score > 0);
    }

    #[test]
    fn test_no_match() {
        let matcher = FuzzyMatcher::new();
        let result = matcher.fuzzy_match("xyz", "Chrome");

        assert!(!result.matched);
    }

    #[test]
    fn test_consecutive_bonus() {
        let matcher = FuzzyMatcher::new();
        let consecutive = matcher.fuzzy_match("code", "code editor");
        let scattered = matcher.fuzzy_match("code", "c_o_d_e");

        assert!(consecutive.score > scattered.score);
    }

    #[test]
    fn test_word_start_bonus() {
        let matcher = FuzzyMatcher::new();
        let word_start = matcher.fuzzy_match("vs", "Visual Studio");
        let mid_word = matcher.fuzzy_match("su", "Visual Studio");

        assert!(word_start.score > mid_word.score);
    }

    #[test]
    fn test_contains_match() {
        let matcher = FuzzyMatcher::new();

        assert!(matcher.contains_match("studio", "Visual Studio Code"));
        assert!(!matcher.contains_match("xyz", "Visual Studio Code"));
    }

    #[test]
    fn test_empty_pattern() {
        let matcher = FuzzyMatcher::new();
        let result = matcher.fuzzy_match("", "anything");

        assert!(result.matched);
    }
}
