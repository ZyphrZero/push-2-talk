// 全局快捷键监听模块
use rdev::{listen, Event, EventType, Key};
use std::sync::{Arc, Mutex};
use std::thread;
use anyhow::Result;

pub struct HotkeyService {
    is_recording: Arc<Mutex<bool>>,
    ctrl_pressed: Arc<Mutex<bool>>,
    win_pressed: Arc<Mutex<bool>>,
}

impl HotkeyService {
    pub fn new() -> Self {
        Self {
            is_recording: Arc::new(Mutex::new(false)),
            ctrl_pressed: Arc::new(Mutex::new(false)),
            win_pressed: Arc::new(Mutex::new(false)),
        }
    }

    pub fn start<F1, F2>(&self, on_start: F1, on_stop: F2) -> Result<()>
    where
        F1: Fn() + Send + 'static,
        F2: Fn() + Send + 'static,
    {
        tracing::info!("启动快捷键监听服务 (Ctrl+Win)");

        let is_recording = Arc::clone(&self.is_recording);
        let ctrl_pressed = Arc::clone(&self.ctrl_pressed);
        let win_pressed = Arc::clone(&self.win_pressed);

        thread::spawn(move || {
            tracing::info!("快捷键监听线程已启动");
            let mut first_key_logged = false;

            let callback = move |event: Event| {
                // 第一次检测到按键时记录（用于确认 rdev 工作正常）
                if !first_key_logged && matches!(event.event_type, EventType::KeyPress(_)) {
                    first_key_logged = true;
                    tracing::info!("✓ rdev 正常工作 - 已检测到键盘事件");
                }

                match event.event_type {
                    EventType::KeyPress(key) => {
                        match key {
                            Key::ControlLeft | Key::ControlRight => {
                                tracing::debug!("检测到 Ctrl 键按下");
                                *ctrl_pressed.lock().unwrap() = true;
                            }
                            Key::MetaLeft | Key::MetaRight => {
                                tracing::debug!("检测到 Win 键按下");
                                *win_pressed.lock().unwrap() = true;
                            }
                            _ => {}
                        }

                        // 检查是否按下了 Ctrl+Win
                        let ctrl = *ctrl_pressed.lock().unwrap();
                        let win = *win_pressed.lock().unwrap();
                        let mut recording = is_recording.lock().unwrap();

                        if ctrl && win && !*recording {
                            *recording = true;
                            tracing::info!("检测到快捷键按下: Ctrl+Win");
                            on_start();
                        }
                    }
                    EventType::KeyRelease(key) => {
                        match key {
                            Key::ControlLeft | Key::ControlRight => {
                                *ctrl_pressed.lock().unwrap() = false;
                            }
                            Key::MetaLeft | Key::MetaRight => {
                                *win_pressed.lock().unwrap() = false;
                            }
                            _ => {}
                        }

                        // 检查是否松开了快捷键
                        let ctrl = *ctrl_pressed.lock().unwrap();
                        let win = *win_pressed.lock().unwrap();
                        let mut recording = is_recording.lock().unwrap();

                        if *recording && (!ctrl || !win) {
                            *recording = false;
                            tracing::info!("检测到快捷键释放");
                            on_stop();
                        }
                    }
                    _ => {}
                }
            };

            if let Err(error) = listen(callback) {
                tracing::error!("无法监听键盘事件: {:?}", error);
            }
        });

        Ok(())
    }
}
