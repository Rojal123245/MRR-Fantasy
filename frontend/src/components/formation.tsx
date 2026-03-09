"use client";

import { motion } from "framer-motion";
import type { Player, Position } from "@/lib/api";

export interface FormationPlayer {
  player: Player;
  assignedPosition: Position;
}

interface FormationProps {
  players: FormationPlayer[];
  captainId?: string | null;
  onRemove?: (player: Player) => void;
}

const positionBadgeClass: Record<Position, string> = {
  GK: "badge-gk",
  DEF: "badge-def",
  MID: "badge-mid",
  FWD: "badge-fwd",
};

/** Vertical percentage (from top) for each row. */
const rowTop: Record<Position, string> = {
  FWD: "14%",
  MID: "38%",
  DEF: "62%",
  GK: "82%",
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

/** Derive formation label from assigned positions (e.g., "2-2-1"). */
export function getFormationLabel(players: FormationPlayer[]): string {
  const def = players.filter((p) => p.assignedPosition === "DEF").length;
  const mid = players.filter((p) => p.assignedPosition === "MID").length;
  const fwd = players.filter((p) => p.assignedPosition === "FWD").length;
  if (def + mid + fwd === 0) return "";
  return `${def}-${mid}-${fwd}`;
}

/** Check which required positions are still missing at least 1 player. */
export function getMissingPositions(players: FormationPlayer[]): Position[] {
  const has: Record<Position, boolean> = { GK: false, DEF: false, MID: false, FWD: false };
  for (const p of players) has[p.assignedPosition] = true;
  return (["GK", "DEF", "MID", "FWD"] as Position[]).filter((pos) => !has[pos]);
}

/**
 * Dynamic formation pitch showing players arranged by their assigned positions.
 * GK row always at bottom, DEF above, MID higher, FWD at top.
 * Each row auto-spreads based on how many players are assigned there.
 */
export default function Formation({ players, captainId, onRemove }: FormationProps) {
  // Group players by assigned position
  const rows: Record<Position, FormationPlayer[]> = { GK: [], DEF: [], MID: [], FWD: [] };
  for (const fp of players) {
    rows[fp.assignedPosition].push(fp);
  }

  const formation = getFormationLabel(players);
  const missing = getMissingPositions(players);

  // Build all slot coordinates
  type Slot = { fp: FormationPlayer | null; position: Position; top: string; left: string };
  const slots: Slot[] = [];

  for (const pos of ["FWD", "MID", "DEF", "GK"] as Position[]) {
    const group = rows[pos];
    const xs = spreadX(group.length);
    group.forEach((fp, i) => {
      slots.push({ fp, position: pos, top: rowTop[pos], left: xs[i] });
    });
  }

  return (
    <div className="mini-pitch relative w-full" style={{ paddingBottom: "140%", minHeight: 300 }}>
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

      {/* Missing position indicators */}
      {missing.length > 0 && players.length > 0 && (
        <div className="absolute bottom-2 left-1/2 -translate-x-1/2 z-10 flex gap-1">
          {missing.map((pos) => (
            <span
              key={pos}
              className="px-2 py-0.5 rounded-full text-[9px] font-bold animate-pulse"
              style={{
                background: "rgba(255,82,82,0.2)",
                border: "1px solid rgba(255,82,82,0.4)",
                color: "#ff8a80",
              }}
            >
              Need {pos}
            </span>
          ))}
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
      {slots.map((slot, i) => (
        <motion.div
          key={slot.fp ? slot.fp.player.id : `empty-${slot.position}-${i}`}
          initial={{ opacity: 0, scale: 0.5 }}
          animate={{ opacity: 1, scale: 1 }}
          transition={{ delay: i * 0.08, type: "spring", stiffness: 300, damping: 20 }}
          className="absolute -translate-x-1/2 -translate-y-1/2 flex flex-col items-center gap-1"
          style={{ top: slot.top, left: slot.left }}
        >
          {slot.fp ? (
            <div
              className="cursor-pointer group"
              onClick={() => onRemove && slot.fp && onRemove(slot.fp.player)}
            >
              <div className="relative">
                <div
                  className="w-11 h-11 rounded-full flex items-center justify-center text-[10px] font-bold border-2 transition-all group-hover:scale-110"
                  style={{
                    background: captainId === slot.fp.player.id ? "rgba(255, 171, 0, 0.3)" : "rgba(0, 230, 118, 0.2)",
                    borderColor: captainId === slot.fp.player.id ? "#ffab00" : "var(--accent-green)",
                    color: captainId === slot.fp.player.id ? "#ffab00" : "var(--accent-green)",
                    fontFamily: "var(--font-display)",
                  }}
                >
                  {slot.fp.assignedPosition}
                </div>
                {captainId === slot.fp.player.id && (
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
                className="text-[10px] text-center mt-1 font-medium max-w-[72px] truncate"
                style={{ color: "white", textShadow: "0 1px 3px rgba(0,0,0,0.8)" }}
              >
                {slot.fp.player.name.split(" ")[0]}
              </p>
              {/* Show "flex" indicator when playing out of primary position */}
              {slot.fp.assignedPosition !== slot.fp.player.position && (
                <span
                  className="text-[8px] text-center font-bold px-1 rounded"
                  style={{ background: "rgba(255,171,0,0.3)", color: "#ffab00" }}
                >
                  FLEX
                </span>
              )}
            </div>
          ) : (
            <div
              className="w-11 h-11 rounded-full border-2 border-dashed flex items-center justify-center text-[10px]"
              style={{ borderColor: "rgba(255,255,255,0.25)", color: "rgba(255,255,255,0.3)" }}
            >
              ?
            </div>
          )}
        </motion.div>
      ))}
    </div>
  );
}
