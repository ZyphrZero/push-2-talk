// 流式音频录制模块
// 支持边录音边发送 PCM 数据块到 WebSocket

use anyhow::Result;
use cpal::Stream;
use crossbeam_channel::{Receiver, Sender, bounded};
use std::sync::{Arc, Mutex};

// API 要求的目标采样率
const TARGET_SAMPLE_RATE: u32 = 16000;
// 每个音频块的样本数（0.2秒 @ 16kHz = 3200 样本）
const CHUNK_SAMPLES: usize = 3200;

/// 流式音频录制器
/// 边录音边输出 PCM 数据块，同时保留完整音频用于备用方案
pub struct StreamingRecorder {
    device_sample_rate: u32,
    channels: u16,
    is_recording: Arc<Mutex<bool>>,
    stream: Option<Stream>,
    // 用于流式输出的通道
    chunk_sender: Option<Sender<Vec<i16>>>,
    // 累积的完整音频数据（用于备用方案）
    full_audio_data: Arc<Mutex<Vec<f32>>>,
}

impl StreamingRecorder {
    pub fn new() -> Result<Self> {
        Ok(Self {
            device_sample_rate: 48000,
            channels: 1,
            is_recording: Arc::new(Mutex::new(false)),
            stream: None,
            chunk_sender: None,
            full_audio_data: Arc::new(Mutex::new(Vec::new())),
        })
    }

    /// 将音频从设备采样率降采样到目标采样率 (16kHz)
    fn resample(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
        if from_rate == to_rate {
            return input.to_vec();
        }

        let ratio = from_rate as f64 / to_rate as f64;
        let output_len = (input.len() as f64 / ratio) as usize;
        let mut output = Vec::with_capacity(output_len);

        for i in 0..output_len {
            let src_idx = i as f64 * ratio;
            let idx_floor = src_idx.floor() as usize;
            let idx_ceil = (idx_floor + 1).min(input.len().saturating_sub(1));
            let frac = src_idx - idx_floor as f64;

            if idx_floor < input.len() {
                let sample = input[idx_floor] as f64 * (1.0 - frac)
                    + input.get(idx_ceil).copied().unwrap_or(0.0) as f64 * frac;
                output.push(sample as f32);
            }
        }

        output
    }

    /// 将多声道音频转换为单声道
    fn to_mono(input: &[f32], channels: u16) -> Vec<f32> {
        if channels == 1 {
            return input.to_vec();
        }

        let channels = channels as usize;
        let output_len = input.len() / channels;
        let mut output = Vec::with_capacity(output_len);

        for i in 0..output_len {
            let mut sum = 0.0f32;
            for ch in 0..channels {
                sum += input[i * channels + ch];
            }
            output.push(sum / channels as f32);
        }

        output
    }

    /// 将 f32 样本转换为 i16
    fn f32_to_i16(samples: &[f32]) -> Vec<i16> {
        samples.iter()
            .map(|&s| (s * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16)
            .collect()
    }

    /// 启动流式录音，返回音频块接收通道
    pub fn start_streaming(&mut self) -> Result<Receiver<Vec<i16>>> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

        tracing::info!("开始流式录音...");

        // 清空之前的数据
        self.full_audio_data.lock().unwrap().clear();
        *self.is_recording.lock().unwrap() = true;

        // 创建音频块通道（缓冲 50 个块，约 10 秒）
        let (chunk_tx, chunk_rx) = bounded::<Vec<i16>>(50);
        self.chunk_sender = Some(chunk_tx.clone());

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow::anyhow!("没有找到默认音频输入设备"))?;

        let supported_config = device
            .default_input_config()
            .map_err(|e| anyhow::anyhow!("无法获取默认音频配置: {}", e))?;

        let config = supported_config.config();
        self.device_sample_rate = config.sample_rate.0;
        self.channels = config.channels;

        tracing::info!("流式录音配置: 采样率={}Hz, 声道={}, 目标采样率={}Hz, 块大小={}样本",
            self.device_sample_rate, self.channels, TARGET_SAMPLE_RATE, CHUNK_SAMPLES);

        let is_recording = Arc::clone(&self.is_recording);
        let full_audio_data = Arc::clone(&self.full_audio_data);
        let device_sample_rate = self.device_sample_rate;
        let channels = self.channels;

        // 用于累积样本直到达到块大小
        let pending_samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
        let pending_samples_clone = Arc::clone(&pending_samples);

        let err_fn = |err| tracing::error!("录音流错误: {}", err);

        let stream = match supported_config.sample_format() {
            cpal::SampleFormat::F32 => device.build_input_stream(
                &config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if !*is_recording.lock().unwrap() {
                        return;
                    }

                    // 保存原始数据用于备用方案
                    full_audio_data.lock().unwrap().extend_from_slice(data);

                    // 处理数据：转单声道 + 降采样
                    let mono = Self::to_mono(data, channels);
                    let resampled = Self::resample(&mono, device_sample_rate, TARGET_SAMPLE_RATE);

                    // 累积样本
                    let mut pending = pending_samples_clone.lock().unwrap();
                    pending.extend(resampled);

                    // 当累积足够样本时，发送块
                    while pending.len() >= CHUNK_SAMPLES {
                        let chunk: Vec<f32> = pending.drain(..CHUNK_SAMPLES).collect();
                        let chunk_i16 = Self::f32_to_i16(&chunk);

                        if chunk_tx.try_send(chunk_i16).is_err() {
                            tracing::warn!("音频块通道已满，丢弃块");
                        }
                    }
                },
                err_fn,
                None,
            )?,
            cpal::SampleFormat::I16 => {
                let is_recording_i16 = Arc::clone(&is_recording);
                let full_audio_data_i16 = Arc::clone(&full_audio_data);
                let pending_samples_i16 = Arc::clone(&pending_samples);
                let chunk_tx_i16 = chunk_tx.clone();

                device.build_input_stream(
                    &config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        if !*is_recording_i16.lock().unwrap() {
                            return;
                        }

                        // 转换为 f32
                        let f32_data: Vec<f32> = data.iter()
                            .map(|&s| s as f32 / i16::MAX as f32)
                            .collect();

                        // 保存原始数据
                        full_audio_data_i16.lock().unwrap().extend(&f32_data);

                        // 处理数据
                        let mono = Self::to_mono(&f32_data, channels);
                        let resampled = Self::resample(&mono, device_sample_rate, TARGET_SAMPLE_RATE);

                        // 累积样本
                        let mut pending = pending_samples_i16.lock().unwrap();
                        pending.extend(resampled);

                        while pending.len() >= CHUNK_SAMPLES {
                            let chunk: Vec<f32> = pending.drain(..CHUNK_SAMPLES).collect();
                            let chunk_i16 = Self::f32_to_i16(&chunk);

                            if chunk_tx_i16.try_send(chunk_i16).is_err() {
                                tracing::warn!("音频块通道已满，丢弃块");
                            }
                        }
                    },
                    err_fn,
                    None,
                )?
            }
            cpal::SampleFormat::U16 => {
                let is_recording_u16 = Arc::clone(&is_recording);
                let full_audio_data_u16 = Arc::clone(&full_audio_data);
                let pending_samples_u16 = Arc::clone(&pending_samples);
                let chunk_tx_u16 = chunk_tx.clone();

                device.build_input_stream(
                    &config,
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        if !*is_recording_u16.lock().unwrap() {
                            return;
                        }

                        // 转换为 f32
                        let f32_data: Vec<f32> = data.iter()
                            .map(|&s| (s as f32 - 32768.0) / 32768.0)
                            .collect();

                        // 保存原始数据
                        full_audio_data_u16.lock().unwrap().extend(&f32_data);

                        // 处理数据
                        let mono = Self::to_mono(&f32_data, channels);
                        let resampled = Self::resample(&mono, device_sample_rate, TARGET_SAMPLE_RATE);

                        // 累积样本
                        let mut pending = pending_samples_u16.lock().unwrap();
                        pending.extend(resampled);

                        while pending.len() >= CHUNK_SAMPLES {
                            let chunk: Vec<f32> = pending.drain(..CHUNK_SAMPLES).collect();
                            let chunk_i16 = Self::f32_to_i16(&chunk);

                            if chunk_tx_u16.try_send(chunk_i16).is_err() {
                                tracing::warn!("音频块通道已满，丢弃块");
                            }
                        }
                    },
                    err_fn,
                    None,
                )?
            }
            _ => return Err(anyhow::anyhow!("不支持的采样格式")),
        };

        stream.play()?;
        self.stream = Some(stream);

        tracing::info!("流式录音已启动");
        Ok(chunk_rx)
    }

    /// 停止流式录音，返回完整的音频数据（WAV 格式，用于备用方案）
    pub fn stop_streaming(&mut self) -> Result<Vec<u8>> {
        use hound::{WavSpec, WavWriter};
        use std::io::Cursor;

        tracing::info!("停止流式录音...");

        // 先等待音频回调完成当前数据写入（stream 还在运行）
        std::thread::sleep(std::time::Duration::from_millis(200));

        // 停止录音标志
        *self.is_recording.lock().unwrap() = false;

        // 再等待一小段时间确保最后的数据被写入
        std::thread::sleep(std::time::Duration::from_millis(100));

        // 最后 drop stream
        self.stream = None;
        self.chunk_sender = None;

        // 获取完整音频数据
        let raw_audio = self.full_audio_data.lock().unwrap().clone();

        if raw_audio.is_empty() {
            return Err(anyhow::anyhow!("没有录制到音频数据"));
        }

        // 转换为单声道
        let mono_audio = Self::to_mono(&raw_audio, self.channels);

        // 降采样到 16kHz
        let resampled_audio = Self::resample(&mono_audio, self.device_sample_rate, TARGET_SAMPLE_RATE);

        // 写入 WAV 格式
        let spec = WavSpec {
            channels: 1,
            sample_rate: TARGET_SAMPLE_RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = WavWriter::new(&mut cursor, spec)?;
            for &sample in resampled_audio.iter() {
                let amplitude = (sample * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
                writer.write_sample(amplitude)?;
            }
            writer.finalize()?;
        }

        let wav_data = cursor.into_inner();
        tracing::info!("流式录音停止，完整音频: {} bytes", wav_data.len());

        Ok(wav_data)
    }

    /// 检查是否正在录音
    pub fn is_recording(&self) -> bool {
        *self.is_recording.lock().unwrap()
    }
}

// 实现 Send 和 Sync traits
unsafe impl Send for StreamingRecorder {}
unsafe impl Sync for StreamingRecorder {}
