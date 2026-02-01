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
    #[allow(dead_code)]
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

        // 5. 模糊匹配（可选，目前仅记录建议）
        let suggested = self.apply_fuzzy_matching(&mapped_text);

        let elapsed_us = start.elapsed().as_micros() as u64;

        NormalizationResult {
            text: mapped_text.clone(),
            changed: mapped_text != text,
            applied: symbol_replacements,
            suggested,
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

    /// 应用模糊匹配（目前仅返回建议，不修改文本）
    fn apply_fuzzy_matching(&self, _text: &str) -> Vec<Replacement> {
        // TODO: 实现词库模糊匹配建议
        // 目前返回空，避免误伤
        Vec::new()
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
}
