"use client";

import { AnimatePresence, motion } from "framer-motion";
import { ChevronDown, EyeOff, Loader2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";

import {
  getGameweekScoreboard,
  getMemberGameweek,
  type GameweekScoreboard as Scoreboard,
  type GameweekScoreboardEntry,
  type MemberGameweek,
} from "@/lib/api";
import { GameweekBreakdown } from "./gameweek-breakdown";

const CHIP_LABEL: Record<string, string> = {
  triple_captain: "Triple Captain",
  bench_boost: "Bench Boost",
};

function Medal({ rank }: { rank: number }) {
  const tone =
    rank === 1 ? "#ffd54f" : rank === 2 ? "#cfd8dc" : rank === 3 ? "#bcaaa4" : undefined;
  return (
    <span
      className="w-6 text-center text-xs font-bold tabular-nums shrink-0"
      style={{ fontFamily: "var(--font-display)", color: tone ?? "var(--text-muted)" }}
    >
      {rank}
    </span>
  );
}

/** One manager's row, expandable into the full derivation of their week. */
function ScoreboardRow({
  entry,
  rank,
  leagueId,
  week,
  token,
  isSelf,
}: {
  entry: GameweekScoreboardEntry;
  rank: number;
  leagueId: string;
  week: number;
  token: string;
  isSelf: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [detail, setDetail] = useState<MemberGameweek | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const toggle = useCallback(async () => {
    const next = !open;
    setOpen(next);
    if (!next || detail || loading) return;

    setLoading(true);
    setError(null);
    try {
      setDetail(await getMemberGameweek(leagueId, entry.user_id, week, token));
    } catch (e) {
      // The server is the authority on who may see a lineup; if it says no,
      // say so plainly rather than pretending the data is missing.
      setError(e instanceof Error ? e.message : "Could not load this lineup");
    } finally {
      setLoading(false);
    }
  }, [open, detail, loading, leagueId, entry.user_id, week, token]);

  return (
    <div
      className="rounded-xl overflow-hidden"
      style={{
        background: "var(--bg-card)",
        border: `1px solid ${isSelf ? "var(--border-glow)" : "var(--border-color)"}`,
      }}
    >
      <button
        onClick={toggle}
        className="w-full flex items-center gap-2.5 px-3 py-2.5 cursor-pointer bg-transparent border-none text-left"
      >
        <Medal rank={rank} />
        <div className="flex-1 min-w-0">
          <p className="text-sm font-medium truncate flex items-center gap-1.5">
            {entry.username}
            {isSelf && (
              <span
                className="text-[9px] font-bold px-1.5 py-0.5 rounded"
                style={{ background: "rgba(0,230,118,0.15)", color: "var(--accent-green)" }}
              >
                YOU
              </span>
            )}
            {entry.chips_played.map((chip) => (
              <span
                key={chip}
                className="text-[9px] font-bold px-1.5 py-0.5 rounded"
                style={{ background: "rgba(255,171,0,0.18)", color: "#ffab00" }}
              >
                {CHIP_LABEL[chip] ?? chip}
              </span>
            ))}
          </p>
          <p className="text-xs truncate" style={{ color: "var(--text-muted)" }}>
            {entry.team_name ?? "No team"}
            {entry.transfer_points_hit ? ` · −${entry.transfer_points_hit} hit` : ""}
          </p>
        </div>
        <span
          className="text-lg font-bold tabular-nums"
          style={{ fontFamily: "var(--font-display)", color: "var(--accent-green)" }}
        >
          {entry.total_points ?? "—"}
        </span>
        <motion.span animate={{ rotate: open ? 180 : 0 }} style={{ color: "var(--text-muted)" }}>
          <ChevronDown size={16} />
        </motion.span>
      </button>

      <AnimatePresence initial={false}>
        {open && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: "auto", opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            style={{ overflow: "hidden", borderTop: "1px solid var(--border-color)" }}
          >
            <div className="p-3">
              {loading && (
                <div
                  className="flex items-center gap-2 justify-center py-6 text-xs"
                  style={{ color: "var(--text-muted)" }}
                >
                  <Loader2 size={14} className="animate-spin" />
                  Working out where every point came from…
                </div>
              )}
              {error && (
                <div
                  className="flex items-start gap-2 px-3 py-4 rounded-lg text-xs"
                  style={{
                    background: "var(--bg-secondary)",
                    border: "1px dashed var(--border-color)",
                    color: "var(--text-muted)",
                  }}
                >
                  <EyeOff size={14} className="shrink-0 mt-0.5" />
                  <span>{error}</span>
                </div>
              )}
              {detail && !loading && !error && <GameweekBreakdown data={detail} />}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

/**
 * Who scored what in one gameweek, every row expandable into the arithmetic
 * behind it.
 */
export function GameweekScoreboard({
  leagueId,
  week,
  token,
  currentUserId,
}: {
  leagueId: string;
  week: number;
  token: string;
  currentUserId?: string;
}) {
  const [board, setBoard] = useState<Scoreboard | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    getGameweekScoreboard(leagueId, week, token)
      .then((data) => !cancelled && setBoard(data))
      .catch((e) => !cancelled && setError(e instanceof Error ? e.message : "Could not load"))
      .finally(() => !cancelled && setLoading(false));
    return () => {
      cancelled = true;
    };
  }, [leagueId, week, token]);

  if (loading) {
    return (
      <div
        className="flex items-center gap-2 justify-center py-10 text-xs"
        style={{ color: "var(--text-muted)" }}
      >
        <Loader2 size={14} className="animate-spin" />
        Loading gameweek {week}…
      </div>
    );
  }

  if (error || !board) {
    return (
      <p className="text-xs text-center py-10" style={{ color: "var(--text-muted)" }}>
        {error ?? "Could not load this gameweek"}
      </p>
    );
  }

  return (
    <div className="space-y-2">
      {!board.is_complete && (
        <div
          className="flex items-start gap-2 px-3 py-2.5 rounded-lg text-xs"
          style={{
            background: "rgba(255,171,0,0.08)",
            border: "1px solid rgba(255,171,0,0.2)",
            color: "#ffab00",
          }}
        >
          <EyeOff size={14} className="shrink-0 mt-0.5" />
          <span>
            Gameweek {week} is not finished. Lineups stay hidden until it locks — you can
            still open your own.
          </span>
        </div>
      )}

      {board.entries.map((entry, i) => (
        <ScoreboardRow
          key={entry.user_id}
          entry={entry}
          rank={i + 1}
          leagueId={leagueId}
          week={week}
          token={token}
          isSelf={entry.user_id === currentUserId}
        />
      ))}

      {board.entries.length === 0 && (
        <p className="text-xs text-center py-10" style={{ color: "var(--text-muted)" }}>
          No managers in this league yet
        </p>
      )}
    </div>
  );
}
