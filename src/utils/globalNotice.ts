import type { ConfigSyncStatus } from "../contexts/ConfigSaveContext";
import type { UpdateStatus } from "../types";
import type { ConfigSyncWindowSource } from "./configSyncWindow";
import { getSyncWindowNoticeMessage } from "./configSyncWindow";

type NoticeTone = "info" | "success" | "warning" | "error";

export type GlobalNoticePayload = {
  message: string;
  loading: boolean;
  tone: NoticeTone;
};

export type ResolveGlobalNoticeParams = {
  syncWindowSource: ConfigSyncWindowSource | null;
  syncStatus: ConfigSyncStatus;
  updateStatus: UpdateStatus;
  updateDownloadProgress?: number;
};

/**
 * 统一全局提示条优先级：
 * 1) 同步窗口（初始化/外部配置）
 * 2) 配置保存状态（syncing/success/error）
 * 3) 更新状态（checking/downloading/ready/available）
 */
export function resolveGlobalNotice({
  syncWindowSource,
  syncStatus,
  updateStatus,
  updateDownloadProgress,
}: ResolveGlobalNoticeParams): GlobalNoticePayload | null {
  const syncWindowMessage = getSyncWindowNoticeMessage(syncWindowSource);
  if (syncWindowMessage) {
    return {
      message: syncWindowMessage,
      loading: true,
      tone: "info",
    };
  }

  if (syncStatus === "syncing") {
    return {
      message: "正在保存配置",
      loading: true,
      tone: "info",
    };
  }

  if (syncStatus === "success") {
    return {
      message: "配置已同步",
      loading: false,
      tone: "success",
    };
  }

  if (syncStatus === "error") {
    return {
      message: "配置保存失败，请稍后重试",
      loading: false,
      tone: "error",
    };
  }

  if (updateStatus === "checking") {
    return {
      message: "正在检查更新",
      loading: true,
      tone: "info",
    };
  }

  if (updateStatus === "downloading") {
    const normalizedProgress = Math.min(
      100,
      Math.max(0, Math.round(updateDownloadProgress ?? 0)),
    );

    return {
      message: `正在下载更新 ${normalizedProgress}%`,
      loading: true,
      tone: "info",
    };
  }

  if (updateStatus === "ready") {
    return {
      message: "更新已就绪，正在重启应用",
      loading: false,
      tone: "success",
    };
  }

  if (updateStatus === "available") {
    return {
      message: "发现新版本，可前往偏好设置更新",
      loading: false,
      tone: "warning",
    };
  }

  return null;
}
