# PushToTalk MVP 需求文档

## 1. MVP 目标

开发一个最小可行的桌面应用，实现**按住快捷键录音，松手自动转录并插入文本**的核心功能。

**验收标准**：
- ✅ 用户按住 Ctrl+Win 开始录音
- ✅ 用户松开按键，录音停止
- ✅ 音频自动上传到 Alibaba Qwen ASR API 转录
- ✅ 转录结果自动插入到活动窗口（光标位置）

**时间目标**：2周内完成

---

## 2. MVP 功能范围

### 2.1 包含的功能

#### ✅ 全局快捷键（Push-to-Talk 模式）
- **FR-MVP-001**: 监听全局快捷键 `Ctrl+Win`
- **FR-MVP-002**: 检测按键按下事件（开始录音）
- **FR-MVP-003**: 检测按键释放事件（停止录音）
- **实现方式**: 使用 `rdev` 库监听全局键盘事件

#### ✅ 音频录制
- **FR-MVP-004**: 使用系统默认麦克风录音
- **FR-MVP-005**: 音频参数固定：
  - 采样率: 16000 Hz
  - 声道: 单声道 (Mono)
  - 格式: WAV (PCM)
- **FR-MVP-006**: 录音保存为临时 WAV 文件
- **实现方式**: 使用 `cpal` 进行音频流捕获，`hound` 写入 WAV

#### ✅ 云端语音转文字
- **FR-MVP-007**: 集成 Alibaba Qwen ASR API (`qwen3-asr-flash` 模型)
- **FR-MVP-008**: 上传 WAV 文件并获取转录文本
- **FR-MVP-009**: 支持中英文混合识别
- **FR-MVP-010**: 基础错误处理（网络失败提示）
- **实现方式**: 使用 `reqwest` HTTP 客户端调用 DashScope API

#### ✅ 文本自动插入
- **FR-MVP-011**: 使用 Clipboard 方法插入文本
- **FR-MVP-012**: 自动模拟 Ctrl+V 粘贴到活动窗口
- **FR-MVP-013**: 插入前保存并恢复原剪贴板内容
- **实现方式**: 使用 `arboard` (剪贴板) + `enigo` (模拟按键)

#### ✅ 基础 GUI 界面
- **UI-MVP-001**: 主窗口显示应用状态
  - 灰色: "准备就绪"
  - 绿色: "运行中 - 按 Ctrl+Shift+Space 录音"
  - 红色: "录音中..."
  - 黄色: "转录中..."
- **UI-MVP-002**: 显示最新转录结果
- **UI-MVP-003**: OpenAI API 密钥输入框
- **UI-MVP-004**: "保存配置" 和 "启动/停止" 按钮
- **UI-MVP-005**: 错误信息显示区域

#### ✅ 配置管理
- **FR-MVP-014**: 保存 OpenAI API 密钥到配置文件
- **FR-MVP-015**: 应用启动时自动加载配置
- **配置文件位置**: `%APPDATA%\PushToTalk\config.json`

---

### 2.2 不包含的功能（后续版本实现）

#### ❌ 高级音频功能
- 静音检测和移除
- 音高保持的加速处理
- 音频设备选择
- 音频反馈（beep音）

#### ❌ 高级转录功能
- 流式输出（逐字符显示）
- AI 文本优化（GPT后处理）
- 自定义术语表
- Alibaba Qwen API

#### ❌ 高级界面功能
- 历史记录面板
- 高级设置面板
- 主题切换
- 快捷键自定义

#### ❌ 其他功能
- Toggle 录音模式
- SendKeys 插入方法
- 调试模式
- 开机自启动

---

## 3. MVP 技术架构

### 3.1 技术栈

**前端**：
- React 18 + TypeScript
- Vite
- Tailwind CSS（基础样式）
- Zustand（轻量状态管理）

**后端 (Rust)**：
- Tauri 2.x
- `rdev` v0.5 - 全局键盘监听
- `cpal` v0.15 - 音频录制
- `hound` v3.5 - WAV 文件写入
- `reqwest` v0.11 - HTTP 客户端（调用 DashScope API）
- `arboard` v3.3 - 剪贴板操作
- `enigo` v0.1 - 键盘模拟
- `tokio` v1.x - 异步运行时
- `serde` + `serde_json` - 配置序列化

### 3.2 简化的架构图

```
┌─────────────────────────────────┐
│      React Frontend (GUI)       │
│  - StatusIndicator              │
│  - TranscriptDisplay            │
│  - ConfigForm (API Key)         │
│  - StartButton                  │
└────────────┬────────────────────┘
             │ Tauri IPC
┌────────────▼────────────────────┐
│       Rust Backend              │
│  1. HotkeyService (rdev)        │
│     ↓                           │
│  2. AudioRecorder (cpal+hound)  │
│     ↓                           │
│  3. QwenASRClient (reqwest)     │
│     ↓                           │
│  4. TextInserter (arboard+enigo)│
└─────────────────────────────────┘
```

### 3.3 数据流（MVP）

```
用户按下 Ctrl+Win
         ↓
rdev 检测到按键按下
         ↓
发送事件到前端: "recording_started"
前端状态: 红色 "录音中..."
         ↓
cpal 开始音频流录制
         ↓
用户松开按键
         ↓
rdev 检测到按键释放
         ↓
停止录制，保存 WAV 文件
         ↓
发送事件到前端: "transcribing"
前端状态: 黄色 "转录中..."
         ↓
上传到 Alibaba Qwen ASR API (DashScope)
         ↓
接收转录文本
         ↓
保存当前剪贴板
复制文本到剪贴板
模拟 Ctrl+V
恢复原剪贴板
         ↓
发送事件到前端: "transcription_complete"
显示转录结果
前端状态: 绿色 "运行中"
         ↓
删除临时音频文件
```

---

## 4. MVP 模块设计

### 4.1 Rust 后端模块

#### 4.1.1 HotkeyService
```rust
pub struct HotkeyService {
    is_recording: Arc<Mutex<bool>>,
}

impl HotkeyService {
    pub fn start(&mut self, on_start: impl Fn(), on_stop: impl Fn());
    pub fn stop(&mut self);
}
```

#### 4.1.2 AudioRecorder
```rust
pub struct AudioRecorder {
    sample_rate: u32,
    channels: u16,
}

impl AudioRecorder {
    pub fn start_recording(&mut self) -> Result<()>;
    pub fn stop_recording(&mut self) -> Result<PathBuf>;
}
```

#### 4.1.3 QwenASRClient
```rust
pub struct QwenASRClient {
    api_key: String,        // DashScope API Key
    model: String,          // "qwen3-asr-flash"
}

impl QwenASRClient {
    pub async fn transcribe(&self, audio_path: &Path) -> Result<String>;
}
```

#### 4.1.4 TextInserter
```rust
pub struct TextInserter;

impl TextInserter {
    pub async fn insert_via_clipboard(text: &str) -> Result<()>;
}
```

#### 4.1.5 Tauri Commands
```rust
#[tauri::command]
async fn start_app(api_key: String) -> Result<(), String>;

#[tauri::command]
async fn stop_app() -> Result<(), String>;

#[tauri::command]
async fn save_config(api_key: String) -> Result<(), String>;

#[tauri::command]
async fn load_config() -> Result<String, String>;
```

### 4.2 前端状态管理

```typescript
interface AppState {
  // 状态
  status: 'idle' | 'running' | 'recording' | 'transcribing';

  // 数据
  apiKey: string;
  latestTranscript: string;
  errorMessage: string | null;

  // 操作
  setApiKey: (key: string) => void;
  startApp: () => Promise<void>;
  stopApp: () => Promise<void>;
  saveConfig: () => Promise<void>;
}
```

---

## 5. MVP 用户界面设计

### 5.1 主界面布局

```
┌─────────────────────────────────────┐
│  PushToTalk - MVP                   │
├─────────────────────────────────────┤
│                                     │
│  状态: ⬤ 运行中                     │
│        按 Ctrl+Win 录音              │
│                                     │
│  ─────────────────────────────────  │
│                                     │
│  最新转录结果:                      │
│  ┌───────────────────────────────┐ │
│  │                               │ │
│  │  [转录文本显示在这里]          │ │
│  │                               │ │
│  └───────────────────────────────┘ │
│                                     │
│  ─────────────────────────────────  │
│                                     │
│  配置:                              │
│  DashScope API Key:                 │
│  [________________________] [显示]  │
│                                     │
│  [保存配置]                         │
│                                     │
│  ─────────────────────────────────  │
│                                     │
│        [    启动应用    ]           │
│                                     │
└─────────────────────────────────────┘
```

### 5.2 状态指示器样式

- **⬤ 灰色** - "准备就绪"（未启动）
- **⬤ 绿色** - "运行中 - 按 Ctrl+Win 录音"
- **⬤ 红色** - "录音中..."
- **⬤ 黄色** - "转录中..."

---

## 6. MVP 配置文件

### 6.1 config.json 结构

```json
{
  "dashscope_api_key": "sk-..."
}
```

存储位置：`%APPDATA%\PushToTalk\config.json`

---

## 7. MVP 开发计划（2周）

### Week 1: 后端核心功能

#### Day 1-2: 项目初始化
- [x] 初始化 Tauri 项目
- [ ] 配置 Rust 依赖
- [ ] 搭建基础前端框架
- [ ] 测试 Tauri IPC 通信

#### Day 3-4: 音频录制
- [ ] 实现 AudioRecorder 模块
- [ ] 使用 cpal 录制音频流
- [ ] 使用 hound 保存 WAV 文件
- [ ] 测试音频录制质量

#### Day 5-7: 快捷键监听
- [ ] 实现 HotkeyService 模块
- [ ] 使用 rdev 监听 Ctrl+Win
- [ ] 处理按键按下和释放事件
- [ ] 集成音频录制触发
- [ ] 测试快捷键响应延迟

### Week 2: API集成和前端

#### Day 8-9: Qwen ASR API 集成
- [ ] 实现 QwenASRClient 模块
- [ ] 上传音频文件到 DashScope API
- [ ] 解析 API 响应（qwen3-asr-flash）
- [ ] 错误处理和重试

#### Day 10-11: 文本插入
- [ ] 实现 TextInserter 模块
- [ ] 剪贴板操作（保存/恢复）
- [ ] 模拟 Ctrl+V 粘贴
- [ ] 测试在不同应用中插入

#### Day 12-13: 前端开发
- [ ] 实现状态管理（Zustand）
- [ ] 创建主界面组件
- [ ] 状态指示器和转录显示
- [ ] API Key 配置表单
- [ ] 启动/停止按钮

#### Day 14: 集成测试和调试
- [ ] 端到端测试完整流程
- [ ] 修复发现的bug
- [ ] 性能优化
- [ ] 打包测试

---

## 8. MVP 验收标准

### 8.1 功能测试

| 测试项 | 测试步骤 | 预期结果 |
|--------|----------|----------|
| 应用启动 | 双击运行 | 窗口正常打开，显示"准备就绪" |
| 配置API | 输入 DashScope API密钥并保存 | 提示保存成功 |
| 启动服务 | 点击"启动应用" | 状态变为"运行中" |
| 录音开始 | 按住 Ctrl+Win | 状态变为"录音中..." |
| 录音停止 | 松开按键 | 状态变为"转录中..." |
| 转录完成 | 等待API返回 | 文本自动插入到活动窗口 |
| 界面显示 | 查看界面 | 转录结果显示在界面上 |
| 错误处理 | 断网后录音 | 显示错误提示 |

### 8.2 性能指标

| 指标 | 目标值 | 测量方法 |
|------|--------|----------|
| 按键响应延迟 | < 100ms | 观察录音开始的速度 |
| API 响应时间 | 取决于网络 | 记录从上传到返回的时间 |
| 内存占用 | < 80MB | 任务管理器查看 |
| 应用启动时间 | < 3秒 | 计时 |

---

## 9. MVP 后续迭代计划

### v0.2.0 - 基础增强（+1周）
- [ ] 添加 Toggle 录音模式
- [ ] 支持自定义快捷键
- [ ] 添加音频反馈（beep音）
- [ ] 历史记录功能

### v0.3.0 - 音频优化（+1周）
- [ ] 音频处理（静音移除）
- [ ] 音频设备选择
- [ ] 音频质量设置

### v0.4.0 - 转录增强（+1周）
- [ ] 添加 OpenAI Whisper API 支持（备选）
- [ ] AI 文本优化
- [ ] 流式输出
- [ ] 自定义术语表

### v1.0.0 - 完整版本（+1周）
- [ ] 完善所有功能
- [ ] 打包优化
- [ ] 用户文档
- [ ] 正式发布

---

## 10. 风险和应对

### 10.1 技术风险

| 风险 | 影响 | 应对措施 |
|------|------|----------|
| rdev 无法捕获按键释放 | 高 | 测试并准备备用方案（tauri-plugin-global-shortcut + 定时器） |
| DashScope API 限流 | 中 | 添加重试机制和错误提示 |
| 音频质量问题 | 中 | 固定使用16kHz，确保兼容性 |
| 文本插入失败 | 中 | 添加错误提示，建议用户手动复制 |

### 10.2 开发风险

| 风险 | 影响 | 应对措施 |
|------|------|----------|
| 开发时间不足 | 高 | 严格按照MVP范围，推迟非核心功能 |
| 环境配置问题 | 中 | 提前准备详细的环境搭建文档 |
| 跨平台兼容性 | 低 | MVP 仅专注 Windows，后续扩展 |

---

## 11. 成功标准

MVP 被认为成功需要满足：

1. ✅ 用户可以通过快捷键录音并获得转录文本
2. ✅ 转录准确率达到 80%+（依赖 Whisper API）
3. ✅ 按键响应延迟 < 100ms
4. ✅ 应用稳定运行，无崩溃
5. ✅ 配置可以持久化保存
6. ✅ 有基本的错误提示和处理

---

**文档版本**: MVP v1.0
**创建日期**: 2025-10-29
**预计完成**: 2025-11-12 (2周)
**目标**: 验证核心价值 - "按键说话，自动转录"
