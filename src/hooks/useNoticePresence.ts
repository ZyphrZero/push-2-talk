import { useState, useEffect, useRef, useCallback } from "react";
import type { GlobalNoticePayload } from "../utils/globalNotice";

/**
 * Notice display phase:
 * - hidden:  capsule not rendered / fully invisible
 * - visible: capsule shown (entering or stable)
 * - exiting: fade-out in progress, old content still displayed
 */
export type NoticePhase = "hidden" | "visible" | "exiting";

const ENTRY_DELAY_MS = 80; // prevent flash for ultra-fast operations
const EXIT_ANIM_MS = 150; // fade-out duration + small buffer

/**
 * Manages the display lifecycle of the floating notice capsule.
 *
 * - Entry delay (80 ms) prevents flicker for instant saves.
 * - syncing → success morphs content/color in place (no exit+re-enter).
 * - On disappear, keeps old content visible during exit animation.
 */
export function useNoticePresence(notice: GlobalNoticePayload | null) {
  const [displayed, setDisplayed] = useState<GlobalNoticePayload | null>(null);
  const [phase, setPhase] = useState<NoticePhase>("hidden");

  // Refs for timers
  const entryDelayTimer = useRef<ReturnType<typeof setTimeout>>();
  const exitTimer = useRef<ReturnType<typeof setTimeout>>();

  // Ref to always hold the latest notice value (for deferred reads)
  const noticeRef = useRef(notice);
  noticeRef.current = notice;

  // Shadow ref for phase to allow synchronous reads inside callbacks
  const phaseRef = useRef(phase);
  const updatePhase = useCallback((p: NoticePhase) => {
    phaseRef.current = p;
    setPhase(p);
  }, []);

  // Derive a stable identity string so the effect only fires on real changes
  const noticeKey = notice
    ? `${notice.tone}|${notice.loading ? 1 : 0}|${notice.message}`
    : "";

  useEffect(() => {
    const current = noticeRef.current;

    if (current) {
      // ---- Notice present ----

      // Cancel any pending exit
      if (exitTimer.current) {
        clearTimeout(exitTimer.current);
        exitTimer.current = undefined;
      }

      if (phaseRef.current === "hidden") {
        // First appearance: apply entry delay
        entryDelayTimer.current = setTimeout(() => {
          // Re-read latest notice in case it changed during delay
          const latest = noticeRef.current;
          if (latest) {
            setDisplayed(latest);
            updatePhase("visible");
          }
        }, ENTRY_DELAY_MS);
      } else {
        // Already visible or exiting → morph content in place
        if (phaseRef.current === "exiting") {
          // Cancel exit, stay visible
        }
        setDisplayed(current);
        updatePhase("visible");
      }
    } else {
      // ---- Notice cleared ----

      // Cancel any pending entry
      if (entryDelayTimer.current) {
        clearTimeout(entryDelayTimer.current);
        entryDelayTimer.current = undefined;
      }

      if (phaseRef.current === "visible") {
        // Start exit animation; keep displayed content for fade-out
        updatePhase("exiting");
        exitTimer.current = setTimeout(() => {
          updatePhase("hidden");
          setDisplayed(null);
        }, EXIT_ANIM_MS);
      }
      // If already hidden or exiting, do nothing
    }

    // Intentionally no cleanup — timers are managed explicitly above
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [noticeKey]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      clearTimeout(entryDelayTimer.current);
      clearTimeout(exitTimer.current);
    };
  }, []);

  return { phase, displayed } as const;
}
