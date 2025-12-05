// src-tauri/src/config.rs

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub dashscope_api_key: String,
    #[serde(default)]
    pub siliconflow_api_key: String,
    #[serde(default = "default_use_realtime_asr")]
    pub use_realtime_asr: bool,
    #[serde(default)]
    pub enable_llm_post_process: bool,
    #[serde(default)]
    pub llm_config: LlmConfig,
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
            use_realtime_asr: default_use_realtime_asr(),
            enable_llm_post_process: false,
            llm_config: LlmConfig::default(),
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
            
            // 修改这里：直接反序列化，不再强制填充默认值
            // 如果用户把 presets 删光了，这里读出来的就是空的，我们尊重用户的选择
            let mut config: AppConfig = serde_json::from_str(&content)?;
            
            // 只有当这是极其古老的配置文件（完全没有 presets 字段时），serde 才会使用 Default trait
            // 这里我们做一个最小的防守：如果当前没有任何 active_preset_id，由于逻辑需要，我们重置为默认的第一个
            // 但如果 presets 列表是空的（用户删光了），我们就不管了，前端会处理显示问题
            if config.llm_config.presets.is_empty() {
                 // 如果用户真的删光了所有预设，为了防止程序出错，我们可以不仅不做操作
                 // 或者你可以选择在这里恢复默认，看你的需求。
                 // 既然你希望"删除了既定的，会永久删除"，那么这里我们什么都不做。
                 tracing::info!("检测到预设列表为空，用户可能删除了所有预设");
            }

            tracing::info!("配置加载成功");
            Ok(config)
        } else {
            tracing::warn!("配置文件不存在，创建并返回默认配置");
            // 只有在第一次运行（没有配置文件）时，才使用 default_presets() 里定义的那个单一预设
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