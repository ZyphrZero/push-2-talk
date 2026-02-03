//! TNL 主引擎
//!
//! 组合分词、技术片段识别、口语符号映射、模糊匹配

use std::time::Instant;
use unicode_normalization::UnicodeNormalization;

use crate::tnl::fuzzy::FuzzyMatcher;
use crate::tnl::rules::{ExtensionWhitelist, SpokenSymbolMap};
use crate::tnl::tech_span::TechSpanDetector;
use crate::tnl::tokenizer::{Token, TokenType, Tokenizer};
use crate::tnl::types::{NormalizationResult, Replacement, ReplacementReason, Span};

/// TNL 引擎（可复用，预编译规则）
pub struct TnlEngine {
    /// 口语符号映射
    spoken_symbol_map: SpokenSymbolMap,
    /// 技术片段检测器
    tech_span_detector: TechSpanDetector,
    /// 模糊匹配器（可选）
    fuzzy_matcher: Option<FuzzyMatcher>,
}

impl TnlEngine {
    /// 创建 TNL 引擎
    ///
    /// # Arguments
    /// * `dictionary` - 已提纯的词库（用于模糊匹配）
    pub fn new(dictionary: Vec<String>) -> Self {
        let spoken_symbol_map = SpokenSymbolMap::new();
        let ext_whitelist = ExtensionWhitelist::new();
        let tech_span_detector = TechSpanDetector::new(ext_whitelist);
        let fuzzy_matcher = if dictionary.is_empty() {
            None
        } else {
            Some(FuzzyMatcher::new(dictionary))
        };

        Self {
            spoken_symbol_map,
            tech_span_detector,
            fuzzy_matcher,
        }
    }

    /// 创建无词库的 TNL 引擎
    pub fn new_without_dictionary() -> Self {
        Self::new(Vec::new())
    }

    /// 规范化文本
    ///
    /// 纯函数，不可失败（失败时返回原文）
    pub fn normalize(&self, text: &str) -> NormalizationResult {
        let start = Instant::now();

        if text.is_empty() {
            return NormalizationResult::unchanged(String::new(), 0);
        }

        // 1. Unicode 归一化 (NFC) + 空白折叠
        let normalized = self.unicode_normalize(text);

        // 2. 分词
        let tokens = Tokenizer::tokenize(&normalized);

        // 3. 检测技术片段
        let tech_spans = self.tech_span_detector.detect(&normalized, &tokens);

        // 4. 应用口语符号映射（仅在技术片段内）
        let (mapped_text, symbol_replacements) =
            self.apply_spoken_symbol_mapping(&normalized, &tokens, &tech_spans);

        // 5. 拼音词库替换（精确匹配，带声调）
        let (replaced_text, pinyin_replacements) = self.apply_pinyin_replacement(&mapped_text);

        // 合并替换记录
        let mut applied = symbol_replacements;
        applied.extend(pinyin_replacements);

        let elapsed_us = start.elapsed().as_micros() as u64;
        let changed = replaced_text != text;

        NormalizationResult {
            text: replaced_text,
            changed,
            applied,
            technical_spans: tech_spans,
            elapsed_us,
        }
    }

    /// Unicode 归一化 + 空白折叠
    fn unicode_normalize(&self, text: &str) -> String {
        // NFC 归一化
        let nfc: String = text.nfc().collect();

        // 空白折叠：多个连续空白 -> 单个空格
        let mut result = String::with_capacity(nfc.len());
        let mut prev_whitespace = false;

        for ch in nfc.chars() {
            if ch.is_whitespace() {
                if !prev_whitespace {
                    result.push(' ');
                    prev_whitespace = true;
                }
            } else {
                result.push(ch);
                prev_whitespace = false;
            }
        }

        result.trim().to_string()
    }

    /// 应用口语符号映射
    ///
    /// 仅在技术片段内进行映射
    fn apply_spoken_symbol_mapping(
        &self,
        text: &str,
        tokens: &[Token],
        tech_spans: &[Span],
    ) -> (String, Vec<Replacement>) {
        let mut result = String::with_capacity(text.len());
        let mut replacements = Vec::new();
        let mut last_end = 0;

        for token in tokens {
            // 添加 token 之前的文本
            if token.start > last_end {
                result.push_str(&text[last_end..token.start]);
            }

            // 检查是否在技术片段内
            let in_tech_span = tech_spans
                .iter()
                .any(|s| token.start >= s.start && token.end <= s.end);

            // 尝试映射口语符号
            if in_tech_span && token.token_type == TokenType::Chinese {
                if let Some(symbol) = self.spoken_symbol_map.try_map(&token.text) {
                    result.push(symbol);
                    replacements.push(Replacement {
                        original: token.text.clone(),
                        replaced: symbol.to_string(),
                        start: token.start,
                        end: token.end,
                        confidence: 1.0,
                        reason: ReplacementReason::SpokenSymbol,
                    });
                    last_end = token.end;
                    continue;
                }
            }

            result.push_str(&token.text);
            last_end = token.end;
        }

        // 添加剩余文本
        if last_end < text.len() {
            result.push_str(&text[last_end..]);
        }

        // 后处理：移除符号前后的空格（仅限技术片段内）
        let cleaned = self.clean_spaces_around_symbols(&result, tech_spans);

        (cleaned, replacements)
    }

    /// 清理符号前后的空格（仅在技术片段内）
    fn clean_spaces_around_symbols(&self, text: &str, _tech_spans: &[Span]) -> String {
        // 简化实现：移除 . - / _ : @ 前后的空格
        let mut result = text.to_string();
        for symbol in ['.', '-', '/', '_', ':', '@'] {
            result = result.replace(&format!(" {}", symbol), &symbol.to_string());
            result = result.replace(&format!("{} ", symbol), &symbol.to_string());
        }
        result
    }

    /// 应用拼音词库替换
    ///
    /// 对连续中文片段尝试精确拼音匹配替换
    ///
    /// 约束条件：
    /// - 拼音 + 声调 100% 完全匹配
    /// - 原词 ≥2 个汉字
    /// - 同键冲突时跳过替换
    fn apply_pinyin_replacement(&self, text: &str) -> (String, Vec<Replacement>) {
        let Some(matcher) = &self.fuzzy_matcher else {
            return (text.to_string(), Vec::new());
        };

        let tokens = Tokenizer::tokenize(text);
        let mut result = String::with_capacity(text.len());
        let mut replacements: Vec<Replacement> = Vec::new();
        let mut last_end = 0;

        for token in tokens {
            // 添加 token 之前的文本
            if token.start > last_end {
                result.push_str(&text[last_end..token.start]);
            }

            if token.token_type == TokenType::Chinese {
                // 对中文 token 尝试拼音替换
                let mut local_idx = 0;
                while local_idx < token.text.len() {
                    let remaining = &token.text[local_idx..];

                    if let Some((replace_word, consumed)) =
                        matcher.try_exact_pinyin_replace(remaining)
                    {
                        let end = local_idx + consumed;
                        let original = &token.text[local_idx..end];

                        result.push_str(&replace_word);

                        // 仅当实际发生替换时记录
                        if replace_word != original {
                            replacements.push(Replacement {
                                original: original.to_string(),
                                replaced: replace_word,
                                start: token.start + local_idx,
                                end: token.start + end,
                                confidence: 1.0,
                                reason: ReplacementReason::DictionaryPinyin,
                            });
                        }

                        local_idx = end;
                        continue;
                    }

                    // 无匹配：复制单个字符
                    if let Some(ch) = remaining.chars().next() {
                        result.push(ch);
                        local_idx += ch.len_utf8();
                    }
                }
            } else {
                result.push_str(&token.text);
            }

            last_end = token.end;
        }

        // 添加剩余文本
        if last_end < text.len() {
            result.push_str(&text[last_end..]);
        }

        (result, replacements)
    }
}

impl Default for TnlEngine {
    fn default() -> Self {
        Self::new_without_dictionary()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_filename() {
        let engine = TnlEngine::default();

        let result = engine.normalize("readme 点 md");
        assert!(result.changed);
        assert_eq!(result.text, "readme.md");
    }

    #[test]
    fn test_normalize_path() {
        let engine = TnlEngine::default();

        let result = engine.normalize("src 斜杠 lib 点 rs");
        assert!(result.changed);
        assert!(result.text.contains('/') || result.text.contains('.'));
    }

    #[test]
    fn test_normalize_version() {
        let engine = TnlEngine::default();

        let result = engine.normalize("v1 点 2 点 3");
        assert!(result.changed);
        // 应该包含点号
        assert!(result.text.contains('.'));
    }

    #[test]
    fn test_no_change_natural_language() {
        let engine = TnlEngine::default();

        let result = engine.normalize("一点都不好");
        // "一点都不好" 中的"点"不在技术片段内，不应转换
        assert!(!result.changed || result.text == "一点都不好");
    }

    #[test]
    fn test_unicode_normalization() {
        let engine = TnlEngine::default();

        // 测试多个空格折叠
        let result = engine.normalize("hello    world");
        assert_eq!(result.text, "hello world");
    }

    #[test]
    fn test_performance() {
        let engine = TnlEngine::default();

        let text = "我修改了 src 斜杠 tauri 斜杠 src 斜杠 tnl 斜杠 engine 点 rs 文件";

        let result = engine.normalize(text);

        // 目标 <10ms = 10000us
        assert!(
            result.elapsed_us < 10000,
            "耗时 {}us 超过 10ms",
            result.elapsed_us
        );
    }

    #[test]
    fn test_normalize_email() {
        let engine = TnlEngine::default();

        // 测试口语邮箱
        let result = engine.normalize("1045535878 艾特 qq 点 com");
        assert!(result.changed);
        assert_eq!(result.text, "1045535878@qq.com");

        // 测试带空格的邮箱
        let result2 = engine.normalize("test 艾特 example 点 com");
        assert!(result2.changed);
        assert_eq!(result2.text, "test@example.com");
    }

    #[test]
    fn test_no_false_positive_at() {
        let engine = TnlEngine::default();

        // "I AM At the school" 不应该被转换
        let result = engine.normalize("I AM At the school");
        assert!(!result.changed || result.text == "I AM At the school");

        // "Look at this" 不应该被转换
        let result2 = engine.normalize("Look at this");
        assert!(!result2.changed || result2.text == "Look at this");
    }

    // === 拼音词库替换集成测试 ===

    #[test]
    fn test_normalize_with_pinyin_replacement() {
        // 使用同音词：事例 (shi4li4) → 示例 (shi4li4)
        let engine = TnlEngine::new(vec!["示例".to_string()]);

        let result = engine.normalize("今天看了一个事例");
        assert!(result.changed);
        assert_eq!(result.text, "今天看了一个示例");
        assert!(result
            .applied
            .iter()
            .any(|r| matches!(r.reason, ReplacementReason::DictionaryPinyin)));
    }

    #[test]
    fn test_normalize_pinyin_tone_strict() {
        // 声调不同不应替换："妈妈" (ma1ma1) vs "骂骂" (ma4ma4)
        let engine = TnlEngine::new(vec!["骂骂".to_string()]);

        let result = engine.normalize("妈妈来了");
        assert!(!result.changed);
        assert_eq!(result.text, "妈妈来了");
    }

    #[test]
    fn test_normalize_pinyin_min_length() {
        // 单字不替换
        let engine = TnlEngine::new(vec!["马".to_string()]);

        let result = engine.normalize("一匹麻");
        // "麻" 是单字，不应被替换
        assert!(!result.changed || !result.text.contains("马"));
    }

    #[test]
    fn test_normalize_pinyin_conflict() {
        // 同音词冲突：公式 vs 公事 (gong1shi4)
        let engine = TnlEngine::new(vec!["公式".to_string(), "公事".to_string()]);

        let result = engine.normalize("处理攻势");
        // 冲突时不替换
        assert!(!result.changed || result.text == "处理攻势");
    }

    #[test]
    fn test_normalize_combined_replacements() {
        // 同时测试口语符号映射和拼音替换
        let engine = TnlEngine::new(vec!["示例".to_string()]);

        let result = engine.normalize("查看 readme 点 md 中的事例");
        assert!(result.changed);
        // 应该同时包含口语符号替换和拼音替换
        assert!(result.text.contains("readme.md"));
        assert!(result.text.contains("示例"));
    }
}
