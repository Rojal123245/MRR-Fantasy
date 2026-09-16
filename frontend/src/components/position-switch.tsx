"use client";

import { ArrowLeftRight } from "lucide-react";
import type { Position } from "@/lib/api";
import { POSITION_BADGE } from "@/lib/lineup";

const FOCUS_RING =
  "focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-green)]";

interface SwitchChipProps {
  playerName: string;
  to: Position;
  /** What the move would do, e.g. "Leaves no DEF". */
  hint: string;
  /** The move would fill a gap in the lineup, so it is the thing to tap next. */
  pulse: boolean;
  disabled?: boolean;
  /** Crowded rows get no extra side padding, so neighbouring chips don't overlap. */
  compact?: boolean;
  onSwitch: () => void;
}

/**
 * The pitch control that moves a two-position starter to their other position
 * in one tap. It shows only the target: the avatar badge already shows where
 * they play now.
 */
export function SwitchChip({ playerName, to, hint, pulse, disabled, compact, onSwitch }: SwitchChipProps) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={(e) => {
        e.stopPropagation();
        onSwitch();
      }}
      aria-label={`Move ${playerName} to ${to}. ${hint}`}
      title={hint}
      // The padding and matching negative margin grow the tap area downwards and
      // sideways without moving anything, and never upwards into the name.
      className={`relative bg-transparent border-none cursor-pointer pt-0.5 pb-2.5 -mb-2.5 ${compact ? "px-0" : "px-1 -mx-1"} rounded-full disabled:cursor-not-allowed disabled:opacity-35 ${FOCUS_RING}`}
    >
      <span
        className={`inline-flex items-center gap-1 rounded-full pl-1 pr-0.5 py-0.5 ${pulse ? "motion-safe:animate-pulse" : ""}`}
        style={{
          background: "rgba(0,0,0,0.65)",
          border: `1px solid ${pulse ? "#ff8a80" : "rgba(255,255,255,0.3)"}`,
        }}
      >
        <ArrowLeftRight size={10} className="text-white/70" aria-hidden />
        <span
          className={`${POSITION_BADGE[to]} rounded-full px-1.5 text-[10px] font-bold text-white leading-[14px]`}
        >
          {to}
        </span>
      </span>
    </button>
  );
}

interface PositionPillProps {
  playerName: string;
  primary: Position;
  secondary: Position;
  current: Position;
  /** Switching would fill a gap in the lineup. */
  pulse?: boolean;
  /** Omitted when the lineup can't be edited: the pill is then a plain label. */
  onSwitch?: () => void;
}

/**
 * Both of a starter's positions, primary first, with the one they play filled
 * in. Tapping it switches them to the other one.
 */
export function PositionPill({ playerName, primary, secondary, current, pulse, onSwitch }: PositionPillProps) {
  const other = current === primary ? secondary : primary;
  const outOfPosition = current !== primary;

  const pill = (
    <span
      className="flex rounded overflow-hidden text-[10px] font-bold leading-4"
      style={{
        // Amber ring = playing their secondary position (what FLEX used to say).
        boxShadow: outOfPosition ? "0 0 0 1px rgba(255,171,0,0.6)" : "0 0 0 1px rgba(255,255,255,0.12)",
      }}
    >
      {[primary, secondary].map((pos) =>
        pos === current ? (
          <span key={pos} className={`${POSITION_BADGE[pos]} text-white px-1.5 py-0.5`}>
            {pos}
          </span>
        ) : (
          <span
            key={pos}
            className={`flex items-center gap-0.5 px-1.5 py-0.5 ${pulse && onSwitch ? "motion-safe:animate-pulse" : ""}`}
            style={{
              background: "rgba(255,255,255,0.05)",
              color: pulse && onSwitch ? "#ff8a80" : "var(--text-muted)",
            }}
          >
            {onSwitch && <ArrowLeftRight size={9} aria-hidden />}
            {pos}
          </span>
        )
      )}
    </span>
  );

  if (!onSwitch) {
    return (
      <span
        className="shrink-0"
        title={`Plays ${current}${outOfPosition ? ` (out of position, usually ${primary})` : ""}`}
      >
        {pill}
      </span>
    );
  }

  return (
    <button
      type="button"
      onClick={(e) => {
        e.stopPropagation();
        onSwitch();
      }}
      aria-label={`${playerName} plays ${current}. Switch to ${other}`}
      title={`Switch to ${other}`}
      // Tall invisible padding makes the whole row height tappable without
      // changing the row's layout.
      className={`relative shrink-0 ml-1 -my-3 py-3 bg-transparent border-none cursor-pointer rounded ${FOCUS_RING}`}
    >
      {pill}
    </button>
  );
}
