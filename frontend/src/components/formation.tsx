"use client";

import { motion, useReducedMotion } from "framer-motion";
import type { Player, Position } from "@/lib/api";
import PlayerAvatar from "@/components/player-avatar";
import { SwitchChip } from "@/components/position-switch";
import {
  POSITION_BADGE,
  getFormationLabel,
  getLineupIssues,
  otherPosition,
  previewMove,
} from "@/lib/lineup";

export interface FormationPlayer {
  player: Player;
  assignedPosition: Position;
}

/**
 * Every handler is optional. Without them the pitch is a read-only picture, as
 * the league page shows it.
 */
interface FormationProps {
  players: FormationPlayer[];
  captainId?: string | null;
  /** The player whose options are open, drawn highlighted. */
  activePlayerId?: string | null;
  /** Quick Swap: which starters the picked bench player can replace. */
  swapHints?: Record<string, "target" | "blocked">;
  /** Quick Swap: the picked bench player, so a token can say what tapping it does. */
  swapBenchName?: string;
  /** Disables the position switches without hiding them. */
  switchDisabled?: boolean;
  onPlayerTap?: (fp: FormationPlayer, el: HTMLButtonElement, viaKeyboard: boolean) => void;
  onSwitchPosition?: (fp: FormationPlayer, to: Position) => void;
  onBackgroundTap?: () => void;
}

const FOCUS_RING =
  "focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-green)]";

/** Vertical percentage (from top) of each row's avatars. */
const rowTop: Record<Position, string> = {
  FWD: "14%",
  MID: "38%",
  DEF: "62%",
  // Low enough that a centre-back's switch clears the keeper on a small phone.
  GK: "84%",
};

/** Compute evenly-spaced horizontal positions for N items. */
function spreadX(count: number): string[] {
  if (count === 0) return [];
  if (count === 1) return ["50%"];
  if (count === 2) return ["32%", "68%"];
  if (count === 3) return ["22%", "50%", "78%"];
  // Fallback for 4+
  return Array.from({ length: count }, (_, i) => `${15 + (70 / (count - 1)) * i}%`);
}

/**
 * Dynamic formation pitch showing players arranged by their assigned positions.
 * GK row always at bottom, DEF above, MID higher, FWD at top.
 * Each row auto-spreads based on how many players are assigned there.
 */
export default function Formation({
  players,
  captainId,
  activePlayerId,
  swapHints,
  swapBenchName,
  switchDisabled,
  onPlayerTap,
  onSwitchPosition,
  onBackgroundTap,
}: FormationProps) {
  const reduce = useReducedMotion();

  // Group players by assigned position
  const rows: Record<Position, FormationPlayer[]> = { GK: [], DEF: [], MID: [], FWD: [] };
  for (const fp of players) {
    rows[fp.assignedPosition].push(fp);
  }

  const formation = getFormationLabel(players);
  const issues = getLineupIssues(players);

  type Slot = { fp: FormationPlayer; top: string; left: string; rowCount: number };
  const slots: Slot[] = [];

  for (const pos of ["FWD", "MID", "DEF", "GK"] as Position[]) {
    const group = rows[pos];
    const xs = spreadX(group.length);
    group.forEach((fp, i) => {
      slots.push({ fp, top: rowTop[pos], left: xs[i], rowCount: group.length });
    });
  }

  return (
    <div>
      <div
        className="mini-pitch relative w-full"
        style={{ paddingBottom: "140%", minHeight: 300 }}
        onClick={onBackgroundTap}
      >
        {/* Pitch markings overlay */}
        <div className="absolute inset-0 rounded-xl overflow-hidden">
          <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-24 h-24 rounded-full border border-white/10" />
          <div className="absolute top-1/2 left-0 right-0 h-px bg-white/10" />
          <div className="absolute top-0 left-1/2 -translate-x-1/2 w-32 h-12 border-b border-l border-r border-white/10 rounded-b-lg" />
          <div className="absolute bottom-0 left-1/2 -translate-x-1/2 w-32 h-12 border-t border-l border-r border-white/10 rounded-t-lg" />
        </div>

        {/* Formation label */}
        {formation && (
          <div
            className="absolute top-2 left-1/2 -translate-x-1/2 z-10 px-3 py-1 rounded-full text-xs font-bold tracking-wider"
            style={{
              fontFamily: "var(--font-display)",
              background: "rgba(0,0,0,0.6)",
              color: "var(--accent-green)",
              border: "1px solid rgba(0,230,118,0.3)",
              backdropFilter: "blur(8px)",
            }}
          >
            {formation}
          </div>
        )}

        {/* Empty state */}
        {players.length === 0 && (
          <div className="absolute inset-0 flex items-center justify-center">
            <div className="text-center">
              <p className="text-xs font-medium" style={{ color: "rgba(255,255,255,0.4)", fontFamily: "var(--font-display)" }}>
                ADD PLAYERS TO
              </p>
              <p className="text-sm font-bold" style={{ color: "rgba(255,255,255,0.5)", fontFamily: "var(--font-display)" }}>
                BUILD YOUR FORMATION
              </p>
            </div>
          </div>
        )}

        {/* Player slots */}
        {slots.map(({ fp, top, left, rowCount }, i) => {
          const { player, assignedPosition } = fp;
          const isCaptain = captainId === player.id;
          const isActive = activePlayerId === player.id;
          const hint = swapHints?.[player.id];
          const outOfPosition = assignedPosition !== player.position;
          const other = onSwitchPosition ? otherPosition(player, assignedPosition) : null;
          const preview = other ? previewMove(players, player.id, other) : null;
          const compact = rowCount >= 4;

          const token = (
            <>
              <div className="relative">
                <div
                  className={`relative rounded-full border-2 transition-all ${onPlayerTap ? "group-hover:scale-110" : ""} ${isActive ? "scale-110" : ""}`}
                  style={{
                    borderColor: isActive ? "#fff" : isCaptain ? "#ffab00" : "var(--accent-green)",
                    boxShadow: isActive
                      ? "0 0 0 3px rgba(255,255,255,0.25)"
                      : hint === "target"
                        ? "0 0 0 2px rgba(0,230,118,0.6), 0 0 12px rgba(0,230,118,0.4)"
                        : isCaptain
                          ? "0 0 10px rgba(251,191,36,0.45)"
                          : "none",
                  }}
                >
                  <PlayerAvatar
                    playerName={player.name}
                    sizeClassName="w-11 h-11"
                    className={isCaptain ? "ring-2 ring-amber-400/40" : ""}
                  />
                </div>
                <span
                  className={`absolute -bottom-1 left-1/2 -translate-x-1/2 ${POSITION_BADGE[assignedPosition]} text-[8px] font-bold px-1.5 py-0.5 rounded-full text-white`}
                  // Where the switch replaces the FLEX tag, an amber ring says it instead.
                  style={other && outOfPosition ? { boxShadow: "0 0 0 1.5px #ffab00" } : undefined}
                >
                  {assignedPosition}
                </span>
                {isCaptain && (
                  <div
                    className="absolute -top-1 -right-1 w-4 h-4 rounded-full flex items-center justify-center text-[8px] font-black"
                    style={{
                      background: "linear-gradient(135deg, #fbbf24, #f59e0b)",
                      color: "#1a1a2e",
                      boxShadow: "0 0 8px rgba(251,191,36,0.6)",
                    }}
                  >
                    C
                  </div>
                )}
              </div>
              <p
                className={`text-[10px] text-center mt-1 font-medium truncate ${compact ? "max-w-[56px]" : "max-w-[72px]"}`}
                style={{ color: "white", textShadow: "0 1px 3px rgba(0,0,0,0.8)" }}
              >
                {player.name.split(" ")[0]}
              </p>
            </>
          );

          return (
            // The avatar's centre sits on the row line, and the name and switch
            // hang below it. Centring the whole stack instead lifted any player
            // with a FLEX tag above the rest of their row.
            <motion.div
              key={player.id}
              initial={false}
              animate={{ top, left }}
              transition={reduce ? { duration: 0 } : { type: "spring", stiffness: 400, damping: 32 }}
              className="absolute -translate-x-1/2 -translate-y-[22px] transition-opacity"
              style={{ opacity: hint === "blocked" ? 0.45 : 1, zIndex: isActive ? 20 : undefined }}
            >
              <motion.div
                className="flex flex-col items-center"
                initial={{ opacity: 0, scale: 0.5 }}
                animate={{ opacity: 1, scale: 1 }}
                transition={{ delay: i * 0.08, type: "spring", stiffness: 300, damping: 20 }}
              >
                {onPlayerTap ? (
                  <button
                    type="button"
                    data-token-id={player.id}
                    // While a bench player is picked, a tap swaps instead of opening options.
                    aria-expanded={hint ? undefined : isActive}
                    aria-disabled={hint === "blocked" ? true : undefined}
                    aria-label={
                      hint === "target"
                        ? `Swap ${swapBenchName ?? "the bench player"} in for ${player.name}`
                        : hint === "blocked"
                          ? `${player.name} can't be swapped for ${swapBenchName ?? "the bench player"}`
                          : `${player.name}, playing ${assignedPosition}${outOfPosition ? ", out of position" : ""}${isCaptain ? ", captain" : ""}. Open player options`
                    }
                    onClick={(e) => {
                      e.stopPropagation();
                      // A keyboard press reports no clicks.
                      onPlayerTap(fp, e.currentTarget, e.detail === 0);
                    }}
                    className={`group flex flex-col items-center bg-transparent border-none p-0 cursor-pointer rounded-lg ${FOCUS_RING}`}
                  >
                    {token}
                  </button>
                ) : (
                  <div className="flex flex-col items-center">{token}</div>
                )}
                {other && preview ? (
                  <SwitchChip
                    playerName={player.name}
                    to={other}
                    hint={preview.text}
                    pulse={preview.tone === "fix"}
                    disabled={switchDisabled}
                    compact={compact}
                    onSwitch={() => onSwitchPosition?.(fp, other)}
                  />
                ) : (
                  outOfPosition && (
                    <span
                      className="text-[8px] text-center font-bold px-1 rounded"
                      style={{ background: "rgba(255,171,0,0.3)", color: "#ffab00" }}
                    >
                      FLEX
                    </span>
                  )
                )}
              </motion.div>
            </motion.div>
          );
        })}
      </div>
      {/* Below the pitch rather than on it, where they would cover a player. */}
      {(issues.missing.length > 0 || issues.extraGk) && players.length > 0 && (
        <div className="mt-2 flex flex-wrap justify-center gap-1">
          {[
            ...issues.missing.map((pos) => `Need ${pos}`),
            ...(issues.extraGk ? ["1 GK only"] : []),
          ].map((label) => (
            <span
              key={label}
              className="px-2 py-0.5 rounded-full text-[9px] font-bold whitespace-nowrap motion-safe:animate-pulse"
              style={{
                background: "rgba(255,82,82,0.2)",
                border: "1px solid rgba(255,82,82,0.4)",
                color: "#ff8a80",
              }}
            >
              {label}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}
