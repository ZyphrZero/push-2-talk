//! TNL 技术片段识别
//!
//! 识别文件名、路径、版本号等技术串

use crate::tnl::rules::ExtensionWhitelist;
use crate::tnl::tokenizer::{Token, TokenType};
use crate::tnl::types::{Span, SpanType};

/// 技术片段识别器
pub struct TechSpanDetector {
    ext_whitelist: ExtensionWhitelist,
}

impl TechSpanDetector {
    pub fn new(ext_whitelist: ExtensionWhitelist) -> Self {
        Self { ext_whitelist }
    }

    /// 检测技术片段
    ///
    /// 返回识别到的技术片段列表
    pub fn detect(&self, text: &str, tokens: &[Token]) -> Vec<Span> {
        let mut spans = Vec::new();

        // 策略 1: 检测文件名模式 (xxx.ext)
        spans.extend(self.detect_file_names(text, tokens));

        // 策略 2: 检测路径模式 (包含 / 或 \)
        spans.extend(self.detect_paths(text, tokens));

        // 策略 3: 检测版本号模式 (v1.2.3 或 1.2.3)
        spans.extend(self.detect_versions(text, tokens));

        // 策略 4: 检测邮箱模式 (xxx@xxx.xxx 或 xxx艾特xxx点xxx)
        spans.extend(self.detect_emails(text, tokens));

        // 去重并合并重叠片段
        self.merge_overlapping(spans)
    }

    /// 检测文件名模式
    fn detect_file_names(&self, text: &str, tokens: &[Token]) -> Vec<Span> {
        let mut spans = Vec::new();

        // 查找 "xxx . ext" 或 "xxx 点 ext" 模式
        for (i, token) in tokens.iter().enumerate() {
            // 查找点号或口语"点"
            let is_dot = token.text == "." || token.text == "点" || token.text == "點";
            if !is_dot {
                continue;
            }

            // 向前查找文件名部分（跳过空白）
            let prev_idx = self.find_prev_ascii(tokens, i);
            // 向后查找扩展名部分（跳过空白）
            let next_idx = self.find_next_ascii(tokens, i);

            if let (Some(prev), Some(next)) = (prev_idx, next_idx) {
                let ext = &tokens[next].text;
                if self.ext_whitelist.contains(ext) {
                    let start = tokens[prev].start;
                    let end = tokens[next].end;
                    spans.push(Span {
                        text: text[start..end].to_string(),
                        start,
                        end,
                        span_type: SpanType::FileName,
                    });
                }
            }
        }

        spans
    }

    /// 检测路径模式
    fn detect_paths(&self, text: &str, tokens: &[Token]) -> Vec<Span> {
        let mut spans = Vec::new();

        // 查找包含 / 或 \ 或 口语"斜杠" 的序列
        let mut path_start: Option<usize> = None;
        let mut last_path_end: Option<usize> = None;

        for (i, token) in tokens.iter().enumerate() {
            let is_path_sep = token.text == "/"
                || token.text == "\\"
                || token.text == "斜杠"
                || token.text == "斜线";

            if is_path_sep {
                if path_start.is_none() {
                    // 向前扩展到 ASCII token
                    if let Some(prev) = self.find_prev_ascii(tokens, i) {
                        path_start = Some(tokens[prev].start);
                    }
                }
                // 向后扩展
                if let Some(next) = self.find_next_ascii(tokens, i) {
                    last_path_end = Some(tokens[next].end);
                }
            } else if path_start.is_some()
                && token.token_type != TokenType::Ascii
                && token.token_type != TokenType::Whitespace
                && token.text != "."
                && token.text != "点"
            {
                // 遇到非路径字符，结束当前路径
                if let (Some(start), Some(end)) = (path_start, last_path_end) {
                    if end > start {
                        spans.push(Span {
                            text: text[start..end].to_string(),
                            start,
                            end,
                            span_type: SpanType::Path,
                        });
                    }
                }
                path_start = None;
                last_path_end = None;
            }
        }

        // 处理末尾的路径
        if let (Some(start), Some(end)) = (path_start, last_path_end) {
            if end > start {
                spans.push(Span {
                    text: text[start..end].to_string(),
                    start,
                    end,
                    span_type: SpanType::Path,
                });
            }
        }

        spans
    }

    /// 检测版本号模式
    fn detect_versions(&self, text: &str, tokens: &[Token]) -> Vec<Span> {
        let mut spans = Vec::new();

        // 查找 v1.2.3 或 1.2.3 模式
        for (i, token) in tokens.iter().enumerate() {
            // 检查是否以 v/V 开头或纯数字
            let starts_version = token.text.starts_with('v')
                || token.text.starts_with('V')
                || token
                    .text
                    .chars()
                    .next()
                    .map(|c| c.is_ascii_digit())
                    .unwrap_or(false);

            if !starts_version || token.token_type != TokenType::Ascii {
                continue;
            }

            // 向后查找 .数字 或 点 数字 序列
            let mut version_end = token.end;
            let mut j = i + 1;
            let mut has_dot = false;

            while j < tokens.len() {
                let t = &tokens[j];

                // 跳过空白
                if t.token_type == TokenType::Whitespace {
                    j += 1;
                    continue;
                }

                // 检查点号
                if t.text == "." || t.text == "点" || t.text == "點" {
                    has_dot = true;
                    j += 1;
                    continue;
                }

                // 检查数字
                if has_dot
                    && t.token_type == TokenType::Ascii
                    && t.text
                        .chars()
                        .all(|c| c.is_ascii_digit() || c == '-' || c.is_ascii_alphabetic())
                {
                    version_end = t.end;
                    has_dot = false; // 重置，等待下一个点
                    j += 1;
                    continue;
                }

                break;
            }

            // 如果有扩展（至少一个点+数字），则认为是版本号
            if version_end > token.end {
                spans.push(Span {
                    text: text[token.start..version_end].to_string(),
                    start: token.start,
                    end: version_end,
                    span_type: SpanType::Version,
                });
            }
        }

        spans
    }

    /// 检测邮箱模式
    ///
    /// 识别 xxx@xxx.xxx 或 xxx艾特xxx点xxx 模式
    fn detect_emails(&self, text: &str, tokens: &[Token]) -> Vec<Span> {
        let mut spans = Vec::new();

        // 查找 @ 或 "艾特" 或 "at"
        for (i, token) in tokens.iter().enumerate() {
            let is_at = token.text == "@" || token.text == "艾特" || token.text.to_lowercase() == "at";
            if !is_at {
                continue;
            }

            // 向前查找用户名部分（ASCII，跳过空白）
            let prev_idx = self.find_prev_ascii(tokens, i);
            // 向后查找域名部分
            let domain_result = self.find_email_domain(tokens, i);

            if let (Some(prev), Some((domain_end_idx, has_dot))) = (prev_idx, domain_result) {
                // 邮箱必须有点号（xxx.com）
                if has_dot {
                    let start = tokens[prev].start;
                    let end = tokens[domain_end_idx].end;
                    spans.push(Span {
                        text: text[start..end].to_string(),
                        start,
                        end,
                        span_type: SpanType::Email,
                    });
                }
            }
        }

        spans
    }

    /// 查找邮箱域名部分
    ///
    /// 返回 (结束 token 索引, 是否包含点号)
    fn find_email_domain(&self, tokens: &[Token], from: usize) -> Option<(usize, bool)> {
        let mut last_ascii_idx: Option<usize> = None;
        let mut has_dot = false;
        let mut j = from + 1;

        while j < tokens.len() {
            let t = &tokens[j];

            // 跳过空白
            if t.token_type == TokenType::Whitespace {
                j += 1;
                continue;
            }

            // 检查点号或口语"点"
            if t.text == "." || t.text == "点" || t.text == "點" {
                has_dot = true;
                j += 1;
                continue;
            }

            // 检查 ASCII（域名部分）
            if t.token_type == TokenType::Ascii {
                last_ascii_idx = Some(j);
                j += 1;
                continue;
            }

            // 遇到其他字符，结束
            break;
        }

        last_ascii_idx.map(|idx| (idx, has_dot))
    }

    /// 向前查找最近的 ASCII token（跳过空白）
    fn find_prev_ascii(&self, tokens: &[Token], from: usize) -> Option<usize> {
        if from == 0 {
            return None;
        }
        for i in (0..from).rev() {
            if tokens[i].token_type == TokenType::Whitespace {
                continue;
            }
            if tokens[i].token_type == TokenType::Ascii {
                return Some(i);
            }
            break;
        }
        None
    }

    /// 向后查找最近的 ASCII token（跳过空白）
    fn find_next_ascii(&self, tokens: &[Token], from: usize) -> Option<usize> {
        for i in (from + 1)..tokens.len() {
            if tokens[i].token_type == TokenType::Whitespace {
                continue;
            }
            if tokens[i].token_type == TokenType::Ascii {
                return Some(i);
            }
            break;
        }
        None
    }

    /// 合并重叠片段
    fn merge_overlapping(&self, mut spans: Vec<Span>) -> Vec<Span> {
        if spans.is_empty() {
            return spans;
        }

        // 按起始位置排序
        spans.sort_by_key(|s| s.start);

        let mut merged = Vec::new();
        let mut current = spans.remove(0);

        for span in spans {
            if span.start <= current.end {
                // 重叠，合并
                if span.end > current.end {
                    current.end = span.end;
                    // 优先级：Path > FileName > Version > Technical
                    if matches!(span.span_type, SpanType::Path) {
                        current.span_type = SpanType::Path;
                    }
                }
            } else {
                merged.push(current);
                current = span;
            }
        }
        merged.push(current);

        merged
    }
}

impl Default for TechSpanDetector {
    fn default() -> Self {
        Self::new(ExtensionWhitelist::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tnl::tokenizer::Tokenizer;

    #[test]
    fn test_detect_filename() {
        let detector = TechSpanDetector::default();
        let text = "修改了 readme 点 md 文件";
        let tokens = Tokenizer::tokenize(text);
        let spans = detector.detect(text, &tokens);

        assert!(!spans.is_empty());
        assert!(spans.iter().any(|s| s.span_type == SpanType::FileName));
    }

    #[test]
    fn test_detect_path() {
        let detector = TechSpanDetector::default();
        let text = "打开 src 斜杠 lib 点 rs";
        let tokens = Tokenizer::tokenize(text);
        let spans = detector.detect(text, &tokens);

        assert!(!spans.is_empty());
    }

    #[test]
    fn test_detect_version() {
        let detector = TechSpanDetector::default();
        let text = "升级到 v1 点 2 点 3";
        let tokens = Tokenizer::tokenize(text);
        let spans = detector.detect(text, &tokens);

        assert!(!spans.is_empty());
        assert!(spans.iter().any(|s| s.span_type == SpanType::Version));
    }

    #[test]
    fn test_detect_email() {
        let detector = TechSpanDetector::default();

        // 测试口语邮箱
        let text = "1045535878 艾特 qq 点 com";
        let tokens = Tokenizer::tokenize(text);
        let spans = detector.detect(text, &tokens);
        assert!(!spans.is_empty());
        assert!(spans.iter().any(|s| s.span_type == SpanType::Email));

        // 测试标准邮箱
        let text2 = "test@example.com";
        let tokens2 = Tokenizer::tokenize(text2);
        let spans2 = detector.detect(text2, &tokens2);
        assert!(!spans2.is_empty());
        assert!(spans2.iter().any(|s| s.span_type == SpanType::Email));
    }
}
