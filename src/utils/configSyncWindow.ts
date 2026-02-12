export type ConfigSyncWindowSource = "initial_load" | "external_config_updated";

export type ConfigSyncWindowSnapshot = {
  isSuppressed: boolean;
  source: ConfigSyncWindowSource | null;
  isExternalSyncing: boolean;
};

export function getSyncWindowNoticeMessage(
  source: ConfigSyncWindowSource | null,
): string | null {
  if (source === "initial_load") {
    return "正在加载初始配置";
  }

  if (source === "external_config_updated") {
    return "正在同步外部配置";
  }

  return null;
}

type QueueMicrotaskLike = (callback: () => void) => void;
type RequestAnimationFrameLike = (callback: () => void) => number;

export type ConfigSyncWindowController = {
  begin: (source: ConfigSyncWindowSource) => number;
  complete: (token: number) => boolean;
  isSuppressed: () => boolean;
  currentSource: () => ConfigSyncWindowSource | null;
  snapshot: () => ConfigSyncWindowSnapshot;
};

type ScheduleSyncWindowReleaseParams = {
  token: number;
  complete: (token: number) => void;
  queueMicrotaskFn?: QueueMicrotaskLike;
  requestAnimationFrameFn?: RequestAnimationFrameLike;
};

const fallbackQueueMicrotask: QueueMicrotaskLike = (callback) => {
  Promise.resolve().then(callback);
};

const fallbackRequestAnimationFrame: RequestAnimationFrameLike = (callback) => {
  window.setTimeout(callback, 0);
  return 0;
};

export function createConfigSyncWindowController(): ConfigSyncWindowController {
  let activeToken: number | null = null;
  let activeSource: ConfigSyncWindowSource | null = null;
  let tokenSeed = 0;

  const getSnapshot = (): ConfigSyncWindowSnapshot => {
    const isSuppressed = activeToken !== null;

    return {
      isSuppressed,
      source: activeSource,
      isExternalSyncing: isSuppressed && activeSource === "external_config_updated",
    };
  };

  return {
    begin(source) {
      tokenSeed += 1;
      activeToken = tokenSeed;
      activeSource = source;
      return activeToken;
    },
    complete(token) {
      if (activeToken !== token) {
        return false;
      }

      activeToken = null;
      activeSource = null;
      return true;
    },
    isSuppressed() {
      return activeToken !== null;
    },
    currentSource() {
      return activeSource;
    },
    snapshot() {
      return getSnapshot();
    },
  };
}

/**
 * 延迟释放同步窗口，避免外部 config_updated 导致的回写循环。
 *
 * 时序说明：
 * 1) microtask：等待当前调用栈内的 setState 批处理提交。
 * 2) 第一个 rAF：等待本帧 React 提交与副作用调度。
 * 3) 第二个 rAF：确保依赖同步窗口状态的 useEffect 已消费到最新 state。
 *
 * 若过早 complete(token)，自动保存 effect 可能在旧状态下触发并回写配置。
 */
export function scheduleSyncWindowRelease({
  token,
  complete,
  queueMicrotaskFn,
  requestAnimationFrameFn,
}: ScheduleSyncWindowReleaseParams): void {
  const runMicrotask = queueMicrotaskFn
    ?? (typeof queueMicrotask === "function" ? queueMicrotask : fallbackQueueMicrotask);
  const runAnimationFrame = requestAnimationFrameFn
    ?? (typeof requestAnimationFrame === "function"
      ? requestAnimationFrame
      : fallbackRequestAnimationFrame);

  runMicrotask(() => {
    runAnimationFrame(() => {
      runAnimationFrame(() => {
        complete(token);
      });
    });
  });
}
