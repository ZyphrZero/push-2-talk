// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio_recorder;
mod audio_utils;
mod asr;
mod beep_player;
mod config;
mod hotkey_service;
mod llm_post_processor;
mod streaming_recorder;
mod text_inserter;

use audio_recorder::AudioRecorder;
use asr::{QwenASRClient, SenseVoiceClient, DoubaoASRClient, QwenRealtimeClient, DoubaoRealtimeClient, DoubaoRealtimeSession, RealtimeSession};
use config::AppConfig;
use hotkey_service::HotkeyService;
use llm_post_processor::LlmPostProcessor;
use streaming_recorder::StreamingRecorder;
use text_inserter::TextInserter;

use std::sync::{Arc, Mutex};
use tauri::{
    AppHandle, Emitter, Manager,
    tray::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState},
    menu::{Menu, MenuItem},
    WindowEvent,
};

// 全局应用状态
struct AppState {
    audio_recorder: Arc<Mutex<Option<AudioRecorder>>>,
    streaming_recorder: Arc<Mutex<Option<StreamingRecorder>>>,
    text_inserter: Arc<Mutex<Option<TextInserter>>>,
    post_processor: Arc<Mutex<Option<LlmPostProcessor>>>,
    is_running: Arc<Mutex<bool>>,
    use_realtime_asr: Arc<Mutex<bool>>,
    enable_post_process: Arc<Mutex<bool>>,
    qwen_client: Arc<Mutex<Option<QwenASRClient>>>,
    sensevoice_client: Arc<Mutex<Option<SenseVoiceClient>>>,
    doubao_client: Arc<Mutex<Option<DoubaoASRClient>>>,
    // 活跃的实时转录会话（用于真正的流式传输）
    active_session: Arc<tokio::sync::Mutex<Option<RealtimeSession>>>,
    doubao_session: Arc<tokio::sync::Mutex<Option<DoubaoRealtimeSession>>>,
    realtime_provider: Arc<Mutex<Option<config::AsrProvider>>>,
    // 音频发送任务句柄
    audio_sender_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

// Tauri Commands

#[tauri::command]
async fn save_config(
    api_key: String,
    fallback_api_key: String,
    use_realtime: Option<bool>,
    enable_post_process: Option<bool>,
    llm_config: Option<config::LlmConfig>,
    close_action: Option<String>,
    asr_config: Option<config::AsrConfig>,
) -> Result<String, String> {
    tracing::info!("保存配置...");
    let has_fallback = !fallback_api_key.is_empty();
    let config = AppConfig {
        dashscope_api_key: api_key.clone(),
        siliconflow_api_key: fallback_api_key.clone(),
        asr_config: asr_config.unwrap_or_else(|| config::AsrConfig {
            primary: config::AsrProviderConfig {
                provider: config::AsrProvider::Qwen,
                api_key: api_key,
                app_id: None,
                access_token: None,
            },
            fallback: if has_fallback {
                Some(config::AsrProviderConfig {
                    provider: config::AsrProvider::SiliconFlow,
                    api_key: fallback_api_key,
                    app_id: None,
                    access_token: None,
                })
            } else {
                None
            },
            enable_fallback: has_fallback,
        }),
        use_realtime_asr: use_realtime.unwrap_or(true),
        enable_llm_post_process: enable_post_process.unwrap_or(false),
        llm_config: llm_config.unwrap_or_default(),
        close_action,
    };

    config
        .save()
        .map_err(|e| format!("保存配置失败: {}", e))?;

    Ok("配置已保存".to_string())
}

#[tauri::command]
async fn load_config() -> Result<AppConfig, String> {
    tracing::info!("加载配置...");
    AppConfig::load().map_err(|e| format!("加载配置失败: {}", e))
}

#[tauri::command]
async fn start_app(
    app_handle: AppHandle,
    api_key: String,
    fallback_api_key: String,
    use_realtime: Option<bool>,
    enable_post_process: Option<bool>,
    llm_config: Option<config::LlmConfig>,
    asr_config: Option<config::AsrConfig>,
) -> Result<String, String> {
    tracing::info!("启动应用...");

    // 获取应用状态
    let state = app_handle.state::<AppState>();

    let mut is_running = state.is_running.lock().unwrap();
    if *is_running {
        return Err("应用已在运行中".to_string());
    }

    // 确定是否使用实时模式
    let use_realtime_mode = use_realtime.unwrap_or(true);
    *state.use_realtime_asr.lock().unwrap() = use_realtime_mode;

    // 确定是否启用 LLM 后处理
    let enable_post_process_mode = enable_post_process.unwrap_or(false);
    *state.enable_post_process.lock().unwrap() = enable_post_process_mode;

    tracing::info!("ASR 模式: {}", if use_realtime_mode { "实时 WebSocket" } else { "HTTP" });
    tracing::info!("LLM 后处理: {}", if enable_post_process_mode { "启用" } else { "禁用" });

    // 根据 asr_config 初始化对应的 ASR 客户端
    {
        *state.qwen_client.lock().unwrap() = None;
        *state.sensevoice_client.lock().unwrap() = None;
        *state.doubao_client.lock().unwrap() = None;

        if let Some(ref cfg) = asr_config {
            match cfg.primary.provider {
                config::AsrProvider::Qwen => {
                    *state.qwen_client.lock().unwrap() = Some(QwenASRClient::new(cfg.primary.api_key.clone()));
                }
                config::AsrProvider::Doubao => {
                    if let (Some(app_id), Some(access_token)) = (&cfg.primary.app_id, &cfg.primary.access_token) {
                        *state.doubao_client.lock().unwrap() = Some(DoubaoASRClient::new(app_id.clone(), access_token.clone()));
                    } else {
                        let parts: Vec<&str> = cfg.primary.api_key.split(':').collect();
                        if parts.len() == 2 {
                            *state.doubao_client.lock().unwrap() = Some(DoubaoASRClient::new(parts[0].to_string(), parts[1].to_string()));
                        } else {
                            tracing::error!("豆包 API Key 格式错误，应为 app_id:access_key");
                        }
                    }
                }
                config::AsrProvider::SiliconFlow => {
                    *state.sensevoice_client.lock().unwrap() = Some(SenseVoiceClient::new(cfg.primary.api_key.clone()));
                }
            }
            if cfg.enable_fallback {
                if let Some(ref fb) = cfg.fallback {
                    match fb.provider {
                        config::AsrProvider::Qwen if state.qwen_client.lock().unwrap().is_none() => {
                            *state.qwen_client.lock().unwrap() = Some(QwenASRClient::new(fb.api_key.clone()));
                        }
                        config::AsrProvider::Doubao if state.doubao_client.lock().unwrap().is_none() => {
                            if let (Some(app_id), Some(access_token)) = (&fb.app_id, &fb.access_token) {
                                *state.doubao_client.lock().unwrap() = Some(DoubaoASRClient::new(app_id.clone(), access_token.clone()));
                            } else {
                                let parts: Vec<&str> = fb.api_key.split(':').collect();
                                if parts.len() == 2 {
                                    *state.doubao_client.lock().unwrap() = Some(DoubaoASRClient::new(parts[0].to_string(), parts[1].to_string()));
                                }
                            }
                        }
                        config::AsrProvider::SiliconFlow if state.sensevoice_client.lock().unwrap().is_none() => {
                            *state.sensevoice_client.lock().unwrap() = Some(SenseVoiceClient::new(fb.api_key.clone()));
                        }
                        _ => {}
                    }
                }
            }
        } else {
            if !api_key.is_empty() {
                *state.qwen_client.lock().unwrap() = Some(QwenASRClient::new(api_key.clone()));
            }
            if !fallback_api_key.is_empty() {
                *state.sensevoice_client.lock().unwrap() = Some(SenseVoiceClient::new(fallback_api_key.clone()));
            }
        }
    }

    // 初始化 LLM 后处理器（复用连接）
    {
        let mut processor_guard = state.post_processor.lock().unwrap();
        let llm_cfg = llm_config.unwrap_or_default();
        if enable_post_process_mode && !llm_cfg.api_key.trim().is_empty() {
            tracing::info!("LLM 后处理器配置: endpoint={}, model={}", llm_cfg.endpoint, llm_cfg.model);
            *processor_guard = Some(LlmPostProcessor::new(llm_cfg));
            tracing::info!("LLM 后处理器已初始化");
        } else {
            *processor_guard = None;
            if enable_post_process_mode {
                tracing::warn!("LLM 后处理已启用但未配置 API Key，将跳过后处理");
            }
        }
    }

    // 初始化文本插入器
    let text_inserter = TextInserter::new()
        .map_err(|e| format!("初始化文本插入器失败: {}", e))?;
    *state.text_inserter.lock().unwrap() = Some(text_inserter);

    // 根据模式初始化录音器
    *state.audio_recorder.lock().unwrap() = None;
    *state.streaming_recorder.lock().unwrap() = None;

    if use_realtime_mode {
        let streaming_recorder = StreamingRecorder::new()
            .map_err(|e| format!("初始化流式录音器失败: {}", e))?;
        *state.streaming_recorder.lock().unwrap() = Some(streaming_recorder);
    } else {
        let audio_recorder = AudioRecorder::new()
            .map_err(|e| format!("初始化音频录制器失败: {}", e))?;
        *state.audio_recorder.lock().unwrap() = Some(audio_recorder);
    }

    // 启动全局快捷键监听
    let hotkey_service = HotkeyService::new();

    // 克隆状态用于回调
    let app_handle_start = app_handle.clone();
    let audio_recorder_start = Arc::clone(&state.audio_recorder);
    let streaming_recorder_start = Arc::clone(&state.streaming_recorder);
    let active_session_start = Arc::clone(&state.active_session);
    let doubao_session_start = Arc::clone(&state.doubao_session);
    let realtime_provider_start = Arc::clone(&state.realtime_provider);
    let audio_sender_handle_start = Arc::clone(&state.audio_sender_handle);
    let use_realtime_start = use_realtime_mode;
    let api_key_start = api_key.clone();
    let is_running_start = Arc::clone(&state.is_running);

    // 保存当前的 provider 配置和凭证
    let (provider_type, doubao_app_id, doubao_access_token) = if let Some(ref cfg) = asr_config {
        *state.realtime_provider.lock().unwrap() = Some(cfg.primary.provider.clone());
        (
            Some(cfg.primary.provider.clone()),
            cfg.primary.app_id.clone(),
            cfg.primary.access_token.clone(),
        )
    } else {
        (None, None, None)
    };
    let doubao_app_id_start = doubao_app_id;
    let doubao_access_token_start = doubao_access_token;

    let app_handle_stop = app_handle.clone();
    let audio_recorder_stop = Arc::clone(&state.audio_recorder);
    let streaming_recorder_stop = Arc::clone(&state.streaming_recorder);
    let active_session_stop = Arc::clone(&state.active_session);
    let audio_sender_handle_stop = Arc::clone(&state.audio_sender_handle);
    let text_inserter_stop = Arc::clone(&state.text_inserter);
    let post_processor_stop = Arc::clone(&state.post_processor);
    let qwen_client_stop = Arc::clone(&state.qwen_client);
    let sensevoice_client_stop = Arc::clone(&state.sensevoice_client);
    let doubao_client_stop = Arc::clone(&state.doubao_client);
    let doubao_session_stop = Arc::clone(&state.doubao_session);
    let realtime_provider_stop = Arc::clone(&state.realtime_provider);
    let use_realtime_stop = use_realtime_mode;
    let is_running_stop = Arc::clone(&state.is_running);

    // 按键按下回调
    let on_start = move || {
        // 检查服务是否仍在运行
        if !*is_running_start.lock().unwrap() {
            tracing::debug!("服务已停止，忽略快捷键按下事件");
            return;
        }

        let app = app_handle_start.clone();
        let recorder = Arc::clone(&audio_recorder_start);
        let streaming_recorder = Arc::clone(&streaming_recorder_start);
        let active_session = Arc::clone(&active_session_start);
        let doubao_session = Arc::clone(&doubao_session_start);
        let realtime_provider = Arc::clone(&realtime_provider_start);
        let audio_sender_handle = Arc::clone(&audio_sender_handle_start);
        let use_realtime = use_realtime_start;
        let api_key = api_key_start.clone();
        let doubao_app_id = doubao_app_id_start.clone();
        let doubao_access_token = doubao_access_token_start.clone();

        // 播放开始录音提示音
        beep_player::play_start_beep();

        tauri::async_runtime::spawn(async move {
            tracing::info!("检测到快捷键按下");
            let _ = app.emit("recording_started", ());

            // 显示录音悬浮窗并移动到屏幕底部居中
            if let Some(overlay) = app.get_webview_window("overlay") {
                // 获取主显示器信息并计算位置
                if let Some(monitor) = overlay.primary_monitor().ok().flatten() {
                    let screen_size = monitor.size();
                    let scale_factor = monitor.scale_factor();
                    let overlay_size = overlay.outer_size().unwrap_or(tauri::PhysicalSize::new(120, 44));

                    // 计算底部居中位置（距底部 100px）
                    let x = ((screen_size.width as f64 / scale_factor) / 2.0 - (overlay_size.width as f64 / scale_factor) / 2.0) as i32;
                    let y = ((screen_size.height as f64 / scale_factor) - (overlay_size.height as f64 / scale_factor) - 100.0) as i32;

                    let _ = overlay.set_position(tauri::PhysicalPosition::new(
                        (x as f64 * scale_factor) as i32,
                        (y as f64 * scale_factor) as i32
                    ));
                }
                let _ = overlay.show();
            }

            if use_realtime {
                // 实时模式：根据 provider 选择启动千问或豆包流式会话
                let provider = realtime_provider.lock().unwrap().clone();

                match provider {
                    Some(config::AsrProvider::Doubao) => {
                        tracing::info!("启动豆包实时流式转录...");

                        // 1. 先启动流式录音（确保有音频数据，即使 WebSocket 连接失败也能用备用方案）
                        let chunk_rx = {
                            let mut streaming_guard = streaming_recorder.lock().unwrap();
                            if let Some(ref mut rec) = *streaming_guard {
                                match rec.start_streaming(Some(app.clone())) {
                                    Ok(rx) => Some(rx),
                                    Err(e) => {
                                        emit_error_and_hide_overlay(&app, format!("录音失败: {}", e));
                                        None
                                    }
                                }
                            } else {
                                emit_error_and_hide_overlay(&app, "流式录音器未初始化".to_string());
                                None
                            }
                        };

                        // 2. 然后建立豆包 WebSocket 连接
                        if let Some(chunk_rx) = chunk_rx {
                            if let (Some(app_id), Some(access_token)) = (doubao_app_id.as_ref(), doubao_access_token.as_ref()) {
                                let realtime_client = DoubaoRealtimeClient::new(app_id.clone(), access_token.clone());
                                match realtime_client.start_session().await {
                                    Ok(session) => {
                                        tracing::info!("豆包 WebSocket 连接已建立");

                                        // 保存会话
                                        *doubao_session.lock().await = Some(session);

                                        // 3. 启动音频发送任务
                                        let session_for_sender = Arc::clone(&doubao_session);
                                        let sender_handle = tokio::spawn(async move {
                                            tracing::info!("豆包音频发送任务启动");
                                            let mut chunk_count = 0;

                                            while let Ok(chunk) = chunk_rx.recv() {
                                                let mut session_guard = session_for_sender.lock().await;
                                                if let Some(ref mut session) = *session_guard {
                                                    if let Err(e) = session.send_audio_chunk(&chunk).await {
                                                        tracing::error!("发送音频块失败: {}", e);
                                                        break;
                                                    }
                                                    chunk_count += 1;
                                                    if chunk_count % 10 == 0 {
                                                        tracing::debug!("已发送 {} 个音频块", chunk_count);
                                                    }
                                                } else {
                                                    break;
                                                }
                                                drop(session_guard);
                                            }

                                            tracing::info!("豆包音频发送任务结束，共发送 {} 个块", chunk_count);
                                        });

                                        *audio_sender_handle.lock().unwrap() = Some(sender_handle);
                                    }
                                    Err(e) => {
                                        tracing::error!("建立豆包 WebSocket 连接失败: {}，录音已启动，将使用备用方案", e);
                                        // 录音已在运行，备用方案可用，不需要再次启动录音
                                    }
                                }
                            } else {
                                tracing::error!("豆包凭证缺失：需要 app_id 和 access_token");
                                // 凭证缺失，录音已启动但无法建立 WebSocket，备用方案会在 on_stop 时处理
                                // 此处不需要隐藏悬浮窗，因为录音仍在进行
                            }
                        }
                    }
                    _ => {
                        // 默认使用千问流式
                        tracing::info!("启动千问实时流式转录...");

                        // 1. 建立千问 WebSocket 连接
                        let realtime_client = QwenRealtimeClient::new(api_key);
                        match realtime_client.start_session().await {
                            Ok(session) => {
                                tracing::info!("千问 WebSocket 连接已建立");

                                // 2. 启动流式录音
                                let chunk_rx = {
                                    let mut streaming_guard = streaming_recorder.lock().unwrap();
                                    if let Some(ref mut rec) = *streaming_guard {
                                        match rec.start_streaming(Some(app.clone())) {
                                            Ok(rx) => Some(rx),
                                            Err(e) => {
                                                emit_error_and_hide_overlay(&app, format!("录音失败: {}", e));
                                                None
                                            }
                                        }
                                    } else {
                                        emit_error_and_hide_overlay(&app, "流式录音器未初始化".to_string());
                                        None
                                    }
                                };

                                if let Some(chunk_rx) = chunk_rx {
                                    // 保存会话
                                    *active_session.lock().await = Some(session);

                                    // 3. 启动音频发送任务
                                    let session_for_sender = Arc::clone(&active_session);
                                    let sender_handle = tokio::spawn(async move {
                                        tracing::info!("千问音频发送任务启动");
                                        let mut chunk_count = 0;

                                        while let Ok(chunk) = chunk_rx.recv() {
                                            let session_guard = session_for_sender.lock().await;
                                            if let Some(ref session) = *session_guard {
                                                if let Err(e) = session.send_audio_chunk(&chunk).await {
                                                    tracing::error!("发送音频块失败: {}", e);
                                                    break;
                                                }
                                                chunk_count += 1;
                                                if chunk_count % 10 == 0 {
                                                    tracing::debug!("已发送 {} 个音频块", chunk_count);
                                                }
                                            } else {
                                                break;
                                            }
                                            drop(session_guard);
                                        }

                                        tracing::info!("千问音频发送任务结束，共发送 {} 个块", chunk_count);
                                    });

                                    *audio_sender_handle.lock().unwrap() = Some(sender_handle);
                                }
                            }
                            Err(e) => {
                                tracing::error!("建立千问 WebSocket 连接失败: {}，回退到普通录音", e);
                                // 注意：这里不发送 error 事件，因为会尝试备用方案

                                // 回退到普通流式录音（录完再传）
                                let mut streaming_guard = streaming_recorder.lock().unwrap();
                                if let Some(ref mut rec) = *streaming_guard {
                                    if let Err(e) = rec.start_streaming(Some(app.clone())) {
                                        // 备用方案也失败了，发送错误并隐藏悬浮窗
                                        emit_error_and_hide_overlay(&app, format!("录音失败: {}", e));
                                    }
                                } else {
                                    emit_error_and_hide_overlay(&app, "录音器未初始化".to_string());
                                }
                            }
                        }
                    }
                }
            } else {
                // HTTP 模式：使用原有录音器
                let mut recorder_guard = recorder.lock().unwrap();
                if let Some(ref mut rec) = *recorder_guard {
                    if let Err(e) = rec.start_recording(Some(app.clone())) {
                        emit_error_and_hide_overlay(&app, format!("录音失败: {}", e));
                    }
                } else {
                    emit_error_and_hide_overlay(&app, "录音器未初始化".to_string());
                }
            }
        });
    };

    // 按键释放回调
    let on_stop = move || {
        // 检查服务是否仍在运行
        if !*is_running_stop.lock().unwrap() {
            tracing::debug!("服务已停止，忽略快捷键释放事件");
            return;
        }

        let app = app_handle_stop.clone();
        let recorder = Arc::clone(&audio_recorder_stop);
        let streaming_recorder = Arc::clone(&streaming_recorder_stop);
        let active_session = Arc::clone(&active_session_stop);
        let audio_sender_handle = Arc::clone(&audio_sender_handle_stop);
        let inserter = Arc::clone(&text_inserter_stop);
        let post_processor = Arc::clone(&post_processor_stop);
        let qwen_client_state = Arc::clone(&qwen_client_stop);
        let sensevoice_client_state = Arc::clone(&sensevoice_client_stop);
        let doubao_client_state = Arc::clone(&doubao_client_stop);
        let doubao_session_state = Arc::clone(&doubao_session_stop);
        let realtime_provider_state = Arc::clone(&realtime_provider_stop);
        let use_realtime = use_realtime_stop;

        // 播放停止录音提示音
        beep_player::play_stop_beep();

        tauri::async_runtime::spawn(async move {
            tracing::info!("检测到快捷键释放");
            let _ = app.emit("recording_stopped", ());

            if use_realtime {
                // 实时模式：停止录音 + commit + 等待结果
                handle_realtime_stop(
                    app,
                    streaming_recorder,
                    active_session,
                    doubao_session_state,
                    realtime_provider_state,
                    audio_sender_handle,
                    inserter,
                    post_processor,
                    qwen_client_state,
                    sensevoice_client_state,
                    doubao_client_state,
                ).await;
            } else {
                // HTTP 模式：使用原有逻辑
                handle_http_transcription(
                    app,
                    recorder,
                    inserter,
                    post_processor,
                    qwen_client_state,
                    sensevoice_client_state,
                    doubao_client_state,
                ).await;
            }
        });
    };

    hotkey_service
        .start(on_start, on_stop)
        .map_err(|e| format!("启动快捷键监听失败: {}", e))?;

    *is_running = true;
    let mode_str = if use_realtime_mode { "实时模式" } else { "HTTP 模式" };
    Ok(format!("应用已启动 ({})，按 Ctrl+Win 开始录音", mode_str))
}

/// HTTP 模式转录处理（原有逻辑）
async fn handle_http_transcription(
    app: AppHandle,
    recorder: Arc<Mutex<Option<AudioRecorder>>>,
    inserter: Arc<Mutex<Option<TextInserter>>>,
    post_processor: Arc<Mutex<Option<LlmPostProcessor>>>,
    qwen_client_state: Arc<Mutex<Option<QwenASRClient>>>,
    sensevoice_client_state: Arc<Mutex<Option<SenseVoiceClient>>>,
    doubao_client_state: Arc<Mutex<Option<DoubaoASRClient>>>,
) {
    // 停止录音并直接获取内存中的音频数据
    let audio_data = {
        let mut recorder_guard = recorder.lock().unwrap();
        if let Some(ref mut rec) = *recorder_guard {
            match rec.stop_recording_to_memory() {
                Ok(data) => Some(data),
                Err(e) => {
                    emit_error_and_hide_overlay(&app, format!("停止录音失败: {}", e));
                    None
                }
            }
        } else {
            None
        }
    };

    if let Some(audio_data) = audio_data {
        let _ = app.emit("transcribing", ());

        let qwen_client = { qwen_client_state.lock().unwrap().clone() };
        let sensevoice_client = { sensevoice_client_state.lock().unwrap().clone() };
        let doubao_client = { doubao_client_state.lock().unwrap().clone() };

        let asr_start = std::time::Instant::now();
        let result = if let Some(doubao) = doubao_client {
            tracing::info!("使用豆包 ASR (HTTP)");
            doubao.transcribe_bytes(&audio_data).await
        } else if let Some(qwen) = qwen_client {
            tracing::info!("使用千问 ASR (HTTP)");
            qwen.transcribe_bytes(&audio_data).await
        } else if let Some(sensevoice) = sensevoice_client {
            tracing::info!("使用 SenseVoice ASR (HTTP)");
            sensevoice.transcribe_bytes(&audio_data).await
        } else {
            tracing::error!("未找到可用的 ASR 客户端");
            Err(anyhow::anyhow!("ASR 客户端未初始化"))
        };
        let asr_time_ms = asr_start.elapsed().as_millis() as u64;

        handle_transcription_result(app, inserter, post_processor, result, asr_time_ms).await;
    }
}

/// 真正的实时模式停止处理（边录边传后的 commit + 等待结果）
async fn handle_realtime_stop(
    app: AppHandle,
    streaming_recorder: Arc<Mutex<Option<StreamingRecorder>>>,
    active_session: Arc<tokio::sync::Mutex<Option<RealtimeSession>>>,
    doubao_session: Arc<tokio::sync::Mutex<Option<DoubaoRealtimeSession>>>,
    realtime_provider: Arc<Mutex<Option<config::AsrProvider>>>,
    audio_sender_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    inserter: Arc<Mutex<Option<TextInserter>>>,
    post_processor: Arc<Mutex<Option<LlmPostProcessor>>>,
    qwen_client_state: Arc<Mutex<Option<QwenASRClient>>>,
    sensevoice_client_state: Arc<Mutex<Option<SenseVoiceClient>>>,
    doubao_client_state: Arc<Mutex<Option<DoubaoASRClient>>>,
) {
    let _ = app.emit("transcribing", ());
    let asr_start = std::time::Instant::now();

    // 1. 停止流式录音，获取完整音频数据（用于备用方案）
    let audio_data = {
        let mut recorder_guard = streaming_recorder.lock().unwrap();
        if let Some(ref mut rec) = *recorder_guard {
            match rec.stop_streaming() {
                Ok(data) => Some(data),
                Err(e) => {
                    tracing::error!("停止流式录音失败: {}", e);
                    None
                }
            }
        } else {
            None
        }
    };

    // 2. 等待音频发送任务完成
    {
        let handle = audio_sender_handle.lock().unwrap().take();
        if let Some(h) = handle {
            tracing::info!("等待音频发送任务完成...");
            let _ = h.await;
        }
    }

    // 3. 检查使用的是哪个 provider
    let provider = realtime_provider.lock().unwrap().clone();

    match provider {
        Some(config::AsrProvider::Doubao) => {
            // 处理豆包流式会话
            let mut doubao_session_guard = doubao_session.lock().await;
            if let Some(ref mut session) = *doubao_session_guard {
                tracing::info!("豆包：发送 finish 并等待转录结果...");

                // 发送 finish
                if let Err(e) = session.finish_audio().await {
                    tracing::error!("豆包发送 finish 失败: {}", e);
                    drop(doubao_session_guard);
                    // 回退到备用方案
                    if let Some(audio_data) = audio_data {
                        fallback_transcription(
                            app,
                            inserter,
                            post_processor,
                            Arc::clone(&qwen_client_state),
                            Arc::clone(&sensevoice_client_state),
                            Arc::clone(&doubao_client_state),
                            audio_data,
                        )
                        .await;
                    }
                    return;
                }

                // 等待转录结果
                match session.wait_for_result().await {
                    Ok(text) => {
                        let asr_time_ms = asr_start.elapsed().as_millis() as u64;
                        tracing::info!("豆包实时转录成功: {} (ASR 耗时: {}ms)", text, asr_time_ms);
                        drop(doubao_session_guard);
                        *doubao_session.lock().await = None;
                        handle_transcription_result(app, inserter, post_processor, Ok(text), asr_time_ms).await;
                    }
                    Err(e) => {
                        tracing::warn!("豆包等待转录结果失败: {}，尝试备用方案", e);
                        drop(doubao_session_guard);
                        *doubao_session.lock().await = None;

                        // 回退到备用方案
                        if let Some(audio_data) = audio_data {
                            fallback_transcription(
                                app,
                                inserter,
                                post_processor,
                                Arc::clone(&qwen_client_state),
                                Arc::clone(&sensevoice_client_state),
                                Arc::clone(&doubao_client_state),
                                audio_data,
                            )
                            .await;
                        } else {
                            emit_error_and_hide_overlay(&app, format!("转录失败: {}", e));
                        }
                    }
                }
            } else {
                // 没有活跃的豆包会话，使用备用方案
                tracing::warn!("没有活跃的豆包 WebSocket 会话，使用备用方案");
                drop(doubao_session_guard);

                if let Some(audio_data) = audio_data {
                    fallback_transcription(
                        app,
                        inserter,
                        post_processor,
                        Arc::clone(&qwen_client_state),
                        Arc::clone(&sensevoice_client_state),
                        Arc::clone(&doubao_client_state),
                        audio_data,
                    )
                    .await;
                } else {
                    emit_error_and_hide_overlay(&app, "没有录制到音频数据".to_string());
                }
            }
        }
        _ => {
            // 处理千问流式会话
            let mut session_guard = active_session.lock().await;
            if let Some(ref mut session) = *session_guard {
                tracing::info!("千问：发送 commit 并等待转录结果...");

                // 发送 commit
                if let Err(e) = session.commit_audio().await {
                    tracing::error!("千问发送 commit 失败: {}", e);
                    drop(session_guard);
                    // 回退到备用方案
                    if let Some(audio_data) = audio_data {
                        fallback_transcription(
                            app,
                            inserter,
                            post_processor,
                            Arc::clone(&qwen_client_state),
                            Arc::clone(&sensevoice_client_state),
                            Arc::clone(&doubao_client_state),
                            audio_data,
                        )
                        .await;
                    }
                    return;
                }

                // 等待转录结果
                match session.wait_for_result().await {
                    Ok(text) => {
                        let asr_time_ms = asr_start.elapsed().as_millis() as u64;
                        tracing::info!("千问实时转录成功: {} (ASR 耗时: {}ms)", text, asr_time_ms);
                        let _ = session.close().await;
                        drop(session_guard);
                        *active_session.lock().await = None;
                        handle_transcription_result(app, inserter, post_processor, Ok(text), asr_time_ms).await;
                    }
                    Err(e) => {
                        tracing::warn!("千问等待转录结果失败: {}，尝试备用方案", e);
                        let _ = session.close().await;
                        drop(session_guard);
                        *active_session.lock().await = None;

                        // 回退到备用方案
                        if let Some(audio_data) = audio_data {
                            fallback_transcription(
                                app,
                                inserter,
                                post_processor,
                                Arc::clone(&qwen_client_state),
                                Arc::clone(&sensevoice_client_state),
                                Arc::clone(&doubao_client_state),
                                audio_data,
                            )
                            .await;
                        } else {
                            emit_error_and_hide_overlay(&app, format!("转录失败: {}", e));
                        }
                    }
                }
            } else {
                // 没有活跃会话，使用备用方案（可能是连接失败时的回退）
                tracing::warn!("没有活跃的千问 WebSocket 会话，使用备用方案");
                drop(session_guard);

                if let Some(audio_data) = audio_data {
                    fallback_transcription(
                        app,
                        inserter,
                        post_processor,
                        Arc::clone(&qwen_client_state),
                        Arc::clone(&sensevoice_client_state),
                        Arc::clone(&doubao_client_state),
                        audio_data,
                    )
                    .await;
                } else {
                    emit_error_and_hide_overlay(&app, "没有录制到音频数据".to_string());
                }
            }
        }
    }
}

/// 备用转录方案（HTTP 模式）
async fn fallback_transcription(
    app: AppHandle,
    inserter: Arc<Mutex<Option<TextInserter>>>,
    post_processor: Arc<Mutex<Option<LlmPostProcessor>>>,
    qwen_client_state: Arc<Mutex<Option<QwenASRClient>>>,
    sensevoice_client_state: Arc<Mutex<Option<SenseVoiceClient>>>,
    doubao_client_state: Arc<Mutex<Option<DoubaoASRClient>>>,
    audio_data: Vec<u8>,
) {
    let qwen_client = { qwen_client_state.lock().unwrap().clone() };
    let sensevoice_client = { sensevoice_client_state.lock().unwrap().clone() };
    let doubao_client = { doubao_client_state.lock().unwrap().clone() };

    let asr_start = std::time::Instant::now();
    let result = if let Some(doubao) = doubao_client {
        tracing::info!("使用豆包 ASR 备用方案");
        doubao.transcribe_bytes(&audio_data).await
    } else if let Some(sensevoice) = sensevoice_client {
        tracing::info!("使用 SenseVoice 备用方案");
        sensevoice.transcribe_bytes(&audio_data).await
    } else if let Some(qwen) = qwen_client {
        tracing::info!("使用 HTTP 模式千问 ASR 备用");
        qwen.transcribe_bytes(&audio_data).await
    } else {
        tracing::error!("未找到可用的 ASR 客户端以处理备用方案");
        Err(anyhow::anyhow!("ASR 客户端未初始化"))
    };
    let asr_time_ms = asr_start.elapsed().as_millis() as u64;

    handle_transcription_result(app, inserter, post_processor, result, asr_time_ms).await;
}

/// 实时模式转录处理（WebSocket）- 录完再传的回退模式
#[allow(dead_code)]
async fn handle_realtime_transcription(
    app: AppHandle,
    streaming_recorder: Arc<Mutex<Option<StreamingRecorder>>>,
    inserter: Arc<Mutex<Option<TextInserter>>>,
    post_processor: Arc<Mutex<Option<LlmPostProcessor>>>,
    key: String,
    qwen_client_state: Arc<Mutex<Option<QwenASRClient>>>,
    sensevoice_client_state: Arc<Mutex<Option<SenseVoiceClient>>>,
) {
    let _ = app.emit("transcribing", ());

    // 停止流式录音，获取完整音频数据
    let audio_data = {
        let mut recorder_guard = streaming_recorder.lock().unwrap();
        if let Some(ref mut rec) = *recorder_guard {
            match rec.stop_streaming() {
                Ok(data) => Some(data),
                Err(e) => {
                    emit_error_and_hide_overlay(&app, format!("停止录音失败: {}", e));
                    None
                }
            }
        } else {
            None
        }
    };

    if audio_data.is_none() {
        return;
    }

    let audio_data = audio_data.unwrap();

    // 尝试使用 WebSocket 实时 API
    tracing::info!("尝试使用 WebSocket 实时 API 转录...");

    let asr_start = std::time::Instant::now();
    let realtime_client = QwenRealtimeClient::new(key.clone());
    let ws_result = realtime_transcribe_audio(&realtime_client, &audio_data).await;
    let asr_time_ms = asr_start.elapsed().as_millis() as u64;

    match ws_result {
        Ok(text) => {
            tracing::info!("WebSocket 实时转录成功: {} (ASR 耗时: {}ms)", text, asr_time_ms);
            handle_transcription_result(app, inserter, post_processor, Ok(text), asr_time_ms).await;
        }
        Err(e) => {
            tracing::warn!("WebSocket 实时转录失败: {}，尝试备用方案", e);
            fallback_transcription(
                app,
                inserter,
                post_processor,
                qwen_client_state,
                sensevoice_client_state,
                Arc::new(Mutex::new(None)), // 此函数未使用豆包
                audio_data,
            )
            .await;
        }
    }
}

/// 使用 WebSocket 实时 API 转录音频
async fn realtime_transcribe_audio(
    client: &QwenRealtimeClient,
    wav_data: &[u8],
) -> anyhow::Result<String> {
    // 创建 WebSocket 会话
    let mut session = client.start_session().await?;

    // 从 WAV 数据中提取 PCM 样本
    let pcm_samples = extract_pcm_from_wav(wav_data)?;

    // 分块发送音频数据（每块 3200 样本 = 0.2秒 @ 16kHz）
    const CHUNK_SIZE: usize = 3200;
    for chunk in pcm_samples.chunks(CHUNK_SIZE) {
        session.send_audio_chunk(chunk).await?;
        // 模拟实时发送的间隔（可选，用于更真实的流式体验）
        // tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    // 提交音频缓冲区
    session.commit_audio().await?;

    // 等待转录结果
    let result = session.wait_for_result().await?;

    // 关闭会话
    let _ = session.close().await;

    Ok(result)
}

/// 从 WAV 数据中提取 PCM 样本（16-bit, 16kHz, 单声道）
fn extract_pcm_from_wav(wav_data: &[u8]) -> anyhow::Result<Vec<i16>> {
    use std::io::Cursor;

    let cursor = Cursor::new(wav_data);
    let reader = hound::WavReader::new(cursor)?;

    let samples: Vec<i16> = reader.into_samples::<i16>()
        .filter_map(|s| s.ok())
        .collect();

    Ok(samples)
}

/// 统一的错误处理辅助函数 - 发送错误事件并隐藏悬浮窗
fn emit_error_and_hide_overlay(app: &AppHandle, error_msg: String) {
    tracing::error!("发送错误并隐藏悬浮窗: {}", error_msg);
    let _ = app.emit("error", error_msg);

    // 隐藏悬浮窗，带重试机制
    if let Some(overlay) = app.get_webview_window("overlay") {
        if let Err(e) = overlay.hide() {
            tracing::error!("隐藏悬浮窗失败: {}", e);
            // 延迟 50ms 重试一次
            std::thread::sleep(std::time::Duration::from_millis(50));
            if let Err(e) = overlay.hide() {
                tracing::error!("隐藏悬浮窗重试仍然失败: {}", e);
            }
        }
    }
}

/// 转录完成事件的 payload
#[derive(Clone, serde::Serialize)]
struct TranscriptionResult {
    text: String,
    original_text: Option<String>, // 原始 ASR 文本（仅开启 LLM 润色时有值）
    asr_time_ms: u64,
    llm_time_ms: Option<u64>,
    total_time_ms: u64,
}

/// 处理转录结果
async fn handle_transcription_result(
    app: AppHandle,
    inserter: Arc<Mutex<Option<TextInserter>>>,
    post_processor: Arc<Mutex<Option<LlmPostProcessor>>>,
    result: anyhow::Result<String>,
    asr_time_ms: u64,
) {
    match result {
        Ok(text) => {
            tracing::info!("转录结果: {} (ASR 耗时: {}ms)", text, asr_time_ms);

            // 如果启用了 LLM 后处理，则进行润色
            let (final_text, original_text, llm_time_ms) = {
                let processor = post_processor.lock().unwrap().clone();
                if let Some(processor) = processor {
                    tracing::info!("开始 LLM 后处理...");
                    let _ = app.emit("post_processing", ());
                    let llm_start = std::time::Instant::now();
                    match processor.polish_transcript(&text).await {
                        Ok(polished) => {
                            let llm_elapsed = llm_start.elapsed().as_millis() as u64;
                            tracing::info!("LLM 后处理完成: {} (耗时: {}ms)", polished, llm_elapsed);
                            (polished, Some(text), Some(llm_elapsed))
                        }
                        Err(e) => {
                            tracing::warn!("LLM 后处理失败，使用原文: {}", e);
                            (text, None, None)
                        }
                    }
                } else {
                    (text, None, None)
                }
            };

            let total_time_ms = asr_time_ms + llm_time_ms.unwrap_or(0);

            // 插入文本
            {
                let mut inserter_guard = inserter.lock().unwrap();
                if let Some(ref mut ins) = *inserter_guard {
                    if let Err(e) = ins.insert_text(&final_text) {
                        tracing::error!("插入文本失败: {}", e);
                        let _ = app.emit("error", format!("插入文本失败: {}", e));
                    }
                }
            } // 释放 inserter_guard

            let result = TranscriptionResult {
                text: final_text,
                original_text,
                asr_time_ms,
                llm_time_ms,
                total_time_ms,
            };

            // 先隐藏录音悬浮窗
            if let Some(overlay) = app.get_webview_window("overlay") {
                if let Err(e) = overlay.hide() {
                    tracing::error!("隐藏悬浮窗失败: {}", e);
                    // 延迟 50ms 重试一次
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    if let Err(e) = overlay.hide() {
                        tracing::error!("隐藏悬浮窗重试仍然失败: {}", e);
                    }
                }
            }

            // 后发送完成事件
            let _ = app.emit("transcription_complete", result);
        }
        Err(e) => {
            // 先隐藏录音悬浮窗
            if let Some(overlay) = app.get_webview_window("overlay") {
                if let Err(e) = overlay.hide() {
                    tracing::error!("隐藏悬浮窗失败: {}", e);
                    // 延迟 50ms 重试一次
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    if let Err(e) = overlay.hide() {
                        tracing::error!("隐藏悬浮窗重试仍然失败: {}", e);
                    }
                }
            }

            // 后发送错误事件
            tracing::error!("转录失败: {}", e);
            let _ = app.emit("error", format!("转录失败: {}", e));
        }
    }
}

#[tauri::command]
async fn stop_app(app_handle: AppHandle) -> Result<String, String> {
    tracing::info!("停止应用...");

    let state = app_handle.state::<AppState>();

    let mut is_running = state.is_running.lock().unwrap();
    if !*is_running {
        return Err("应用未在运行".to_string());
    }

    *state.audio_recorder.lock().unwrap() = None;
    *state.streaming_recorder.lock().unwrap() = None;
    *state.text_inserter.lock().unwrap() = None;
    *state.post_processor.lock().unwrap() = None;
    *state.qwen_client.lock().unwrap() = None;
    *state.sensevoice_client.lock().unwrap() = None;
    *state.doubao_client.lock().unwrap() = None;
    *is_running = false;

    Ok("应用已停止".to_string())
}

#[tauri::command]
async fn hide_to_tray(app_handle: AppHandle) -> Result<String, String> {
    if let Some(window) = app_handle.get_webview_window("main") {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok("已最小化到托盘".to_string())
}

#[tauri::command]
async fn quit_app(app_handle: AppHandle) -> Result<(), String> {
    // 先停止服务
    let state = app_handle.state::<AppState>();
    {
        let mut is_running = state.is_running.lock().unwrap();
        if *is_running {
            *state.audio_recorder.lock().unwrap() = None;
            *state.streaming_recorder.lock().unwrap() = None;
            *state.text_inserter.lock().unwrap() = None;
            *state.post_processor.lock().unwrap() = None;
            *state.qwen_client.lock().unwrap() = None;
            *state.sensevoice_client.lock().unwrap() = None;
            *state.doubao_client.lock().unwrap() = None;
            *is_running = false;
        }
    }
    app_handle.exit(0);
    Ok(())
}

#[tauri::command]
async fn cancel_transcription(app_handle: AppHandle) -> Result<String, String> {
    tracing::info!("取消转录...");

    let state = app_handle.state::<AppState>();

    // 1. 停止流式录音
    {
        let mut recorder_guard = state.streaming_recorder.lock().unwrap();
        if let Some(ref mut rec) = *recorder_guard {
            let _ = rec.stop_streaming();
        }
    }

    // 2. 停止普通录音
    {
        let mut recorder_guard = state.audio_recorder.lock().unwrap();
        if let Some(ref mut rec) = *recorder_guard {
            let _ = rec.stop_recording_to_memory();
        }
    }

    // 3. 取消音频发送任务
    {
        let handle = state.audio_sender_handle.lock().unwrap().take();
        if let Some(h) = handle {
            h.abort();
            tracing::info!("已取消音频发送任务");
        }
    }

    // 4. 关闭 WebSocket 会话
    {
        let mut session_guard = state.active_session.lock().await;
        if let Some(ref session) = *session_guard {
            let _ = session.close().await;
            tracing::info!("已关闭 WebSocket 会话");
        }
        *session_guard = None;
    }

    // 5. 隐藏录音悬浮窗（带重试机制）
    if let Some(overlay) = app_handle.get_webview_window("overlay") {
        if let Err(e) = overlay.hide() {
            tracing::error!("取消转录时隐藏悬浮窗失败，准备重试: {}", e);
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            if let Err(e) = overlay.hide() {
                tracing::error!("取消转录时隐藏悬浮窗重试仍然失败: {}", e);
            }
        }
    }

    // 6. 发送取消事件
    let _ = app_handle.emit("transcription_cancelled", ());

    Ok("已取消转录".to_string())
}

/// 显示录音悬浮窗
#[tauri::command]
async fn show_overlay(app_handle: AppHandle) -> Result<(), String> {
    if let Some(overlay) = app_handle.get_webview_window("overlay") {
        overlay.show().map_err(|e| e.to_string())?;
        overlay.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 隐藏录音悬浮窗（带重试机制）
#[tauri::command]
async fn hide_overlay(app_handle: AppHandle) -> Result<(), String> {
    if let Some(overlay) = app_handle.get_webview_window("overlay") {
        // 第一次尝试
        if let Err(e) = overlay.hide() {
            tracing::error!("隐藏悬浮窗失败，准备重试: {}", e);
            // 延迟 50ms 重试
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            overlay.hide().map_err(|e| {
                tracing::error!("隐藏悬浮窗重试仍然失败: {}", e);
                e.to_string()
            })?;
        }
    }
    Ok(())
}

/// 设置开机自启动
#[tauri::command]
async fn set_autostart(app: AppHandle, enabled: bool) -> Result<String, String> {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(|e| e.to_string())?;
    } else {
        manager.disable().map_err(|e| e.to_string())?;
    }
    Ok(if enabled { "已启用开机自启" } else { "已禁用开机自启" }.to_string())
}

/// 获取开机自启动状态
#[tauri::command]
async fn get_autostart(app: AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 初始化日志
    tracing_subscriber::fmt::init();

    // 检查是否静默启动（开机自启时）
    let args: Vec<String> = std::env::args().collect();
    let start_minimized = args.contains(&"--minimized".to_string());

    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .setup(move |app| {
            // 如果是静默启动，隐藏主窗口
            if start_minimized {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                    tracing::info!("静默启动模式：主窗口已隐藏");
                }
            }

            // 初始化应用状态
            let app_state = AppState {
                audio_recorder: Arc::new(Mutex::new(None)),
                streaming_recorder: Arc::new(Mutex::new(None)),
                text_inserter: Arc::new(Mutex::new(None)),
                post_processor: Arc::new(Mutex::new(None)),
                is_running: Arc::new(Mutex::new(false)),
                use_realtime_asr: Arc::new(Mutex::new(true)),
                enable_post_process: Arc::new(Mutex::new(false)),
                qwen_client: Arc::new(Mutex::new(None)),
                sensevoice_client: Arc::new(Mutex::new(None)),
                doubao_client: Arc::new(Mutex::new(None)),
                active_session: Arc::new(tokio::sync::Mutex::new(None)),
                doubao_session: Arc::new(tokio::sync::Mutex::new(None)),
                realtime_provider: Arc::new(Mutex::new(None)),
                audio_sender_handle: Arc::new(Mutex::new(None)),
            };
            app.manage(app_state);

            // 创建托盘菜单
            let show_item = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出程序", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            // 创建系统托盘
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("PushToTalk - AI 语音转写助手")
                .on_menu_event(|app, event| {
                    match event.id.as_ref() {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                        if let Some(window) = tray.app_handle().get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.emit("close_requested", ());
            }
        })
        .invoke_handler(tauri::generate_handler![
            save_config,
            load_config,
            start_app,
            stop_app,
            cancel_transcription,
            hide_to_tray,
            quit_app,
            show_overlay,
            hide_overlay,
            set_autostart,
            get_autostart,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
