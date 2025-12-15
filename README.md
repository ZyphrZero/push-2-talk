# PushToTalk - 语音输入助手

<div align="center">

**按住快捷键说话，松开自动转录并插入文本**

[![Tauri](https://img.shields.io/badge/Tauri-2.0-blue.svg)](https://tauri.app/)
[![React](https://img.shields.io/badge/React-18-blue.svg)](https://react.dev/)
[![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org/)

</div>

---

PushToTalk 是一个高性能的桌面语音输入工具。它不仅仅是一个语音转文字工具，更集成了大语言模型（LLM）能力。你可以按住 **Ctrl+Win** 说话，松开后应用会自动将你的语音转为文字，并根据你的设定进行**润色、翻译或整理成邮件**，最后自动粘贴到当前光标位置。

### ✨ 核心特性

- ⚡ **支持实时流式转录** - 采用 WebSocket 边录边传，极低延迟，松手即出字。
- 🧠 **LLM 智能后处理** - 内置 "文本润色"、"邮件整理"、"中译英" 等预设，支持自定义 Prompt。
- 🎤 **全局快捷键** - 在任何应用中（包括全屏游戏或 IDE）按住 `Ctrl+Win` 即可录音。
- 🔄 **多 ASR 引擎支持** - 支持阿里云 Qwen、豆包 Doubao、SiliconFlow SenseVoice，自动故障转移。
- 🎨 **可视化反馈** - 录音状态悬浮窗，实时波形显示，操作一目了然。
- 🔊 **音频反馈** - 录音开始/结束时的清脆提示音，盲操也放心。
- 📜 **历史记录** - 自动保存转录历史，支持搜索、复制、清空。
- 🚀 **系统托盘** - 支持最小化到托盘、开机自启动。
- 💾 **多配置管理** - 支持保存多套 LLM 预设，通过界面快速切换不同场景。

---

## 🎬 快速开始

### 安装

1. 下载最新版本的安装包

2. 运行安装程序完成安装

3. 右键点击应用图标，选择"以管理员身份运行"

### 配置

1. 启动应用，点击右上角设置图标。

2. **ASR 配置** (至少配置一个):
   - **阿里云 Qwen** (推荐)
     - 输入 DashScope API Key（超大量的免费额度，明年3月前基本上用不完）
     - [获取 API Key](https://bailian.console.aliyun.com/?tab=model#/api-key)

   - **豆包 Doubao** (可选)
     - 输入 App ID 和 Access Token
     - [录音文件识别大模型-极速版开通](https://console.volcengine.com/ark/region:ark+cn-beijing/tts/recordingRecognition)
     - [流式语音识别大模型-小时版开通](https://console.volcengine.com/ark/region:ark+cn-beijing/tts/speechRecognition)
     - 注意：App ID 和 Access Token 在网页下方

   - **硅基流动 SenseVoice** (可选，免费)
     - 输入 API Key
     - [获取 API Key](https://cloud.siliconflow.cn/me/account/ak)

3. **LLM 配置** (可选):
   - 开启 "LLM 智能润色"
   - 输入对应的 API Key (支持 OpenAI 兼容接口)
   - 推荐使用免费的智谱 GLM-4-Flash
   - [获取智谱 API Key](https://docs.bigmodel.cn/cn/guide/models/free/glm-4-flash-250414)
   - 可添加多个自定义润色预设（文本润色、中译英、邮件整理等）

4. **系统设置** (可选):
   - 开启 "关闭时最小化到托盘" - 关闭窗口时保持后台运行
   - 开启 "开机自启动" - 系统启动时自动运行（需要管理员权限）

5. 点击 "保存配置" 并 "启动助手"。

### 使用

1. 将光标定位在任何输入框（微信、Word、VS Code）。
2. 按住 **`Ctrl` + `Win`** 键，听到 "滴" 声后开始说话。
3. 说完松开按键，听到结束提示音，屏幕上会显示录音状态悬浮窗。
4. 等待片刻（悬浮窗显示处理状态），处理后的文本将自动打字上屏。
5. 在主界面的 "历史记录" 标签页可查看所有转录记录。

---

## 🛠️ 技术栈

### 前端
- **React 18** - UI 框架
- **TypeScript** - 类型安全
- **Tailwind CSS** - 样式框架
- **Vite** - 构建工具

### 后端 (Rust)
- **Tauri 2.0** - 跨平台桌面框架
- **rdev** - 全局键盘监听
- **cpal** - 实时音频录制
- **hound** - WAV 音频处理
- **tokio-tungstenite** - WebSocket 异步客户端
- **reqwest** - HTTP 客户端
- **arboard** - 剪贴板操作
- **enigo** - 输入模拟
- **rodio** - 音频播放（提示音）

### AI 服务
- **Alibaba Qwen ASR** - 阿里云语音识别（实时/HTTP）
- **Doubao ASR** - 豆包语音识别（实时/HTTP）
- **SiliconFlow SenseVoice** - 硅基流动语音识别（HTTP）
- **OpenAI-Compatible LLM** - 大语言模型后处理

---

## ⚙️ 高级配置

### ASR (语音识别)
应用支持多种 ASR 引擎，可在设置界面选择：

- **Qwen Realtime (推荐)**: WebSocket 实时流式，延迟最低（< 500ms）
- **Qwen HTTP**: 传统 HTTP 模式，稳定性更好
- **Doubao Realtime**: 豆包实时流式
- **Doubao HTTP**: 豆包 HTTP 模式
- **SenseVoice**: SiliconFlow 备用引擎

### LLM (文本润色)
你可以定义不同的预设来处理识别后的文本：
- **文本润色**: 去除口语词（嗯、啊），修正标点，使语句通顺。
- **中译英**: 直接将中文语音翻译成地道的英文输出。
- **邮件整理**: 将口语化的指令转换为正式的邮件格式。

可以在设置界面添加、删除或修改这些预设的 System Prompt。

### 系统托盘
- **最小化到托盘**: 关闭窗口时应用不会退出，而是隐藏到系统托盘
- **开机自启动**: Windows 注册表方式实现（需要管理员权限）
- **托盘菜单**: 右键托盘图标可显示/隐藏窗口或退出应用

---

## 🚀 开发指南

### 环境要求

- **Node.js** >= 18.0.0
- **Rust** >= 1.70.0
- **Windows** 10/11 (64-bit)

### 开发环境搭建

```bash
# 1. 克隆项目
git clone <repository-url>
cd push-2-talk

# 2. 安装前端依赖
npm install

# 3. 运行开发服务器（需要管理员权限）
npm run tauri dev
```

### 构建生产版本

```bash
npm run tauri build
```

生成的安装包位于：`src-tauri/target/release/bundle/`

### 测试 API

使用独立的测试工具验证 Qwen ASR API：

```bash
cd src-tauri
cargo run --bin test_api
```

详细说明请参考 [测试工具使用说明.md](./测试工具使用说明.md)

---

## 📁 项目结构

```
├── src                          # 前端源码
│   ├── App.tsx                  # 主窗口（配置界面、历史记录）
│   ├── OverlayWindow.tsx        # 悬浮窗（录音状态显示）
│   ├── index.css                # 全局样式
│   ├── main.tsx                 # 主窗口入口
│   └── overlay-main.tsx         # 悬浮窗入口
├── src-tauri                    # 后端源码
│   ├── capabilities             # Tauri 权限配置
│   │   └── default.json
│   ├── icons                    # 应用图标
│   │   └── icon.ico
│   ├── src
│   │   ├── asr                  # ASR 模块（重构后的架构）
│   │   │   ├── http             # HTTP 模式 ASR
│   │   │   │   ├── doubao.rs
│   │   │   │   ├── qwen.rs
│   │   │   │   └── sensevoice.rs
│   │   │   ├── realtime         # 实时流式 ASR
│   │   │   │   ├── doubao.rs
│   │   │   │   └── qwen.rs
│   │   │   ├── mod.rs
│   │   │   ├── race_strategy.rs # 并发请求竞速策略
│   │   │   └── utils.rs
│   │   ├── audio_recorder.rs    # 录音（非流式）
│   │   ├── streaming_recorder.rs # 录音（流式）
│   │   ├── audio_utils.rs       # 音频工具（VAD、RMS）
│   │   ├── beep_player.rs       # 提示音播放
│   │   ├── config.rs            # 配置管理
│   │   ├── hotkey_service.rs    # 全局快捷键
│   │   ├── lib.rs               # Tauri 主入口
│   │   ├── llm_post_processor.rs # LLM 后处理
│   │   ├── main.rs              # Rust 主函数
│   │   ├── test_api.rs          # API 测试工具
│   │   └── text_inserter.rs     # 文本插入
│   ├── build.rs                 # 构建脚本
│   ├── Cargo.toml               # Rust 依赖配置
│   └── tauri.conf.json          # Tauri 配置
├── CLAUDE.md                    # Claude Code 项目指南
├── LICENSE                      # MIT 许可证
├── README.md                    # 项目说明
├── package.json                 # 前端依赖配置
└── vite.config.ts               # Vite 构建配置
```

---

## ⚙️ 配置说明

### 配置文件位置
```
%APPDATA%\PushToTalk\config.json
```

### 配置文件格式示例
```json
{
  "dashscope_api_key": "sk-your-dashscope-key",
  "doubao_app_id": "your-app-id",
  "doubao_access_token": "your-access-token",
  "siliconflow_api_key": "sk-your-siliconflow-key",
  "selected_asr_provider": "qwen_realtime",
  "llm_enabled": true,
  "llm_api_key": "sk-your-llm-key",
  "llm_base_url": "https://open.bigmodel.cn/api/paas/v4",
  "llm_model": "glm-4-flash",
  "llm_system_prompt": "你是一个专业的文本润色助手...",
  "llm_presets": [
    {
      "name": "文本润色",
      "prompt": "去除口语化表达，修正语法和标点..."
    },
    {
      "name": "中译英",
      "prompt": "将中文翻译成地道的英文..."
    }
  ],
  "minimize_to_tray": true
}
```

### 获取 API Key

| 服务商 | 用途 | 获取地址 | 费用 |
|--------|------|----------|------|
| 阿里云 DashScope | Qwen ASR | [控制台](https://bailian.console.aliyun.com/?tab=model#/api-key) | 大量免费额度 |
| 豆包 (字节跳动) | Doubao ASR | [录音识别](https://console.volcengine.com/ark/region:ark+cn-beijing/tts/recordingRecognition) / [流式识别](https://console.volcengine.com/ark/region:ark+cn-beijing/tts/speechRecognition) | 按量计费 |
| 硅基流动 | SenseVoice ASR | [账户管理](https://cloud.siliconflow.cn/me/account/ak) | 免费 |
| 智谱 AI | GLM-4-Flash LLM | [模型文档](https://docs.bigmodel.cn/cn/guide/models/free/glm-4-flash-250414) | 免费 |

---

## 🎯 使用技巧

### 最佳实践

1. **录音环境** - 在安静环境下录音，清晰发音
2. **文本插入** - 确保目标窗口处于活动状态，光标可见
3. **快捷键使用** - 按住完整组合键（Ctrl+Win）再说话
4. **ASR 引擎选择** - 实时模式延迟低，HTTP 模式稳定性好
5. **LLM 预设** - 针对不同场景创建多个预设，快速切换

### 常见问题

**Q: 按快捷键没有反应？**
- A: 确保以管理员身份运行应用

**Q: 转录失败？**
- A: 检查网络连接和 API Key 是否有效。应用会自动重试最多2次

**Q: 转录一直处于"转录中"状态？**
- A: 应用有6秒超时机制，超时后会自动重试。如果持续失败，请检查网络和 API 服务状态

**Q: 文本未插入？**
- A: 确保目标应用窗口处于前台且光标可见

**Q: 悬浮窗不显示？**
- A: 检查是否被其他窗口遮挡，或尝试重启应用

**Q: 开机自启动设置失败？**
- A: 需要以管理员身份运行应用才能修改 Windows 注册表

**Q: 历史记录在哪里？**
- A: 在主界面切换到 "历史记录" 标签页即可查看，支持搜索和清空

---

## 📊 性能指标

| 指标 | 实时模式 (Realtime) | HTTP 模式 |
|------|-------------------|-----------|
| **首字延迟** | < 500ms | ~1.5s |
| **转录精度** | 98%+ (Qwen3/Doubao) | 98%+ (SenseVoice/Qwen) |
| **内存占用** | ~65MB | ~60MB |
| **网络消耗** | 持续小包传输 | 单次大包传输 |
| **超时重试** | 6s 超时，最多2次重试 | 6s 超时，最多2次重试 |

---

## 🔄 更新日志

### v0.1.0 (最新版本)

**新增功能：**
- 支持豆包 ASR（实时流式 + HTTP 模式）
- 录音状态悬浮窗，实时波形可视化
- 历史记录功能，支持搜索和清空
- 最小化到托盘，开机自启动
- LLM 自定义预设管理

**架构改进：**
- 重构 ASR 模块，统一接口设计
- HTTP 连接池优化，防止连接耗尽
- 代理配置优化

**Bug 修复：**
- 修复悬浮窗卡死问题
- 修复停止服务时的异常状态检测
---


## 🙏 致谢

感谢以下开源项目和服务：

- [Tauri](https://tauri.app/) - 强大的桌面应用框架
- [Alibaba Cloud](https://www.aliyun.com/) - 提供 Qwen ASR 服务
- [Rust Audio](https://github.com/RustAudio) - 音频处理库
- 所有贡献者和用户的支持

---

## 📄 许可证

MIT

---

<div align="center">

**⭐ 如果这个项目对你有帮助，请给它一个 Star！**

Made with ❤️ by PushToTalk Team

</div>
