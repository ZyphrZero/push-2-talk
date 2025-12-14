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

/// 音频级别发送器
/// 封装计数器逻辑，控制发送频率
#[allow(dead_code)]
pub struct AudioLevelEmitter {
    counter: u32,
    /// 每 N 次调用发送一次
    emit_interval: u32,
    /// 每 N 次调用打印一次日志（0 表示不打印）
    log_interval: u32,
}

impl AudioLevelEmitter {
    /// 创建新的音频级别发送器
    /// - emit_interval: 每 N 次调用发送一次事件
    /// - log_interval: 每 N 次调用打印一次日志（0 表示不打印）
    pub fn new(emit_interval: u32, log_interval: u32) -> Self {
        Self {
            counter: 0,
            emit_interval,
            log_interval,
        }
    }

    /// 处理音频样本并可能发送事件
    /// 返回是否发送了事件
    pub fn process(&mut self, app: &AppHandle, samples: &[f32]) -> bool {
        self.counter += 1;

        if self.counter % self.emit_interval == 0 {
            let level = calculate_audio_level(samples);

            // 打印日志
            if self.log_interval > 0 && self.counter % self.log_interval == 0 {
                tracing::info!("[AudioLevel] 发送音频级别: {:.4}", level);
            }

            emit_audio_level(app, level);
            return true;
        }

        false
    }

    /// 重置计数器
    pub fn reset(&mut self) {
        self.counter = 0;
    }
}
