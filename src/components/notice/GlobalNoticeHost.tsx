import type { GlobalNoticePayload } from "../../utils/globalNotice";
import { useNoticeMachine } from "../../hooks/useNoticeMachine";
import { NoticeCapsule } from "./NoticeCapsule";

export type GlobalNoticeHostProps = {
  notice: GlobalNoticePayload | null;
};

/**
 * Fixed-position host for the floating notice capsule.
 *
 * Renders at the top-center of the viewport, completely outside
 * the document flow. pointer-events: none ensures it never steals
 * clicks from underlying content.
 *
 * Mount this at the App root level, alongside (not inside) the main layout.
 */
export function GlobalNoticeHost({ notice }: GlobalNoticeHostProps) {
  const { phase, displayed } = useNoticeMachine(notice);

  if (phase === "hidden" || !displayed) return null;

  const isEntering = phase === "entering";
  const isExiting = phase === "exiting";
  const isStable = phase === "stable";

  return (
    <div
      role="status"
      aria-live="polite"
      style={{
        position: "fixed",
        top: 12,
        left: "50%",
        transform: "translateX(-50%)",
        zIndex: 1200,
        pointerEvents: "none",
        /* No layout participation whatsoever */
        width: 0,
        height: 0,
        overflow: "visible",
        display: "flex",
        justifyContent: "center",
      }}
    >
      <div
        style={{
          /* Enter/exit animation via opacity + translateY */
          opacity: isEntering || isStable ? 1 : 0,
          transform: isEntering || isStable
            ? "translateY(0)"
            : isExiting
              ? "translateY(-2px)"
              : "translateY(-6px)",
          transitionProperty: "opacity, transform",
          transitionDuration: isExiting ? "120ms" : "180ms",
          transitionTimingFunction: isExiting
            ? "ease-out"
            : "cubic-bezier(0.22, 1, 0.36, 1)",
        }}
      >
        <NoticeCapsule payload={displayed} morphing={isStable} />
      </div>
    </div>
  );
}
