// Pipeline 核心类型定义
//
// 定义了处理管道所需的所有类型，包括：
// - 转录模式 (TranscriptionMode)
// - 转录上下文 (TranscriptionContext)
// - 处理结果 (PipelineResult)

use serde::{Deserialize, Serialize};

/// 转录处理模式
///
/// 决定 ASR 结果如何被后续处理
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionMode {
    /// 普通模式：ASR → 可选LLM润色 → 自动插入文本
    #[default]
    Normal,
    /// AI 助手模式：语音指令 → ASR → LLM处理 → 插入结果
    Assistant,
}

impl TranscriptionMode {
    /// 获取模式的显示名称
    pub fn display_name(&self) -> &'static str {
        match self {
            TranscriptionMode::Normal => "普通模式",
            TranscriptionMode::Assistant => "AI助手",
        }
    }

    /// 该模式是否需要自动插入文本
    pub fn should_auto_insert(&self) -> bool {
        match self {
            TranscriptionMode::Normal => true,
            TranscriptionMode::Assistant => true,
        }
    }

    /// 该模式是否必须使用 LLM 处理
    pub fn requires_llm(&self) -> bool {
        match self {
            TranscriptionMode::Normal => false,  // LLM 是可选的
            TranscriptionMode::Assistant => true,  // 必须有 LLM
        }
    }
}

/// 转录上下文
///
/// 用于智能指令模式等需要额外上下文信息的场景
#[derive(Debug, Clone, Default)]
pub struct TranscriptionContext {
    /// 屏幕截图（Base64 编码）
    pub screen_capture: Option<String>,
    /// 当前活动窗口标题
    pub active_window_title: Option<String>,
    /// 当前活动窗口的进程名
    pub active_window_process: Option<String>,
    /// 用户选中的文本
    pub selected_text: Option<String>,
    /// 剪贴板内容
    pub clipboard_content: Option<String>,
}

impl TranscriptionContext {
    /// 创建空上下文
    pub fn empty() -> Self {
        Self::default()
    }

    /// 检查上下文是否为空
    pub fn is_empty(&self) -> bool {
        self.screen_capture.is_none()
            && self.active_window_title.is_none()
            && self.active_window_process.is_none()
            && self.selected_text.is_none()
            && self.clipboard_content.is_none()
    }

    /// 构建上下文描述（用于 LLM prompt）
    pub fn build_description(&self) -> String {
        let mut parts = Vec::new();

        if let Some(ref title) = self.active_window_title {
            parts.push(format!("当前窗口: {}", title));
        }
        if let Some(ref process) = self.active_window_process {
            parts.push(format!("应用程序: {}", process));
        }
        if let Some(ref text) = self.selected_text {
            parts.push(format!("选中文本: {}", text));
        }
        if let Some(ref clipboard) = self.clipboard_content {
            parts.push(format!("剪贴板: {}", clipboard));
        }
        if self.screen_capture.is_some() {
            parts.push("（包含屏幕截图）".to_string());
        }

        if parts.is_empty() {
            "无上下文信息".to_string()
        } else {
            parts.join("\n")
        }
    }
}

/// Pipeline 处理结果
///
/// 兼容现有的 TranscriptionResult，同时支持扩展字段
#[derive(Debug, Clone, Serialize)]
pub struct PipelineResult {
    /// 最终处理后的文本
    pub text: String,
    /// 原始 ASR 文本（仅在 LLM 处理后与 text 不同时有值）
    pub original_text: Option<String>,
    /// ASR 耗时（毫秒）
    pub asr_time_ms: u64,
    /// LLM 处理耗时（毫秒）
    pub llm_time_ms: Option<u64>,
    /// 总耗时（毫秒）
    pub total_time_ms: u64,
    /// 处理模式
    pub mode: TranscriptionMode,
    /// 是否已自动插入文本
    pub inserted: bool,
}

impl PipelineResult {
    /// 创建成功结果
    pub fn success(
        text: String,
        original_text: Option<String>,
        asr_time_ms: u64,
        llm_time_ms: Option<u64>,
        mode: TranscriptionMode,
        inserted: bool,
    ) -> Self {
        Self {
            text,
            original_text,
            asr_time_ms,
            llm_time_ms,
            total_time_ms: asr_time_ms + llm_time_ms.unwrap_or(0),
            mode,
            inserted,
        }
    }

    /// 创建仅 ASR 的结果（无 LLM 处理）
    pub fn asr_only(text: String, asr_time_ms: u64, mode: TranscriptionMode, inserted: bool) -> Self {
        Self::success(text, None, asr_time_ms, None, mode, inserted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transcription_mode_properties() {
        assert!(TranscriptionMode::Normal.should_auto_insert());
        assert!(TranscriptionMode::Assistant.should_auto_insert());

        assert!(!TranscriptionMode::Normal.requires_llm());
        assert!(TranscriptionMode::Assistant.requires_llm());
    }

    #[test]
    fn test_context_description() {
        let ctx = TranscriptionContext {
            active_window_title: Some("VS Code".to_string()),
            selected_text: Some("Hello World".to_string()),
            ..Default::default()
        };

        let desc = ctx.build_description();
        assert!(desc.contains("VS Code"));
        assert!(desc.contains("Hello World"));
    }
}
