import { Download, RefreshCw, X, Sparkles } from "lucide-react";

export type UpdateInfo = { version: string; notes?: string };
export type UpdateStatus = "idle" | "checking" | "available" | "downloading" | "ready";

export type UpdateModalProps = {
  open: boolean;
  updateInfo: UpdateInfo | null;
  updateStatus: UpdateStatus;
  downloadProgress: number;
  onDismiss: () => void;
  onDownloadAndInstall: () => void;
};

export function UpdateModal({
  open,
  updateInfo,
  updateStatus,
  downloadProgress,
  onDismiss,
  onDownloadAndInstall,
}: UpdateModalProps) {
  if (!open || !updateInfo) return null;

  const isDownloading = updateStatus === "downloading";

  return (
    <div className="fixed inset-0 bg-black/40 backdrop-blur-sm flex items-center justify-center z-50 animate-in fade-in duration-200">
      <div className="bg-[var(--paper)] border border-[var(--stone)] rounded-3xl shadow-2xl w-full max-w-md mx-4 overflow-hidden animate-in zoom-in-95 duration-200 font-sans">
        {/* Header */}
        <div className="px-6 py-4 border-b border-[var(--stone)] bg-[rgba(120,140,93,0.08)]">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className="p-2 bg-[rgba(120,140,93,0.14)] rounded-xl text-[var(--sage)]">
                <Sparkles size={20} />
              </div>
              <div>
                <h3 className="text-lg font-bold text-[var(--ink)]">发现新版本</h3>
                <p className="text-xs text-[var(--stone-dark)]">v{updateInfo.version} 已发布</p>
              </div>
            </div>
            <button
              onClick={onDismiss}
              disabled={isDownloading}
              className="p-2 hover:bg-[var(--panel)] rounded-xl text-[var(--stone-dark)] hover:text-[var(--ink)] transition-colors disabled:opacity-50"
            >
              <X size={18} />
            </button>
          </div>
        </div>

        {/* Body */}
        <div className="p-6 space-y-4">
          {updateInfo.notes && (
            <div className="p-4 bg-[var(--panel)] border border-[var(--stone)] rounded-2xl">
              <h4 className="text-xs font-bold text-[var(--stone-dark)] uppercase tracking-widest mb-2">更新内容</h4>
              <div className="max-h-64 overflow-y-auto pr-1">
                <p className="text-sm text-[var(--ink)] whitespace-pre-wrap leading-relaxed">{updateInfo.notes}</p>
              </div>
            </div>
          )}

          {/* Progress Bar */}
          {isDownloading && (
            <div className="space-y-2">
              <div className="flex justify-between text-xs font-medium">
                <span className="text-[var(--stone-dark)]">正在下载更新...</span>
                <span className="text-[var(--sage)] tabular-nums">{downloadProgress}%</span>
              </div>
              <div className="w-full h-2 bg-[var(--panel)] border border-[var(--stone)] rounded-full overflow-hidden">
                <div
                  className="h-full bg-[var(--sage)] transition-all duration-300 ease-out"
                  style={{ width: `${downloadProgress}%` }}
                />
              </div>
            </div>
          )}

          {/* Actions */}
          <div className="flex gap-3 pt-2">
            <button
              onClick={onDismiss}
              disabled={isDownloading}
              className="flex-1 px-4 py-2.5 text-sm font-bold text-[var(--stone-dark)] bg-[var(--panel)] hover:bg-[rgba(232,230,220,0.85)] border border-[var(--stone)] rounded-2xl transition-colors disabled:opacity-50"
            >
              稍后更新
            </button>
            <button
              onClick={onDownloadAndInstall}
              disabled={isDownloading}
              className="flex-1 px-4 py-2.5 text-sm font-bold text-white bg-[var(--sage)] hover:opacity-90 rounded-2xl shadow-lg shadow-[rgba(120,140,93,0.25)] transition-all disabled:opacity-70 flex items-center justify-center gap-2"
            >
              {isDownloading ? (
                <>
                  <RefreshCw size={16} className="animate-spin" />
                  下载中...
                </>
              ) : (
                <>
                  <Download size={16} />
                  立即更新
                </>
              )}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

