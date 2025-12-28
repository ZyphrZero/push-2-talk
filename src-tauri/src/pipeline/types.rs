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

/// 转录上下文
///
/// 用于智能指令模式等需要额外上下文信息的场景
#[derive(Debug, Clone, Default)]
pub struct TranscriptionContext {
    /// 用户选中的文本
    pub selected_text: Option<String>,
}

impl TranscriptionContext {
    /// 创建空上下文
    pub fn empty() -> Self {
        Self::default()
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
}
