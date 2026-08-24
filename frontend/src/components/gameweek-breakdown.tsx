"use client";

import { motion } from "framer-motion";
import { Lock, Trophy, Users } from "lucide-react";

import type { GameweekPlayerLine, MemberGameweek, Position } from "@/lib/api";

const POSITION_BADGE: Record<Position, string> = {
  GK: "badge-gk",
  DEF: "badge-def",
  MID: "badge-mid",
  FWD: "badge-fwd",
};

/** The six components behind a player's line, in the order they are summed. */
const COLUMN_KEYS = [
  "goal_points",
  "assist_points",
  "clean_sheet_points",
  "save_points",
  "minutes_points",
  "deduction_points",
] as const;

const COLUMN_LABELS = ["GLS", "AST", "CS", "SV", "MIN", "PEN"];

const COLUMN_TITLES = [
  "Goals, at the rate for the position they were played in",
  "Assists, 5 each",
  "Clean sheets, goalkeepers and defenders only",
  "Saves (1 point per 5) and penalty saves",
  "2 for 35 minutes or more, 1 for any appearance",
  "Own goals, penalty misses and fouls",
];

function signed(value: number) {
  if (value === 0) return "0";
  return value > 0 ? `${value}` : `${value}`;
}

function toneFor(value: number) {
  if (value > 0) return "var(--accent-green)";
  if (value < 0) return "var(--danger)";
  return "var(--text-muted)";
}

/** One player's row: six components, their sum, the multiplier, the total. */
function PlayerRow({ line }: { line: GameweekPlayerLine }) {
  const dimmed = !line.counted;

  return (
    <div
      className="grid items-center gap-1 px-2 py-2 rounded-lg text-xs"
      style={{
        gridTemplateColumns: "minmax(120px, 1.6fr) repeat(6, 30px) 40px 26px 42px",
        background: dimmed ? "transparent" : "var(--bg-secondary)",
        border: `1px solid ${dimmed ? "transparent" : "var(--border-color)"}`,
        opacity: dimmed ? 0.45 : 1,
      }}
    >
      <div className="flex items-center gap-1.5 min-w-0">
        <span
          className={`text-[9px] font-bold px-1.5 py-0.5 rounded-full text-white shrink-0 ${
            POSITION_BADGE[line.played_as]
          }`}
          title={
            line.played_as === line.position
              ? `Played as ${line.played_as}`
              : `A ${line.position} played as ${line.played_as}, so scored at ${line.played_as} rates`
          }
        >
          {line.played_as}
        </span>
        <span className="truncate font-medium">{line.name}</span>
        {line.is_captain && (
          <span
            className="text-[9px] font-bold px-1 py-0.5 rounded shrink-0"
            style={{ background: "rgba(255,171,0,0.2)", color: "#ffab00" }}
            title={line.multiplier === 3 ? "Captain, Triple Captain played" : "Captain"}
          >
            {line.multiplier === 3 ? "TC" : "C"}
          </span>
        )}
      </div>

      {COLUMN_KEYS.map((key, i) => (
        <span
          key={key}
          className="text-center tabular-nums"
          title={COLUMN_TITLES[i]}
          style={{ color: toneFor(line[key] as number) }}
        >
          {signed(line[key] as number)}
        </span>
      ))}

      <span
        className="text-center tabular-nums font-bold"
        style={{ color: "var(--text-secondary)" }}
        title="The six columns to the left, added up"
      >
        {line.base_points}
      </span>
      <span
        className="text-center tabular-nums"
        style={{ color: line.multiplier > 1 ? "#ffab00" : "var(--text-muted)" }}
        title={`Multiplied by ${line.multiplier}`}
      >
        ×{line.multiplier}
      </span>
      <span
        className="text-right tabular-nums font-bold"
        style={{
          fontFamily: "var(--font-display)",
          color: line.counted ? "var(--accent-green)" : "var(--text-muted)",
        }}
      >
        {line.counted ? line.total_points : "—"}
      </span>
    </div>
  );
}

function ColumnHeader() {
  return (
    <div
      className="grid items-center gap-1 px-2 pb-1 text-[9px] uppercase tracking-wider"
      style={{
        gridTemplateColumns: "minmax(120px, 1.6fr) repeat(6, 30px) 40px 26px 42px",
        color: "var(--text-muted)",
        fontFamily: "var(--font-display)",
      }}
    >
      <span>Player</span>
      {COLUMN_LABELS.map((label, i) => (
        <span key={label} className="text-center" title={COLUMN_TITLES[i]}>
          {label}
        </span>
      ))}
      <span className="text-center">Sum</span>
      <span className="text-center">×</span>
      <span className="text-right">Pts</span>
    </div>
  );
}

function Subtotal({ label, value, hint }: { label: string; value: number; hint?: string }) {
  return (
    <div className="flex items-baseline justify-between px-2 py-1.5">
      <span className="text-xs" style={{ color: "var(--text-muted)" }}>
        {label}
        {hint && (
          <span className="ml-1.5 text-[10px]" style={{ color: "var(--text-muted)" }}>
            {hint}
          </span>
        )}
      </span>
      <span
        className="text-sm font-bold tabular-nums"
        style={{ fontFamily: "var(--font-display)", color: "var(--text-secondary)" }}
      >
        {value}
      </span>
    </div>
  );
}

/**
 * A manager's completed gameweek, player by player.
 *
 * Every number on screen is one a manager can check: the six columns add to the
 * player's sum, the sum times the multiplier is their points, and the players
 * add to the gross the league table shows.
 */
export function GameweekBreakdown({ data }: { data: MemberGameweek }) {
  if (!data.has_snapshot) {
    return (
      <div
        className="flex flex-col items-center gap-2 px-4 py-8 rounded-lg text-center"
        style={{ background: "var(--bg-secondary)", border: "1px dashed var(--border-color)" }}
      >
        <Lock size={18} style={{ color: "var(--text-muted)" }} />
        <p className="text-sm font-medium">No lineup was recorded for this gameweek</p>
        <p className="text-xs max-w-xs" style={{ color: "var(--text-muted)" }}>
          This week was played before lineups were frozen, so who {data.username} actually
          fielded is not known. Their total below still stands — we just will not guess the
          eleven behind it.
        </p>
        {data.total_points !== null && (
          <p
            className="text-2xl font-bold mt-1"
            style={{ fontFamily: "var(--font-display)", color: "var(--accent-green)" }}
          >
            {data.total_points}
          </p>
        )}
      </div>
    );
  }

  const startersTotal = data.starters.reduce((sum, l) => sum + l.total_points, 0);
  const benchTotal = data.bench.reduce((sum, l) => sum + l.total_points, 0);
  const gross = startersTotal + benchTotal;
  const hit = data.transfer_points_hit ?? 0;
  const benchBoost = data.chip_played === "bench_boost";

  return (
    <div className="space-y-3">
      <div className="overflow-x-auto">
        <div style={{ minWidth: 460 }}>
          <div className="flex items-center gap-1.5 px-2 mb-1">
            <Users size={12} style={{ color: "var(--text-muted)" }} />
            <h4
              className="text-[10px] uppercase tracking-wider"
              style={{ fontFamily: "var(--font-display)", color: "var(--text-muted)" }}
            >
              Starting 6
            </h4>
          </div>
          <ColumnHeader />
          <div className="space-y-1">
            {data.starters.map((line) => (
              <PlayerRow key={line.id} line={line} />
            ))}
          </div>
          <Subtotal label="Starters" value={startersTotal} />

          {data.bench.length > 0 && (
            <>
              <div className="flex items-center gap-1.5 px-2 mt-3 mb-1">
                <h4
                  className="text-[10px] uppercase tracking-wider"
                  style={{ fontFamily: "var(--font-display)", color: "var(--text-muted)" }}
                >
                  Bench
                </h4>
                <span
                  className="text-[9px] font-bold px-1.5 py-0.5 rounded"
                  style={
                    benchBoost
                      ? { background: "rgba(0,230,118,0.15)", color: "var(--accent-green)" }
                      : { background: "rgba(255,255,255,0.05)", color: "var(--text-muted)" }
                  }
                >
                  {benchBoost ? "BENCH BOOST — counts" : "did not count"}
                </span>
              </div>
              <div className="space-y-1">
                {data.bench.map((line) => (
                  <PlayerRow key={line.id} line={line} />
                ))}
              </div>
              <Subtotal
                label="Bench"
                value={benchTotal}
                hint={benchBoost ? undefined : "no Bench Boost this week"}
              />
            </>
          )}
        </div>
      </div>

      <div
        className="rounded-lg px-2 py-1"
        style={{ background: "var(--bg-secondary)", border: "1px solid var(--border-color)" }}
      >
        <Subtotal label="Gross" value={gross} />
        {hit !== 0 && (
          <Subtotal label="Transfer hit" value={-hit} hint="charged to the team, not a player" />
        )}
        <div
          className="flex items-baseline justify-between px-2 py-2 mt-1"
          style={{ borderTop: "1px solid var(--border-color)" }}
        >
          <span
            className="text-xs uppercase tracking-wider"
            style={{ fontFamily: "var(--font-display)", color: "var(--text-secondary)" }}
          >
            Gameweek total
          </span>
          <span
            className="text-xl font-bold tabular-nums"
            style={{ fontFamily: "var(--font-display)", color: "var(--accent-green)" }}
          >
            {data.total_points ?? gross - hit}
          </span>
        </div>
      </div>

      {data.chip_played === "triple_captain" && (
        <motion.p
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          className="flex items-center gap-1.5 text-[11px] px-2"
          style={{ color: "#ffab00" }}
        >
          <Trophy size={12} />
          Triple Captain played — the armband is worth ×3 this week
        </motion.p>
      )}
    </div>
  );
}
