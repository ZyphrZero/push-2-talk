// 豆包流式 ASR WebSocket 客户端（二进制协议）
use anyhow::Result;
use flate2::{write::GzEncoder, read::GzDecoder, Compression};
use futures_util::{SinkExt, StreamExt};
use std::io::{Write, Read};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message, tungstenite::http};

const WEBSOCKET_URL: &str = "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_nostream";
const RESOURCE_ID: &str = "volc.bigasr.sauc";
const TRANSCRIPTION_TIMEOUT_SECS: u64 = 10;

pub struct DoubaoRealtimeSession {
    sender: mpsc::Sender<SessionCommand>,
    result_receiver: mpsc::Receiver<Result<String>>,
    sequence: i32,
}

enum SessionCommand {
    SendAudio(Vec<u8>),
    Finish,
    Close,
}

impl DoubaoRealtimeSession {
    pub async fn send_audio_chunk(&mut self, pcm_data: &[i16]) -> Result<()> {
        let bytes: Vec<u8> = pcm_data.iter()
            .flat_map(|&s| s.to_le_bytes())
            .collect();
        self.sender.send(SessionCommand::SendAudio(bytes)).await
            .map_err(|_| anyhow::anyhow!("发送音频块失败"))
    }

    pub async fn finish_audio(&mut self) -> Result<()> {
        self.sender.send(SessionCommand::Finish).await
            .map_err(|_| anyhow::anyhow!("发送结束标志失败"))
    }

    pub async fn wait_for_result(&mut self) -> Result<String> {
        match timeout(Duration::from_secs(TRANSCRIPTION_TIMEOUT_SECS), self.result_receiver.recv()).await {
            Ok(Some(result)) => result,
            Ok(None) => Err(anyhow::anyhow!("通道已关闭")),
            Err(_) => Err(anyhow::anyhow!("转录超时")),
        }
    }

    pub async fn close(&self) -> Result<()> {
        let _ = self.sender.send(SessionCommand::Close).await;
        Ok(())
    }
}

pub struct DoubaoRealtimeClient {
    app_id: String,
    access_key: String,
}

impl DoubaoRealtimeClient {
    pub fn new(app_id: String, access_key: String) -> Self {
        Self { app_id, access_key }
    }

    pub async fn start_session(&self) -> Result<DoubaoRealtimeSession> {
        let request = http::Request::builder()
            .uri(WEBSOCKET_URL)
            .header("X-Api-App-Key", &self.app_id)
            .header("X-Api-Access-Key", &self.access_key)
            .header("X-Api-Resource-Id", RESOURCE_ID)
            .body(())?;

        let (ws_stream, _) = connect_async(request).await?;
        let (mut write, mut read) = ws_stream.split();

        let (cmd_tx, mut cmd_rx) = mpsc::channel::<SessionCommand>(100);
        let (result_tx, result_rx) = mpsc::channel::<Result<String>>(1);

        // 发送 Full Client Request
        let config = serde_json::json!({
            "user": {"uid": &self.app_id},
            "audio": {"format": "pcm", "sample_rate": 16000, "bits": 16, "channel": 1},
            "request": {"model_name": "bigmodel", "enable_itn": true}
        });
        let msg = build_message(0x1, 0x1, 1, &serde_json::to_vec(&config)?)?;
        write.send(Message::Binary(msg)).await?;

        let mut sequence = 1i32;
        tokio::spawn(async move {
            while let Some(cmd) = cmd_rx.recv().await {
                match cmd {
                    SessionCommand::SendAudio(audio) => {
                        sequence += 1;
                        if let Ok(msg) = build_message(0x2, 0x1, sequence, &audio) {
                            let _ = write.send(Message::Binary(msg)).await;
                        }
                    }
                    SessionCommand::Finish => {
                        sequence = -sequence;
                        if let Ok(msg) = build_message(0x2, 0x3, sequence, &[]) {
                            let _ = write.send(Message::Binary(msg)).await;
                        }
                    }
                    SessionCommand::Close => {
                        let _ = write.close().await;
                        break;
                    }
                }
            }
        });

        tokio::spawn(async move {
            while let Some(msg) = read.next().await {
                if let Ok(Message::Binary(data)) = msg {
                    if let Ok(text) = parse_response(&data) {
                        let _ = result_tx.send(Ok(text)).await;
                        break;
                    }
                }
            }
        });

        Ok(DoubaoRealtimeSession { sender: cmd_tx, result_receiver: result_rx, sequence: 1 })
    }
}

fn build_message(msg_type: u8, flags: u8, sequence: i32, payload: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(payload)?;
    let compressed = encoder.finish()?;

    let mut msg = vec![0x11, (msg_type << 4) | flags, 0x11, 0x00];
    msg.extend_from_slice(&sequence.to_be_bytes());
    msg.extend_from_slice(&(compressed.len() as u32).to_be_bytes());
    msg.extend_from_slice(&compressed);
    Ok(msg)
}

fn parse_response(data: &[u8]) -> Result<String> {
    if data.len() < 12 { return Err(anyhow::anyhow!("响应太短")); }
    let payload_size = u32::from_be_bytes([data[8], data[9], data[10], data[11]]) as usize;
    if data.len() < 12 + payload_size { return Err(anyhow::anyhow!("数据不完整")); }

    let mut decoder = GzDecoder::new(&data[12..12 + payload_size]);
    let mut json_str = String::new();
    decoder.read_to_string(&mut json_str)?;

    let result: serde_json::Value = serde_json::from_str(&json_str)?;
    result["result"]["text"].as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("无法解析转录结果"))
}
