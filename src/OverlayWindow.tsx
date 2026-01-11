// src/OverlayWindow.tsx
// 录音状态悬浮窗组件 - iOS风格的精美设计
// v5: 物理液体感声波 - 正态分布 + 扩散波 + 随机扰动 + 自然下落
// 优化：合并 RAF 循环，统一时间驱动

import { useState, useEffect, useRef } from "react";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

// 音频级别事件 payload 类型
interface AudioLevelPayload {
  level: number;
}

// 状态类型
type OverlayStatus = "recording" | "transcribing";

// 静音阈值常量（与后端 NOISE_FLOOR 对齐）
// 低于此值视为静音，防止浮点数抖动导致的微小波动
const SILENCE_THRESHOLD = 0.005;

// Hook 返回类型
interface AudioVisualizationState {
  level: number;  // 平滑后的音频级别
  time: number;   // 动画时间（用于波浪效果）
}

// ==========================================
// 核心：高性能音频可视化 Hook
// 单一 RAF 循环同时驱动音量平滑和波浪动画
// ==========================================
const useSmoothAudioLevel = (isRecording: boolean): AudioVisualizationState => {
  const [state, setState] = useState<AudioVisualizationState>({ level: 0, time: 0 });
  const targetRef = useRef(0);
  const currentRef = useRef(0);
  const timeRef = useRef(0);
  const animationRef = useRef<number>(0);

  // 监听后端数据
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    const setup = async () => {
      unlisten = await listen<AudioLevelPayload>("audio_level_update", (event) => {
        // 非线性映射：Math.pow(x, 0.45) 提升小音量表现
        targetRef.current = Math.min(Math.pow(event.payload.level, 0.45), 1.0);
      });
    };
    setup();
    return () => { if (unlisten) unlisten(); };
  }, []);

  // 单一 60FPS 渲染循环 - 同时驱动音量平滑和波浪动画
  useEffect(() => {
    // 不录音时：重置状态，不启动动画循环
    if (!isRecording) {
      cancelAnimationFrame(animationRef.current);
      currentRef.current = 0;
      targetRef.current = 0;
      timeRef.current = 0;
      setState({ level: 0, time: 0 });
      return;
    }

    // 录音时：启动动画循环
    const animate = () => {
      const target = targetRef.current;
      const current = currentRef.current;

      // 物理平滑 (Lerp)
      // Attack (上升): 极快 (~0.5)，捕捉说话瞬间
      // Release (下降): 较慢 (~0.06)，实现"慢慢掉下去"的物理质感
      const speed = target > current ? 0.5 : 0.06;
      currentRef.current += (target - current) * speed;

      // 阈值截断：低于静音阈值强制归零，防止浮点数抖动
      if (currentRef.current < SILENCE_THRESHOLD) {
        currentRef.current = 0;
      }

      // 时间递增（用于波浪动画）
      // 循环重置：防止长时间运行后浮点精度问题
      // Math.PI * 100 ≈ 314，足够长的周期避免视觉跳跃
      timeRef.current = (timeRef.current + 0.08) % (Math.PI * 100);

      setState({
        level: currentRef.current,
        time: timeRef.current
      });

      animationRef.current = requestAnimationFrame(animate);
    };

    animationRef.current = requestAnimationFrame(animate);
    return () => cancelAnimationFrame(animationRef.current);
  }, [isRecording]);

  return state;
};

// ==========================================
// 组件部分
// ==========================================

// 声波条组件
function WaveBar({ height }: { height: number }) {
  return (
    <div
      className="wave-bar"
      style={{ height: `${height}px` }}
    />
  );
}

// 声波动画组件 - 物理液体感
// 正态分布 + 扩散波 + 随机扰动 + 自然下落
// 优化：time 由父级 Hook 统一驱动，避免多个 RAF 循环
function WaveformBars({ level, time }: { level: number; time: number }) {
  const minHeight = 3;
  const maxHeight = 28;

  // 基础结构：正态分布（中间高，两边低）
  // 索引:     0     1     2     3     4     5     6     7     8
  // 距离:     4     3     2     1     0     1     2     3     4
  const baseScales = [0.3, 0.45, 0.65, 0.85, 1.0, 0.85, 0.65, 0.45, 0.3];

  return (
    <div className="wave-container">
      {baseScales.map((baseScale, i) => {
        // 静音检测：level 为 0 时直接返回最小高度，保证绝对静止
        // （Hook 已将低于 SILENCE_THRESHOLD 的值归零）
        if (level === 0) {
          return <WaveBar key={i} height={minHeight} />;
        }

        // 计算到中心的距离（用于扩散波）
        const distanceFromCenter = Math.abs(i - 4);

        // 1. 扩散波 (Flow)：从中间向两边传播
        // sin(time - distance * phase) 模拟波从中心向外扩散
        const flowWave = Math.sin(time * 1.2 - distanceFromCenter * 0.6) * 0.15;

        // 2. 随机扰动 (Turbulence)：打破对称感
        // 关键：扰动幅度必须乘以 level，这样下落时扰动也会减弱
        const turbulence = Math.cos(time * 2.5 + i * 1.7) * 0.12 * level;

        // 3. 组合所有因子
        // 基础高度 + 扩散波 + 随机扰动，都乘以 level
        const dynamicScale = baseScale + flowWave * level + turbulence;

        // 计算最终高度
        const height = minHeight + level * dynamicScale * (maxHeight - minHeight);

        return <WaveBar key={i} height={Math.max(minHeight, Math.min(maxHeight, height))} />;
      })}
    </div>
  );
}

// 转写加载组件
function LoadingIndicator() {
  return (
    <div className="loading-container">
      <div className="dots-container">
        {[...Array(9)].map((_, i) => (
          <div key={i} className="dot" style={{ animationDelay: `${i * 0.1}s` }} />
        ))}
      </div>
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

// 松手模式控制组件 - 同样的物理液体感
// 优化：time 由父级 Hook 统一驱动
function LockedControls({
  onFinish,
  onCancel,
  level,
  time,
  disabled
}: {
  onFinish: () => void;
  onCancel: () => void;
  level: number;
  time: number;
  disabled: boolean;
}) {
  const minHeight = 3;
  const maxHeight = 26;

  // 7条的正态分布（中间高，两边低）
  const baseScales = [0.35, 0.55, 0.8, 1.0, 0.8, 0.55, 0.35];

  return (
    <div className="locked-controls">
      <button
        onClick={onCancel}
        disabled={disabled}
        className={`locked-btn locked-btn-cancel ${disabled ? 'opacity-50' : ''}`}
        title="取消 (Esc)"
      >
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
          <line x1="18" y1="6" x2="6" y2="18" />
          <line x1="6" y1="6" x2="18" y2="18" />
        </svg>
      </button>

      <div className="locked-wave-mini">
        {baseScales.map((baseScale, i) => {
          // 静音检测：level 为 0 时直接返回最小高度
          // （Hook 已将低于 SILENCE_THRESHOLD 的值归零）
          if (level === 0) {
            return (
              <div key={i} className="wave-bar-mini" style={{ height: `${minHeight}px` }} />
            );
          }

          const distanceFromCenter = Math.abs(i - 3);  // 7条时中心是索引3
          const flowWave = Math.sin(time * 1.2 - distanceFromCenter * 0.6) * 0.15;
          const turbulence = Math.cos(time * 2.5 + i * 1.7) * 0.12 * level;
          const dynamicScale = baseScale + flowWave * level + turbulence;
          const height = minHeight + level * dynamicScale * (maxHeight - minHeight);

          return (
            <div
              key={i}
              className="wave-bar-mini"
              style={{ height: `${Math.max(minHeight, Math.min(maxHeight, height))}px` }}
            />
          );
        })}
      </div>

      <button
        onClick={onFinish}
        disabled={disabled}
        className={`locked-btn locked-btn-finish ${disabled ? 'opacity-50' : ''}`}
        title="发送 (Enter)"
      >
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
          <polyline points="20 6 9 17 4 12" />
        </svg>
      </button>
    </div>
  );
}

// 主悬浮窗组件
export default function OverlayWindow() {
  const [status, setStatus] = useState<OverlayStatus>("recording");
  const [isLocked, setIsLocked] = useState(false);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const listenersSetup = useRef(false);

  // 使用 Hook 获取平滑的音频级别和动画时间
  const { level: audioLevel, time: animationTime } = useSmoothAudioLevel(status === "recording");

  useEffect(() => {
    if (listenersSetup.current) return;
    listenersSetup.current = true;

    const unlistenFns: UnlistenFn[] = [];

    const setup = async () => {
      const unlistenStart = await listen("recording_started", () => {
        setStatus("recording");
        setIsLocked(false);
        setIsSubmitting(false);
      });
      unlistenFns.push(unlistenStart);

      const unlistenLocked = await listen("recording_locked", () => {
        console.log("进入松手模式");
        setIsLocked(true);
        setIsSubmitting(false);
      });
      unlistenFns.push(unlistenLocked);

      const unlistenStop = await listen("recording_stopped", () => {
        setStatus("transcribing");
      });
      unlistenFns.push(unlistenStop);

      const unlistenTranscribing = await listen("transcribing", () => {
        setStatus("transcribing");
      });
      unlistenFns.push(unlistenTranscribing);

      const unlistenComplete = await listen("transcription_complete", () => {
        setStatus("recording");
        setIsLocked(false);
        setIsSubmitting(false);
      });
      unlistenFns.push(unlistenComplete);

      const unlistenError = await listen("error", () => {
        setStatus("recording");
        setIsLocked(false);
        setIsSubmitting(false);
      });
      unlistenFns.push(unlistenError);

      const unlistenCancel = await listen("transcription_cancelled", () => {
        setStatus("recording");
        setIsLocked(false);
        setIsSubmitting(false);
      });
      unlistenFns.push(unlistenCancel);
    };

    setup();

    return () => {
      unlistenFns.forEach(fn => fn());
      listenersSetup.current = false;
    };
  }, []);

  // 超时保护机制
  useEffect(() => {
    if (status === "transcribing") {
      const timeout = setTimeout(async () => {
        console.warn("转写超时 15 秒，强制调用隐藏悬浮窗");
        try {
          await invoke("hide_overlay");
          setStatus("recording");
          setIsLocked(false);
          setIsSubmitting(false);
        } catch (e) {
          console.error("强制隐藏悬浮窗失败:", e);
        }
      }, 15000);
      return () => clearTimeout(timeout);
    }
  }, [status]);

  // 松手模式超时保护
  useEffect(() => {
    if (isLocked && !isSubmitting) {
      const timeout = setTimeout(async () => {
        console.warn("松手模式超时 60 秒，自动取消");
        setIsSubmitting(true);
        try {
          await invoke("cancel_locked_recording");
        } catch (e) {
          console.error("取消锁定录音失败:", e);
        }
      }, 60000);
      return () => clearTimeout(timeout);
    }
  }, [isLocked, isSubmitting]);

  const handleFinish = async () => {
    if (isSubmitting) return;
    setIsSubmitting(true);
    try {
      await invoke("finish_locked_recording");
    } catch (e) {
      console.error("完成录音失败:", e);
      setIsSubmitting(false);
    }
  };

  const handleCancel = async () => {
    if (isSubmitting) return;
    setIsSubmitting(true);
    try {
      await invoke("cancel_locked_recording");
    } catch (e) {
      console.error("取消录音失败:", e);
      setIsSubmitting(false);
    }
  };

  return (
    <div className="overlay-root">
      <div className={`overlay-pill ${isLocked ? 'overlay-pill-locked' : ''}`}>
        {status === "recording" ? (
          isLocked ? (
            <LockedControls
              onFinish={handleFinish}
              onCancel={handleCancel}
              level={audioLevel}
              time={animationTime}
              disabled={isSubmitting}
            />
          ) : (
            <WaveformBars level={audioLevel} time={animationTime} />
          )
        ) : (
          <LoadingIndicator />
        )}
      </div>
    </div>
  );
}
