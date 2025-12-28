// 音频处理通用工具模块
// 提供音频级别计算、事件发送等共享功能

use tauri::{AppHandle, Emitter};

/// 音频级别事件 payload
#[derive(Clone, serde::Serialize)]
pub struct AudioLevelPayload {
    pub level: f32,
}

/// 计算音频样本的 RMS 音量级别（0.0 到 1.0）
pub fn calculate_audio_level(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }

    // 计算 RMS (Root Mean Square)
    let sum: f64 = samples.iter()
        .map(|&s| (s as f64).powi(2))
        .sum();
    let rms = (sum / samples.len() as f64).sqrt() as f32;

    // 将 RMS 值映射到 0.0-1.0 范围，并应用一些增益使其更敏感
    // 语音通常在 0.01-0.3 RMS 范围内
    let normalized = (rms * 5.0).min(1.0);

    // 应用简单的对数缩放使低音量更明显
    if normalized > 0.0 {
        (normalized.ln() + 4.0) / 4.0
    } else {
        0.0
    }.max(0.0).min(1.0)
}

/// 发送音频级别事件到前端
pub fn emit_audio_level(app: &AppHandle, level: f32) {
    let _ = app.emit("audio_level_update", AudioLevelPayload { level });
}
