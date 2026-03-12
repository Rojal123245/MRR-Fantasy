"use client";

import { useEffect, useState, useCallback } from "react";
import { useRouter } from "next/navigation";
import { motion } from "framer-motion";
import { Shield, Save, Plus, ChevronDown, AlertCircle, Check, Loader2 } from "lucide-react";
import Nav from "@/components/nav";
import {
  createGameweek,
  getWeekStatsAdmin,
  submitWeekStats,
  type AdminPlayerStats,
  type PlayerStatInput,
} from "@/lib/api";
import { getToken, getUser, isAuthenticated } from "@/lib/auth";

const STAT_FIELDS = [
  { key: "goals", label: "G", title: "Goals" },
  { key: "assists", label: "A", title: "Assists" },
  { key: "clean_sheets", label: "CS", title: "Clean Sheets" },
  { key: "saves", label: "SV", title: "Saves" },
  { key: "penalty_saves", label: "PS", title: "Penalty Saves" },
  { key: "own_goals", label: "OG", title: "Own Goals" },
  { key: "penalty_misses", label: "PM", title: "Penalty Misses" },
  { key: "regular_fouls", label: "RF", title: "Regular Fouls" },
  { key: "serious_fouls", label: "SF", title: "Serious Fouls" },
  { key: "minutes_played", label: "MIN", title: "Minutes Played" },
] as const;

type StatKey = (typeof STAT_FIELDS)[number]["key"];

const POS_COLORS: Record<string, string> = {
  GK: "var(--accent-amber)",
  DEF: "#60a5fa",
  MID: "var(--accent-green)",
  FWD: "#f87171",
};

export default function AdminPage() {
  const router = useRouter();
  const [weekNumber, setWeekNumber] = useState(1);
  const [players, setPlayers] = useState<AdminPlayerStats[]>([]);
  const [edits, setEdits] = useState<Record<string, Record<StatKey, number>>>({});
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const [success, setSuccess] = useState("");
  const [creating, setCreating] = useState(false);

  useEffect(() => {
    if (!isAuthenticated()) {
      router.push("/login");
      return;
    }
    const user = getUser();
    if (!user?.is_admin) {
      router.push("/dashboard");
      return;
    }
  }, [router]);

  const loadStats = useCallback(async () => {
    const token = getToken();
    if (!token) return;
    setLoading(true);
    setError("");
    try {
      const data = await getWeekStatsAdmin(weekNumber, token);
      setPlayers(data);
      const initial: Record<string, Record<StatKey, number>> = {};
      for (const p of data) {
        initial[p.player_id] = {
          goals: p.goals,
          assists: p.assists,
          clean_sheets: p.clean_sheets,
          saves: p.saves,
          penalty_saves: p.penalty_saves,
          own_goals: p.own_goals,
          penalty_misses: p.penalty_misses,
          regular_fouls: p.regular_fouls,
          serious_fouls: p.serious_fouls,
          minutes_played: p.minutes_played,
        };
      }
      setEdits(initial);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : "Failed to load stats");
    } finally {
      setLoading(false);
    }
  }, [weekNumber]);

  useEffect(() => {
    loadStats();
  }, [loadStats]);

  const handleCreate = async () => {
    const token = getToken();
    if (!token) return;
    setCreating(true);
    setError("");
    try {
      const today = new Date().toISOString().split("T")[0];
      await createGameweek(weekNumber, today, today, token);
      setSuccess(`Gameweek ${weekNumber} created`);
      setTimeout(() => setSuccess(""), 3000);
      await loadStats();
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : "Failed to create gameweek");
    } finally {
      setCreating(false);
    }
  };

  const updateStat = (playerId: string, field: StatKey, value: number) => {
    setEdits((prev) => ({
      ...prev,
      [playerId]: {
        ...prev[playerId],
        [field]: value,
      },
    }));
  };

  const handleSubmit = async () => {
    const token = getToken();
    if (!token) return;
    setSaving(true);
    setError("");
    setSuccess("");
    try {
      const stats: PlayerStatInput[] = Object.entries(edits).map(
        ([player_id, s]) => ({
          player_id,
          goals: s.goals,
          assists: s.assists,
          clean_sheets: s.clean_sheets,
          saves: s.saves,
          penalty_saves: s.penalty_saves,
          own_goals: s.own_goals,
          penalty_misses: s.penalty_misses,
          regular_fouls: s.regular_fouls,
          serious_fouls: s.serious_fouls,
          minutes_played: s.minutes_played,
        })
      );
      await submitWeekStats(weekNumber, stats, token);
      setSuccess("Stats saved and points recalculated!");
      setTimeout(() => setSuccess(""), 4000);
      await loadStats();
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : "Failed to save stats");
    } finally {
      setSaving(false);
    }
  };

  const grouped = players.reduce<Record<string, AdminPlayerStats[]>>(
    (acc, p) => {
      const pos = p.position;
      if (!acc[pos]) acc[pos] = [];
      acc[pos].push(p);
      return acc;
    },
    {}
  );

  const posOrder = ["GK", "DEF", "MID", "FWD"];

  return (
    <div className="min-h-screen pitch-pattern">
      <Nav />
      <div className="pt-24 pb-32 max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        {/* Header */}
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          className="mb-8"
        >
          <div className="flex items-center gap-3 mb-2">
            <Shield size={28} style={{ color: "var(--accent-green)" }} />
            <h1
              className="text-3xl font-bold tracking-tight"
              style={{ fontFamily: "var(--font-display)" }}
            >
              Admin Panel
            </h1>
          </div>
          <p style={{ color: "var(--text-muted)", fontFamily: "var(--font-body)" }}>
            Enter match stats for each gameweek. Points are calculated automatically.
          </p>
        </motion.div>

        {/* Gameweek controls */}
        <motion.div
          initial={{ opacity: 0, y: 10 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.1 }}
          className="flex flex-wrap items-center gap-4 mb-8 p-5 rounded-2xl"
          style={{
            background: "var(--bg-card)",
            border: "1px solid var(--border-color)",
          }}
        >
          <div className="flex items-center gap-2">
            <label
              className="text-sm font-medium"
              style={{ color: "var(--text-secondary)", fontFamily: "var(--font-body)" }}
            >
              Gameweek
            </label>
            <div className="relative">
              <select
                value={weekNumber}
                onChange={(e) => setWeekNumber(Number(e.target.value))}
                className="appearance-none px-4 py-2 pr-8 rounded-xl text-sm font-medium cursor-pointer"
                style={{
                  background: "var(--bg-elevated)",
                  color: "var(--text-primary)",
                  border: "1px solid var(--border-color)",
                  fontFamily: "var(--font-body)",
                }}
              >
                {Array.from({ length: 30 }, (_, i) => i + 1).map((w) => (
                  <option key={w} value={w}>
                    GW {w}
                  </option>
                ))}
              </select>
              <ChevronDown
                size={14}
                className="absolute right-2.5 top-1/2 -translate-y-1/2 pointer-events-none"
                style={{ color: "var(--text-muted)" }}
              />
            </div>
          </div>

          <button
            onClick={handleCreate}
            disabled={creating}
            className="flex items-center gap-2 px-4 py-2 rounded-xl text-sm font-semibold cursor-pointer border-none transition-all"
            style={{
              background: "rgba(0, 230, 118, 0.1)",
              color: "var(--accent-green)",
              fontFamily: "var(--font-body)",
              opacity: creating ? 0.6 : 1,
            }}
          >
            {creating ? <Loader2 size={14} className="animate-spin" /> : <Plus size={14} />}
            Create / Activate GW {weekNumber}
          </button>
        </motion.div>

        {/* Status messages */}
        {error && (
          <div
            className="flex items-center gap-2 mb-6 px-4 py-3 rounded-xl text-sm"
            style={{
              background: "rgba(239, 68, 68, 0.1)",
              color: "var(--danger)",
              border: "1px solid rgba(239, 68, 68, 0.2)",
            }}
          >
            <AlertCircle size={16} />
            {error}
          </div>
        )}
        {success && (
          <div
            className="flex items-center gap-2 mb-6 px-4 py-3 rounded-xl text-sm"
            style={{
              background: "rgba(0, 230, 118, 0.1)",
              color: "var(--accent-green)",
              border: "1px solid rgba(0, 230, 118, 0.2)",
            }}
          >
            <Check size={16} />
            {success}
          </div>
        )}

        {/* Stats table */}
        {loading ? (
          <div className="flex items-center justify-center py-20">
            <Loader2 size={32} className="animate-spin" style={{ color: "var(--accent-green)" }} />
          </div>
        ) : (
          <div className="space-y-6">
            {posOrder.map((pos) => {
              const group = grouped[pos];
              if (!group || group.length === 0) return null;
              return (
                <motion.div
                  key={pos}
                  initial={{ opacity: 0, y: 10 }}
                  animate={{ opacity: 1, y: 0 }}
                  className="rounded-2xl overflow-hidden"
                  style={{
                    background: "var(--bg-card)",
                    border: "1px solid var(--border-color)",
                  }}
                >
                  {/* Position header */}
                  <div
                    className="px-5 py-3 flex items-center gap-2"
                    style={{
                      background: `${POS_COLORS[pos]}10`,
                      borderBottom: `1px solid ${POS_COLORS[pos]}20`,
                    }}
                  >
                    <div
                      className="w-2 h-2 rounded-full"
                      style={{ background: POS_COLORS[pos] }}
                    />
                    <span
                      className="text-sm font-bold tracking-wider"
                      style={{
                        color: POS_COLORS[pos],
                        fontFamily: "var(--font-display)",
                      }}
                    >
                      {pos === "GK"
                        ? "GOALKEEPERS"
                        : pos === "DEF"
                          ? "DEFENDERS"
                          : pos === "MID"
                            ? "MIDFIELDERS"
                            : "FORWARDS"}
                    </span>
                  </div>

                  {/* Table */}
                  <div className="overflow-x-auto">
                    <table className="w-full" style={{ fontFamily: "var(--font-body)" }}>
                      <thead>
                        <tr
                          style={{
                            borderBottom: "1px solid var(--border-color)",
                          }}
                        >
                          <th
                            className="text-left px-4 py-3 text-xs font-semibold tracking-wider"
                            style={{ color: "var(--text-muted)", minWidth: 160 }}
                          >
                            PLAYER
                          </th>
                          {STAT_FIELDS.map((f) => (
                            <th
                              key={f.key}
                              className="px-2 py-3 text-center text-xs font-semibold tracking-wider"
                              style={{ color: "var(--text-muted)", minWidth: 52 }}
                              title={f.title}
                            >
                              {f.label}
                            </th>
                          ))}
                        </tr>
                      </thead>
                      <tbody>
                        {group.map((player) => (
                          <tr
                            key={player.player_id}
                            style={{
                              borderBottom: "1px solid var(--border-color)",
                            }}
                            className="hover:bg-white/2 transition-colors"
                          >
                            <td className="px-4 py-3">
                              <span
                                className="text-sm font-medium"
                                style={{ color: "var(--text-primary)" }}
                              >
                                {player.player_name}
                              </span>
                            </td>
                            {STAT_FIELDS.map((f) => (
                              <td key={f.key} className="px-1 py-2 text-center">
                                <input
                                  type="number"
                                  min={0}
                                  value={edits[player.player_id]?.[f.key] ?? 0}
                                  onChange={(e) =>
                                    updateStat(
                                      player.player_id,
                                      f.key,
                                      parseInt(e.target.value) || 0
                                    )
                                  }
                                  className="w-12 text-center text-sm py-1.5 rounded-lg border-none outline-none focus:ring-1"
                                  style={{
                                    background: "var(--bg-elevated)",
                                    color: "var(--text-primary)",
                                    fontFamily: "var(--font-body)",
                                    // @ts-expect-error CSS custom property
                                    "--tw-ring-color": "var(--accent-green)",
                                  }}
                                />
                              </td>
                            ))}
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                </motion.div>
              );
            })}
          </div>
        )}
      </div>

      {/* Sticky submit bar */}
      {!loading && players.length > 0 && (
        <div
          className="fixed bottom-0 left-0 right-0 z-40"
          style={{
            background: "rgba(0, 0, 0, 0.8)",
            backdropFilter: "blur(20px)",
            borderTop: "1px solid var(--border-color)",
          }}
        >
          <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-4 flex items-center justify-between">
            <span
              className="text-sm"
              style={{ color: "var(--text-muted)", fontFamily: "var(--font-body)" }}
            >
              {players.length} players · Gameweek {weekNumber}
            </span>
            <button
              onClick={handleSubmit}
              disabled={saving}
              className="flex items-center gap-2 px-6 py-2.5 rounded-xl text-sm font-bold cursor-pointer border-none transition-all"
              style={{
                background: saving
                  ? "var(--accent-green-dim)"
                  : "var(--accent-green)",
                color: "#000",
                fontFamily: "var(--font-display)",
                opacity: saving ? 0.7 : 1,
              }}
            >
              {saving ? (
                <Loader2 size={16} className="animate-spin" />
              ) : (
                <Save size={16} />
              )}
              {saving ? "Saving..." : "Save & Calculate Points"}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
