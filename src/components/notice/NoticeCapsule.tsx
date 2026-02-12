import { Loader2, Check, Info, AlertTriangle, XCircle } from "lucide-react";
import type { GlobalNoticePayload } from "../../utils/globalNotice";

type NoticeTone = NonNullable<GlobalNoticePayload["tone"]>;

const TONE_STYLES: Record<
  NoticeTone,
  { 
    color: string;       // 图标和文字的主色调
    bgColor: string;     //极其微弱的背景色倾向
    borderColor: string; // 边框颜色
    Icon: typeof Check 
  }
> = {
  info: {
    color: "#57534e",       // stone-600 (暖深灰)
    bgColor: "#ffffff",     // 纯白
    borderColor: "#e7e5e4", // stone-200 (暖灰边框)
    Icon: Info,
  },
  success: {
    color: "#15803d",       // green-700 (自然绿)
    bgColor: "#f0fdf4",     // green-50 (极淡的绿背景，像信纸)
    borderColor: "#dcfce7", // green-100
    Icon: Check,
  },
  warning: {
    color: "#b45309",       // amber-700
    bgColor: "#fffbeb",     // amber-50
    borderColor: "#fef3c7", // amber-100
    Icon: AlertTriangle,
  },
  error: {
    color: "#b91c1c",       // red-700
    bgColor: "#fef2f2",     // red-50
    borderColor: "#fee2e2", // red-100
    Icon: XCircle,
  },
};

export type NoticeCapsuleProps = {
  payload: GlobalNoticePayload;
  morphing?: boolean;
};

export function NoticeCapsule({ payload, morphing }: NoticeCapsuleProps) {
  const tone = payload.tone ?? "info";
  const style = TONE_STYLES[tone];
  const Icon = style.Icon;

  return (
    <div
      className="flex items-center gap-2.5 font-sans select-none whitespace-nowrap overflow-hidden"
      style={{
        height: 36, // 高度再减小一点，更秀气，匹配你的“运行时长”标签
        paddingLeft: 12,
        paddingRight: 16,
        borderRadius: 8, // 【关键修改】不要全圆角，用 8px-10px 的微圆角，匹配你界面里的卡片和按钮风格
        
        backgroundColor: style.bgColor,
        border: `1px solid ${style.borderColor}`,
        
        // 【关键修改】暖色阴影：用棕色调的阴影，而不是黑色，这会让它看起来像纸张
        boxShadow: "0 2px 8px -2px rgba(87, 83, 78, 0.08), 0 1px 2px -1px rgba(87, 83, 78, 0.04)",
        
        transition: "all 300ms cubic-bezier(0.2, 0.8, 0.2, 1)",
      }}
    >
      {/* Icon - 去掉背景球，回归极简 */}
      <div className="flex items-center justify-center flex-shrink-0">
        {payload.loading ? (
          <Loader2
            size={15}
            className="animate-spin"
            style={{ color: "#78716c" }} // stone-500
          />
        ) : (
          <Icon
            size={15}
            strokeWidth={2.5}
            style={{
              color: style.color,
              // 加一点点微弱的滤镜，让图标看起来像印上去的
              filter: "contrast(1.1)", 
            }}
          />
        )}
      </div>

      {/* Message text */}
      <span
        className="text-[13px] font-medium tracking-wide truncate"
        style={{
          color: style.color, // 文字颜色跟随状态色（或者用 #44403c 暖深灰也可以）
          opacity: 0.9,
          transition: morphing ? "color 140ms ease" : "none",
        }}
      >
        {payload.message}
      </span>
    </div>
  );
}