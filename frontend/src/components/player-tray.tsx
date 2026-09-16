"use client";

import { useEffect, useId, useRef } from "react";
import { motion, useReducedMotion } from "framer-motion";
import { AlertCircle, ArrowLeftRight, Check, Lock, Trash2, X } from "lucide-react";
import type { Position } from "@/lib/api";
import type { FormationPlayer } from "@/components/formation";
import PlayerAvatar from "@/components/player-avatar";
import { POSITION_BADGE, findSwapPartner, playablePositions, previewMove } from "@/lib/lineup";

const FOCUS_RING =
  "focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-green)]";

/** The tray's last button: what the manager can do with the player besides moving them. */
export type TrayAction =
  | { kind: "remove"; onRemove: (viaKeyboard: boolean) => void }
  | { kind: "transfer"; blockedReason: string | null; onTransfer: (viaKeyboard: boolean) => void }
  | null;

interface PlayerTrayProps {
  slot: FormationPlayer;
  /** The whole starting six, to preview what a move does to it. */
  starters: FormationPlayer[];
  isCaptain: boolean;
  captainBlockedReason: string | null;
  action: TrayAction;
  /** Opened from the keyboard: take focus so the options can be reached. */
  autoFocus: boolean;
  onMove: (to: Position) => void;
  onSwap: (partnerId: string) => void;
  onCaptain: () => void;
  onClose: () => void;
  className?: string;
}

/**
 * Options for one starter, opened by tapping them on the pitch. It has no
 * backdrop, so the pitch stays visible and the tray stays open while the
 * manager moves the player about.
 */
export function PlayerTray({
  slot,
  starters,
  isCaptain,
  captainBlockedReason,
  action,
  autoFocus,
  onMove,
  onSwap,
  onCaptain,
  onClose,
  className = "",
}: PlayerTrayProps) {
  const reduce = useReducedMotion();
  const reasonId = useId();
  const rootRef = useRef<HTMLDivElement>(null);

  const { player, assignedPosition } = slot;
  const firstName = player.name.split(" ")[0];
  const playable = playablePositions(player);
  const other = playable.find((pos) => pos !== assignedPosition) ?? null;
  const preview = other ? previewMove(starters, player.id, other) : null;
  const partner = other && preview?.tone === "warn" ? findSwapPartner(starters, player.id, other) : null;
  const outOfPosition = assignedPosition !== player.position;
  const blockedReason = action?.kind === "transfer" ? action.blockedReason : null;

  useEffect(() => {
    const root = rootRef.current;
    if (!root || root.getClientRects().length === 0) return;
    // Under a tall pitch the desktop tray can open below the fold. The mobile
    // one is in a fixed bar, where this does nothing.
    root.scrollIntoView({ block: "nearest", behavior: reduce ? "auto" : "smooth" });
    if (autoFocus) {
      root.querySelector<HTMLButtonElement>("[data-tray-focus]:not(:disabled)")?.focus({ preventScroll: true });
    }
  }, [autoFocus, player.id, reduce]);

  return (
    <motion.div
      ref={rootRef}
      role="region"
      aria-label={`Options for ${player.name}`}
      initial={reduce ? false : { opacity: 0, y: 12 }}
      animate={{ opacity: 1, y: 0 }}
      exit={reduce ? { opacity: 0, transition: { duration: 0 } } : { opacity: 0, y: 12, transition: { duration: 0.15 } }}
      transition={reduce ? { duration: 0 } : { type: "spring", damping: 30, stiffness: 300 }}
      className={`rounded-xl p-3 text-left ${className}`}
      style={{
        background: "var(--bg-card)",
        border: "1px solid rgba(0,230,118,0.3)",
        boxShadow: "0 -8px 24px rgba(0,0,0,0.5)",
      }}
      onClick={(e) => e.stopPropagation()}
    >
      {/* Who */}
      <div className="flex items-center gap-2 mb-3">
        <PlayerAvatar playerName={player.name} sizeClassName="w-8 h-8" />
        <span
          className="flex-1 min-w-0 truncate text-sm font-bold"
          style={{ fontFamily: "var(--font-display)", color: "var(--text-primary)" }}
        >
          {player.name}
        </span>
        {outOfPosition && (
          <span
            className="text-[9px] font-bold px-1 rounded shrink-0"
            style={{ background: "rgba(255,171,0,0.2)", color: "#ffab00" }}
          >
            FLEX
          </span>
        )}
        <button
          type="button"
          onClick={onClose}
          aria-label="Close player options"
          className={`w-9 h-9 -my-1 -mr-1 shrink-0 flex items-center justify-center rounded-lg bg-transparent border-none cursor-pointer ${FOCUS_RING}`}
          style={{ color: "var(--text-muted)" }}
        >
          <X size={18} />
        </button>
      </div>

      <div className="flex gap-2 items-start">
        {/* Where they play */}
        {playable.length > 1 ? (
          playable.map((pos) => {
            const isCurrent = pos === assignedPosition;
            const warn = !isCurrent && preview?.tone === "warn";
            return (
              <div key={pos} className="flex-1 min-w-0 flex flex-col">
                <button
                  type="button"
                  data-tray-focus
                  aria-pressed={isCurrent}
                  aria-label={
                    isCurrent
                      ? `${player.name} plays ${pos} now`
                      : `Play ${player.name} at ${pos}. ${preview?.text ?? ""}`
                  }
                  onClick={() => {
                    if (!isCurrent) onMove(pos);
                  }}
                  className={`h-11 rounded-xl ${POSITION_BADGE[pos]} text-white font-bold text-sm border-none flex items-center justify-center gap-1.5 ${isCurrent ? "cursor-default" : "cursor-pointer"} ${FOCUS_RING}`}
                  style={{
                    fontFamily: "var(--font-display)",
                    letterSpacing: "0.05em",
                    boxShadow: isCurrent ? "0 0 0 2px #fff" : undefined,
                    opacity: isCurrent ? 1 : 0.85,
                    // A ring rather than an outline, which belongs to the focus indicator.
                    ...(warn ? { boxShadow: "0 0 0 2px var(--accent-amber)" } : {}),
                  }}
                >
                  {isCurrent ? <Check size={14} aria-hidden /> : <ArrowLeftRight size={14} aria-hidden />}
                  {pos}
                  {isCurrent && <span className="text-[9px] opacity-80">NOW</span>}
                </button>
                <p
                  className="text-[10px] mt-1 text-center leading-tight flex items-center justify-center gap-0.5"
                  style={{
                    color: isCurrent
                      ? "var(--text-muted)"
                      : preview?.tone === "fix"
                        ? "var(--accent-green)"
                        : preview?.tone === "warn"
                          ? "var(--accent-amber)"
                          : "var(--text-muted)",
                  }}
                >
                  {warn && <AlertCircle size={10} className="shrink-0" aria-hidden />}
                  {isCurrent ? "Playing now" : preview?.text}
                </p>
              </div>
            );
          })
        ) : (
          <div className="flex-1 min-w-0 flex flex-col">
            <span
              className={`h-11 rounded-xl ${POSITION_BADGE[assignedPosition]} text-white font-bold text-sm flex items-center justify-center`}
              style={{ fontFamily: "var(--font-display)", letterSpacing: "0.05em" }}
            >
              {assignedPosition}
            </span>
            <p className="text-[10px] mt-1 text-center leading-tight" style={{ color: "var(--text-muted)" }}>
              Only plays {assignedPosition}
            </p>
          </div>
        )}

        {/* Captain */}
        <div className="w-11 shrink-0 flex flex-col items-center">
          <button
            type="button"
            data-tray-focus
            aria-pressed={isCaptain}
            aria-label={
              captainBlockedReason
                ? `Cannot captain ${player.name}: ${captainBlockedReason}`
                : isCaptain
                  ? `${player.name} is captain`
                  : `Make ${player.name} captain`
            }
            disabled={!!captainBlockedReason}
            onClick={onCaptain}
            className={`w-11 h-11 rounded-xl text-sm font-black border-none cursor-pointer disabled:cursor-not-allowed disabled:opacity-40 ${FOCUS_RING}`}
            style={{
              background: isCaptain ? "linear-gradient(135deg, #fbbf24, #f59e0b)" : "rgba(255,255,255,0.1)",
              color: isCaptain ? "#1a1a2e" : "var(--text-primary)",
              boxShadow: isCaptain ? "0 0 8px rgba(251,191,36,0.5)" : "none",
            }}
          >
            C
          </button>
          <p className="text-[10px] mt-1 text-center leading-tight" style={{ color: "var(--text-muted)" }}>
            {captainBlockedReason ? "Can't captain" : "Captain"}
          </p>
        </div>

        {/* Remove or transfer */}
        {action?.kind === "remove" && (
          <div className="w-11 shrink-0 flex flex-col items-center">
            <button
              type="button"
              aria-label={`Remove ${player.name} from your squad`}
              // A keyboard press reports no clicks.
              onClick={(e) => action.onRemove(e.detail === 0)}
              className={`w-11 h-11 rounded-xl flex items-center justify-center border-none cursor-pointer ${FOCUS_RING}`}
              style={{ background: "rgba(255,82,82,0.15)", color: "var(--danger)" }}
            >
              <Trash2 size={16} />
            </button>
            <p className="text-[10px] mt-1 text-center leading-tight" style={{ color: "var(--text-muted)" }}>
              Remove
            </p>
          </div>
        )}
        {action?.kind === "transfer" && (
          <div className="w-11 shrink-0 flex flex-col items-center">
            <button
              type="button"
              aria-label={`Transfer ${player.name} out`}
              aria-describedby={blockedReason ? reasonId : undefined}
              disabled={!!blockedReason}
              onClick={(e) => action.onTransfer(e.detail === 0)}
              className={`w-11 h-11 rounded-xl flex items-center justify-center border-none cursor-pointer disabled:cursor-not-allowed disabled:opacity-30 ${FOCUS_RING}`}
              style={{ background: "rgba(0,230,118,0.1)", color: "var(--accent-green)" }}
            >
              <ArrowLeftRight size={16} />
            </button>
            <p className="text-[10px] mt-1 text-center leading-tight" style={{ color: "var(--text-muted)" }}>
              Transfer
            </p>
          </div>
        )}
      </div>

      {/* Trading places keeps the lineup valid when a plain move would not. */}
      {partner && other && (
        <button
          type="button"
          onClick={() => {
            onSwap(partner.player.id);
            // After the swap the suggestion usually no longer applies and this
            // button goes, so hand focus to the position the player now has.
            requestAnimationFrame(() => {
              const root = rootRef.current;
              if (root && !root.contains(document.activeElement)) {
                root.querySelector<HTMLButtonElement>('[data-tray-focus][aria-pressed="true"]')?.focus();
              }
            });
          }}
          aria-label={`Swap positions with ${partner.player.name}: ${player.name} to ${other}, ${partner.player.name} to ${assignedPosition}. Formation stays the same.`}
          className={`w-full mt-2 min-h-10 px-2 py-1.5 rounded-lg text-[11px] font-bold cursor-pointer flex flex-wrap items-center justify-center gap-x-1.5 gap-y-0.5 ${FOCUS_RING}`}
          style={{
            background: "rgba(0,230,118,0.08)",
            border: "1px solid rgba(0,230,118,0.45)",
            color: "var(--accent-green)",
            fontFamily: "var(--font-display)",
          }}
        >
          <span
            className="text-[8px] px-1 py-0.5 rounded"
            style={{ background: "rgba(0,230,118,0.18)", letterSpacing: "0.05em" }}
          >
            RECOMMENDED
          </span>
          <span className="flex items-center gap-1">
            <ArrowLeftRight size={12} aria-hidden />
            Swap with {partner.player.name.split(" ")[0]}
          </span>
          <span className="font-medium" style={{ color: "var(--text-muted)", fontFamily: "var(--font-body)" }}>
            {firstName} → {other}, {partner.player.name.split(" ")[0]} → {assignedPosition}
          </span>
        </button>
      )}

      {blockedReason && (
        <p
          id={reasonId}
          className="text-[10px] mt-2 flex items-center gap-1"
          style={{ color: "var(--text-muted)" }}
        >
          <Lock size={10} className="shrink-0" aria-hidden />
          {blockedReason}
        </p>
      )}
    </motion.div>
  );
}
