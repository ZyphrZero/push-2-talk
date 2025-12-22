// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod assistant_processor;
mod audio_recorder;
mod audio_utils;
mod asr;
mod beep_player;
mod clipboard_manager;
mod config;
mod hotkey_service;
mod llm_post_processor;
mod openai_client;
mod pipeline;
mod streaming_recorder;
mod text_inserter;

use audio_recorder::AudioRecorder;
use asr::{QwenASRClient, SenseVoiceClient, DoubaoASRClient, QwenRealtimeClient, DoubaoRealtimeClient, DoubaoRealtimeSession, RealtimeSession};
use assistant_processor::AssistantProcessor;
use config::AppConfig;
use hotkey_service::HotkeyService;
use llm_post_processor::LlmPostProcessor;
use pipeline::{AssistantPipeline, NormalPipeline, TranscriptionContext};
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
    /// AI 助手处理器（支持双系统提示词）
    assistant_processor: Arc<Mutex<Option<AssistantProcessor>>>,
    is_running: Arc<Mutex<bool>>,
    use_realtime_asr: Arc<Mutex<bool>>,
    enable_post_process: Arc<Mutex<bool>>,
    enable_fallback: Arc<Mutex<bool>>,
    qwen_client: Arc<Mutex<Option<QwenASRClient>>>,
    sensevoice_client: Arc<Mutex<Option<SenseVoiceClient>>>,
    doubao_client: Arc<Mutex<Option<DoubaoASRClient>>>,
    // 活跃的实时转录会话（用于真正的流式传输）
    active_session: Arc<tokio::sync::Mutex<Option<RealtimeSession>>>,
    doubao_session: Arc<tokio::sync::Mutex<Option<DoubaoRealtimeSession>>>,
    realtime_provider: Arc<Mutex<Option<config::AsrProvider>>>,
    // 音频发送任务句柄
    audio_sender_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    // 单例热键服务
    hotkey_service: Arc<HotkeyService>,
    /// 当前触发模式（听写/AI助手）
    current_trigger_mode: Arc<Mutex<Option<config::TriggerMode>>>,
}

// Tauri Commands

#[tauri::command]
async fn save_config(
    api_key: String,
    fallback_api_key: String,
    use_realtime: Option<bool>,
    enable_post_process: Option<bool>,
    llm_config: Option<config::LlmConfig>,
    smart_command_config: Option<config::SmartCommandConfig>,
    close_action: Option<String>,
    asr_config: Option<config::AsrConfig>,
    hotkey_config: Option<config::HotkeyConfig>,
    dual_hotkey_config: Option<config::DualHotkeyConfig>,
    assistant_config: Option<config::AssistantConfig>,
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
        smart_command_config: smart_command_config.unwrap_or_default(),
        assistant_config: assistant_config.unwrap_or_default(),
        close_action,
        hotkey_config,
        dual_hotkey_config: dual_hotkey_config.unwrap_or_default(),
        transcription_mode: config::TranscriptionMode::default(),
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

/// 处理录音开始的核心逻辑
async fn handle_recording_start(
    app: AppHandle,
    recorder: Arc<Mutex<Option<AudioRecorder>>>,
    streaming_recorder: Arc<Mutex<Option<StreamingRecorder>>>,
    active_session: Arc<tokio::sync::Mutex<Option<RealtimeSession>>>,
    doubao_session: Arc<tokio::sync::Mutex<Option<DoubaoRealtimeSession>>>,
    realtime_provider: Arc<Mutex<Option<config::AsrProvider>>>,
    audio_sender_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    use_realtime: bool,
    api_key: String,
    doubao_app_id: Option<String>,
    doubao_access_token: Option<String>,
) {
    tracing::info!("检测到快捷键按下");
    let _ = app.emit("recording_started", ());

    // 显示录音悬浮窗并移动到屏幕底部居中
    if let Some(overlay) = app.get_webview_window("overlay") {
        if let Some(monitor) = overlay.primary_monitor().ok().flatten() {
            let screen_size = monitor.size();
            let scale_factor = monitor.scale_factor();
            let overlay_size = overlay.outer_size().unwrap_or(tauri::PhysicalSize::new(120, 44));

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
        let provider = realtime_provider.lock().unwrap().clone();
        match provider {
            Some(config::AsrProvider::Doubao) => {
                handle_doubao_realtime_start(app, streaming_recorder, doubao_session, audio_sender_handle, doubao_app_id, doubao_access_token).await;
            }
            _ => {
                handle_qwen_realtime_start(app, streaming_recorder, active_session, audio_sender_handle, api_key).await;
            }
        }
    } else {
        let mut recorder_guard = recorder.lock().unwrap();
        if let Some(ref mut rec) = *recorder_guard {
            // 检查是否已在录音，如果是则先停止
            if rec.is_recording() {
                tracing::warn!("发现正在进行的录音，先停止它");
                let _ = rec.stop_recording_to_memory();
            }
            if let Err(e) = rec.start_recording(Some(app.clone())) {
                emit_error_and_hide_overlay(&app, format!("录音失败: {}", e));
            }
        } else {
            emit_error_and_hide_overlay(&app, "录音器未初始化".to_string());
        }
    }
}

/// 处理豆包实时模式启动
async fn handle_doubao_realtime_start(
    app: AppHandle,
    streaming_recorder: Arc<Mutex<Option<StreamingRecorder>>>,
    doubao_session: Arc<tokio::sync::Mutex<Option<DoubaoRealtimeSession>>>,
    audio_sender_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    doubao_app_id: Option<String>,
    doubao_access_token: Option<String>,
) {
    tracing::info!("启动豆包实时流式转录...");

    let chunk_rx = {
        let mut streaming_guard = streaming_recorder.lock().unwrap();
        if let Some(ref mut rec) = *streaming_guard {
            // 检查是否已在录音，如果是则先停止
            if rec.is_recording() {
                tracing::warn!("发现正在进行的流式录音，先停止它");
                let _ = rec.stop_streaming();
            }
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
        if let (Some(app_id), Some(access_token)) = (doubao_app_id.as_ref(), doubao_access_token.as_ref()) {
            let realtime_client = DoubaoRealtimeClient::new(app_id.clone(), access_token.clone());
            // 清理旧的会话和任务（防止资源泄漏）
            {
                let mut session_guard = doubao_session.lock().await;
                if let Some(mut old_session) = session_guard.take() {
                    tracing::warn!("发现旧的豆包会话，先关闭它");
                    let _ = old_session.finish_audio().await;
                }
            }
            {
                if let Some(old_handle) = audio_sender_handle.lock().unwrap().take() {
                    tracing::warn!("发现旧的音频发送任务，先取消它");
                    old_handle.abort();
                }
            }

            match realtime_client.start_session().await {
                Ok(session) => {
                    tracing::info!("豆包 WebSocket 连接已建立");
                    *doubao_session.lock().await = Some(session);

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
                }
            }
        } else {
            tracing::error!("豆包凭证缺失：需要 app_id 和 access_token");
        }
    }
}

/// 处理千问实时模式启动
async fn handle_qwen_realtime_start(
    app: AppHandle,
    streaming_recorder: Arc<Mutex<Option<StreamingRecorder>>>,
    active_session: Arc<tokio::sync::Mutex<Option<RealtimeSession>>>,
    audio_sender_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    api_key: String,
) {
    tracing::info!("启动千问实时流式转录...");

    // 清理旧的会话和任务（防止资源泄漏）
    {
        let mut session_guard = active_session.lock().await;
        if let Some(old_session) = session_guard.take() {
            tracing::warn!("发现旧的千问会话，先关闭它");
            let _ = old_session.close().await;
        }
    }
    {
        if let Some(old_handle) = audio_sender_handle.lock().unwrap().take() {
            tracing::warn!("发现旧的音频发送任务，先取消它");
            old_handle.abort();
        }
    }

    let realtime_client = QwenRealtimeClient::new(api_key);
    match realtime_client.start_session().await {
        Ok(session) => {
            tracing::info!("千问 WebSocket 连接已建立");

            let chunk_rx = {
                let mut streaming_guard = streaming_recorder.lock().unwrap();
                if let Some(ref mut rec) = *streaming_guard {
                    // 检查是否已在录音，如果是则先停止
                    if rec.is_recording() {
                        tracing::warn!("发现正在进行的流式录音，先停止它");
                        let _ = rec.stop_streaming();
                    }
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
                *active_session.lock().await = Some(session);

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

            let mut streaming_guard = streaming_recorder.lock().unwrap();
            if let Some(ref mut rec) = *streaming_guard {
                // 检查是否已在录音，如果是则先停止
                if rec.is_recording() {
                    tracing::warn!("发现正在进行的流式录音，先停止它");
                    let _ = rec.stop_streaming();
                }
                if let Err(e) = rec.start_streaming(Some(app.clone())) {
                    emit_error_and_hide_overlay(&app, format!("录音失败: {}", e));
                }
            } else {
                emit_error_and_hide_overlay(&app, "录音器未初始化".to_string());
            }
        }
    }
}

#[tauri::command]
async fn start_app(
    app_handle: AppHandle,
    api_key: String,
    fallback_api_key: String,
    use_realtime: Option<bool>,
    enable_post_process: Option<bool>,
    llm_config: Option<config::LlmConfig>,
    smart_command_config: Option<config::SmartCommandConfig>,
    asr_config: Option<config::AsrConfig>,
    hotkey_config: Option<config::HotkeyConfig>,
    dual_hotkey_config: Option<config::DualHotkeyConfig>,
    assistant_config: Option<config::AssistantConfig>,
) -> Result<String, String> {
    tracing::info!("启动应用...");

    // 获取应用状态
    tracing::info!("[DEBUG] 获取应用状态...");
    let state = app_handle.state::<AppState>();
    tracing::info!("[DEBUG] 应用状态已获取");

    // 先检查是否已在运行（快速获取并释放锁）
    tracing::info!("[DEBUG] 检查运行状态...");
    let need_stop = {
        let is_running = state.is_running.lock().unwrap();
        tracing::info!("[DEBUG] 当前运行状态: {}", *is_running);
        *is_running
    }; // 锁在这里释放

    if need_stop {
        tracing::info!("[DEBUG] 检测到应用已在运行，自动停止中...");
        // 先停止应用（忽略停止时的错误）
        if let Err(e) = stop_app(app_handle.clone()).await {
            tracing::warn!("[DEBUG] 停止应用时出现警告: {}", e);
        }
        tracing::info!("[DEBUG] 应用已停止，继续启动流程");
    }

    tracing::info!("[DEBUG] 开始初始化...");

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

    // 存储 fallback 配置
    {
        let enable_fb = asr_config
            .as_ref()
            .map(|c| c.enable_fallback)
            .unwrap_or(false);
        *state.enable_fallback.lock().unwrap() = enable_fb;
        tracing::info!("并行 fallback: {}", if enable_fb { "启用" } else { "禁用" });
    }

    // 初始化 LLM 后处理器（复用连接）
    {
        let mut processor_guard = state.post_processor.lock().unwrap();
        let llm_cfg = llm_config.unwrap_or_default();
        tracing::info!("[DEBUG] LLM 后处理配置: enabled={}, api_key_len={}", enable_post_process_mode, llm_cfg.api_key.len());
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

    // 初始化 AI 助手处理器（独立配置，支持双系统提示词，永远开启只需检查配置有效性）
    tracing::info!("[DEBUG] 初始化 AI 助手处理器...");
    {
        let mut processor_guard = state.assistant_processor.lock().unwrap();
        let assistant_cfg = assistant_config.unwrap_or_default();
        tracing::info!("[DEBUG] AI 助手配置: api_key_len={}", assistant_cfg.api_key.len());
        if assistant_cfg.is_valid() {
            tracing::info!("AI 助手处理器配置: endpoint={}, model={}", assistant_cfg.endpoint, assistant_cfg.model);
            *processor_guard = Some(AssistantProcessor::new(assistant_cfg));
            tracing::info!("AI 助手处理器已初始化");
        } else {
            *processor_guard = None;
            tracing::info!("AI 助手未配置 API，Alt+Space 模式不可用");
        }
    }
    tracing::info!("[DEBUG] AI 助手处理器初始化完成");

    // 初始化文本插入器
    tracing::info!("[DEBUG] 初始化文本插入器...");
    let text_inserter = TextInserter::new()
        .map_err(|e| format!("初始化文本插入器失败: {}", e))?;
    *state.text_inserter.lock().unwrap() = Some(text_inserter);
    tracing::info!("[DEBUG] 文本插入器初始化完成");

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

    // 启动全局快捷键监听（双模式支持）
    tracing::info!("[DEBUG] 准备热键配置...");
    let dual_hotkey_cfg = dual_hotkey_config.unwrap_or_default();

    // 验证热键配置
    tracing::info!("[DEBUG] 验证热键配置...");
    dual_hotkey_cfg.validate()
        .map_err(|e| format!("热键配置无效: {}", e))?;
    tracing::info!("[DEBUG] 热键配置验证通过");

    let hotkey_service = Arc::clone(&state.hotkey_service);

    // 克隆状态用于回调（听写模式）
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
    // AI 助手模式专用
    let current_trigger_mode_start = Arc::clone(&state.current_trigger_mode);

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
    let post_processor_stop = Arc::clone(&state.post_processor);
    let assistant_processor_stop = Arc::clone(&state.assistant_processor);
    let text_inserter_stop = Arc::clone(&state.text_inserter);
    let qwen_client_stop = Arc::clone(&state.qwen_client);
    let sensevoice_client_stop = Arc::clone(&state.sensevoice_client);
    let doubao_client_stop = Arc::clone(&state.doubao_client);
    let doubao_session_stop = Arc::clone(&state.doubao_session);
    let realtime_provider_stop = Arc::clone(&state.realtime_provider);
    let use_realtime_stop = use_realtime_mode;
    let is_running_stop = Arc::clone(&state.is_running);
    let enable_fallback_stop = Arc::clone(&state.enable_fallback);

    // 按键按下回调（支持双模式）
    let on_start = move |trigger_mode: config::TriggerMode| {
        if !*is_running_start.lock().unwrap() {
            tracing::debug!("服务已停止，忽略快捷键按下事件");
            return;
        }

        // 保存当前触发模式
        *current_trigger_mode_start.lock().unwrap() = Some(trigger_mode);
        tracing::info!("触发模式: {:?}", trigger_mode);

        // 注意：剪贴板捕获已移至 on_stop 回调
        // 原因：在 on_start 时物理按键仍被按住，模拟 Ctrl+C 会与 Alt/Meta 等修饰键冲突

        beep_player::play_start_beep();

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

        tauri::async_runtime::spawn(async move {
            handle_recording_start(
                app,
                recorder,
                streaming_recorder,
                active_session,
                doubao_session,
                realtime_provider,
                audio_sender_handle,
                use_realtime,
                api_key,
                doubao_app_id,
                doubao_access_token,
            ).await;
        });
    };

    // 按键释放回调（支持双模式）
    let on_stop = move |trigger_mode: config::TriggerMode| {
        // 检查服务是否仍在运行
        if !*is_running_stop.lock().unwrap() {
            tracing::debug!("服务已停止，忽略快捷键释放事件");
            return;
        }

        tracing::info!("检测到快捷键释放，模式: {:?}", trigger_mode);

        let app = app_handle_stop.clone();
        let recorder = Arc::clone(&audio_recorder_stop);
        let streaming_recorder = Arc::clone(&streaming_recorder_stop);
        let active_session = Arc::clone(&active_session_stop);
        let audio_sender_handle = Arc::clone(&audio_sender_handle_stop);
        let qwen_client_state = Arc::clone(&qwen_client_stop);
        let sensevoice_client_state = Arc::clone(&sensevoice_client_stop);
        let doubao_client_state = Arc::clone(&doubao_client_stop);
        let doubao_session_state = Arc::clone(&doubao_session_stop);
        let realtime_provider_state = Arc::clone(&realtime_provider_stop);
        let enable_fallback_state = Arc::clone(&enable_fallback_stop);
        let use_realtime = use_realtime_stop;

        // 根据触发模式选择处理器
        let post_processor = Arc::clone(&post_processor_stop);
        let assistant_processor = Arc::clone(&assistant_processor_stop);
        let text_inserter = Arc::clone(&text_inserter_stop);

        // 播放停止录音提示音
        beep_player::play_stop_beep();

        tauri::async_runtime::spawn(async move {
            let _ = app.emit("recording_stopped", ());

            match trigger_mode {
                config::TriggerMode::Dictation => {
                    // 听写模式：使用 NormalPipeline（纯转录 + 可选润色）
                    tracing::info!("使用听写模式处理");
                    if use_realtime {
                        handle_realtime_stop(
                            app,
                            streaming_recorder,
                            active_session,
                            doubao_session_state,
                            realtime_provider_state,
                            audio_sender_handle,
                            post_processor,
                            text_inserter,
                            qwen_client_state,
                            sensevoice_client_state,
                            doubao_client_state,
                            enable_fallback_state,
                        ).await;
                    } else {
                        handle_http_transcription(
                            app,
                            recorder,
                            post_processor,
                            text_inserter,
                            qwen_client_state,
                            sensevoice_client_state,
                            doubao_client_state,
                            enable_fallback_state,
                        ).await;
                    }
                }
                config::TriggerMode::AiAssistant => {
                    // AI 助手模式：使用 AssistantPipeline
                    tracing::info!("使用 AI 助手模式处理");

                    // 等待物理按键完全释放后再捕获剪贴板
                    // 原因：在 on_start 时物理按键仍被按住，模拟 Ctrl+C 会与 Alt/Meta 等修饰键冲突
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

                    // 捕获选中文本（此时用户已松开热键，Ctrl+C 模拟安全）
                    tracing::info!("AI 助手模式：开始捕获选中文本...");
                    let (clipboard_guard, selected_text) = match clipboard_manager::get_selected_text() {
                        Ok((guard, text)) => {
                            if let Some(ref t) = text {
                                tracing::info!("已捕获选中文本: {} 字符", t.len());
                            } else {
                                tracing::info!("无选中文本，将使用问答模式");
                            }
                            (Some(guard), text)
                        }
                        Err(e) => {
                            tracing::warn!("捕获选中文本失败: {}，继续处理但无上下文", e);
                            (None, None)
                        }
                    };

                    handle_assistant_mode(
                        app,
                        recorder,
                        streaming_recorder,
                        active_session,
                        doubao_session_state,
                        realtime_provider_state,
                        audio_sender_handle,
                        assistant_processor,
                        clipboard_guard,
                        selected_text,
                        qwen_client_state,
                        sensevoice_client_state,
                        doubao_client_state,
                        enable_fallback_state,
                        use_realtime,
                    ).await;
                }
            }
        });
    };

    tracing::info!("[DEBUG] 准备激活热键服务...");
    hotkey_service
        .activate_dual(dual_hotkey_cfg.clone(), on_start, on_stop)
        .map_err(|e| format!("启动快捷键监听失败: {}", e))?;
    tracing::info!("[DEBUG] 热键服务已激活");

    // 标记为运行中（重新获取锁）
    *state.is_running.lock().unwrap() = true;
    tracing::info!("[DEBUG] 启动完成!");
    let mode_str = if use_realtime_mode { "实时模式" } else { "HTTP 模式" };
    let dictation_display = dual_hotkey_cfg.dictation.format_display();
    let assistant_display = dual_hotkey_cfg.assistant.format_display();
    Ok(format!(
        "应用已启动 ({})，听写: {}，AI助手: {}",
        mode_str, dictation_display, assistant_display
    ))
}

/// AI 助手模式处理
///
/// 使用 AssistantPipeline 进行上下文感知的 LLM 处理
async fn handle_assistant_mode(
    app: AppHandle,
    recorder: Arc<Mutex<Option<AudioRecorder>>>,
    streaming_recorder: Arc<Mutex<Option<StreamingRecorder>>>,
    active_session: Arc<tokio::sync::Mutex<Option<RealtimeSession>>>,
    doubao_session: Arc<tokio::sync::Mutex<Option<DoubaoRealtimeSession>>>,
    realtime_provider: Arc<Mutex<Option<config::AsrProvider>>>,
    audio_sender_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    assistant_processor: Arc<Mutex<Option<AssistantProcessor>>>,
    clipboard_guard: Option<clipboard_manager::ClipboardGuard>,
    selected_text: Option<String>,
    qwen_client_state: Arc<Mutex<Option<QwenASRClient>>>,
    sensevoice_client_state: Arc<Mutex<Option<SenseVoiceClient>>>,
    doubao_client_state: Arc<Mutex<Option<DoubaoASRClient>>>,
    enable_fallback_state: Arc<Mutex<bool>>,
    use_realtime: bool,
) {
    let _ = app.emit("transcribing", ());
    let asr_start = std::time::Instant::now();

    // 1. 停止录音并获取音频数据
    let (asr_result, audio_data) = if use_realtime {
        // 实时模式：先停止流式录音
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

        // 等待音频发送任务完成
        {
            let handle = audio_sender_handle.lock().unwrap().take();
            if let Some(h) = handle {
                tracing::info!("等待音频发送任务完成...");
                let _ = h.await;
            }
        }

        // 获取实时转录结果
        let provider = realtime_provider.lock().unwrap().clone();
        let result = match provider {
            Some(config::AsrProvider::Doubao) => {
                let mut session_guard = doubao_session.lock().await;
                if let Some(ref mut session) = *session_guard {
                    let _ = session.finish_audio().await;
                    let res = session.wait_for_result().await;
                    drop(session_guard);
                    *doubao_session.lock().await = None;
                    res
                } else {
                    Err(anyhow::anyhow!("没有活跃的豆包会话"))
                }
            }
            _ => {
                let mut session_guard = active_session.lock().await;
                if let Some(ref mut session) = *session_guard {
                    let _ = session.commit_audio().await;
                    let res = session.wait_for_result().await;
                    let _ = session.close().await;
                    drop(session_guard);
                    *active_session.lock().await = None;
                    res
                } else {
                    Err(anyhow::anyhow!("没有活跃的千问会话"))
                }
            }
        };

        (result, audio_data)
    } else {
        // HTTP 模式：停止录音并获取数据
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

        let result = if let Some(ref data) = audio_data {
            // 使用 HTTP ASR
            let enable_fb = *enable_fallback_state.lock().unwrap();
            let qwen = { qwen_client_state.lock().unwrap().clone() };
            let doubao = { doubao_client_state.lock().unwrap().clone() };
            let sensevoice = { sensevoice_client_state.lock().unwrap().clone() };

            transcribe_with_available_clients(qwen, doubao, sensevoice, data, enable_fb, "(AI助手HTTP) ").await
        } else {
            Err(anyhow::anyhow!("未获取到音频数据"))
        };

        (result, audio_data)
    };

    let asr_time_ms = asr_start.elapsed().as_millis() as u64;

    // 2. 如果实时模式失败且有音频数据，尝试 HTTP 备用
    let final_result = if asr_result.is_err() && audio_data.is_some() {
        tracing::warn!("实时 ASR 失败，尝试 HTTP 备用");
        let data = audio_data.unwrap();
        let enable_fb = *enable_fallback_state.lock().unwrap();
        let qwen = { qwen_client_state.lock().unwrap().clone() };
        let doubao = { doubao_client_state.lock().unwrap().clone() };
        let sensevoice = { sensevoice_client_state.lock().unwrap().clone() };

        transcribe_with_available_clients(qwen, doubao, sensevoice, &data, enable_fb, "(AI助手备用) ").await
    } else {
        asr_result
    };

    // 3. 使用 AssistantPipeline 处理
    let processor = { assistant_processor.lock().unwrap().clone() };
    let pipeline = AssistantPipeline::new();

    let context = TranscriptionContext {
        selected_text,
        ..Default::default()
    };

    let pipeline_result = pipeline
        .process(&app, processor, clipboard_guard, final_result, asr_time_ms, context)
        .await;

    // 4. 处理结果
    match pipeline_result {
        Ok(result) => {
            hide_overlay_window(&app).await;

            let transcription_result = TranscriptionResult {
                text: result.text,
                original_text: result.original_text,
                asr_time_ms: result.asr_time_ms,
                llm_time_ms: result.llm_time_ms,
                total_time_ms: result.total_time_ms,
                mode: Some(format!("{:?}", result.mode).to_lowercase()),
                inserted: Some(result.inserted),
            };

            let _ = app.emit("transcription_complete", transcription_result);
        }
        Err(e) => {
            hide_overlay_window(&app).await;
            tracing::error!("AI 助手处理失败: {}", e);
            let _ = app.emit("error", format!("AI 助手处理失败: {}", e));
        }
    }
}

/// 统一的 HTTP ASR 转录逻辑
///
/// 根据可用的 ASR 客户端和 fallback 配置选择合适的转录方式
async fn transcribe_with_available_clients(
    qwen: Option<QwenASRClient>,
    doubao: Option<DoubaoASRClient>,
    sensevoice: Option<SenseVoiceClient>,
    audio_data: &[u8],
    enable_fallback: bool,
    log_prefix: &str,
) -> anyhow::Result<String> {
    if enable_fallback {
        match (&qwen, &doubao, &sensevoice) {
            (Some(q), _, Some(s)) => {
                tracing::info!("{}使用千问+SenseVoice并行竞速", log_prefix);
                asr::transcribe_with_fallback_clients(q.clone(), s.clone(), audio_data.to_vec()).await
            }
            (_, Some(d), Some(s)) => {
                tracing::info!("{}使用豆包+SenseVoice并行竞速", log_prefix);
                asr::transcribe_doubao_sensevoice_race(d.clone(), s.clone(), audio_data.to_vec()).await
            }
            (Some(q), _, _) => {
                tracing::info!("{}使用千问 ASR (无备用)", log_prefix);
                q.transcribe_bytes(audio_data).await
            }
            (_, Some(d), _) => {
                tracing::info!("{}使用豆包 ASR (无备用)", log_prefix);
                d.transcribe_bytes(audio_data).await
            }
            (_, _, Some(s)) => {
                tracing::info!("{}使用 SenseVoice ASR (无备用)", log_prefix);
                s.transcribe_bytes(audio_data).await
            }
            _ => {
                tracing::error!("{}未找到可用的 ASR 客户端", log_prefix);
                Err(anyhow::anyhow!("ASR 客户端未初始化"))
            }
        }
    } else {
        // 非 fallback 模式：按优先级使用单一客户端
        if let Some(d) = doubao {
            tracing::info!("{}使用豆包 ASR", log_prefix);
            d.transcribe_bytes(audio_data).await
        } else if let Some(q) = qwen {
            tracing::info!("{}使用千问 ASR", log_prefix);
            q.transcribe_bytes(audio_data).await
        } else if let Some(s) = sensevoice {
            tracing::info!("{}使用 SenseVoice ASR", log_prefix);
            s.transcribe_bytes(audio_data).await
        } else {
            tracing::error!("{}未找到可用的 ASR 客户端", log_prefix);
            Err(anyhow::anyhow!("ASR 客户端未初始化"))
        }
    }
}

/// HTTP 模式转录处理（听写模式专用）
async fn handle_http_transcription(
    app: AppHandle,
    recorder: Arc<Mutex<Option<AudioRecorder>>>,
    post_processor: Arc<Mutex<Option<LlmPostProcessor>>>,
    text_inserter: Arc<Mutex<Option<TextInserter>>>,
    qwen_client_state: Arc<Mutex<Option<QwenASRClient>>>,
    sensevoice_client_state: Arc<Mutex<Option<SenseVoiceClient>>>,
    doubao_client_state: Arc<Mutex<Option<DoubaoASRClient>>>,
    enable_fallback_state: Arc<Mutex<bool>>,
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

        let enable_fallback = *enable_fallback_state.lock().unwrap();
        let qwen = { qwen_client_state.lock().unwrap().clone() };
        let doubao = { doubao_client_state.lock().unwrap().clone() };
        let sensevoice = { sensevoice_client_state.lock().unwrap().clone() };

        let asr_start = std::time::Instant::now();
        let result = transcribe_with_available_clients(
            qwen, doubao, sensevoice, &audio_data, enable_fallback, "(HTTP) "
        ).await;
        let asr_time_ms = asr_start.elapsed().as_millis() as u64;

        handle_transcription_result(app, post_processor, text_inserter, result, asr_time_ms).await;
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
    post_processor: Arc<Mutex<Option<LlmPostProcessor>>>,
    text_inserter: Arc<Mutex<Option<TextInserter>>>,
    qwen_client_state: Arc<Mutex<Option<QwenASRClient>>>,
    sensevoice_client_state: Arc<Mutex<Option<SenseVoiceClient>>>,
    doubao_client_state: Arc<Mutex<Option<DoubaoASRClient>>>,
    enable_fallback_state: Arc<Mutex<bool>>,
) {
    let _ = app.emit("transcribing", ());
    let asr_start = std::time::Instant::now();
    let enable_fb = *enable_fallback_state.lock().unwrap();

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
                            post_processor,
                            text_inserter,
                            Arc::clone(&qwen_client_state),
                            Arc::clone(&sensevoice_client_state),
                            Arc::clone(&doubao_client_state),
                            audio_data,
                            enable_fb,
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
                        handle_transcription_result(app, post_processor, text_inserter, Ok(text), asr_time_ms).await;
                    }
                    Err(e) => {
                        tracing::warn!("豆包等待转录结果失败: {}，尝试备用方案", e);
                        drop(doubao_session_guard);
                        *doubao_session.lock().await = None;

                        // 回退到备用方案
                        if let Some(audio_data) = audio_data {
                            fallback_transcription(
                                app,
                                post_processor,
                                text_inserter,
                                Arc::clone(&qwen_client_state),
                                Arc::clone(&sensevoice_client_state),
                                Arc::clone(&doubao_client_state),
                                audio_data,
                                enable_fb,
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
                        post_processor,
                        text_inserter,
                        Arc::clone(&qwen_client_state),
                        Arc::clone(&sensevoice_client_state),
                        Arc::clone(&doubao_client_state),
                        audio_data,
                        enable_fb,
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
                            post_processor,
                            text_inserter,
                            Arc::clone(&qwen_client_state),
                            Arc::clone(&sensevoice_client_state),
                            Arc::clone(&doubao_client_state),
                            audio_data,
                            enable_fb,
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
                        handle_transcription_result(app, post_processor, text_inserter, Ok(text), asr_time_ms).await;
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
                                post_processor,
                                text_inserter,
                                Arc::clone(&qwen_client_state),
                                Arc::clone(&sensevoice_client_state),
                                Arc::clone(&doubao_client_state),
                                audio_data,
                                enable_fb,
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
                        post_processor,
                        text_inserter,
                        Arc::clone(&qwen_client_state),
                        Arc::clone(&sensevoice_client_state),
                        Arc::clone(&doubao_client_state),
                        audio_data,
                        enable_fb,
                    )
                    .await;
                } else {
                    emit_error_and_hide_overlay(&app, "没有录制到音频数据".to_string());
                }
            }
        }
    }
}

/// 备用转录方案（HTTP 模式，听写模式专用）
async fn fallback_transcription(
    app: AppHandle,
    post_processor: Arc<Mutex<Option<LlmPostProcessor>>>,
    text_inserter: Arc<Mutex<Option<TextInserter>>>,
    qwen_client_state: Arc<Mutex<Option<QwenASRClient>>>,
    sensevoice_client_state: Arc<Mutex<Option<SenseVoiceClient>>>,
    doubao_client_state: Arc<Mutex<Option<DoubaoASRClient>>>,
    audio_data: Vec<u8>,
    enable_fallback: bool,
) {
    let qwen = { qwen_client_state.lock().unwrap().clone() };
    let sensevoice = { sensevoice_client_state.lock().unwrap().clone() };
    let doubao = { doubao_client_state.lock().unwrap().clone() };

    let asr_start = std::time::Instant::now();
    let result = transcribe_with_available_clients(
        qwen, doubao, sensevoice, &audio_data, enable_fallback, "(备用) "
    ).await;
    let asr_time_ms = asr_start.elapsed().as_millis() as u64;

    handle_transcription_result(app, post_processor, text_inserter, result, asr_time_ms).await;
}

/// 实时模式转录处理（WebSocket）- 录完再传的回退模式
/// 注意：此函数已不再使用，保留用于向后兼容
#[allow(dead_code)]
async fn handle_realtime_transcription(
    app: AppHandle,
    streaming_recorder: Arc<Mutex<Option<StreamingRecorder>>>,
    post_processor: Arc<Mutex<Option<LlmPostProcessor>>>,
    text_inserter: Arc<Mutex<Option<TextInserter>>>,
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
            handle_transcription_result(app, post_processor, text_inserter, Ok(text), asr_time_ms).await;
        }
        Err(e) => {
            tracing::warn!("WebSocket 实时转录失败: {}，尝试备用方案", e);
            fallback_transcription(
                app,
                post_processor,
                text_inserter,
                qwen_client_state,
                sensevoice_client_state,
                Arc::new(Mutex::new(None)), // 此函数未使用豆包
                audio_data,
                false, // 此函数未使用，默认禁用并行 fallback
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
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<String>,  // 新增：处理模式
    #[serde(skip_serializing_if = "Option::is_none")]
    inserted: Option<bool>, // 新增：是否已自动插入
}

/// 处理转录结果（听写模式专用，使用 NormalPipeline）
///
/// 听写模式（Ctrl+Win）使用此函数处理 ASR 结果
/// AI 助手模式（Alt+Space）使用独立的 handle_assistant_mode 函数
async fn handle_transcription_result(
    app: AppHandle,
    post_processor: Arc<Mutex<Option<LlmPostProcessor>>>,
    text_inserter: Arc<Mutex<Option<TextInserter>>>,
    result: anyhow::Result<String>,
    asr_time_ms: u64,
) {
    // 从锁中提取处理器（clone 后立即释放锁）
    let post_proc = { post_processor.lock().unwrap().clone() };

    // 听写模式：只使用 NormalPipeline
    let pipeline = NormalPipeline::new();
    let mut inserter = { text_inserter.lock().unwrap().take() };
    let pipeline_result = pipeline
        .process(&app, post_proc, &mut inserter, result, asr_time_ms, TranscriptionContext::empty())
        .await;
    // 归还 text_inserter
    *text_inserter.lock().unwrap() = inserter;

    // 处理管道结果
    match pipeline_result {
        Ok(result) => {
            // 先隐藏录音悬浮窗
            hide_overlay_window(&app).await;

            // 构建兼容的 TranscriptionResult
            let transcription_result = TranscriptionResult {
                text: result.text,
                original_text: result.original_text,
                asr_time_ms: result.asr_time_ms,
                llm_time_ms: result.llm_time_ms,
                total_time_ms: result.total_time_ms,
                mode: Some(format!("{:?}", result.mode).to_lowercase()),
                inserted: Some(result.inserted),
            };

            // 发送完成事件
            let _ = app.emit("transcription_complete", transcription_result);
        }
        Err(e) => {
            // 先隐藏录音悬浮窗
            hide_overlay_window(&app).await;

            // 发送错误事件
            tracing::error!("转录处理失败: {}", e);
            let _ = app.emit("error", format!("转录失败: {}", e));
        }
    }
}

/// 隐藏悬浮窗的辅助函数
async fn hide_overlay_window(app: &AppHandle) {
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
}

/// 旧版处理转录结果（保留用于向后兼容）
///
/// 此函数保留原有的处理逻辑，供不使用 Pipeline 架构的调用点使用
#[allow(dead_code)]
async fn handle_transcription_result_legacy(
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
                mode: None,
                inserted: None,
            };

            // 先隐藏录音悬浮窗
            hide_overlay_window(&app).await;

            // 后发送完成事件
            let _ = app.emit("transcription_complete", result);
        }
        Err(e) => {
            // 先隐藏录音悬浮窗
            hide_overlay_window(&app).await;

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

    {
        let is_running = state.is_running.lock().unwrap();
        if !*is_running {
            return Err("应用未在运行".to_string());
        }
    }

    // 停用热键服务（不终止线程）
    state.hotkey_service.deactivate();

    // 显式关闭活跃的 WebSocket Session
    {
        let mut session_guard = state.active_session.lock().await;
        if let Some(session) = session_guard.take() {
            let _ = session.close().await;
            tracing::info!("已关闭千问 WebSocket 会话");
        }
    }
    {
        let mut session_guard = state.doubao_session.lock().await;
        if let Some(mut session) = session_guard.take() {
            let _ = session.finish_audio().await;
            tracing::info!("已关闭豆包 WebSocket 会话");
        }
    }

    *state.audio_recorder.lock().unwrap() = None;
    *state.streaming_recorder.lock().unwrap() = None;
    *state.text_inserter.lock().unwrap() = None;
    *state.post_processor.lock().unwrap() = None;
    *state.assistant_processor.lock().unwrap() = None;
    *state.qwen_client.lock().unwrap() = None;
    *state.sensevoice_client.lock().unwrap() = None;
    *state.doubao_client.lock().unwrap() = None;
    *state.is_running.lock().unwrap() = false;

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
            state.hotkey_service.deactivate();
            *state.audio_recorder.lock().unwrap() = None;
            *state.streaming_recorder.lock().unwrap() = None;
            *state.text_inserter.lock().unwrap() = None;
            *state.post_processor.lock().unwrap() = None;
            *state.assistant_processor.lock().unwrap() = None;
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

/// 重置热键状态（用于手动修复状态卡死问题）
#[tauri::command]
async fn reset_hotkey_state(app_handle: AppHandle) -> Result<String, String> {
    let state = app_handle.state::<AppState>();
    state.hotkey_service.reset_state();
    Ok("热键状态已重置".to_string())
}

/// 获取热键调试信息
#[tauri::command]
async fn get_hotkey_debug_info(app_handle: AppHandle) -> Result<String, String> {
    let state = app_handle.state::<AppState>();
    Ok(state.hotkey_service.get_debug_info())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 初始化日志
    tracing_subscriber::fmt::init();

    // 检查是否静默启动（开机自启时）
    let args: Vec<String> = std::env::args().collect();
    let start_minimized = args.contains(&"--minimized".to_string());

    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
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
                assistant_processor: Arc::new(Mutex::new(None)),
                is_running: Arc::new(Mutex::new(false)),
                use_realtime_asr: Arc::new(Mutex::new(true)),
                enable_post_process: Arc::new(Mutex::new(false)),
                enable_fallback: Arc::new(Mutex::new(false)),
                qwen_client: Arc::new(Mutex::new(None)),
                sensevoice_client: Arc::new(Mutex::new(None)),
                doubao_client: Arc::new(Mutex::new(None)),
                active_session: Arc::new(tokio::sync::Mutex::new(None)),
                doubao_session: Arc::new(tokio::sync::Mutex::new(None)),
                realtime_provider: Arc::new(Mutex::new(None)),
                audio_sender_handle: Arc::new(Mutex::new(None)),
                hotkey_service: Arc::new(HotkeyService::new()),
                current_trigger_mode: Arc::new(Mutex::new(None)),
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
            reset_hotkey_state,
            get_hotkey_debug_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
