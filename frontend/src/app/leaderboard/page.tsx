"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { motion } from "framer-motion";
import { Trophy, Crown, Medal, Award, TrendingUp } from "lucide-react";
import Nav from "@/components/nav";
import { getPlayers, type Player } from "@/lib/api";
import { isAuthenticated } from "@/lib/auth";

export default function LeaderboardPage() {
  const router = useRouter();
  const [players, setPlayers] = useState<Player[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (!isAuthenticated()) {
      router.push("/login");
      return;
    }

    getPlayers()
      .then((data) => {
        // Sort by total_points descending for a "top players" leaderboard
        const sorted = data.sort((a, b) => b.total_points - a.total_points);
        setPlayers(sorted);
      })
      .catch(console.error)
      .finally(() => setLoading(false));
  }, [router]);

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

  return (
    <div className="min-h-screen pitch-pattern">
      <Nav />
      <div className="pt-24 pb-12 max-w-3xl mx-auto px-4 sm:px-6 lg:px-8">
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          className="mb-8"
        >
          <div className="flex items-center gap-3 mb-2">
            <Trophy size={28} style={{ color: "var(--accent-amber)" }} />
            <h1 className="text-3xl font-bold" style={{ fontFamily: "var(--font-display)" }}>
              LEADER<span style={{ color: "var(--accent-amber)" }}>BOARD</span>
            </h1>
          </div>
          <p style={{ color: "var(--text-muted)" }}>Top performing players this season</p>
        </motion.div>

        {loading ? (
          <div className="flex items-center justify-center py-20">
            <div className="w-8 h-8 border-2 border-t-transparent rounded-full animate-spin" style={{ borderColor: "var(--accent-green)", borderTopColor: "transparent" }} />
          </div>
        ) : (
          <>
            {/* Top 3 podium */}
            {players.length >= 3 && (
              <motion.div
                initial={{ opacity: 0, y: 20 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.1 }}
                className="grid grid-cols-3 gap-4 mb-10"
              >
                {/* 2nd place */}
                <div className="flex flex-col items-center pt-8">
                  <motion.div
                    initial={{ opacity: 0, scale: 0.5 }}
                    animate={{ opacity: 1, scale: 1 }}
                    transition={{ delay: 0.3 }}
                    className="glass-card p-4 text-center w-full"
                  >
                    <div className="w-12 h-12 rounded-full flex items-center justify-center mx-auto mb-2" style={{ background: "rgba(192, 192, 192, 0.15)", border: "2px solid rgba(192, 192, 192, 0.4)" }}>
                      <Medal size={24} style={{ color: "#c0c0c0" }} />
                    </div>
                    <p className="text-sm font-bold truncate" style={{ fontFamily: "var(--font-display)" }}>{players[1].name.split(" ").pop()}</p>
                    <p className="text-xs mb-2" style={{ color: "var(--text-muted)" }}>{players[1].team_name}</p>
                    <span className="text-xl font-bold" style={{ fontFamily: "var(--font-display)", color: "#c0c0c0" }}>{players[1].total_points}</span>
                  </motion.div>
                </div>

                {/* 1st place */}
                <div className="flex flex-col items-center">
                  <motion.div
                    initial={{ opacity: 0, scale: 0.5 }}
                    animate={{ opacity: 1, scale: 1 }}
                    transition={{ delay: 0.2 }}
                    className="glass-card p-4 text-center w-full glow-amber"
                  >
                    <div className="w-14 h-14 rounded-full flex items-center justify-center mx-auto mb-2" style={{ background: "rgba(255, 215, 0, 0.15)", border: "2px solid rgba(255, 215, 0, 0.4)" }}>
                      <Crown size={28} style={{ color: "#ffd700" }} />
                    </div>
                    <p className="text-base font-bold truncate" style={{ fontFamily: "var(--font-display)" }}>{players[0].name.split(" ").pop()}</p>
                    <p className="text-xs mb-2" style={{ color: "var(--text-muted)" }}>{players[0].team_name}</p>
                    <span className="text-2xl font-bold text-glow-amber" style={{ fontFamily: "var(--font-display)", color: "#ffd700" }}>{players[0].total_points}</span>
                  </motion.div>
                </div>

                {/* 3rd place */}
                <div className="flex flex-col items-center pt-12">
                  <motion.div
                    initial={{ opacity: 0, scale: 0.5 }}
                    animate={{ opacity: 1, scale: 1 }}
                    transition={{ delay: 0.4 }}
                    className="glass-card p-4 text-center w-full"
                  >
                    <div className="w-12 h-12 rounded-full flex items-center justify-center mx-auto mb-2" style={{ background: "rgba(205, 127, 50, 0.15)", border: "2px solid rgba(205, 127, 50, 0.4)" }}>
                      <Award size={24} style={{ color: "#cd7f32" }} />
                    </div>
                    <p className="text-sm font-bold truncate" style={{ fontFamily: "var(--font-display)" }}>{players[2].name.split(" ").pop()}</p>
                    <p className="text-xs mb-2" style={{ color: "var(--text-muted)" }}>{players[2].team_name}</p>
                    <span className="text-xl font-bold" style={{ fontFamily: "var(--font-display)", color: "#cd7f32" }}>{players[2].total_points}</span>
                  </motion.div>
                </div>
              </motion.div>
            )}

            {/* Full rankings list */}
            <div className="glass-card overflow-hidden">
              <div className="px-6 py-4 flex items-center gap-2" style={{ borderBottom: "1px solid var(--border-color)" }}>
                <TrendingUp size={16} style={{ color: "var(--accent-green)" }} />
                <h3 className="text-sm uppercase tracking-wider" style={{ fontFamily: "var(--font-display)", color: "var(--text-muted)" }}>
                  Full Rankings
                </h3>
              </div>

              <div>
                {players.map((player, i) => (
                  <motion.div
                    key={player.id}
                    initial={{ opacity: 0, x: -10 }}
                    animate={{ opacity: 1, x: 0 }}
                    transition={{ delay: 0.05 * Math.min(i, 15) }}
                    className="flex items-center gap-4 px-6 py-3"
                    style={{
                      background: getRankBg(i),
                      borderBottom: getRankBorder(i),
                    }}
                  >
                    <div className="w-8 h-8 rounded-full flex items-center justify-center" style={{
                      background: i < 3 ? "transparent" : "var(--bg-secondary)",
                    }}>
                      {getRankIcon(i)}
                    </div>

                    <span className={`badge-${player.position.toLowerCase()} text-[10px] font-bold px-2 py-0.5 rounded-full text-white`}>
                      {player.position}
                    </span>

                    <div className="flex-1 min-w-0">
                      <p className="text-sm font-medium truncate">{player.name}</p>
                      <p className="text-xs" style={{ color: "var(--text-muted)" }}>{player.team_name}</p>
                    </div>

                    <div className="text-right">
                      <span className="text-lg font-bold" style={{ fontFamily: "var(--font-display)", color: "var(--accent-green)" }}>
                        {player.total_points}
                      </span>
                      <p className="text-[10px] uppercase" style={{ color: "var(--text-muted)", fontFamily: "var(--font-display)" }}>
                        pts
                      </p>
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
