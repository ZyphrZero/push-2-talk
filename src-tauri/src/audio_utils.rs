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

/// 计算原始 RMS（不带归一化）
pub fn calculate_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() { return 0.0; }
    let sum: f64 = samples.iter().map(|&s| (s as f64).powi(2)).sum();
    (sum / samples.len() as f64).sqrt() as f32
}

/// AGC：自动增益控制（带平滑处理）
/// current_gain: 当前增益状态，用于平滑过渡
pub fn apply_agc(samples: &mut [f32], current_gain: &mut f32) {
    const TARGET_RMS: f32 = 0.10;   // 目标 RMS，平衡小声音放大
    const MAX_GAIN: f32 = 5.0;      // 最大增益，平衡微弱声音和抗噪能力
    const MIN_GAIN: f32 = 0.1;      // 允许大幅衰减，压住大嗓门
    const NOISE_FLOOR: f32 = 0.003; // 底噪阈值，平衡灵敏度和抗噪能力

    let rms = calculate_rms(samples);

    // 计算目标增益，底噪时保持 1.0
    let target_gain = if rms < NOISE_FLOOR {
        1.0
    } else {
        (TARGET_RMS / rms).clamp(MIN_GAIN, MAX_GAIN)
    };

    // 增益平滑：Attack 快（防爆音），Release 慢（防呼吸效应）
    let alpha = if target_gain < *current_gain { 0.5 } else { 0.1 };
    *current_gain = *current_gain * (1.0 - alpha) + target_gain * alpha;

    for s in samples.iter_mut() {
        *s = (*s * *current_gain).tanh();
    }
}

/// VAD：基于 RMS 阈值判断是否有语音
pub fn is_voice_active(samples: &[f32]) -> bool {
    const THRESHOLD: f32 = 0.003; // 与 NOISE_FLOOR 对齐，平衡灵敏度和抗噪能力
    calculate_rms(samples) > THRESHOLD
}
