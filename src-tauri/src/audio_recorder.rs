// 音频录制模块
use hound::{WavSpec, WavWriter};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use anyhow::Result;
use cpal::Stream;

pub struct AudioRecorder {
    sample_rate: u32,
    channels: u16,
    audio_data: Arc<Mutex<Vec<f32>>>,
    is_recording: Arc<Mutex<bool>>,
    stream: Option<Stream>,  // 保存 stream 引用
}

impl AudioRecorder {
    pub fn new() -> Result<Self> {
        Ok(Self {
            sample_rate: 16000,
            channels: 1,
            audio_data: Arc::new(Mutex::new(Vec::new())),
            is_recording: Arc::new(Mutex::new(false)),
            stream: None,
        })
    }

    pub fn start_recording(&mut self) -> Result<()> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

        tracing::info!("开始录音...");

        // 清空之前的音频数据
        self.audio_data.lock().unwrap().clear();
        *self.is_recording.lock().unwrap() = true;

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow::anyhow!("没有找到默认音频输入设备"))?;

        // 获取设备支持的配置
        let supported_config = device
            .default_input_config()
            .map_err(|e| anyhow::anyhow!("无法获取默认音频配置: {}", e))?;

        tracing::info!("设备支持的配置: {:?}", supported_config);

        // 使用设备支持的配置
        let config = supported_config.config();

        // 更新采样率和声道为设备实际支持的值
        self.sample_rate = config.sample_rate.0;
        self.channels = config.channels;

        tracing::info!("使用配置: 采样率={}Hz, 声道={}", self.sample_rate, self.channels);

        let audio_data = Arc::clone(&self.audio_data);
        let is_recording = Arc::clone(&self.is_recording);
        let err_fn = |err| tracing::error!("录音流错误: {}", err);

        // 根据采样格式创建不同的 stream
        let stream = match supported_config.sample_format() {
            cpal::SampleFormat::F32 => device.build_input_stream(
                &config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if *is_recording.lock().unwrap() {
                        let mut buffer = audio_data.lock().unwrap();
                        buffer.extend_from_slice(data);
                    }
                },
                err_fn,
                None,
            )?,
            cpal::SampleFormat::I16 => {
                let audio_data_i16 = Arc::clone(&audio_data);
                let is_recording_i16 = Arc::clone(&is_recording);
                device.build_input_stream(
                    &config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        if *is_recording_i16.lock().unwrap() {
                            let mut buffer = audio_data_i16.lock().unwrap();
                            // 转换 i16 到 f32
                            for &sample in data.iter() {
                                let normalized = sample as f32 / i16::MAX as f32;
                                buffer.push(normalized);
                            }
                        }
                    },
                    err_fn,
                    None,
                )?
            }
            cpal::SampleFormat::U16 => {
                let audio_data_u16 = Arc::clone(&audio_data);
                let is_recording_u16 = Arc::clone(&is_recording);
                device.build_input_stream(
                    &config,
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        if *is_recording_u16.lock().unwrap() {
                            let mut buffer = audio_data_u16.lock().unwrap();
                            // 转换 u16 到 f32
                            for &sample in data.iter() {
                                let normalized = (sample as f32 - 32768.0) / 32768.0;
                                buffer.push(normalized);
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

        // 保存 stream 引用，保持录音流活跃
        self.stream = Some(stream);

        Ok(())
    }

    pub fn stop_recording(&mut self) -> Result<PathBuf> {
        tracing::info!("停止录音...");

        // 停止录音
        *self.is_recording.lock().unwrap() = false;

        // Drop stream，停止音频流
        self.stream = None;

        // 等待一小段时间确保所有数据都已写入
        std::thread::sleep(std::time::Duration::from_millis(200));

        // 保存音频文件
        let temp_dir = std::env::temp_dir();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        let file_path = temp_dir.join(format!("recording_{}.wav", timestamp));

        let spec = WavSpec {
            channels: self.channels,
            sample_rate: self.sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut writer = WavWriter::create(&file_path, spec)?;
        let audio_data = self.audio_data.lock().unwrap();

        for &sample in audio_data.iter() {
            let amplitude = (sample * i16::MAX as f32) as i16;
            writer.write_sample(amplitude)?;
        }

        writer.finalize()?;
        tracing::info!("音频已保存到: {:?}, 采样率: {}Hz", file_path, self.sample_rate);

        Ok(file_path)
    }
}

// 实现 Send 和 Sync traits
unsafe impl Send for AudioRecorder {}
unsafe impl Sync for AudioRecorder {}
