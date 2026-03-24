"use client";

import type { ReactNode } from "react";
import { useEffect, useMemo, useState } from "react";
import { useRouter } from "next/navigation";
import { motion } from "framer-motion";
import {
  Trophy,
  Crown,
  Medal,
  Award,
  TrendingUp,
  Users,
  Target,
  Share2,
  Shield,
} from "lucide-react";
import Nav from "@/components/nav";
import { getPlayerLeaderboard, type PlayerLeaderboard } from "@/lib/api";
import { isAuthenticated } from "@/lib/auth";

type PositionFilter = "ALL" | "GK" | "DEF" | "MID" | "FWD";

type LeaderboardTab = "overall" | "goals" | "assists" | "gk_cs";

const MAIN_TABS: { id: LeaderboardTab; label: string; hint: string; icon: ReactNode }[] = [
  {
    id: "overall",
    label: "Overall",
    hint: "Ownership and season fantasy points",
    icon: <TrendingUp size={14} />,
  },
  {
    id: "goals",
    label: "Top goalscorers",
    hint: "Ranked by total goals",
    icon: <Target size={14} />,
  },
  {
    id: "assists",
    label: "Top assisters",
    hint: "Ranked by total assists",
    icon: <Share2 size={14} />,
  },
  {
    id: "gk_cs",
    label: "Top GK (clean sheets)",
    hint: "Goalkeepers by clean sheets",
    icon: <Shield size={14} />,
  },
];

function StatsForPosition({ player }: { player: PlayerLeaderboard }) {
  switch (player.position) {
    case "GK":
      return (
        <div className="flex items-center flex-wrap gap-1.5 sm:gap-3 text-[10px] sm:text-xs justify-end">
          <StatPill label="G" value={player.goals} />
          <StatPill label="A" value={player.assists} />
          <StatPill label="CS" value={player.clean_sheets} />
          <StatPill label="SV" value={player.saves} />
        </div>
      );
    case "DEF":
      return (
        <div className="flex items-center flex-wrap gap-1.5 sm:gap-3 text-[10px] sm:text-xs justify-end">
          <StatPill label="G" value={player.goals} />
          <StatPill label="A" value={player.assists} />
          <StatPill label="CS" value={player.clean_sheets} />
        </div>
      );
    case "MID":
    case "FWD":
    default:
      return (
        <div className="flex items-center flex-wrap gap-1.5 sm:gap-3 text-[10px] sm:text-xs justify-end">
          <StatPill label="G" value={player.goals} />
          <StatPill label="A" value={player.assists} />
        </div>
      );
  }
}

function RowHighlightStat({
  tab,
  player,
}: {
  tab: LeaderboardTab;
  player: PlayerLeaderboard;
}) {
  if (tab === "goals") {
    return (
      <span
        className="flex items-center gap-1 px-2 py-0.5 rounded-md font-bold text-xs shrink-0"
        style={{ background: "rgba(255, 193, 7, 0.12)", fontFamily: "var(--font-display)" }}
      >
        <Target size={12} style={{ color: "var(--accent-amber)" }} />
        <span style={{ color: "var(--text-muted)" }}>G</span>
        <span style={{ color: "var(--text-primary)" }}>{player.goals}</span>
      </span>
    );
  }
  if (tab === "assists") {
    return (
      <span
        className="flex items-center gap-1 px-2 py-0.5 rounded-md font-bold text-xs shrink-0"
        style={{ background: "rgba(0, 230, 118, 0.1)", fontFamily: "var(--font-display)" }}
      >
        <Share2 size={12} style={{ color: "var(--accent-green)" }} />
        <span style={{ color: "var(--text-muted)" }}>A</span>
        <span style={{ color: "var(--text-primary)" }}>{player.assists}</span>
      </span>
    );
  }
  if (tab === "gk_cs") {
    return (
      <span
        className="flex items-center gap-1 px-2 py-0.5 rounded-md font-bold text-xs shrink-0"
        style={{ background: "rgba(100, 181, 246, 0.12)", fontFamily: "var(--font-display)" }}
      >
        <Shield size={12} style={{ color: "#64b5f6" }} />
        <span style={{ color: "var(--text-muted)" }}>CS</span>
        <span style={{ color: "var(--text-primary)" }}>{player.clean_sheets}</span>
      </span>
    );
  }
  return null;
}

function StatPill({ label, value }: { label: string; value: number }) {
  return (
    <span
      className="flex items-center gap-1 px-1.5 sm:px-2 py-0.5 rounded-md font-bold leading-none"
      style={{ background: "var(--bg-secondary)", fontFamily: "var(--font-display)" }}
    >
      <span className="text-[8px] sm:text-[9px]" style={{ color: "var(--text-muted)" }}>{label}</span>
      <span className="text-[10px] sm:text-xs" style={{ color: "var(--text-primary)" }}>{value}</span>
    </span>
  );
}

function PodiumStats({
  player,
  tab,
}: {
  player: PlayerLeaderboard;
  tab: LeaderboardTab;
}) {
  if (tab === "goals") {
    return (
      <div className="flex flex-wrap justify-center gap-1 mt-1">
        <MiniStat label="Goals" value={player.goals} />
        <MiniStat label="Pts" value={player.total_points} />
      </div>
    );
  }
  if (tab === "assists") {
    return (
      <div className="flex flex-wrap justify-center gap-1 mt-1">
        <MiniStat label="Assists" value={player.assists} />
        <MiniStat label="Pts" value={player.total_points} />
      </div>
    );
  }
  if (tab === "gk_cs") {
    return (
      <div className="flex flex-wrap justify-center gap-1 mt-1">
        <MiniStat label="Clean sheets" value={player.clean_sheets} />
        <MiniStat label="Pts" value={player.total_points} />
      </div>
    );
  }
  switch (player.position) {
    case "GK":
      return (
        <div className="flex flex-wrap justify-center gap-1 mt-1">
          <MiniStat label="Goals" value={player.goals} />
          <MiniStat label="Assists" value={player.assists} />
          <MiniStat label="CS" value={player.clean_sheets} />
          <MiniStat label="Saves" value={player.saves} />
        </div>
      );
    case "DEF":
      return (
        <div className="flex flex-wrap justify-center gap-1 mt-1">
          <MiniStat label="Goals" value={player.goals} />
          <MiniStat label="Assists" value={player.assists} />
          <MiniStat label="CS" value={player.clean_sheets} />
        </div>
      );
    default:
      return (
        <div className="flex flex-wrap justify-center gap-1 mt-1">
          <MiniStat label="Goals" value={player.goals} />
          <MiniStat label="Assists" value={player.assists} />
        </div>
      );
  }
}

function MiniStat({ label, value }: { label: string; value: number }) {
  return (
    <span className="text-[10px]" style={{ color: "var(--text-muted)" }}>
      {label}: <span className="font-bold" style={{ color: "var(--text-primary)" }}>{value}</span>
    </span>
  );
}

export default function LeaderboardPage() {
  const router = useRouter();
  const [players, setPlayers] = useState<PlayerLeaderboard[]>([]);
  const [loading, setLoading] = useState(true);
  const [posFilter, setPosFilter] = useState<PositionFilter>("ALL");
  const [mainTab, setMainTab] = useState<LeaderboardTab>("overall");

  useEffect(() => {
    if (!isAuthenticated()) {
      router.push("/login");
      return;
    }

    setLoading(true);
    const pos =
      mainTab === "overall" && posFilter !== "ALL" ? posFilter : undefined;
    getPlayerLeaderboard(pos)
      .then(setPlayers)
      .catch(console.error)
      .finally(() => setLoading(false));
  }, [router, posFilter, mainTab]);

  const displayPlayers = useMemo(() => {
    const list = [...players];
    if (mainTab === "overall") return list;
    const byName = (a: PlayerLeaderboard, b: PlayerLeaderboard) =>
      a.name.localeCompare(b.name);
    if (mainTab === "goals") {
      return list.sort(
        (a, b) =>
          b.goals - a.goals ||
          b.total_points - a.total_points ||
          byName(a, b),
      );
    }
    if (mainTab === "assists") {
      return list.sort(
        (a, b) =>
          b.assists - a.assists ||
          b.total_points - a.total_points ||
          byName(a, b),
      );
    }
    return list
      .filter((p) => p.position === "GK")
      .sort(
        (a, b) =>
          b.clean_sheets - a.clean_sheets ||
          b.total_points - a.total_points ||
          byName(a, b),
      );
  }, [players, mainTab]);

  const activeHint =
    MAIN_TABS.find((t) => t.id === mainTab)?.hint ??
    "Top performing players this season";

  const getRankIcon = (rank: number) => {
    switch (rank) {
      case 0: return <Crown size={20} style={{ color: "#ffd700" }} />;
      case 1: return <Medal size={20} style={{ color: "#c0c0c0" }} />;
      case 2: return <Award size={20} style={{ color: "#cd7f32" }} />;
      default: return <span className="text-sm font-bold" style={{ fontFamily: "var(--font-display)", color: "var(--text-muted)" }}>{rank + 1}</span>;
    }
  };

  const getRankBorder = (rank: number) => {
    switch (rank) {
      case 0: return "1px solid rgba(255, 215, 0, 0.3)";
      case 1: return "1px solid rgba(192, 192, 192, 0.3)";
      case 2: return "1px solid rgba(205, 127, 50, 0.3)";
      default: return "1px solid var(--border-color)";
    }
  };

  const getRankBg = (rank: number) => {
    switch (rank) {
      case 0: return "rgba(255, 215, 0, 0.04)";
      case 1: return "rgba(192, 192, 192, 0.04)";
      case 2: return "rgba(205, 127, 50, 0.04)";
      default: return "var(--bg-elevated)";
    }
  };

  const podiumColors = ["#ffd700", "#c0c0c0", "#cd7f32"];
  const podiumIcons = [
    <Crown key="crown" size={28} style={{ color: "#ffd700" }} />,
    <Medal key="medal" size={24} style={{ color: "#c0c0c0" }} />,
    <Award key="award" size={24} style={{ color: "#cd7f32" }} />,
  ];
  const podiumOrder = [1, 0, 2];

  return (
    <div className="min-h-screen pitch-pattern">
      <Nav />
      <div className="pt-24 pb-12 max-w-3xl mx-auto px-4 sm:px-6 lg:px-8">
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          className="mb-6"
        >
          <div className="flex items-center gap-3 mb-2">
            <Trophy size={28} style={{ color: "var(--accent-amber)" }} />
            <h1 className="text-3xl font-bold" style={{ fontFamily: "var(--font-display)" }}>
              LEADER<span style={{ color: "var(--accent-amber)" }}>BOARD</span>
            </h1>
          </div>
          <p style={{ color: "var(--text-muted)" }}>{activeHint}</p>
        </motion.div>

        {/* Main leaderboard tabs */}
        <div className="flex gap-2 mb-4 flex-wrap">
          {MAIN_TABS.map((tab) => (
            <button
              key={tab.id}
              type="button"
              onClick={() => setMainTab(tab.id)}
              className="px-3 sm:px-4 py-2 rounded-lg text-[10px] sm:text-xs font-bold transition-all cursor-pointer border-none flex items-center gap-1.5"
              style={{
                fontFamily: "var(--font-display)",
                background:
                  mainTab === tab.id ? "var(--accent-amber)" : "var(--bg-secondary)",
                color:
                  mainTab === tab.id ? "var(--bg-primary)" : "var(--text-muted)",
              }}
            >
              {tab.icon}
              {tab.label}
            </button>
          ))}
        </div>

        {/* Position filter (overall only) */}
        {mainTab === "overall" && (
          <div className="flex gap-2 mb-8 flex-wrap">
            {(["ALL", "GK", "DEF", "MID", "FWD"] as PositionFilter[]).map((pos) => (
              <button
                key={pos}
                type="button"
                onClick={() => setPosFilter(pos)}
                className="px-4 py-2 rounded-lg text-xs font-bold transition-all cursor-pointer border-none"
                style={{
                  fontFamily: "var(--font-display)",
                  background: posFilter === pos ? "var(--accent-green)" : "var(--bg-secondary)",
                  color: posFilter === pos ? "var(--bg-primary)" : "var(--text-muted)",
                }}
              >
                {pos}
              </button>
            ))}
          </div>
        )}

        {mainTab !== "overall" && <div className="mb-8" />}

        {loading ? (
          <div className="flex items-center justify-center py-20">
            <div className="w-8 h-8 border-2 border-t-transparent rounded-full animate-spin" style={{ borderColor: "var(--accent-green)", borderTopColor: "transparent" }} />
          </div>
        ) : displayPlayers.length === 0 ? (
          <div
            className="glass-card p-8 text-center text-sm"
            style={{ color: "var(--text-muted)" }}
          >
            {mainTab === "gk_cs"
              ? "No goalkeepers in the player pool for this leaderboard."
              : "No players to show yet."}
          </div>
        ) : (
          <>
            {/* Top 3 podium */}
            {displayPlayers.length >= 3 && (
              <motion.div
                initial={{ opacity: 0, y: 20 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.1 }}
                className="grid grid-cols-3 gap-4 mb-10"
              >
                {podiumOrder.map((idx) => {
                  const player = displayPlayers[idx];
                  const isFirst = idx === 0;
                  return (
                    <div key={player.id} className={`flex flex-col items-center ${idx === 1 ? "pt-8" : idx === 2 ? "pt-12" : ""}`}>
                      <motion.div
                        initial={{ opacity: 0, scale: 0.5 }}
                        animate={{ opacity: 1, scale: 1 }}
                        transition={{ delay: 0.2 + idx * 0.1 }}
                        className={`glass-card p-4 text-center w-full ${isFirst ? "glow-amber" : ""}`}
                      >
                        <div
                          className={`${isFirst ? "w-14 h-14" : "w-12 h-12"} rounded-full flex items-center justify-center mx-auto mb-2`}
                          style={{ background: `${podiumColors[idx]}22`, border: `2px solid ${podiumColors[idx]}66` }}
                        >
                          {podiumIcons[idx]}
                        </div>
                        <p className={`${isFirst ? "text-base" : "text-sm"} font-bold truncate`} style={{ fontFamily: "var(--font-display)" }}>
                          {player.name.split(" ").pop()}
                        </p>
                        <p className="text-xs mb-1" style={{ color: "var(--text-muted)" }}>{player.team_name}</p>
                        <span
                          className={`badge-${player.position.toLowerCase()} text-[9px] font-bold px-1.5 py-0.5 rounded-full text-white`}
                        >
                          {player.position}
                        </span>
                        <PodiumStats player={player} tab={mainTab} />
                        <div className="flex items-center justify-center gap-1 mt-2">
                          <Users size={10} style={{ color: "var(--text-muted)" }} />
                          <span className="text-[10px]" style={{ color: "var(--text-muted)" }}>
                            {player.chosen_by_percent.toFixed(0)}% owned
                          </span>
                        </div>
                      </motion.div>
                    </div>
                  );
                })}
              </motion.div>
            )}

            {/* Full rankings list */}
            <div className="glass-card overflow-hidden">
              <div className="px-4 sm:px-6 py-3 sm:py-4 flex items-center gap-2" style={{ borderBottom: "1px solid var(--border-color)" }}>
                <TrendingUp size={16} style={{ color: "var(--accent-green)" }} />
                <h3 className="text-sm uppercase tracking-wider" style={{ fontFamily: "var(--font-display)", color: "var(--text-muted)" }}>
                  Full Rankings
                </h3>
              </div>

              <div>
                {displayPlayers.map((player, i) => (
                  <motion.div
                    key={player.id}
                    initial={{ opacity: 0, x: -10 }}
                    animate={{ opacity: 1, x: 0 }}
                    transition={{ delay: 0.05 * Math.min(i, 15) }}
                    className="flex flex-col sm:flex-row sm:items-center gap-2 sm:gap-4 px-3 sm:px-6 py-3"
                    style={{
                      background: getRankBg(i),
                      borderBottom: getRankBorder(i),
                    }}
                  >
                    <div className="flex items-center gap-2 sm:gap-4 w-full min-w-0">
                      <div className="w-7 h-7 sm:w-8 sm:h-8 rounded-full flex items-center justify-center shrink-0" style={{
                        background: i < 3 ? "transparent" : "var(--bg-secondary)",
                      }}>
                        {getRankIcon(i)}
                      </div>

                      <span className={`badge-${player.position.toLowerCase()} text-[10px] font-bold px-2 py-0.5 rounded-full text-white shrink-0`}>
                        {player.position}
                      </span>

                      <div className="flex-1 min-w-0">
                        <p className="text-sm font-medium truncate">{player.name}</p>
                        <p className="text-xs truncate" style={{ color: "var(--text-muted)" }}>{player.team_name}</p>
                      </div>
                    </div>

                    <div className="w-full sm:w-auto sm:ml-auto flex items-center justify-between sm:justify-end gap-2 sm:gap-3">
                      <div className="flex items-center flex-wrap gap-1.5 sm:gap-2 justify-end">
                        {mainTab === "overall" ? (
                          <StatsForPosition player={player} />
                        ) : (
                          <>
                            <RowHighlightStat tab={mainTab} player={player} />
                            <StatPill label="Pts" value={player.total_points} />
                          </>
                        )}
                      </div>

                      <div
                        className="flex items-center gap-1 px-2 py-1 rounded-md shrink-0"
                        style={{ background: "rgba(0, 230, 118, 0.08)" }}
                      >
                        <Users size={10} style={{ color: "var(--text-muted)" }} />
                        <span className="text-[10px] font-bold" style={{ fontFamily: "var(--font-display)", color: "var(--accent-green)" }}>
                          {player.chosen_by_percent.toFixed(0)}%
                        </span>
                      </div>
                    </div>
                  </motion.div>
                ))}
              </div>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
