// src-tauri/src/config.rs

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AsrProvider {
    Qwen,
    Doubao,
    #[serde(rename = "siliconflow")]
    SiliconFlow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsrProviderConfig {
    pub provider: AsrProvider,
    pub api_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsrConfig {
    pub primary: AsrProviderConfig,
    #[serde(default)]
    pub fallback: Option<AsrProviderConfig>,
    #[serde(default)]
    pub enable_fallback: bool,
}

impl Default for AsrConfig {
    fn default() -> Self {
        Self {
            primary: AsrProviderConfig {
                provider: AsrProvider::Qwen,
                api_key: String::new(),
                app_id: None,
                access_token: None,
            },
            fallback: None,
            enable_fallback: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub dashscope_api_key: String,
    #[serde(default)]
    pub siliconflow_api_key: String,
    #[serde(default)]
    pub asr_config: AsrConfig,
    #[serde(default = "default_use_realtime_asr")]
    pub use_realtime_asr: bool,
    #[serde(default)]
    pub enable_llm_post_process: bool,
    #[serde(default)]
    pub llm_config: LlmConfig,
    /// 关闭行为: "close" = 直接关闭, "minimize" = 最小化到托盘, None = 每次询问
    #[serde(default)]
    pub close_action: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmPreset {
    pub id: String,
    pub name: String,
    pub system_prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    #[serde(default = "default_llm_endpoint")]
    pub endpoint: String,
    #[serde(default = "default_llm_model")]
    pub model: String,
    #[serde(default)]
    pub api_key: String,
    
    // 新增：预设列表和当前选中的预设ID
    #[serde(default = "default_presets")]
    pub presets: Vec<LlmPreset>,
    #[serde(default = "default_active_preset_id")]
    pub active_preset_id: String,
}

fn default_llm_endpoint() -> String {
    "https://open.bigmodel.cn/api/paas/v4/chat/completions".to_string()
}

fn default_llm_model() -> String {
    "glm-4-flash-250414".to_string()
}

// 默认预设生成逻辑
fn default_presets() -> Vec<LlmPreset> {
    vec![
        LlmPreset {
            id: "polishing".to_string(),
            name: "文本润色".to_string(),
            system_prompt: "你是一个语音转写润色助手。请在不改变原意的前提下：1）删除重复或意义相近的句子；2）合并同一主题的内容；3）去除「嗯」「啊」等口头禅；4）保留数字与关键信息；5）相关数字和时间不要使用中文；6）整理成自然的段落。输出纯文本即可。".to_string(),
        },
        LlmPreset {
            id: "translation".to_string(),
            name: "中译英".to_string(),
            system_prompt: "你是一个专业的翻译助手。请将用户的中文语音转写内容翻译成地道、流畅的英文。不要输出任何解释性文字，只输出翻译结果。".to_string(),
        }
    ]
}

fn default_active_preset_id() -> String {
    "polishing".to_string()
}

// 为了兼容旧版本配置，如果反序列化时 presets 为空，手动填充默认值
impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            endpoint: default_llm_endpoint(),
            model: default_llm_model(),
            api_key: String::new(),
            presets: default_presets(),
            active_preset_id: default_active_preset_id(),
        }
    }
}

fn default_use_realtime_asr() -> bool {
    true
}

impl AppConfig {
    pub fn new() -> Self {
        Self {
            dashscope_api_key: String::new(),
            siliconflow_api_key: String::new(),
            asr_config: AsrConfig::default(),
            use_realtime_asr: default_use_realtime_asr(),
            enable_llm_post_process: false,
            llm_config: LlmConfig::default(),
            close_action: None,
        }
    }

    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("无法获取配置目录"))?;
        let app_dir = config_dir.join("PushToTalk");
        std::fs::create_dir_all(&app_dir)?;
        Ok(app_dir.join("config.json"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        tracing::info!("尝试从以下路径加载配置: {:?}", path);
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let mut config: AppConfig = serde_json::from_str(&content)?;

            // 迁移逻辑：如果 asr_config 为空但旧字段有值，自动迁移
            if config.asr_config.primary.api_key.is_empty() && !config.dashscope_api_key.is_empty() {
                tracing::info!("检测到旧配置格式，自动迁移到新格式");
                config.asr_config.primary = AsrProviderConfig {
                    provider: AsrProvider::Qwen,
                    api_key: config.dashscope_api_key.clone(),
                    app_id: None,
                    access_token: None,
                };
                if !config.siliconflow_api_key.is_empty() {
                    config.asr_config.fallback = Some(AsrProviderConfig {
                        provider: AsrProvider::SiliconFlow,
                        api_key: config.siliconflow_api_key.clone(),
                        app_id: None,
                        access_token: None,
                    });
                    config.asr_config.enable_fallback = true;
                }
            }

            if config.llm_config.presets.is_empty() {
                 tracing::info!("检测到预设列表为空，用户可能删除了所有预设");
            }

            tracing::info!("配置加载成功");
            Ok(config)
        } else {
            tracing::warn!("配置文件不存在，创建并返回默认配置");
            Ok(Self::new())
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        let content = serde_json::to_string_pretty(self)?;
        tracing::info!("保存配置到: {:?}", path);
        std::fs::write(&path, content)?;
        tracing::info!("配置保存成功");
        Ok(())
    }
}