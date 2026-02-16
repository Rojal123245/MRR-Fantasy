"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import Link from "next/link";
import { motion } from "framer-motion";
import { Shield, Users, Trophy, Zap, ChevronRight, Plus } from "lucide-react";
import Nav from "@/components/nav";
import PointsBadge from "@/components/points-badge";
import { getMyTeam, type FantasyTeam } from "@/lib/api";
import { getToken, getUser, isAuthenticated } from "@/lib/auth";

export default function DashboardPage() {
  const router = useRouter();
  const [team, setTeam] = useState<FantasyTeam | null>(null);
  const [loading, setLoading] = useState(true);
  const [noTeam, setNoTeam] = useState(false);

  useEffect(() => {
    if (!isAuthenticated()) {
      router.push("/login");
      return;
    }

    const token = getToken();
    if (!token) return;

    getMyTeam(token)
      .then(setTeam)
      .catch(() => setNoTeam(true))
      .finally(() => setLoading(false));
  }, [router]);

  const user = getUser();

  return (
    <div className="min-h-screen pitch-pattern">
      <Nav />
      <div className="pt-24 pb-12 max-w-6xl mx-auto px-4 sm:px-6 lg:px-8">
        {/* Welcome header */}
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          className="mb-10"
        >
          <h1 className="text-3xl sm:text-4xl font-bold mb-2" style={{ fontFamily: "var(--font-display)" }}>
            WELCOME BACK, <span style={{ color: "var(--accent-green)" }}>{user?.username?.toUpperCase() || "MANAGER"}</span>
          </h1>
          <p style={{ color: "var(--text-muted)" }}>Here&apos;s your fantasy overview</p>
        </motion.div>

        {loading ? (
          <div className="flex items-center justify-center py-20">
            <div className="w-8 h-8 border-2 border-t-transparent rounded-full animate-spin" style={{ borderColor: "var(--accent-green)", borderTopColor: "transparent" }} />
          </div>
        ) : (
          <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
            {/* Team Overview */}
            <motion.div
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ delay: 0.1 }}
              className="lg:col-span-2"
            >
              <div className="glass-card p-6">
                <div className="flex items-center justify-between mb-6">
                  <div className="flex items-center gap-3">
                    <div className="w-10 h-10 rounded-xl flex items-center justify-center" style={{ background: "rgba(0, 230, 118, 0.1)" }}>
                      <Shield size={20} style={{ color: "var(--accent-green)" }} />
                    </div>
                    <div>
                      <h2 className="text-lg font-bold" style={{ fontFamily: "var(--font-display)" }}>
                        {team ? team.name : "MY TEAM"}
                      </h2>
                      <p className="text-xs" style={{ color: "var(--text-muted)" }}>
                        {team ? `${team.players.length}/6 starters, ${team.bench?.length || 0}/3 bench` : "No team yet"}
                      </p>
                    </div>
                  </div>
                  <Link href="/team" className="btn-secondary text-xs py-2 px-4 no-underline flex items-center gap-1">
                    {team ? "Edit Team" : "Build Team"}
                    <ChevronRight size={14} />
                  </Link>
                </div>

                {noTeam || !team ? (
                  <div className="text-center py-12">
                    <div className="w-16 h-16 rounded-2xl flex items-center justify-center mx-auto mb-4" style={{ background: "rgba(0, 230, 118, 0.08)" }}>
                      <Plus size={32} style={{ color: "var(--accent-green)" }} />
                    </div>
                    <p className="text-lg font-bold mb-2" style={{ fontFamily: "var(--font-display)" }}>No Team Yet</p>
                    <p className="text-sm mb-6" style={{ color: "var(--text-muted)" }}>Build your 9-player squad (6 starters + 3 bench) to start earning points</p>
                    <Link href="/team" className="btn-primary text-sm no-underline inline-flex items-center gap-2">
                      Build Your Team
                      <ChevronRight size={16} />
                    </Link>
                  </div>
                ) : (
                  <div className="grid grid-cols-2 sm:grid-cols-3 gap-3">
                    {team.players.map((player, i) => (
                      <motion.div
                        key={player.id}
                        initial={{ opacity: 0, scale: 0.9 }}
                        animate={{ opacity: 1, scale: 1 }}
                        transition={{ delay: 0.1 + i * 0.05 }}
                        className="rounded-xl p-3 flex items-center gap-3"
                        style={{ background: "var(--bg-elevated)", border: "1px solid var(--border-color)" }}
                      >
                        <span className={`badge-${(player.assigned_position || player.position).toLowerCase()} text-[10px] font-bold px-2 py-0.5 rounded-full text-white`}>
                          {player.assigned_position || player.position}
                        </span>
                        <div className="flex-1 min-w-0">
                          <p className="text-sm font-medium truncate">{player.name}</p>
                          <p className="text-xs" style={{ color: "var(--text-muted)" }}>{player.team_name}</p>
                        </div>
                        <span className="text-sm font-bold" style={{ fontFamily: "var(--font-display)", color: "var(--accent-amber)" }}>
                          {player.total_points}
                        </span>
                      </motion.div>
                    ))}
                  </div>
                )}
              </div>
            </motion.div>

            {/* Points & Quick Links */}
            <div className="space-y-6">
              <motion.div
                initial={{ opacity: 0, y: 20 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.2 }}
              >
                <PointsBadge
                  points={team?.total_points || 0}
                  label="Total Points"
                  size="lg"
                  variant="green"
                />
              </motion.div>

              <motion.div
                initial={{ opacity: 0, y: 20 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.3 }}
                className="space-y-3"
              >
                <Link href="/league" className="glass-card p-4 flex items-center gap-3 no-underline group" style={{ display: "flex" }}>
                  <div className="w-10 h-10 rounded-xl flex items-center justify-center" style={{ background: "rgba(255, 171, 0, 0.1)" }}>
                    <Users size={18} style={{ color: "var(--accent-amber)" }} />
                  </div>
                  <div className="flex-1">
                    <p className="text-sm font-bold" style={{ fontFamily: "var(--font-display)", color: "var(--text-primary)" }}>Leagues</p>
                    <p className="text-xs" style={{ color: "var(--text-muted)" }}>Create or join a league</p>
                  </div>
                  <ChevronRight size={16} style={{ color: "var(--text-muted)" }} />
                </Link>

                <Link href="/leaderboard" className="glass-card p-4 flex items-center gap-3 no-underline group" style={{ display: "flex" }}>
                  <div className="w-10 h-10 rounded-xl flex items-center justify-center" style={{ background: "rgba(0, 230, 118, 0.1)" }}>
                    <Trophy size={18} style={{ color: "var(--accent-green)" }} />
                  </div>
                  <div className="flex-1">
                    <p className="text-sm font-bold" style={{ fontFamily: "var(--font-display)", color: "var(--text-primary)" }}>Leaderboard</p>
                    <p className="text-xs" style={{ color: "var(--text-muted)" }}>See the rankings</p>
                  </div>
                  <ChevronRight size={16} style={{ color: "var(--text-muted)" }} />
                </Link>
              </motion.div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
