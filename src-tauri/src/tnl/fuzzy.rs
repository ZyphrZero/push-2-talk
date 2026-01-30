//! TNL 模糊匹配
//!
//! 支持编辑距离和拼音相似度匹配

use pinyin::ToPinyin;
use strsim::levenshtein;

/// 模糊匹配器
pub struct FuzzyMatcher {
    /// 词库（已提纯）
    dictionary: Vec<String>,
    /// 词库拼音缓存
    pinyin_cache: Vec<String>,
}

/// 匹配结果
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct FuzzyMatch {
    /// 匹配到的词库词
    pub word: String,
    /// 置信度 (0.0 - 1.0)
    pub confidence: f32,
    /// 匹配类型
    pub match_type: FuzzyMatchType,
}

/// 匹配类型
#[derive(Debug, Clone, PartialEq)]
pub enum FuzzyMatchType {
    /// 精确匹配
    Exact,
    /// 编辑距离匹配
    EditDistance,
    /// 拼音匹配
    Pinyin,
}

impl FuzzyMatcher {
    /// 创建模糊匹配器
    ///
    /// # Arguments
    /// * `dictionary` - 已提纯的词库
    pub fn new(dictionary: Vec<String>) -> Self {
        // 预计算词库拼音
        let pinyin_cache: Vec<String> = dictionary
            .iter()
            .map(|word| Self::to_pinyin_str(word))
            .collect();

        Self {
            dictionary,
            pinyin_cache,
        }
    }

    /// 尝试匹配
    ///
    /// 返回最佳匹配结果（如果有）
    #[allow(dead_code)]
    pub fn try_match(&self, text: &str) -> Option<FuzzyMatch> {
        if text.is_empty() || self.dictionary.is_empty() {
            return None;
        }

        // 1. 精确匹配
        if let Some(exact) = self.try_exact_match(text) {
            return Some(exact);
        }

        // 2. 编辑距离匹配
        if let Some(fuzzy) = self.try_edit_distance_match(text) {
            return Some(fuzzy);
        }

        // 3. 拼音匹配
        if let Some(pinyin) = self.try_pinyin_match(text) {
            return Some(pinyin);
        }

        None
    }

    /// 精确匹配
    fn try_exact_match(&self, text: &str) -> Option<FuzzyMatch> {
        let text_lower = text.to_lowercase();
        for word in &self.dictionary {
            if word.to_lowercase() == text_lower {
                return Some(FuzzyMatch {
                    word: word.clone(),
                    confidence: 1.0,
                    match_type: FuzzyMatchType::Exact,
                });
            }
        }
        None
    }

    /// 编辑距离匹配
    fn try_edit_distance_match(&self, text: &str) -> Option<FuzzyMatch> {
        let text_len = text.chars().count();
        // 阈值：max(1, len/4)
        let threshold = std::cmp::max(1, text_len / 4);

        let mut best_match: Option<(String, usize)> = None;

        for word in &self.dictionary {
            let word_len = word.chars().count();
            // 长度差异过大则跳过
            if (word_len as i32 - text_len as i32).abs() > threshold as i32 {
                continue;
            }

            let distance = levenshtein(text, word);
            if distance <= threshold {
                if best_match.is_none() || distance < best_match.as_ref().unwrap().1 {
                    best_match = Some((word.clone(), distance));
                }
            }
        }

        best_match.map(|(word, distance)| {
            // 置信度：1.0 - (distance / max_distance)
            let max_distance = std::cmp::max(text_len, word.chars().count());
            let confidence = if max_distance > 0 {
                1.0 - (distance as f32 / max_distance as f32)
            } else {
                1.0
            };
            // 模糊匹配基础置信度 0.8
            FuzzyMatch {
                word,
                confidence: confidence * 0.8,
                match_type: FuzzyMatchType::EditDistance,
            }
        })
    }

    /// 拼音匹配
    fn try_pinyin_match(&self, text: &str) -> Option<FuzzyMatch> {
        let text_pinyin = Self::to_pinyin_str(text);
        if text_pinyin.is_empty() {
            return None;
        }

        for (i, word_pinyin) in self.pinyin_cache.iter().enumerate() {
            if word_pinyin.is_empty() {
                continue;
            }

            // 全拼完全匹配
            if text_pinyin == *word_pinyin {
                return Some(FuzzyMatch {
                    word: self.dictionary[i].clone(),
                    confidence: 0.7,
                    match_type: FuzzyMatchType::Pinyin,
                });
            }

            // 首字母匹配（仅当词库词较长时）
            let word_initials = Self::to_pinyin_initials(&self.dictionary[i]);
            let text_initials = Self::to_pinyin_initials(text);
            if !word_initials.is_empty()
                && word_initials == text_initials
                && self.dictionary[i].chars().count() >= 2
            {
                return Some(FuzzyMatch {
                    word: self.dictionary[i].clone(),
                    confidence: 0.6,
                    match_type: FuzzyMatchType::Pinyin,
                });
            }
        }

        None
    }

    /// 转换为拼音字符串（全拼，无声调）
    fn to_pinyin_str(text: &str) -> String {
        let mut result = String::new();
        for ch in text.chars() {
            if let Some(pinyin) = ch.to_pinyin() {
                result.push_str(pinyin.plain());
            } else if ch.is_ascii_alphanumeric() {
                result.push(ch.to_ascii_lowercase());
            }
        }
        result
    }

    /// 转换为拼音首字母
    fn to_pinyin_initials(text: &str) -> String {
        let mut result = String::new();
        for ch in text.chars() {
            if let Some(pinyin) = ch.to_pinyin() {
                if let Some(first) = pinyin.plain().chars().next() {
                    result.push(first);
                }
            } else if ch.is_ascii_alphabetic() {
                result.push(ch.to_ascii_lowercase());
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        let matcher = FuzzyMatcher::new(vec!["Claude".to_string(), "Tauri".to_string()]);

        let result = matcher.try_match("claude");
        assert!(result.is_some());
        let m = result.unwrap();
        assert_eq!(m.word, "Claude");
        assert_eq!(m.match_type, FuzzyMatchType::Exact);
    }

    #[test]
    fn test_edit_distance_match() {
        let matcher = FuzzyMatcher::new(vec!["readme".to_string()]);

        // "readm" 距离 "readme" 编辑距离为 1
        let result = matcher.try_match("readm");
        assert!(result.is_some());
        let m = result.unwrap();
        assert_eq!(m.match_type, FuzzyMatchType::EditDistance);
    }

    #[test]
    fn test_pinyin_match() {
        let matcher = FuzzyMatcher::new(vec!["配置".to_string()]);

        let result = matcher.try_match("peizhi");
        assert!(result.is_some());
        let m = result.unwrap();
        assert_eq!(m.word, "配置");
        assert_eq!(m.match_type, FuzzyMatchType::Pinyin);
    }

    #[test]
    fn test_no_match() {
        let matcher = FuzzyMatcher::new(vec!["hello".to_string()]);

        let result = matcher.try_match("xyz");
        assert!(result.is_none());
    }
}
