// src/OverlayWindow.tsx
// 录音状态悬浮窗组件 - iOS风格的精美设计

import { useState, useEffect, useRef } from "react";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

// 音频级别事件 payload 类型
interface AudioLevelPayload {
  level: number;
}

// 状态类型
type OverlayStatus = "recording" | "transcribing";

// 声波条组件 - 纯白色极简设计
function WaveBar({ height }: { height: number }) {
  return (
    <div
      className="wave-bar"
      style={{ height: `${height}px` }}
    />
  );
}

// 声波动画组件 - 仅显示声波条，无任何文字
function WaveformBars({ level }: { level: number }) {
  // 9个条，创造更密集的声波效果（类似截图）
  const barMultipliers = [0.4, 0.6, 0.8, 0.95, 1.0, 0.95, 0.8, 0.6, 0.4];

  // 最小高度 4px，最大高度 24px
  const minHeight = 4;
  const maxHeight = 24;

  // 放大音量让跳动更明显
  const amplifiedLevel = Math.min(level * 1.5, 1.0);

  return (
    <div className="wave-container">
      {barMultipliers.map((multiplier, i) => {
        const height = minHeight + (amplifiedLevel * multiplier * (maxHeight - minHeight));
        return <WaveBar key={i} height={height} />;
      })}
    </div>
  );
}

// 转写加载组件 - 点阵 + 旋转太阳图标（如截图所示）
function LoadingIndicator() {
  return (
    <div className="loading-container">
      {/* 左侧点阵 */}
      <div className="dots-container">
        {[...Array(9)].map((_, i) => (
          <div key={i} className="dot" style={{ animationDelay: `${i * 0.1}s` }} />
        ))}
      </div>
      {/* 右侧旋转图标 */}
      <div className="spinner-icon">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
          <circle cx="12" cy="12" r="4" />
          <line x1="12" y1="2" x2="12" y2="6" />
          <line x1="12" y1="18" x2="12" y2="22" />
          <line x1="2" y1="12" x2="6" y2="12" />
          <line x1="18" y1="12" x2="22" y2="12" />
          <line x1="4.93" y1="4.93" x2="7.76" y2="7.76" />
          <line x1="16.24" y1="16.24" x2="19.07" y2="19.07" />
          <line x1="4.93" y1="19.07" x2="7.76" y2="16.24" />
          <line x1="16.24" y1="7.76" x2="19.07" y2="4.93" />
        </svg>
      </div>
    </div>
  );
}

// 主悬浮窗组件
export default function OverlayWindow() {
  const [audioLevel, setAudioLevel] = useState(0);
  const [status, setStatus] = useState<OverlayStatus>("recording");
  // 使用 ref 来存储平滑值，避免闭包问题
  const smoothedLevelRef = useRef(0);
  // 标记监听器是否已设置
  const listenersSetup = useRef(false);

  useEffect(() => {
    // 防止重复设置监听器
    if (listenersSetup.current) return;
    listenersSetup.current = true;

    const unlistenFns: UnlistenFn[] = [];

    // 立即设置监听器（不使用 async wrapper）
    const setup = async () => {
      // 监听音频级别更新
      const unlistenAudioLevel = await listen<AudioLevelPayload>("audio_level_update", (event) => {
        const newLevel = event.payload.level;
        // 更激进的平滑处理：快速上升，较快下降，保持动感
        if (newLevel > smoothedLevelRef.current) {
          // 上升时快速响应
          smoothedLevelRef.current = smoothedLevelRef.current * 0.3 + newLevel * 0.7;
        } else {
          // 下降时也保持一定速度，避免粘滞感
          smoothedLevelRef.current = smoothedLevelRef.current * 0.6 + newLevel * 0.4;
        }
        setAudioLevel(smoothedLevelRef.current);
      });
      unlistenFns.push(unlistenAudioLevel);

      // 监听录音开始
      const unlistenStart = await listen("recording_started", () => {
        setStatus("recording");
        smoothedLevelRef.current = 0;
        setAudioLevel(0);
      });
      unlistenFns.push(unlistenStart);

      // 监听录音停止/转写开始
      const unlistenStop = await listen("recording_stopped", () => {
        setStatus("transcribing");
      });
      unlistenFns.push(unlistenStop);

      const unlistenTranscribing = await listen("transcribing", () => {
        setStatus("transcribing");
      });
      unlistenFns.push(unlistenTranscribing);

      // 监听转写完成
      const unlistenComplete = await listen("transcription_complete", () => {
        setStatus("recording");
        smoothedLevelRef.current = 0;
        setAudioLevel(0);
      });
      unlistenFns.push(unlistenComplete);

      // 监听错误
      const unlistenError = await listen("error", () => {
        setStatus("recording");
        smoothedLevelRef.current = 0;
        setAudioLevel(0);
      });
      unlistenFns.push(unlistenError);

      // 监听取消
      const unlistenCancel = await listen("transcription_cancelled", () => {
        setStatus("recording");
        smoothedLevelRef.current = 0;
        setAudioLevel(0);
      });
      unlistenFns.push(unlistenCancel);
    };

    setup();

    // 清理函数
    return () => {
      unlistenFns.forEach(fn => fn());
      listenersSetup.current = false;
    };
  }, []);

  // 超时保护机制：如果转写状态超过 15 秒，强制调用隐藏
  useEffect(() => {
    if (status === "transcribing") {
      const timeout = setTimeout(async () => {
        console.warn("转写超时 15 秒，强制调用隐藏悬浮窗");
        try {
          await invoke("hide_overlay");
          setStatus("recording");
          smoothedLevelRef.current = 0;
          setAudioLevel(0);
        } catch (e) {
          console.error("强制隐藏悬浮窗失败:", e);
        }
      }, 15000);
      return () => clearTimeout(timeout);
    }
  }, [status]);

  return (
    <div className="overlay-root">
      <div className="overlay-pill">
        {status === "recording" ? (
          <WaveformBars level={audioLevel} />
        ) : (
          <LoadingIndicator />
        )}
      </div>
    </div>
  );
}
