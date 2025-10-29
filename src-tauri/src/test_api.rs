// API 测试工具 - 独立测试 Qwen ASR API
use anyhow::Result;
use base64::{Engine as _, engine::general_purpose};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    println!("=== Qwen ASR API 测试工具 ===\n");

    // 1. 获取 API Key
    let api_key = std::env::var("DASHSCOPE_API_KEY")
        .or_else(|_| {
            println!("请输入 DashScope API Key:");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            Ok::<String, std::io::Error>(input.trim().to_string())
        })?;

    if api_key.is_empty() {
        anyhow::bail!("API Key 不能为空");
    }

    println!("✓ API Key: {}...\n", &api_key[..10]);

    // 2. 获取音频文件路径
    println!("请输入音频文件路径 (WAV 格式):");
    let mut audio_path = String::new();
    std::io::stdin().read_line(&mut audio_path)?;
    let audio_path = audio_path.trim();

    let audio_file = PathBuf::from(audio_path);
    if !audio_file.exists() {
        anyhow::bail!("文件不存在: {}", audio_path);
    }

    println!("✓ 音频文件: {}\n", audio_path);

    // 3. 读取音频文件
    println!("正在读取音频文件...");
    let audio_data = tokio::fs::read(&audio_file).await?;
    println!("✓ 文件大小: {} bytes\n", audio_data.len());

    // 4. 转换为 base64
    println!("正在编码为 base64...");
    let audio_base64 = general_purpose::STANDARD.encode(&audio_data);
    println!("✓ Base64 长度: {} 字符\n", audio_base64.len());

    // 5. 构建请求
    println!("正在构建 API 请求...");
    let request_body = serde_json::json!({
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
                            "audio": format!("data:audio/wav;base64,{}", audio_base64)
                        }
                    ]
                }
            ]
        },
        "parameters": {
            "result_format": "message",
            "enable_itn": true
        }
    });

    println!("✓ 请求体已构建\n");

    // 6. 发送请求
    let url = "https://dashscope.aliyuncs.com/api/v1/services/aigc/multimodal-generation/generation";
    println!("正在发送请求到: {}", url);

    let client = reqwest::Client::new();
    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    let status = response.status();
    println!("✓ 响应状态: {}\n", status);

    // 7. 处理响应
    if !status.is_success() {
        let error_text = response.text().await?;
        println!("❌ API 错误:\n{}\n", error_text);
        anyhow::bail!("API 请求失败");
    }

    let result: serde_json::Value = response.json().await?;

    println!("=== 完整 API 响应 ===");
    println!("{}\n", serde_json::to_string_pretty(&result)?);

    // 8. 提取转录文本
    println!("=== 提取转录结果 ===");

    let text = result["output"]["choices"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|choice| choice["message"]["content"].as_array())
        .and_then(|content| content.first())
        .and_then(|item| item["text"].as_str())
        .ok_or_else(|| anyhow::anyhow!("无法解析转录结果"))?;

    println!("✅ 转录成功！\n");
    println!("📝 转录结果:");
    println!("─────────────────────────────────");
    println!("{}", text);
    println!("─────────────────────────────────\n");

    Ok(())
}
