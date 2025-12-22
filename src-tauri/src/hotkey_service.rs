// 全局快捷键监听模块 - 单例模式重构 + 双模式支持
use rdev::{listen, Event, EventType, Key};
use std::sync::{Arc, Mutex, RwLock};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use std::collections::HashSet;
use anyhow::Result;
use crate::config::{HotkeyConfig, HotkeyKey, TriggerMode, DualHotkeyConfig};

// 看门狗检查间隔（毫秒）
const WATCHDOG_INTERVAL_MS: u64 = 100;
// 按键释放后的稳定时间（毫秒）
const KEY_RELEASE_STABLE_MS: u64 = 200;

/// 热键状态
#[derive(Debug, Default)]
struct HotkeyState {
    is_recording: bool,
    pressed_keys: HashSet<HotkeyKey>,
    watchdog_running: bool,
    /// 当前触发的模式（如果正在录音）
    current_trigger_mode: Option<TriggerMode>,
}

/// 回调函数类型（接收触发模式参数）
type Callback = Arc<dyn Fn(TriggerMode) + Send + Sync>;

/// 单例热键服务（支持双模式）
pub struct HotkeyService {
    /// 服务是否激活（控制是否响应热键事件）
    is_active: Arc<AtomicBool>,
    /// 听写模式快捷键配置
    dictation_config: Arc<RwLock<HotkeyConfig>>,
    /// AI助手模式快捷键配置
    assistant_config: Arc<RwLock<HotkeyConfig>>,
    /// 内部状态
    state: Arc<Mutex<HotkeyState>>,
    /// 监听线程是否已启动
    listener_started: Arc<AtomicBool>,
    /// 回调函数（现在接收 TriggerMode 参数）
    on_start: Arc<RwLock<Option<Callback>>>,
    on_stop: Arc<RwLock<Option<Callback>>>,
}

impl HotkeyService {
    pub fn new() -> Self {
        Self {
            is_active: Arc::new(AtomicBool::new(false)),
            dictation_config: Arc::new(RwLock::new(HotkeyConfig::default())),
            assistant_config: Arc::new(RwLock::new(HotkeyConfig {
                keys: vec![HotkeyKey::AltLeft, HotkeyKey::Space],
                mode: crate::config::HotkeyMode::Press,
            })),
            state: Arc::new(Mutex::new(HotkeyState::default())),
            listener_started: Arc::new(AtomicBool::new(false)),
            on_start: Arc::new(RwLock::new(None)),
            on_stop: Arc::new(RwLock::new(None)),
        }
    }

    /// 将 rdev::Key 转换为 HotkeyKey
    fn rdev_to_hotkey_key(key: Key) -> Option<HotkeyKey> {
        match key {
            Key::ControlLeft => Some(HotkeyKey::ControlLeft),
            Key::ControlRight => Some(HotkeyKey::ControlRight),
            Key::ShiftLeft => Some(HotkeyKey::ShiftLeft),
            Key::ShiftRight => Some(HotkeyKey::ShiftRight),
            Key::Alt => Some(HotkeyKey::AltLeft),
            Key::AltGr => Some(HotkeyKey::AltRight),
            Key::MetaLeft => Some(HotkeyKey::MetaLeft),
            Key::MetaRight => Some(HotkeyKey::MetaRight),
            Key::Space => Some(HotkeyKey::Space),
            Key::Tab => Some(HotkeyKey::Tab),
            Key::CapsLock => Some(HotkeyKey::CapsLock),
            Key::Escape => Some(HotkeyKey::Escape),
            Key::F1 => Some(HotkeyKey::F1),
            Key::F2 => Some(HotkeyKey::F2),
            Key::F3 => Some(HotkeyKey::F3),
            Key::F4 => Some(HotkeyKey::F4),
            Key::F5 => Some(HotkeyKey::F5),
            Key::F6 => Some(HotkeyKey::F6),
            Key::F7 => Some(HotkeyKey::F7),
            Key::F8 => Some(HotkeyKey::F8),
            Key::F9 => Some(HotkeyKey::F9),
            Key::F10 => Some(HotkeyKey::F10),
            Key::F11 => Some(HotkeyKey::F11),
            Key::F12 => Some(HotkeyKey::F12),
            Key::KeyA => Some(HotkeyKey::KeyA),
            Key::KeyB => Some(HotkeyKey::KeyB),
            Key::KeyC => Some(HotkeyKey::KeyC),
            Key::KeyD => Some(HotkeyKey::KeyD),
            Key::KeyE => Some(HotkeyKey::KeyE),
            Key::KeyF => Some(HotkeyKey::KeyF),
            Key::KeyG => Some(HotkeyKey::KeyG),
            Key::KeyH => Some(HotkeyKey::KeyH),
            Key::KeyI => Some(HotkeyKey::KeyI),
            Key::KeyJ => Some(HotkeyKey::KeyJ),
            Key::KeyK => Some(HotkeyKey::KeyK),
            Key::KeyL => Some(HotkeyKey::KeyL),
            Key::KeyM => Some(HotkeyKey::KeyM),
            Key::KeyN => Some(HotkeyKey::KeyN),
            Key::KeyO => Some(HotkeyKey::KeyO),
            Key::KeyP => Some(HotkeyKey::KeyP),
            Key::KeyQ => Some(HotkeyKey::KeyQ),
            Key::KeyR => Some(HotkeyKey::KeyR),
            Key::KeyS => Some(HotkeyKey::KeyS),
            Key::KeyT => Some(HotkeyKey::KeyT),
            Key::KeyU => Some(HotkeyKey::KeyU),
            Key::KeyV => Some(HotkeyKey::KeyV),
            Key::KeyW => Some(HotkeyKey::KeyW),
            Key::KeyX => Some(HotkeyKey::KeyX),
            Key::KeyY => Some(HotkeyKey::KeyY),
            Key::KeyZ => Some(HotkeyKey::KeyZ),
            Key::Num0 => Some(HotkeyKey::Num0),
            Key::Num1 => Some(HotkeyKey::Num1),
            Key::Num2 => Some(HotkeyKey::Num2),
            Key::Num3 => Some(HotkeyKey::Num3),
            Key::Num4 => Some(HotkeyKey::Num4),
            Key::Num5 => Some(HotkeyKey::Num5),
            Key::Num6 => Some(HotkeyKey::Num6),
            Key::Num7 => Some(HotkeyKey::Num7),
            Key::Num8 => Some(HotkeyKey::Num8),
            Key::Num9 => Some(HotkeyKey::Num9),
            Key::UpArrow => Some(HotkeyKey::Up),
            Key::DownArrow => Some(HotkeyKey::Down),
            Key::LeftArrow => Some(HotkeyKey::Left),
            Key::RightArrow => Some(HotkeyKey::Right),
            Key::Return => Some(HotkeyKey::Return),
            Key::Backspace => Some(HotkeyKey::Backspace),
            Key::Delete => Some(HotkeyKey::Delete),
            Key::Insert => Some(HotkeyKey::Insert),
            Key::Home => Some(HotkeyKey::Home),
            Key::End => Some(HotkeyKey::End),
            Key::PageUp => Some(HotkeyKey::PageUp),
            Key::PageDown => Some(HotkeyKey::PageDown),
            _ => None,
        }
    }

    /// 初始化监听线程（只调用一次，带自动重启机制）
    pub fn init_listener(&self) -> Result<()> {
        // 防止重复启动
        if self.listener_started.swap(true, Ordering::SeqCst) {
            tracing::debug!("监听线程已启动，跳过重复初始化");
            return Ok(());
        }

        tracing::info!("初始化全局快捷键监听线程（双模式）");

        let is_active = Arc::clone(&self.is_active);
        let dictation_config = Arc::clone(&self.dictation_config);
        let assistant_config = Arc::clone(&self.assistant_config);
        let state = Arc::clone(&self.state);
        let on_start = Arc::clone(&self.on_start);
        let on_stop = Arc::clone(&self.on_stop);

        thread::spawn(move || {
            tracing::info!("快捷键监听线程已启动");

            // 外层循环：如果 rdev 监听器崩溃则自动重启
            loop {
                let mut first_key_logged = false;

                // 克隆变量供闭包使用
                let is_active_inner = Arc::clone(&is_active);
                let dictation_config_inner = Arc::clone(&dictation_config);
                let assistant_config_inner = Arc::clone(&assistant_config);
                let state_inner = Arc::clone(&state);
                let on_start_inner = Arc::clone(&on_start);
                let on_stop_inner = Arc::clone(&on_stop);

                let callback = move |event: Event| {
                    // 检查服务是否激活
                    if !is_active_inner.load(Ordering::Relaxed) {
                        return;
                    }

                    // 第一次检测到按键时记录
                    if !first_key_logged && matches!(event.event_type, EventType::KeyPress(_)) {
                        first_key_logged = true;
                        tracing::info!("✓ rdev 正常工作 - 已检测到键盘事件");
                    }

                    match event.event_type {
                        EventType::KeyPress(key) => {
                            if let Some(hotkey_key) = Self::rdev_to_hotkey_key(key) {
                                let dictation_cfg = dictation_config_inner.read().unwrap().clone();
                                let assistant_cfg = assistant_config_inner.read().unwrap().clone();
                                let mut s = state_inner.lock().unwrap();

                                s.pressed_keys.insert(hotkey_key);

                                // 调试日志：检测按键数量异常（可能有键卡死）
                                let max_keys = dictation_cfg.keys.len().max(assistant_cfg.keys.len());
                                if s.pressed_keys.len() > max_keys + 2 {
                                    // 仅在确实异常时输出，使用 debug 级别避免日志刷屏
                                    tracing::debug!(
                                        "当前按下按键数 ({}) 异常偏多，可能有按键状态卡死: {:?}",
                                        s.pressed_keys.len(),
                                        s.pressed_keys
                                    );
                                }

                                // 严格匹配：检查是否匹配两个配置中的任意一个
                                let (matches_dictation, matches_assistant) = {
                                    let contains_dictation = dictation_cfg.keys.iter().all(|k| s.pressed_keys.contains(k));
                                    let count_dictation = s.pressed_keys.len() == dictation_cfg.keys.len();

                                    let contains_assistant = assistant_cfg.keys.iter().all(|k| s.pressed_keys.contains(k));
                                    let count_assistant = s.pressed_keys.len() == assistant_cfg.keys.len();

                                    (contains_dictation && count_dictation, contains_assistant && count_assistant)
                                };

                                if !s.is_recording {
                                    // 确定触发模式（听写优先）
                                    let trigger_mode = if matches_dictation {
                                        Some(TriggerMode::Dictation)
                                    } else if matches_assistant {
                                        Some(TriggerMode::AiAssistant)
                                    } else {
                                        None
                                    };

                                    if let Some(mode) = trigger_mode {
                                        s.is_recording = true;
                                        s.current_trigger_mode = Some(mode);
                                        let mode_name = mode.display_name();
                                        tracing::info!("检测到快捷键按下: {}，开始录音", mode_name);

                                        // 启动看门狗
                                        if s.watchdog_running {
                                            drop(s);
                                            if let Some(cb) = on_start_inner.read().unwrap().as_ref() {
                                                cb(mode);
                                            }
                                            return;
                                        }

                                        s.watchdog_running = true;
                                        drop(s);

                                        // 启动看门狗线程
                                        let state_wd = Arc::clone(&state_inner);
                                        let dictation_cfg_wd = Arc::clone(&dictation_config_inner);
                                        let assistant_cfg_wd = Arc::clone(&assistant_config_inner);
                                        let is_active_wd = Arc::clone(&is_active_inner);
                                        let on_stop_wd = Arc::clone(&on_stop_inner);

                                        thread::spawn(move || {
                                            tracing::debug!("看门狗线程已启动");
                                            let mut release_detected_count: u64 = 0;
                                            let required_count = (KEY_RELEASE_STABLE_MS / WATCHDOG_INTERVAL_MS).max(1);

                                            loop {
                                                thread::sleep(Duration::from_millis(WATCHDOG_INTERVAL_MS));

                                                // 检查服务是否仍然激活
                                                if !is_active_wd.load(Ordering::Relaxed) {
                                                    let mut s = state_wd.lock().unwrap();
                                                    s.watchdog_running = false;
                                                    s.is_recording = false;
                                                    s.current_trigger_mode = None;
                                                    tracing::debug!("看门狗线程退出（服务已停止）");
                                                    break;
                                                }

                                                let s = state_wd.lock().unwrap();
                                                if !s.watchdog_running || !s.is_recording {
                                                    tracing::debug!("看门狗线程正常退出");
                                                    break;
                                                }

                                                // 根据当前触发模式检查对应的按键
                                                let all_pressed = match s.current_trigger_mode {
                                                    Some(TriggerMode::Dictation) => {
                                                        let cfg = dictation_cfg_wd.read().unwrap();
                                                        cfg.keys.iter().all(|k| s.pressed_keys.contains(k))
                                                    }
                                                    Some(TriggerMode::AiAssistant) => {
                                                        let cfg = assistant_cfg_wd.read().unwrap();
                                                        cfg.keys.iter().all(|k| s.pressed_keys.contains(k))
                                                    }
                                                    None => false,
                                                };
                                                drop(s);

                                                if !all_pressed {
                                                    release_detected_count += 1;
                                                    if release_detected_count >= required_count {
                                                        let mut s = state_wd.lock().unwrap();
                                                        if s.is_recording {
                                                            let mode = s.current_trigger_mode.unwrap_or(TriggerMode::Dictation);
                                                            s.is_recording = false;
                                                            s.watchdog_running = false;
                                                            s.current_trigger_mode = None;
                                                            drop(s);
                                                            tracing::warn!("看门狗检测到按键释放事件丢失，强制停止录音");
                                                            if let Some(cb) = on_stop_wd.read().unwrap().as_ref() {
                                                                cb(mode);
                                                            }
                                                        }
                                                        break;
                                                    }
                                                } else {
                                                    release_detected_count = 0;
                                                }
                                            }

                                            let mut s = state_wd.lock().unwrap();
                                            s.watchdog_running = false;
                                        });

                                        if let Some(cb) = on_start_inner.read().unwrap().as_ref() {
                                            cb(mode);
                                        }
                                    }
                                }
                            }
                        }
                        EventType::KeyRelease(key) => {
                            if let Some(hotkey_key) = Self::rdev_to_hotkey_key(key) {
                                let dictation_cfg = dictation_config_inner.read().unwrap().clone();
                                let assistant_cfg = assistant_config_inner.read().unwrap().clone();
                                let mut s = state_inner.lock().unwrap();

                                s.pressed_keys.remove(&hotkey_key);

                                // 增强的防呆逻辑：如果释放的是修饰键且未录音，检查是否所有修饰键都已释放
                                if hotkey_key.is_modifier() && !s.is_recording {
                                    let has_any_modifier = s.pressed_keys.iter().any(|k| k.is_modifier());
                                    if !has_any_modifier && !s.pressed_keys.is_empty() {
                                        // 所有修饰键已释放，但还有其他键残留，可能是状态卡死
                                        tracing::warn!(
                                            "所有修饰键已释放但仍有残留按键: {:?}，强制清理",
                                            s.pressed_keys
                                        );
                                        s.pressed_keys.clear();
                                    } else if !has_any_modifier {
                                        s.pressed_keys.clear();
                                        tracing::debug!("所有修饰键已释放，强制清理按键状态");
                                    }
                                }

                                if !s.is_recording {
                                    return;
                                }

                                // 根据当前触发模式检查对应的按键是否全部按下
                                let all_pressed = match s.current_trigger_mode {
                                    Some(TriggerMode::Dictation) => {
                                        dictation_cfg.keys.iter().all(|k| s.pressed_keys.contains(k))
                                    }
                                    Some(TriggerMode::AiAssistant) => {
                                        assistant_cfg.keys.iter().all(|k| s.pressed_keys.contains(k))
                                    }
                                    None => false,
                                };

                                if !all_pressed {
                                    let mode = s.current_trigger_mode.unwrap_or(TriggerMode::Dictation);
                                    s.is_recording = false;
                                    s.watchdog_running = false;
                                    s.current_trigger_mode = None;
                                    drop(s);

                                    tracing::info!("检测到快捷键释放，停止录音");
                                    if let Some(cb) = on_stop_inner.read().unwrap().as_ref() {
                                        cb(mode);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                };

                // 执行监听
                tracing::info!("开始执行 rdev listen...");
                if let Err(error) = listen(callback) {
                    tracing::error!("rdev 监听器发生错误退出: {:?}。将在 2 秒后重启监听。", error);
                } else {
                    tracing::warn!("rdev 监听器意外正常返回（通常不应发生）。将在 2 秒后重启监听。");
                }

                // 重启前重置状态，防止按键卡死
                {
                    let mut s = state.lock().unwrap();
                    s.pressed_keys.clear();
                    s.is_recording = false;
                    s.watchdog_running = false;
                    s.current_trigger_mode = None;
                }

                // 等待一会再重启，避免死循环占用 CPU
                thread::sleep(Duration::from_secs(2));
                tracing::info!("正在重启 rdev 监听器...");
            }
        });

        Ok(())
    }

    /// 激活双模式快捷键服务（新接口）
    ///
    /// # Arguments
    /// * `config` - 双快捷键配置（听写模式 + AI助手模式）
    /// * `on_start` - 开始录音回调（接收 TriggerMode 参数）
    /// * `on_stop` - 停止录音回调（接收 TriggerMode 参数）
    pub fn activate_dual<F1, F2>(
        &self,
        config: DualHotkeyConfig,
        on_start: F1,
        on_stop: F2,
    ) -> Result<()>
    where
        F1: Fn(TriggerMode) + Send + Sync + 'static,
        F2: Fn(TriggerMode) + Send + Sync + 'static,
    {
        // 验证配置
        config.validate()?;

        tracing::info!(
            "激活双模式快捷键服务 (听写: {}, AI助手: {})",
            config.dictation.format_display(),
            config.assistant.format_display()
        );

        // 更新配置
        *self.dictation_config.write().unwrap() = config.dictation;
        *self.assistant_config.write().unwrap() = config.assistant;

        // 更新回调
        *self.on_start.write().unwrap() = Some(Arc::new(on_start));
        *self.on_stop.write().unwrap() = Some(Arc::new(on_stop));

        // 重置状态
        {
            let mut s = self.state.lock().unwrap();
            s.is_recording = false;
            s.pressed_keys.clear();
            s.watchdog_running = false;
            s.current_trigger_mode = None;
        }

        // 确保监听线程已启动
        self.init_listener()?;

        // 激活服务
        self.is_active.store(true, Ordering::SeqCst);

        Ok(())
    }

    /// 更新配置并激活服务（旧接口，向后兼容）
    ///
    /// 注意：此方法已过时，仅用于向后兼容。新代码应使用 activate_dual()
    #[deprecated(note = "Use activate_dual() instead")]
    #[allow(dead_code)]
    pub fn activate<F1, F2>(&self, config: HotkeyConfig, on_start: F1, on_stop: F2) -> Result<()>
    where
        F1: Fn() + Send + Sync + 'static,
        F2: Fn() + Send + Sync + 'static,
    {
        let hotkey_display = config.format_display();
        tracing::info!("激活快捷键服务 ({})", hotkey_display);

        // 将单配置映射到双配置（听写模式）
        let dual_config = DualHotkeyConfig {
            dictation: config,
            assistant: HotkeyConfig {
                keys: vec![HotkeyKey::AltLeft, HotkeyKey::Space],
                mode: crate::config::HotkeyMode::Press,
            },
        };

        // 包装回调来忽略 TriggerMode 参数
        let on_start_wrapped = move |_mode: TriggerMode| {
            on_start();
        };
        let on_stop_wrapped = move |_mode: TriggerMode| {
            on_stop();
        };

        self.activate_dual(dual_config, on_start_wrapped, on_stop_wrapped)
    }

    /// 停用服务（不终止线程）
    pub fn deactivate(&self) {
        tracing::info!("停用快捷键服务");
        self.is_active.store(false, Ordering::SeqCst);

        // 重置状态
        let mut s = self.state.lock().unwrap();
        s.is_recording = false;
        s.pressed_keys.clear();
        s.watchdog_running = false;
        s.current_trigger_mode = None;
    }

    /// 检查服务是否激活
    pub fn is_active(&self) -> bool {
        self.is_active.load(Ordering::Relaxed)
    }

    /// 强制重置热键状态（用于手动修复状态卡死问题）
    pub fn reset_state(&self) {
        let mut s = self.state.lock().unwrap();
        tracing::info!(
            "强制重置热键状态。清理前按键: {:?}, is_recording: {}",
            s.pressed_keys,
            s.is_recording
        );
        s.pressed_keys.clear();
        s.is_recording = false;
        s.watchdog_running = false;
        s.current_trigger_mode = None;
    }

    /// 获取当前状态信息（用于调试）
    pub fn get_debug_info(&self) -> String {
        let s = self.state.lock().unwrap();
        let dictation_cfg = self.dictation_config.read().unwrap();
        let assistant_cfg = self.assistant_config.read().unwrap();
        format!(
            "is_active: {}, is_recording: {}, pressed_keys: {:?}, trigger_mode: {:?}, dictation_hotkey: {}, assistant_hotkey: {}",
            self.is_active.load(Ordering::Relaxed),
            s.is_recording,
            s.pressed_keys,
            s.current_trigger_mode,
            dictation_cfg.format_display(),
            assistant_cfg.format_display()
        )
    }
}
