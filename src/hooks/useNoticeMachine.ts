import { useState, useEffect, useRef, useCallback } from "react";
import type { GlobalNoticePayload } from "../utils/globalNotice";

/**
 * Notice capsule display phase:
 *
 *   Hidden ──(notice arrives)──▸ Entering ──(180ms)──▸ Stable
 *     ▴                                                  │
 *     │                        ┌─(morph content)─────────┤
 *     │                        ▼                         │
 *   Hidden ◂──(120ms)── Exiting ◂──(notice clears)──────┘
 *                          │
 *                          └──(new notice)──▸ Entering
 */
export type NoticePhase = "hidden" | "entering" | "stable" | "exiting";

/* ── Timing constants ── */
const ENTRY_DELAY_MS = 80; // skip ultra-fast operations
const ENTER_ANIM_MS = 180; // enter animation duration
const EXIT_ANIM_MS = 120; // exit fade-out duration
const MIN_VISIBLE_MS = 600; // minimum time capsule stays visible
const SUCCESS_HOLD_MS = 1200; // how long success stays after source clears
const ERROR_HOLD_MS = 2200; // how long error stays after source clears

type NoticeTone = NonNullable<GlobalNoticePayload["tone"]>;

const HOLD_BY_TONE: Record<NoticeTone, number> = {
  info: 0,
  success: SUCCESS_HOLD_MS,
  warning: SUCCESS_HOLD_MS,
  error: ERROR_HOLD_MS,
};

/**
 * Manages the display lifecycle of the floating notice capsule.
 *
 * Guarantees:
 * - 80ms entry delay prevents flicker for instant saves
 * - syncing→success morphs content in place (no exit + re-enter)
 * - exit respects minimum visible time + tone-specific hold
 * - new notice during exit cancels exit and re-enters
 */
export function useNoticeMachine(notice: GlobalNoticePayload | null) {
  const [phase, setPhase] = useState<NoticePhase>("hidden");
  const [displayed, setDisplayed] = useState<GlobalNoticePayload | null>(null);

  // Refs for timers
  const entryDelayRef = useRef<ReturnType<typeof setTimeout>>();
  const enterAnimRef = useRef<ReturnType<typeof setTimeout>>();
  const holdRef = useRef<ReturnType<typeof setTimeout>>();
  const exitAnimRef = useRef<ReturnType<typeof setTimeout>>();

  // Ref tracking when we became stable (for minVisible calculation)
  const stableAtRef = useRef(0);

  // Shadow refs for synchronous reads inside callbacks
  const phaseRef = useRef(phase);
  const displayedRef = useRef(displayed);
  const noticeRef = useRef(notice);
  noticeRef.current = notice;

  const updatePhase = useCallback((p: NoticePhase) => {
    phaseRef.current = p;
    setPhase(p);
  }, []);

  const updateDisplayed = useCallback((d: GlobalNoticePayload | null) => {
    displayedRef.current = d;
    setDisplayed(d);
  }, []);

  const clearAllTimers = useCallback(() => {
    clearTimeout(entryDelayRef.current);
    clearTimeout(enterAnimRef.current);
    clearTimeout(holdRef.current);
    clearTimeout(exitAnimRef.current);
    entryDelayRef.current = undefined;
    enterAnimRef.current = undefined;
    holdRef.current = undefined;
    exitAnimRef.current = undefined;
  }, []);

  // Derive stable identity for the notice
  const noticeKey = notice
    ? `${notice.tone}|${notice.loading ? 1 : 0}|${notice.message}`
    : "";

  useEffect(() => {
    const current = noticeRef.current;
    const currentPhase = phaseRef.current;

    if (current) {
      /* ── Notice present ── */

      // Cancel any pending exit or hold
      clearTimeout(holdRef.current);
      holdRef.current = undefined;
      clearTimeout(exitAnimRef.current);
      exitAnimRef.current = undefined;

      if (currentPhase === "hidden") {
        // First appearance → apply entry delay
        clearTimeout(entryDelayRef.current);
        entryDelayRef.current = setTimeout(() => {
          const latest = noticeRef.current;
          if (!latest) return; // cleared during delay
          updateDisplayed(latest);
          updatePhase("entering");
          // After enter animation completes → stable
          enterAnimRef.current = setTimeout(() => {
            stableAtRef.current = Date.now();
            updatePhase("stable");
          }, ENTER_ANIM_MS);
        }, ENTRY_DELAY_MS);
      } else if (currentPhase === "exiting") {
        // Cancel exit, re-enter with new content
        updateDisplayed(current);
        updatePhase("entering");
        enterAnimRef.current = setTimeout(() => {
          stableAtRef.current = Date.now();
          updatePhase("stable");
        }, ENTER_ANIM_MS);
      } else {
        // Already entering or stable → morph content in place
        updateDisplayed(current);
        if (currentPhase === "entering") {
          // Let the existing enter animation finish naturally
        }
        // If stable, stableAtRef stays unchanged (we don't reset minVisible)
      }
    } else {
      /* ── Notice cleared ── */

      // Cancel any pending entry
      clearTimeout(entryDelayRef.current);
      entryDelayRef.current = undefined;
      clearTimeout(enterAnimRef.current);
      enterAnimRef.current = undefined;

      if (currentPhase === "entering" || currentPhase === "stable") {
        // Calculate how long to hold before exiting
        const tone = displayedRef.current?.tone ?? "info";
        const toneHold = HOLD_BY_TONE[tone];
        const elapsed = Date.now() - stableAtRef.current;
        const remainingMinVisible = Math.max(0, MIN_VISIBLE_MS - elapsed);
        const holdTime = Math.max(remainingMinVisible, toneHold);

        if (holdTime > 0) {
          // Wait before starting exit
          holdRef.current = setTimeout(() => {
            // Re-check: a new notice might have arrived during hold
            if (noticeRef.current) return;
            startExit();
          }, holdTime);
        } else {
          startExit();
        }
      }
      // If hidden or already exiting, do nothing
    }

    function startExit() {
      updatePhase("exiting");
      exitAnimRef.current = setTimeout(() => {
        updatePhase("hidden");
        updateDisplayed(null);
      }, EXIT_ANIM_MS);
    }

    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [noticeKey]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      clearAllTimers();
    };
  }, [clearAllTimers]);

  return { phase, displayed } as const;
}
