import { Loader2 } from "lucide-react";

type GlobalNoticeTone = "info" | "success" | "warning" | "error";

export type GlobalNoticeBarProps = {
  message: string;
  loading?: boolean;
  tone?: GlobalNoticeTone;
  className?: string;
};

const TONE_STYLES: Record<GlobalNoticeTone, string> = {
  info: "border-[rgba(106,155,204,0.18)] bg-[rgba(106,155,204,0.1)] text-[var(--steel)]",
  success: "border-[rgba(120,140,93,0.2)] bg-[rgba(120,140,93,0.12)] text-[var(--sage)]",
  warning: "border-[rgba(217,119,87,0.2)] bg-[rgba(217,119,87,0.12)] text-[var(--crail)]",
  error: "border-[rgba(215,74,74,0.2)] bg-[rgba(215,74,74,0.1)] text-[rgb(176,40,40)]",
};

export function GlobalNoticeBar({
  message,
  loading = false,
  tone = "info",
  className,
}: GlobalNoticeBarProps) {
  return (
    <div
      role="status"
      aria-live="polite"
      className={[
        "px-6 py-2 border-b flex items-center gap-2 text-xs font-semibold",
        TONE_STYLES[tone],
        className,
      ]
        .filter(Boolean)
        .join(" ")}
    >
      {loading ? (
        <Loader2 className="w-3.5 h-3.5 animate-spin" />
      ) : (
        <span className="w-1.5 h-1.5 rounded-full bg-current opacity-75" />
      )}
      <span>{message}</span>
    </div>
  );
}
