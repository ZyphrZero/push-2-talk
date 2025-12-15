# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

PushToTalk is a desktop application built with Tauri 2.0 that enables voice-to-text input via global keyboard shortcuts. The architecture follows a clear separation between:
- **React frontend** (TypeScript + Tailwind CSS) for UI
- **Rust backend** (Tauri) for system-level operations

The application flow: User presses Ctrl+Win → Records audio → Releases key → Transcribes via ASR API (Qwen/Doubao/SenseVoice) → Optional LLM post-processing → Auto-inserts text into active window.

### Key Features
- **Multi-ASR Support**: Alibaba Qwen (realtime/HTTP), Doubao (realtime/HTTP), SiliconFlow SenseVoice
- **LLM Post-Processing**: Optional text polishing, translation, or formatting via OpenAI-compatible APIs
- **Visual Feedback**: Overlay window shows recording status with real-time waveform
- **Transcription History**: Automatic history tracking with search and copy functionality
- **System Tray**: Minimize to tray on close, auto-start on boot support
- **Multi-Configuration**: Save and switch between different LLM prompt presets

## Development Commands

### Development
```bash
npm install                    # Install frontend dependencies
npm run tauri dev             # Run dev server (requires admin rights on Windows)
```

⚠️ **Critical**: Must run with administrator privileges on Windows for global keyboard hook (`rdev`) to function.

### Building
```bash
npm run tauri build           # Build production bundles (MSI + NSIS installers)
```

Output location: `src-tauri/target/release/bundle/`

### Testing API Integration
```bash
cd src-tauri
cargo run --bin test_api      # Standalone tool to test Qwen ASR API
```

See `测试工具使用说明.md` for detailed usage.

### Rust-only Development
```bash
cd src-tauri
cargo build                   # Build Rust backend only
cargo check                   # Fast compile check
```

## Architecture & Key Patterns

### Backend Modules (src-tauri/src/)

The Rust backend is organized into independent modules that communicate through the main lib.rs orchestrator:

1. **hotkey_service.rs** - Global keyboard listener using `rdev`
   - Monitors Ctrl+Win key combination
   - Thread-safe state management with `Arc<Mutex<bool>>`
   - Callback-based: `on_start()` and `on_stop()` closures passed to `start()`
   - **Platform requirement**: Windows admin rights mandatory

2. **audio_recorder.rs** - Real-time audio capture (non-streaming mode)
   - Uses `cpal` for cross-platform audio I/O
   - Handles F32/I16/U16 sample format conversion automatically
   - Audio stream lifecycle: Must keep stream alive in memory during recording
   - Outputs WAV files via `hound` to system temp directory

3. **streaming_recorder.rs** - Real-time streaming audio capture
   - For WebSocket-based realtime ASR (Qwen/Doubao)
   - Emits audio chunks via callback for low-latency transmission
   - Includes audio visualization data (RMS levels) for overlay window

4. **asr/** - Multi-provider ASR module (refactored architecture)
   - **asr/http/qwen.rs** - Qwen HTTP mode (multimodal-generation endpoint)
   - **asr/http/doubao.rs** - Doubao HTTP mode
   - **asr/http/sensevoice.rs** - SiliconFlow SenseVoice fallback
   - **asr/realtime/qwen.rs** - Qwen WebSocket realtime mode
   - **asr/realtime/doubao.rs** - Doubao WebSocket realtime mode
   - **asr/race_strategy.rs** - Parallel ASR request racing with automatic fallback
   - **Timeout & Retry**: 6s request timeout with automatic retry (max 2 retries)
   - Base64 encodes audio before upload in HTTP mode

5. **llm_post_processor.rs** - Optional LLM text refinement
   - Sends transcribed text to OpenAI-compatible API with custom system prompts
   - Supports multiple presets: text polishing, translation, email formatting, etc.
   - Users can define custom scenarios via UI

6. **text_inserter.rs** - Clipboard-based text injection
   - Strategy: Save clipboard → Copy text → Simulate Ctrl+V → Restore clipboard
   - Uses `arboard` (clipboard) + `enigo` (keyboard simulation)

7. **audio_utils.rs** - Audio processing utilities
   - VAD (Voice Activity Detection) for silence trimming
   - RMS calculation for waveform visualization
   - Audio format conversion helpers

8. **config.rs** - Persistent configuration
   - Stores all API keys and settings in `%APPDATA%\PushToTalk\config.json`
   - Supports multiple LLM prompt presets with custom names
   - Manages minimize-to-tray and auto-start preferences
   - Uses `dirs` crate for cross-platform app data directory

### Frontend Architecture (src/)

Multi-page React app with Tauri IPC communication:

- **Main Window (App.tsx)**: Configuration UI with tabbed interface
  - ASR provider selection and API key management
  - LLM post-processing settings with custom prompt presets
  - Transcription history display with search and copy
  - Minimize-to-tray and auto-start toggles

- **Overlay Window (OverlayWindow.tsx)**: Floating recording status indicator
  - Displays real-time recording state (listening/processing/success/error)
  - Shows live audio waveform visualization
  - Auto-hides when idle, appears during recording
  - Always-on-top, click-through window

- **State Management**: React hooks (useState, useEffect) for local state
- **Tauri Communication**:
  - `invoke()` for commands: `save_config`, `load_config`, `start_app`, `stop_app`, `get_history`, `get_auto_start_enabled`, `set_auto_start`, etc.
  - `listen()` for events: `recording_started`, `recording_stopped`, `transcribing`, `transcription_complete`, `error`, `audio_level`, `overlay_update`

### Critical Event Flow

```
User presses Ctrl+Win
  → hotkey_service detects via rdev callback
  → Calls on_start() closure
  → Emits "recording_started" event to frontend
  → Emits "overlay_update" with state: "listening"
  → streaming_recorder.start_recording() / audio_recorder.start_recording()
  → Periodic "audio_level" events for waveform visualization

User releases key
  → hotkey_service detects release
  → Calls on_stop() closure
  → Emits "recording_stopped" event
  → Emits "overlay_update" with state: "processing"
  → streaming_recorder.stop_recording() / audio_recorder.stop_recording()
  → Emits "transcribing" event

Realtime Mode (WebSocket):
  → Audio chunks sent during recording via WebSocket
  → Partial results received and accumulated
  → Final transcription on stream close

HTTP Mode:
  → Complete WAV file uploaded after recording stops
  → Single transcription result returned

Post-Processing:
  → (Optional) llm_post_processor.process() refines text
  → text_inserter.insert_text() injects result
  → Emits "transcription_complete" with final text
  → Emits "overlay_update" with state: "success"
  → Saves to history
  → Deletes temp audio file (if applicable)
```

### Tauri IPC Commands (lib.rs)

All backend functions exposed via `#[tauri::command]`:

- `save_config(config: Config)` - Persist full configuration to disk
- `load_config()` - Load saved configuration
- `start_app(window: Window, config: Config)` - Initialize all services and start hotkey listener
- `stop_app()` - Cleanup and stop services
- `get_history()` - Retrieve transcription history
- `clear_history()` - Delete all history records
- `get_auto_start_enabled()` - Check if auto-start is enabled
- `set_auto_start(enable: bool)` - Toggle auto-start on boot

The `AppState` struct manages shared mutable state across all services using `Arc<Mutex<>>`.

### System Tray Integration

- **Minimize to Tray**: Configurable option to minimize instead of close
- **Auto-Start**: Windows registry-based auto-start on boot (requires admin)
- **Tray Menu**: Show/Hide window, Quit application

## Important Implementation Details

### Audio Recording Lifecycle
The audio stream from `cpal` is NOT Send-safe. The current solution spawns a dedicated thread that owns the stream and polls `is_recording` flag. Alternative approaches (storing stream in struct) will fail compilation.

### Global Hotkey Detection
`rdev` requires system-level permissions. On Windows, this means:
- Must launch with administrator privileges
- Alternative: Use `tauri-plugin-global-shortcut` (not implemented in MVP)

### API Response Format
**Qwen ASR response structure:**
```json
{
  "output": {
    "choices": [{
      "message": {
        "content": [{"text": "transcribed text"}]
      }
    }]
  }
}
```

Parse via: `result["output"]["choices"][0]["message"]["content"][0]["text"]`

**Doubao ASR WebSocket message:**
- Event-driven: `"speech_start"`, `"partial_result"`, `"final_result"`, `"speech_end"`
- Partial results contain incremental text that gets accumulated
- Final result emitted on stream closure

**SenseVoice HTTP response:**
```json
{
  "data": {
    "text": "transcribed text"
  }
}
```

### Binary Configuration
The project has two binaries defined in Cargo.toml:
- `push-to-talk` (main app) - default-run
- `test_api` (standalone API tester)

Run specific binary: `cargo run --bin test_api`

## Common Issues & Solutions

### "Audio file is empty" error
- Cause: Audio stream dropped too early
- Current fix: Thread-based stream ownership in audio_recorder.rs

### "No keyboard events detected"
- Cause: Missing administrator privileges
- Solution: Right-click → Run as Administrator

### Compilation error with single quotes in char array
- Cause: Rust requires escaping single quotes in char literals
- Fix: Use `'\''` instead of `'''`

### "Transcription timeout" or API hangs
- Cause: API request taking too long or network issues
- Solution: Automatic 6s timeout with 2 retry attempts
- Implementation: Uses `reqwest::Client` with timeout configuration

### "HTTP connection pool exhausted"
- Cause: Default reqwest pool size too small for concurrent requests
- Solution: Configure custom HTTP client with increased pool limits (see llm_post_processor.rs)
- Implementation: `.pool_max_idle_per_host()` and `.pool_idle_timeout()`

### Overlay window not showing
- Cause: Window creation race condition or Tauri event timing
- Solution: Ensure overlay window is created in `tauri.conf.json` with `"visible": false` initially
- Trigger visibility via IPC events after main window ready

### Auto-start not working
- Cause: Windows registry requires admin rights to modify
- Solution: Must run installer with admin privileges
- Registry path: `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run`

## Configuration

Config file location: `%APPDATA%\PushToTalk\config.json`

### Configuration Structure
```json
{
  "dashscope_api_key": "sk-...",
  "doubao_app_id": "...",
  "doubao_access_token": "...",
  "siliconflow_api_key": "sk-...",
  "llm_enabled": true,
  "llm_api_key": "sk-...",
  "llm_base_url": "https://open.bigmodel.cn/api/paas/v4",
  "llm_model": "glm-4-flash",
  "llm_system_prompt": "...",
  "llm_presets": [
    {"name": "文本润色", "prompt": "..."},
    {"name": "中译英", "prompt": "..."}
  ],
  "minimize_to_tray": true,
  "selected_asr_provider": "qwen_realtime"
}
```

### API Key Sources
- **DashScope (Qwen)**: https://bailian.console.aliyun.com/?tab=model#/api-key
- **Doubao (ByteDance)**:
  - Recording file recognition: https://console.volcengine.com/ark/region:ark+cn-beijing/tts/recordingRecognition
  - Streaming recognition: https://console.volcengine.com/ark/region:ark+cn-beijing/tts/speechRecognition
- **SiliconFlow**: https://cloud.siliconflow.cn/me/account/ak
- **ZhipuAI (GLM-4-Flash)**: https://docs.bigmodel.cn/cn/guide/models/free/glm-4-flash-250414
