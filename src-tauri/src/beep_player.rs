use rodio::{Decoder, OutputStream, Sink, Source};
use std::io::Cursor;

// 在编译时嵌入音效文件
const NOTIFICATION_SOUND: &[u8] = include_bytes!("../resources/notification.ogg");

// 音量系数 (0.0 - 1.0)，调小这个值可以降低音量
const VOLUME: f32 = 0.2;

/// 播放提示音（非阻塞）
pub fn play_notification() {
    // 在新线程中播放，避免阻塞主线程
    std::thread::spawn(|| {
        if let Err(e) = play_notification_blocking() {
            tracing::error!("播放提示音失败: {}", e);
        }
    });
}

/// 阻塞式播放提示音
fn play_notification_blocking() -> Result<(), Box<dyn std::error::Error>> {
    // 获取音频输出流
    let (_stream, stream_handle) = OutputStream::try_default()?;
    let sink = Sink::try_new(&stream_handle)?;

    // 从嵌入的字节数据创建解码器
    let cursor = Cursor::new(NOTIFICATION_SOUND);
    let source = Decoder::new(cursor)?.amplify(VOLUME); // 降低音量

    sink.append(source);
    sink.sleep_until_end(); // 等待播放完成

    Ok(())
}

/// 播放"开始录音"提示音
pub fn play_start_beep() {
    play_notification();
}

/// 播放"停止录音"提示音
pub fn play_stop_beep() {
    play_notification();
}
