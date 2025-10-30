use rodio::{OutputStream, Sink, Source};
use std::time::Duration;

/// 播放一个短促的提示音（非阻塞）
///
/// frequency: 音调频率（Hz），建议 800-1200
/// duration_ms: 持续时间（毫秒），建议 100-200
pub fn play_beep(frequency: u32, duration_ms: u64) {
    // 在新线程中播放，避免阻塞主线程
    std::thread::spawn(move || {
        if let Err(e) = play_beep_blocking(frequency, duration_ms) {
            tracing::error!("播放提示音失败: {}", e);
        }
    });
}

/// 阻塞式播放提示音
fn play_beep_blocking(frequency: u32, duration_ms: u64) -> Result<(), Box<dyn std::error::Error>> {
    // 获取音频输出流
    let (_stream, stream_handle) = OutputStream::try_default()?;
    let sink = Sink::try_new(&stream_handle)?;

    // 生成正弦波音频源
    let source = rodio::source::SineWave::new(frequency as f32)
        .take_duration(Duration::from_millis(duration_ms))
        .amplify(0.3); // 音量调整为 30% 避免太刺耳

    sink.append(source);
    sink.sleep_until_end(); // 等待播放完成

    Ok(())
}

/// 播放"开始录音"提示音（较高音调）
pub fn play_start_beep() {
    play_beep(1000, 100); // 1000 Hz, 100ms
}

/// 播放"停止录音"提示音（较低音调）
pub fn play_stop_beep() {
    play_beep(800, 150); // 800 Hz, 150ms
}
