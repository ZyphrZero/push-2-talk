# qwen3-asr-flash API 测试

如果你想用 curl 测试 API，可以这样做：

```bash
# 1. 准备音频文件的 base64 编码
base64 -w 0 your_audio.wav > audio_base64.txt

# 2. 调用 API
curl -X POST https://dashscope.aliyuncs.com/api/v1/services/aigc/multimodal-generation/generation \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "qwen3-asr-flash",
    "input": {
      "messages": [
        {
          "role": "system",
          "content": [{"text": ""}]
        },
        {
          "role": "user",
          "content": [
            {
              "audio": "data:audio/wav;base64,YOUR_BASE64_AUDIO_HERE"
            }
          ]
        }
      ]
    },
    "parameters": {
      "result_format": "message",
      "enable_itn": true
    }
  }'
```

## 响应格式

成功响应示例：
```json
{
  "output": {
    "choices": [
      {
        "message": {
          "content": [
            {
              "text": "你好，这是转录的文本"
            }
          ]
        }
      }
    ]
  },
  "usage": {...},
  "request_id": "..."
}
```

## 对比 Python SDK 和 HTTP API

| 项目 | Python SDK | HTTP REST API (Rust) |
|------|-----------|---------------------|
| 端点 | 自动处理 | `/api/v1/services/aigc/multimodal-generation/generation` |
| 音频传递 | `file://path` | `data:audio/wav;base64,{BASE64}` |
| 认证 | 环境变量/参数 | `Authorization: Bearer {API_KEY}` |
| 响应解析 | SDK 自动 | 手动解析 JSON |
