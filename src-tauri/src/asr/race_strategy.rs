use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use anyhow::Result;
use super::{QwenASRClient, SenseVoiceClient};

pub async fn transcribe_with_fallback(
    qwen_api_key: String,
    sensevoice_api_key: String,
    audio_path: &Path,
) -> Result<String> {
    let audio_data = tokio::fs::read(audio_path).await?;
    transcribe_with_fallback_bytes(qwen_api_key, sensevoice_api_key, audio_data).await
}

pub async fn transcribe_with_fallback_bytes(
    qwen_api_key: String,
    sensevoice_api_key: String,
    audio_data: Vec<u8>,
) -> Result<String> {
    let qwen_client = QwenASRClient::new(qwen_api_key);
    let sensevoice_client = SenseVoiceClient::new(sensevoice_api_key);
    transcribe_with_fallback_clients(qwen_client, sensevoice_client, audio_data).await
}

pub async fn transcribe_with_fallback_clients(
    qwen_client: QwenASRClient,
    sensevoice_client: SenseVoiceClient,
    audio_data: Vec<u8>,
) -> Result<String> {
    tracing::info!("启动主备并行转录 (内存模式), 音频大小: {} bytes", audio_data.len());

    let audio_data_sensevoice = audio_data.clone();
    let sensevoice_result: Arc<Mutex<Option<Result<String>>>> = Arc::new(Mutex::new(None));
    let sensevoice_result_clone = Arc::clone(&sensevoice_result);

    let sensevoice_handle = tokio::spawn(async move {
        tracing::info!("🚀 SenseVoice 任务启动");
        let result = sensevoice_client.transcribe_bytes(&audio_data_sensevoice).await;
        match &result {
            Ok(text) => tracing::info!("✅SenseVoice 转录成功: {}", text),
            Err(e) => tracing::error!("❌SenseVoice 转录失败: {}", e),
        }
        *sensevoice_result_clone.lock().unwrap() = Some(result);
    });

    let max_retries = 2;
    let mut qwen_last_error = None;

    for attempt in 0..=max_retries {
        if attempt > 0 {
            tracing::warn!("⏳千问第{} 次重试前，检查 SenseVoice 结果...", attempt);

            if let Some(sv_result) = sensevoice_result.lock().unwrap().as_ref() {
                match sv_result {
                    Ok(text) => {
                        tracing::info!("✅千问重试前发现 SenseVoice 已成功，立即使用: {}", text);
                        return Ok(text.clone());
                    }
                    Err(e) => {
                        tracing::warn!("⚠️ SenseVoice 也失败了: {}，继续千问重试", e);
                    }
                }
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        tracing::info!("🔄 千问第{} 次尝试(共{} 次)", attempt + 1, max_retries + 1);
        match qwen_client.transcribe_from_memory(&audio_data).await {
            Ok(text) => {
                tracing::info!("✅千问转录成功: {}", text);
                return Ok(text);
            }
            Err(e) => {
                tracing::error!("❌千问第{} 次尝试失败 {}", attempt + 1, e);
                qwen_last_error = Some(e);
            }
        }
    }

    tracing::warn!("⚠️ 千问全部失败，等待 SenseVoice 最终结果...");
    let _ = sensevoice_handle.await;

    if let Some(result) = sensevoice_result.lock().unwrap().take() {
        match result {
            Ok(text) => {
                tracing::info!("✅使用 SenseVoice 备用结果: {}", text);
                return Ok(text);
            }
            Err(sensevoice_error) => {
                tracing::error!("❌两个 API 都失败了");
                tracing::error!("   千问错误: {:?}", qwen_last_error);
                tracing::error!("   SenseVoice 错误: {:?}", sensevoice_error);
                return Err(anyhow::anyhow!(
                    "两个 API 都失败- 千问: {:?}, SenseVoice: {}",
                    qwen_last_error,
                    sensevoice_error
                ));
            }
        }
    }

    Err(anyhow::anyhow!("所有API都失败"))
}
