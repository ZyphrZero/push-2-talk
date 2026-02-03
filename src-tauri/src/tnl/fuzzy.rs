//! TNL 模糊匹配
//!
//! 支持编辑距离和拼音相似度匹配

use std::collections::{hash_map::Entry, HashMap};

use pinyin::ToPinyin;
use strsim::levenshtein;

/// 模糊匹配器
pub struct FuzzyMatcher {
    /// 词库（已提纯）
    dictionary: Vec<String>,
    /// 词库拼音缓存（无声调）
    pinyin_cache: Vec<String>,
    /// 拼音（带声调）→ 词库索引映射，None 表示同键冲突
    pinyin_tone_map: HashMap<String, Option<usize>>,
    /// 词库中参与声调匹配的最大词长（字符数）
    max_word_len: usize,
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
        // 预计算词库拼音（无声调）
        let pinyin_cache: Vec<String> = dictionary
            .iter()
            .map(|word| Self::to_pinyin_str(word))
            .collect();

        // 预计算带声调拼音映射
        let mut pinyin_tone_map: HashMap<String, Option<usize>> = HashMap::new();
        let mut max_word_len: usize = 0;

        for (i, word) in dictionary.iter().enumerate() {
            let key = Self::to_pinyin_with_tone(word);

            let word_len = word.chars().count();
            // 仅处理 ≥2 字的词
            if word_len < 2 || key.is_empty() {
                continue;
            }

            max_word_len = std::cmp::max(max_word_len, word_len);

            match pinyin_tone_map.entry(key) {
                Entry::Vacant(e) => {
                    e.insert(Some(i));
                }
                Entry::Occupied(mut e) => {
                    // 同键冲突：标记为 None，跳过替换
                    e.insert(None);
                }
            }
        }

        Self {
            dictionary,
            pinyin_cache,
            pinyin_tone_map,
            max_word_len,
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

    /// 转换为带声调数字的拼音（如 "张三" → "zhang1san1"）
    ///
    /// 如果包含非汉字字符，返回空字符串
    fn to_pinyin_with_tone(text: &str) -> String {
        let mut result = String::new();
        for ch in text.chars() {
            if let Some(pinyin) = ch.to_pinyin() {
                result.push_str(pinyin.with_tone_num());
            } else {
                // 非汉字字符，返回空字符串（不参与拼音匹配）
                return String::new();
            }
        }
        result
    }

    /// 精确拼音替换（带声调）
    ///
    /// 返回 `Some((替换词, 消费字节数))` 如果匹配成功
    ///
    /// 匹配条件：
    /// - 拼音 + 声调 100% 完全匹配
    /// - 原词 ≥2 个汉字
    /// - 无同键冲突（一键多值时跳过）
    ///
    /// 使用最长匹配策略
    pub fn try_exact_pinyin_replace(&self, text: &str) -> Option<(String, usize)> {
        if text.is_empty() || self.dictionary.is_empty() || self.max_word_len < 2 {
            return None;
        }

        // 收集字符结束位置（按 max_word_len 截断）
        let mut char_ends: Vec<usize> = Vec::with_capacity(self.max_word_len);
        for (idx, ch) in text.char_indices() {
            char_ends.push(idx + ch.len_utf8());
            if char_ends.len() >= self.max_word_len {
                break;
            }
        }

        // 至少需要 2 个字符
        if char_ends.len() < 2 {
            return None;
        }

        // 从长到短尝试匹配（最长匹配优先）
        for len in (2..=char_ends.len()).rev() {
            let end_byte = char_ends[len - 1];
            let candidate = &text[..end_byte];
            let key = Self::to_pinyin_with_tone(candidate);
            if key.is_empty() {
                continue;
            }

            match self.pinyin_tone_map.get(&key) {
                Some(Some(idx)) => {
                    // 找到唯一匹配，返回词库中的词
                    return Some((self.dictionary[*idx].clone(), end_byte));
                }
                Some(None) => {
                    // 同键冲突，返回原文（不替换但消费字符）
                    return Some((candidate.to_string(), end_byte));
                }
                None => {
                    // 无匹配，继续尝试更短的前缀
                }
            }
        }

        None
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

    // === 拼音精确替换测试 ===

    #[test]
    fn test_pinyin_replace_basic() {
        // "掌伞" (zhang3san3) 应该被替换为 "张三" (zhang1san1)... 不对，它们声调不同
        // 使用同音词测试：事例 (shi4li4) vs 示例 (shi4li4)
        let matcher = FuzzyMatcher::new(vec!["示例".to_string()]);

        let result = matcher.try_exact_pinyin_replace("事例");
        assert!(result.is_some());
        let (word, len) = result.unwrap();
        assert_eq!(word, "示例");
        assert_eq!(len, "事例".len());
    }

    #[test]
    fn test_pinyin_replace_tone_strict() {
        // "妈妈" (ma1ma1) vs "骂骂" (ma4ma4) - 声调不同，不应替换
        let matcher = FuzzyMatcher::new(vec!["骂骂".to_string()]);

        let result = matcher.try_exact_pinyin_replace("妈妈");
        assert!(result.is_none());
    }

    #[test]
    fn test_pinyin_replace_min_length() {
        // 单字不应替换
        let matcher = FuzzyMatcher::new(vec!["马".to_string()]);

        let result = matcher.try_exact_pinyin_replace("麻");
        assert!(result.is_none());
    }

    #[test]
    fn test_pinyin_replace_conflict_skip() {
        // 同音词冲突：公式 vs 公事 (gong1shi4) - 应返回原文
        let matcher = FuzzyMatcher::new(vec!["公式".to_string(), "公事".to_string()]);

        let result = matcher.try_exact_pinyin_replace("攻势");
        assert!(result.is_some());
        let (word, _) = result.unwrap();
        // 冲突时返回原文
        assert_eq!(word, "攻势");
    }

    #[test]
    fn test_pinyin_replace_longest_match() {
        // 最长匹配测试（避免多音字以确保测试稳定）
        // "示例" (shi4li4) vs "示例库" (shi4li4ku4)
        let matcher = FuzzyMatcher::new(vec!["示例".to_string(), "示例库".to_string()]);

        // "事例酷" (shi4li4ku4) 应匹配 "示例库"（最长匹配）
        let result = matcher.try_exact_pinyin_replace("事例酷");
        assert!(result.is_some());
        let (word, _) = result.unwrap();
        assert_eq!(word, "示例库");
    }

    #[test]
    fn test_pinyin_replace_non_chinese_skip() {
        // 非中文不参与拼音匹配
        let matcher = FuzzyMatcher::new(vec!["readme".to_string()]);

        let result = matcher.try_exact_pinyin_replace("readme");
        assert!(result.is_none());
    }
}
