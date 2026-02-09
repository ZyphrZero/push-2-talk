use crate::asr::utils;
use crate::config::AsrLanguageMode;
use crate::dictionary_utils::entries_to_words;
use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};

const DOUBAO_API_URL: &str = "https://openspeech.bytedance.com/api/v3/auc/bigmodel/recognize/flash";
const RESOURCE_ID: &str = "volc.bigasr.auc_turbo";

fn build_context_data(language_mode: AsrLanguageMode) -> serde_json::Value {
    match language_mode {
        AsrLanguageMode::Auto => serde_json::json!([
            {"text": "当前场景为技术听写，中英文混合"},
            {"text": "保留英文专有名词和技术术语，如 Kubernetes, GPT-4o, Claude"},
            {"text": "保留语气词，去除尾部句号"},
        ]),
        AsrLanguageMode::Zh => serde_json::json!([
            {"text": "你好，请问有什么可以帮您的"},
            {"text": "豆包语音识别真的不错呀"},
            {"text": "当前聊天的场景是日常聊天，因此保留语气词，去除尾部句号"},
        ]),
    }
}

#[derive(Clone)]
pub struct DoubaoASRClient {
    app_id: String,
    access_key: String,
    client: reqwest::Client,
    dictionary: Vec<String>,
    language_mode: AsrLanguageMode,
}

impl DoubaoASRClient {
    pub fn new(
        app_id: String,
        access_key: String,
        dictionary: Vec<String>,
        language_mode: AsrLanguageMode,
    ) -> Self {
        Self {
            app_id,
            access_key,
            client: utils::create_http_client(),
            dictionary,
            language_mode,
        }
    }

    /// 热更新词库
    pub fn update_dictionary(&mut self, dictionary: Vec<String>) {
        self.dictionary = dictionary;
    }

    pub async fn transcribe_bytes(&self, audio_data: &[u8]) -> Result<String> {
        let audio_base64 = general_purpose::STANDARD.encode(audio_data);
        tracing::info!("豆包 ASR: 音频数据大小 {} bytes", audio_data.len());

        // 构建词库 hotwords JSON（提纯后）
        let corpus = if !self.dictionary.is_empty() {
            let purified_words = entries_to_words(&self.dictionary);
            let hotwords: Vec<serde_json::Value> = purified_words
                .iter()
                .map(|w| serde_json::json!({"word": w}))
                .collect();
            let context_data = build_context_data(self.language_mode);
            let context = serde_json::json!({
                "context_type": "dialog_ctx",
                "context_data": context_data,
                "hotwords": hotwords,
            })
            .to_string();
            tracing::info!(
                "豆包 HTTP ASR 词库: {} 个词（已提纯）, context={}",
                purified_words.len(),
                context
            );
            Some(serde_json::json!({"context": context}))
        } else {
            let context_data = build_context_data(self.language_mode);
            let context = serde_json::json!({
                "context_type": "dialog_ctx",
                "context_data": context_data,
            })
            .to_string();
            tracing::info!("豆包 HTTP ASR 词库: 未配置");
            Some(serde_json::json!({"context": context}))
        };

        let mut request_obj = serde_json::json!({"model_name": "bigmodel"});
        if let Some(c) = corpus {
            request_obj["corpus"] = c;
        }
        //NOTE: 实验性功能，能提升性能
        request_obj["model_version"] = "400".into();
        request_obj["enable_ddc"] = true.into();

        let request_body = serde_json::json!({
            "user": {
                "uid": &self.app_id
            },
            "audio": {
                "data": audio_base64
            },
            "request": request_obj
        });

        let request_id = uuid::Uuid::new_v4().to_string();

        let response = self
            .client
            .post(DOUBAO_API_URL)
            .header("X-Api-App-Key", &self.app_id)
            .header("X-Api-Access-Key", &self.access_key)
            .header("X-Api-Resource-Id", RESOURCE_ID)
            .header("X-Api-Request-Id", &request_id)
            .header("X-Api-Sequence", "-1")
            .json(&request_body)
            .send()
            .await?;

        // 检查响应头中的状态码
        let status_code = response
            .headers()
            .get("X-Api-Status-Code")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        let api_message = response
            .headers()
            .get("X-Api-Message")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        tracing::info!(
            "豆包 ASR 响应: status_code={}, message={}",
            status_code,
            api_message
        );

        if status_code != "20000000" {
            anyhow::bail!("豆包 ASR 失败 ({}): {}", status_code, api_message);
        }

        let result: serde_json::Value = response.json().await?;
        tracing::debug!(
            "豆包 ASR 响应体: {}",
            serde_json::to_string_pretty(&result)?
        );

        let mut text = result["result"]["text"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("无法解析豆包转录结果"))?
            .to_string();

        utils::strip_trailing_punctuation(&mut text);
        tracing::info!("豆包 ASR 转录完成: {}", text);
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::build_context_data;
    use crate::config::AsrLanguageMode;

    #[test]
    fn build_context_data_uses_mixed_prompt_for_auto() {
        let context_data = build_context_data(AsrLanguageMode::Auto);
        let joined = context_data
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|item| item["text"].as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(joined.contains("中英文混合"));
    }

    #[test]
    fn build_context_data_uses_chat_prompt_for_zh() {
        let context_data = build_context_data(AsrLanguageMode::Zh);
        let joined = context_data
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|item| item["text"].as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(joined.contains("日常聊天"));
    }
}
