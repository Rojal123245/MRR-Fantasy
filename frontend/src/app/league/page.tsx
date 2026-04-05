"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { motion, AnimatePresence } from "framer-motion";
import { Users, Plus, ArrowRight, Copy, Check, AlertCircle, Trophy, Crown, Shield, Eye, X, Lock, Calendar, ChevronLeft, ChevronRight, Star, Target, Timer, ShieldCheck, ChevronDown } from "lucide-react";
import Nav from "@/components/nav";
import Formation from "@/components/formation";
import type { FormationPlayer } from "@/components/formation";
import { createLeague, joinLeague, getLeague, getMyLeagues, getLockStatus, getMemberLineup, getLeagueGameweek, getWeekPoints, type League, type LeagueDetail, type MyLeague, type MemberLineup, type LeagueGameweekDetail, type PlayerPointsDisplay } from "@/lib/api";
import { getToken, getUser, isAuthenticated } from "@/lib/auth";

export default function LeaguePage() {
  const router = useRouter();
  const [tab, setTab] = useState<"my" | "create" | "join" | "view">("my");
  const [leagueName, setLeagueName] = useState("");
  const [inviteCode, setInviteCode] = useState("");
  const [createdLeague, setCreatedLeague] = useState<League | null>(null);
  const [joinedLeague, setJoinedLeague] = useState<League | null>(null);
  const [leagueDetail, setLeagueDetail] = useState<LeagueDetail | null>(null);
  const [myLeagues, setMyLeagues] = useState<MyLeague[]>([]);
  const [myLeaguesLoading, setMyLeaguesLoading] = useState(true);
  const [viewLeagueId, setViewLeagueId] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [copied, setCopied] = useState(false);
  const [isLocked, setIsLocked] = useState(false);
  const [lineupModal, setLineupModal] = useState<MemberLineup | null>(null);
  const [lineupLoading, setLineupLoading] = useState(false);
  const [viewSubTab, setViewSubTab] = useState<"standings" | "gameweek">("standings");
  const [selectedWeek, setSelectedWeek] = useState(1);
  const [gameweekData, setGameweekData] = useState<LeagueGameweekDetail | null>(null);
  const [gameweekLoading, setGameweekLoading] = useState(false);
  const [weekPlayers, setWeekPlayers] = useState<PlayerPointsDisplay[]>([]);
  const [weekPlayersLoading, setWeekPlayersLoading] = useState(false);
  const [showPlayerStats, setShowPlayerStats] = useState(true);

  const loadMyLeagues = async () => {
    const token = getToken();
    if (!token) return;
    setMyLeaguesLoading(true);
    try {
      const leagues = await getMyLeagues(token);
      setMyLeagues(leagues);
    } catch {
      // silently fail - user may not have leagues
    } finally {
      setMyLeaguesLoading(false);
    }
  };

  useEffect(() => {
    if (!isAuthenticated()) {
      router.push("/login");
      return;
    }
    loadMyLeagues();
    getLockStatus().then((s) => setIsLocked(s.locked)).catch(() => {});
  }, [router]);

  const loadGameweekPoints = async (leagueId: string, week: number) => {
    setGameweekLoading(true);
    setWeekPlayersLoading(true);
    try {
      const [data, players] = await Promise.all([
        getLeagueGameweek(leagueId, week),
        getWeekPoints(week),
      ]);
      setGameweekData(data);
      setWeekPlayers(players.filter((p) => p.total_points > 0));
    } catch {
      setGameweekData(null);
      setWeekPlayers([]);
    } finally {
      setGameweekLoading(false);
      setWeekPlayersLoading(false);
    }
  };

  const handleViewLineup = async (leagueId: string, userId: string) => {
    const token = getToken();
    if (!token) return;
    setLineupLoading(true);
    setError("");
    try {
      const lineup = await getMemberLineup(leagueId, userId, token);
      setLineupModal(lineup);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Cannot view lineup");
    } finally {
      setLineupLoading(false);
    }
  };

  const handleCreateLeague = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    setLoading(true);

    const token = getToken();
    if (!token) return;

    try {
      const league = await createLeague(leagueName, token);
      setCreatedLeague(league);
      setLeagueName("");
      loadMyLeagues();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to create league");
    } finally {
      setLoading(false);
    }
  };

  const handleJoinLeague = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    setLoading(true);

    const token = getToken();
    if (!token) return;

    try {
      const league = await joinLeague(inviteCode.toUpperCase(), token);
      setJoinedLeague(league);
      setInviteCode("");
      loadMyLeagues();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to join league");
    } finally {
      setLoading(false);
    }
  };

  const handleViewLeague = async (leagueId: string) => {
    setError("");
    setLoading(true);
    try {
      const detail = await getLeague(leagueId);
      setLeagueDetail(detail);
      setTab("view");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load league");
    } finally {
      setLoading(false);
    }
  };

  const copyInviteCode = (code: string) => {
    navigator.clipboard.writeText(code);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
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
          <h1 className="text-3xl font-bold mb-2" style={{ fontFamily: "var(--font-display)" }}>
            <span style={{ color: "var(--accent-amber)" }}>LEAGUES</span>
          </h1>
          <p style={{ color: "var(--text-muted)" }}>Create or join leagues to compete with friends</p>
        </motion.div>

        {/* Tabs */}
        <div className="flex gap-2 mb-8 flex-wrap">
          {[
            { id: "my" as const, label: "My Leagues", icon: Shield },
            { id: "create" as const, label: "Create League", icon: Plus },
            { id: "join" as const, label: "Join League", icon: ArrowRight },
          ].map((t) => {
            const Icon = t.icon;
            return (
              <button
                key={t.id}
                onClick={() => { setTab(t.id); setError(""); }}
                className="flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition-all cursor-pointer border-none"
                style={{
                  fontFamily: "var(--font-display)",
                  background: tab === t.id ? "var(--accent-green)" : "var(--bg-secondary)",
                  color: tab === t.id ? "var(--bg-primary)" : "var(--text-muted)",
                }}
              >
                <Icon size={16} />
                {t.label}
              </button>
            );
          })}
        </div>

        {/* Error */}
        <AnimatePresence>
          {error && (
            <motion.div
              initial={{ opacity: 0, y: -10 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0 }}
              className="flex items-center gap-2 p-3 rounded-lg mb-6 text-sm"
              style={{ background: "rgba(255, 82, 82, 0.1)", border: "1px solid rgba(255, 82, 82, 0.3)", color: "var(--danger)" }}
            >
              <AlertCircle size={16} />
              {error}
            </motion.div>
          )}
        </AnimatePresence>

        {/* My Leagues */}
        {tab === "my" && (
          <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }}>
            <div className="flex items-center gap-3 mb-6">
              <div className="w-10 h-10 rounded-xl flex items-center justify-center" style={{ background: "rgba(0, 230, 118, 0.1)" }}>
                <Shield size={20} style={{ color: "var(--accent-green)" }} />
              </div>
              <div>
                <h2 className="text-lg font-bold" style={{ fontFamily: "var(--font-display)" }}>My Leagues</h2>
                <p className="text-xs" style={{ color: "var(--text-muted)" }}>Leagues you&apos;ve joined or created</p>
              </div>
            </div>

            {myLeaguesLoading ? (
              <div className="flex items-center justify-center py-12">
                <div className="w-8 h-8 border-2 border-t-transparent rounded-full animate-spin" style={{ borderColor: "var(--accent-green)", borderTopColor: "transparent" }} />
              </div>
            ) : myLeagues.length === 0 ? (
              <div className="glass-card p-8 text-center">
                <div className="w-14 h-14 rounded-2xl flex items-center justify-center mx-auto mb-4" style={{ background: "rgba(255, 171, 0, 0.08)" }}>
                  <Users size={28} style={{ color: "var(--accent-amber)" }} />
                </div>
                <p className="text-lg font-bold mb-2" style={{ fontFamily: "var(--font-display)" }}>No Leagues Yet</p>
                <p className="text-sm mb-4" style={{ color: "var(--text-muted)" }}>Create a league or join one with an invite code</p>
                <div className="flex gap-3 justify-center">
                  <button onClick={() => setTab("create")} className="btn-primary text-sm">Create League</button>
                  <button onClick={() => setTab("join")} className="btn-secondary text-sm">Join League</button>
                </div>
              </div>
            ) : (
              <div className="space-y-3">
                {myLeagues.map((league, i) => (
                  <motion.div
                    key={league.id}
                    initial={{ opacity: 0, y: 10 }}
                    animate={{ opacity: 1, y: 0 }}
                    transition={{ delay: i * 0.05 }}
                    className="glass-card p-5 cursor-pointer hover:scale-[1.01] transition-transform"
                    onClick={() => handleViewLeague(league.id)}
                  >
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-3">
                        <div className="w-10 h-10 rounded-xl flex items-center justify-center" style={{ background: "rgba(255, 171, 0, 0.1)" }}>
                          <Trophy size={18} style={{ color: "var(--accent-amber)" }} />
                        </div>
                        <div>
                          <h3 className="text-base font-bold" style={{ fontFamily: "var(--font-display)" }}>{league.name}</h3>
                          <div className="flex items-center gap-3 mt-1">
                            <span className="text-xs" style={{ color: "var(--text-muted)" }}>
                              <Users size={12} className="inline mr-1" />{league.member_count} member{league.member_count !== 1 ? "s" : ""}
                            </span>
                            <span className="text-xs" style={{ color: "var(--accent-amber)", fontFamily: "var(--font-display)" }}>
                              {league.invite_code}
                            </span>
                          </div>
                        </div>
                      </div>
                      <ArrowRight size={16} style={{ color: "var(--text-muted)" }} />
                    </div>
                  </motion.div>
                ))}
              </div>
            )}
          </motion.div>
        )}

        {/* Create League */}
        {tab === "create" && (
          <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} className="glass-card p-6">
            <div className="flex items-center gap-3 mb-6">
              <div className="w-10 h-10 rounded-xl flex items-center justify-center" style={{ background: "rgba(0, 230, 118, 0.1)" }}>
                <Users size={20} style={{ color: "var(--accent-green)" }} />
              </div>
              <div>
                <h2 className="text-lg font-bold" style={{ fontFamily: "var(--font-display)" }}>Create a League</h2>
                <p className="text-xs" style={{ color: "var(--text-muted)" }}>Get an invite code to share with friends</p>
              </div>
            </div>

            <form onSubmit={handleCreateLeague} className="space-y-4">
              <div>
                <label className="block text-xs uppercase tracking-wider mb-2" style={{ color: "var(--text-muted)", fontFamily: "var(--font-display)" }}>League Name</label>
                <input
                  type="text"
                  value={leagueName}
                  onChange={(e) => setLeagueName(e.target.value)}
                  className="input-field"
                  placeholder="e.g., Premier League Legends"
                  required
                />
              </div>
              <button type="submit" disabled={loading} className="btn-primary flex items-center gap-2 text-sm disabled:opacity-50">
                {loading ? "Creating..." : "Create League"}
                <Plus size={16} />
              </button>
            </form>

            {/* Created league result */}
            {createdLeague && (
              <motion.div
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                className="mt-6 p-4 rounded-xl"
                style={{ background: "rgba(0, 230, 118, 0.06)", border: "1px solid rgba(0, 230, 118, 0.2)" }}
              >
                <p className="text-sm font-medium mb-2" style={{ color: "var(--accent-green)" }}>
                  League created successfully!
                </p>
                <p className="text-sm mb-3" style={{ color: "var(--text-muted)" }}>
                  Share this invite code with friends:
                </p>
                <div className="flex items-center gap-3">
                  <code className="text-2xl font-bold tracking-widest px-4 py-2 rounded-lg" style={{ fontFamily: "var(--font-display)", color: "var(--accent-amber)", background: "var(--bg-primary)" }}>
                    {createdLeague.invite_code}
                  </code>
                  <button
                    onClick={() => copyInviteCode(createdLeague.invite_code)}
                    className="p-2 rounded-lg cursor-pointer bg-transparent border-none"
                    style={{ color: copied ? "var(--accent-green)" : "var(--text-muted)" }}
                  >
                    {copied ? <Check size={20} /> : <Copy size={20} />}
                  </button>
                </div>
                <button
                  onClick={() => handleViewLeague(createdLeague.id)}
                  className="btn-secondary text-xs mt-4 py-2 px-4"
                >
                  View League
                </button>
              </motion.div>
            )}
          </motion.div>
        )}

        {/* Join League */}
        {tab === "join" && (
          <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} className="glass-card p-6">
            <div className="flex items-center gap-3 mb-6">
              <div className="w-10 h-10 rounded-xl flex items-center justify-center" style={{ background: "rgba(255, 171, 0, 0.1)" }}>
                <ArrowRight size={20} style={{ color: "var(--accent-amber)" }} />
              </div>
              <div>
                <h2 className="text-lg font-bold" style={{ fontFamily: "var(--font-display)" }}>Join a League</h2>
                <p className="text-xs" style={{ color: "var(--text-muted)" }}>Enter the invite code shared by a friend</p>
              </div>
            </div>

            <form onSubmit={handleJoinLeague} className="space-y-4">
              <div>
                <label className="block text-xs uppercase tracking-wider mb-2" style={{ color: "var(--text-muted)", fontFamily: "var(--font-display)" }}>Invite Code</label>
                <input
                  type="text"
                  value={inviteCode}
                  onChange={(e) => setInviteCode(e.target.value.toUpperCase())}
                  className="input-field text-center text-2xl tracking-widest"
                  style={{ fontFamily: "var(--font-display)" }}
                  placeholder="ABCD1234"
                  maxLength={8}
                  required
                />
              </div>
              <button type="submit" disabled={loading} className="btn-primary flex items-center gap-2 text-sm disabled:opacity-50">
                {loading ? "Joining..." : "Join League"}
                <ArrowRight size={16} />
              </button>
            </form>

            {joinedLeague && (
              <motion.div
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                className="mt-6 p-4 rounded-xl"
                style={{ background: "rgba(0, 230, 118, 0.06)", border: "1px solid rgba(0, 230, 118, 0.2)" }}
              >
                <p className="text-sm font-medium" style={{ color: "var(--accent-green)" }}>
                  Successfully joined &quot;{joinedLeague.name}&quot;!
                </p>
                <button
                  onClick={() => handleViewLeague(joinedLeague.id)}
                  className="btn-secondary text-xs mt-3 py-2 px-4"
                >
                  View League
                </button>
              </motion.div>
            )}
          </motion.div>
        )}

        {/* View League */}
        {tab === "view" && leagueDetail && (
          <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }}>
            <div className="glass-card p-6 mb-4">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-3">
                  <div className="w-10 h-10 rounded-xl flex items-center justify-center" style={{ background: "rgba(255, 171, 0, 0.1)" }}>
                    <Trophy size={20} style={{ color: "var(--accent-amber)" }} />
                  </div>
                  <div>
                    <h2 className="text-lg font-bold" style={{ fontFamily: "var(--font-display)" }}>{leagueDetail.league.name}</h2>
                    <p className="text-xs" style={{ color: "var(--text-muted)" }}>
                      Code: <span style={{ color: "var(--accent-amber)", fontFamily: "var(--font-display)" }}>{leagueDetail.league.invite_code}</span>
                    </p>
                  </div>
                </div>
                <button
                  onClick={() => copyInviteCode(leagueDetail.league.invite_code)}
                  className="p-2 rounded-lg cursor-pointer bg-transparent border-none"
                  style={{ color: "var(--text-muted)" }}
                  title="Copy invite code"
                >
                  <Copy size={18} />
                </button>
              </div>
            </div>

            {/* Sub-tabs: Standings / Gameweek Points */}
            <div className="flex gap-2 mb-4">
              {[
                { id: "standings" as const, label: "Standings", icon: Trophy },
                { id: "gameweek" as const, label: "Gameweek Points", icon: Calendar },
              ].map((st) => {
                const Icon = st.icon;
                return (
                  <button
                    key={st.id}
                    onClick={() => {
                      setViewSubTab(st.id);
                      if (st.id === "gameweek" && !gameweekData) {
                        loadGameweekPoints(leagueDetail.league.id, selectedWeek);
                      }
                    }}
                    className="flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition-all cursor-pointer border-none"
                    style={{
                      fontFamily: "var(--font-display)",
                      background: viewSubTab === st.id ? "var(--accent-green)" : "var(--bg-secondary)",
                      color: viewSubTab === st.id ? "var(--bg-primary)" : "var(--text-muted)",
                    }}
                  >
                    <Icon size={14} />
                    {st.label}
                  </button>
                );
              })}
            </div>

            {/* Standings sub-tab */}
            {viewSubTab === "standings" && (
              <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} className="glass-card p-6">
                <h3 className="text-sm uppercase tracking-wider mb-4" style={{ fontFamily: "var(--font-display)", color: "var(--text-muted)" }}>
                  Members ({leagueDetail.members.length})
                </h3>

                <div className="space-y-2">
                  {leagueDetail.members.map((member, i) => {
                    const currentUser = getUser();
                    const isMe = currentUser?.id === member.user_id;
                    return (
                      <motion.div
                        key={member.user_id}
                        initial={{ opacity: 0, x: -10 }}
                        animate={{ opacity: 1, x: 0 }}
                        transition={{ delay: i * 0.05 }}
                        className="flex items-center gap-3 p-3 rounded-xl"
                        style={{ background: "var(--bg-elevated)", border: i === 0 ? "1px solid rgba(255, 171, 0, 0.3)" : "1px solid var(--border-color)" }}
                      >
                        <div className="w-8 h-8 rounded-full flex items-center justify-center text-sm font-bold" style={{
                          fontFamily: "var(--font-display)",
                          background: i === 0 ? "rgba(255, 171, 0, 0.15)" : "var(--bg-secondary)",
                          color: i === 0 ? "var(--accent-amber)" : "var(--text-muted)",
                        }}>
                          {i === 0 ? <Crown size={16} /> : i + 1}
                        </div>
                        <div className="flex-1">
                          <p className="text-sm font-medium">{member.username}</p>
                          <p className="text-xs" style={{ color: "var(--text-muted)" }}>{member.full_name || member.username}</p>
                        </div>
                        {!isMe && isLocked && member.team_name && (
                          <button
                            onClick={() => handleViewLineup(leagueDetail.league.id, member.user_id)}
                            disabled={lineupLoading}
                            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all cursor-pointer border-none"
                            style={{
                              fontFamily: "var(--font-display)",
                              background: "rgba(0, 230, 118, 0.1)",
                              color: "var(--accent-green)",
                              border: "1px solid rgba(0, 230, 118, 0.2)",
                            }}
                            title="View starting lineup"
                          >
                            <Eye size={13} />
                            Lineup
                          </button>
                        )}
                        {!isMe && !isLocked && member.team_name && (
                          <div
                            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs"
                            style={{ color: "var(--text-muted)" }}
                            title="Lineups visible after gameweek starts"
                          >
                            <Lock size={12} />
                          </div>
                        )}
                        <span className="text-lg font-bold" style={{ fontFamily: "var(--font-display)", color: "var(--accent-green)" }}>
                          {member.total_points}
                        </span>
                      </motion.div>
                    );
                  })}
                </div>
              </motion.div>
            )}

            {/* Gameweek Points sub-tab */}
            {viewSubTab === "gameweek" && (
              <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} className="glass-card p-6">
                {/* Week selector */}
                <div className="flex items-center justify-between mb-6">
                  <button
                    onClick={() => {
                      const prev = Math.max(1, selectedWeek - 1);
                      setSelectedWeek(prev);
                      loadGameweekPoints(leagueDetail.league.id, prev);
                    }}
                    disabled={selectedWeek <= 1}
                    className="p-2 rounded-lg cursor-pointer bg-transparent border-none disabled:opacity-30"
                    style={{ color: "var(--text-muted)" }}
                  >
                    <ChevronLeft size={20} />
                  </button>
                  <div className="text-center">
                    <p className="text-xs uppercase tracking-wider" style={{ fontFamily: "var(--font-display)", color: "var(--text-muted)" }}>Gameweek</p>
                    <p className="text-2xl font-bold" style={{ fontFamily: "var(--font-display)", color: "var(--accent-amber)" }}>{selectedWeek}</p>
                  </div>
                  <button
                    onClick={() => {
                      const next = selectedWeek + 1;
                      setSelectedWeek(next);
                      loadGameweekPoints(leagueDetail.league.id, next);
                    }}
                    className="p-2 rounded-lg cursor-pointer bg-transparent border-none"
                    style={{ color: "var(--text-muted)" }}
                  >
                    <ChevronRight size={20} />
                  </button>
                </div>

                {gameweekLoading ? (
                  <div className="flex items-center justify-center py-12">
                    <div className="w-8 h-8 border-2 border-t-transparent rounded-full animate-spin" style={{ borderColor: "var(--accent-green)", borderTopColor: "transparent" }} />
                  </div>
                ) : gameweekData && gameweekData.members.length > 0 ? (
                  <div className="space-y-2">
                    {gameweekData.members.map((member, i) => {
                      const isTopScorer = i === 0 && member.gameweek_points > 0;
                      return (
                        <motion.div
                          key={member.user_id}
                          initial={{ opacity: 0, x: -10 }}
                          animate={{ opacity: 1, x: 0 }}
                          transition={{ delay: i * 0.05 }}
                          className="flex items-center gap-3 p-3 rounded-xl"
                          style={{
                            background: "var(--bg-elevated)",
                            border: isTopScorer ? "1px solid rgba(255, 171, 0, 0.3)" : "1px solid var(--border-color)",
                          }}
                        >
                          <div className="w-8 h-8 rounded-full flex items-center justify-center text-sm font-bold" style={{
                            fontFamily: "var(--font-display)",
                            background: isTopScorer ? "rgba(255, 171, 0, 0.15)" : "var(--bg-secondary)",
                            color: isTopScorer ? "var(--accent-amber)" : "var(--text-muted)",
                          }}>
                            {isTopScorer ? <Star size={16} /> : i + 1}
                          </div>
                          <div className="flex-1">
                            <p className="text-sm font-medium">
                              {member.username}
                              {isTopScorer && (
                                <span className="ml-2 text-[10px] font-bold px-1.5 py-0.5 rounded" style={{ background: "rgba(255,171,0,0.2)", color: "#ffab00" }}>
                                  TOP SCORER
                                </span>
                              )}
                            </p>
                            <p className="text-xs" style={{ color: "var(--text-muted)" }}>{member.full_name || member.username}</p>
                          </div>
                          <span className="text-lg font-bold" style={{ fontFamily: "var(--font-display)", color: "var(--accent-green)" }}>
                            {member.gameweek_points}
                          </span>
                        </motion.div>
                      );
                    })}
                  </div>
                ) : (
                  <div className="text-center py-8">
                    <Calendar size={32} className="mx-auto mb-3" style={{ color: "var(--text-muted)", opacity: 0.5 }} />
                    <p className="text-sm" style={{ color: "var(--text-muted)" }}>No points recorded for Gameweek {selectedWeek}</p>
                  </div>
                )}
              </motion.div>
            )}

            {/* Player Performances */}
            {viewSubTab === "gameweek" && !gameweekLoading && (
              <motion.div
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.1 }}
                className="glass-card p-6 mt-4"
              >
                <button
                  onClick={() => setShowPlayerStats(!showPlayerStats)}
                  className="w-full flex items-center justify-between mb-4 bg-transparent border-none cursor-pointer p-0"
                >
                  <div className="flex items-center gap-3">
                    <div className="w-10 h-10 rounded-xl flex items-center justify-center" style={{ background: "rgba(0, 230, 118, 0.1)" }}>
                      <Target size={20} style={{ color: "var(--accent-green)" }} />
                    </div>
                    <div className="text-left">
                      <h3 className="text-sm uppercase tracking-wider font-bold" style={{ fontFamily: "var(--font-display)", color: "var(--text-primary)" }}>
                        Player Performances
                      </h3>
                      <p className="text-xs" style={{ color: "var(--text-muted)" }}>
                        {weekPlayers.length} player{weekPlayers.length !== 1 ? "s" : ""} scored points
                      </p>
                    </div>
                  </div>
                  <motion.div
                    animate={{ rotate: showPlayerStats ? 180 : 0 }}
                    transition={{ duration: 0.2 }}
                  >
                    <ChevronDown size={18} style={{ color: "var(--text-muted)" }} />
                  </motion.div>
                </button>

                <AnimatePresence>
                  {showPlayerStats && (
                    <motion.div
                      initial={{ height: 0, opacity: 0 }}
                      animate={{ height: "auto", opacity: 1 }}
                      exit={{ height: 0, opacity: 0 }}
                      transition={{ duration: 0.3 }}
                      style={{ overflow: "hidden" }}
                    >
                      {weekPlayersLoading ? (
                        <div className="flex items-center justify-center py-8">
                          <div className="w-8 h-8 border-2 border-t-transparent rounded-full animate-spin" style={{ borderColor: "var(--accent-green)", borderTopColor: "transparent" }} />
                        </div>
                      ) : weekPlayers.length > 0 ? (
                        <div className="space-y-2">
                          {weekPlayers.map((player, i) => {
                            const isTopPoints = i === 0;
                            return (
                              <motion.div
                                key={player.player_id}
                                initial={{ opacity: 0, x: -10 }}
                                animate={{ opacity: 1, x: 0 }}
                                transition={{ delay: i * 0.03 }}
                                className="rounded-xl p-3"
                                style={{
                                  background: "var(--bg-elevated)",
                                  border: isTopPoints ? "1px solid rgba(255, 171, 0, 0.3)" : "1px solid var(--border-color)",
                                }}
                              >
                                <div className="flex items-center gap-3 mb-2">
                                  <span
                                    className={`text-[10px] font-bold px-2 py-0.5 rounded-full text-white ${
                                      player.position === "GK" ? "badge-gk" :
                                      player.position === "DEF" ? "badge-def" :
                                      player.position === "MID" ? "badge-mid" : "badge-fwd"
                                    }`}
                                  >
                                    {player.position}
                                  </span>
                                  <div className="flex-1 min-w-0">
                                    <p className="text-sm font-medium truncate">
                                      {player.player_name}
                                      {isTopPoints && (
                                        <span className="ml-2 text-[10px] font-bold px-1.5 py-0.5 rounded" style={{ background: "rgba(255,171,0,0.2)", color: "#ffab00" }}>
                                          MVP
                                        </span>
                                      )}
                                    </p>
                                  </div>
                                  <span className="text-lg font-bold" style={{ fontFamily: "var(--font-display)", color: "var(--accent-green)" }}>
                                    {player.total_points} pts
                                  </span>
                                </div>

                                {/* Stats row */}
                                <div className="flex flex-wrap gap-2 ml-8">
                                  {player.goals > 0 && (
                                    <div className="flex items-center gap-1 px-2 py-1 rounded-lg text-xs" style={{ background: "rgba(0, 230, 118, 0.08)", color: "var(--accent-green)" }}>
                                      <Target size={12} />
                                      <span className="font-bold">{player.goals}</span>
                                      <span style={{ color: "var(--text-muted)" }}>Goal{player.goals !== 1 ? "s" : ""}</span>
                                    </div>
                                  )}
                                  {player.assists > 0 && (
                                    <div className="flex items-center gap-1 px-2 py-1 rounded-lg text-xs" style={{ background: "rgba(255, 171, 0, 0.08)", color: "var(--accent-amber)" }}>
                                      <Star size={12} />
                                      <span className="font-bold">{player.assists}</span>
                                      <span style={{ color: "var(--text-muted)" }}>Assist{player.assists !== 1 ? "s" : ""}</span>
                                    </div>
                                  )}
                                  {player.clean_sheets > 0 && (
                                    <div className="flex items-center gap-1 px-2 py-1 rounded-lg text-xs" style={{ background: "rgba(33, 150, 243, 0.08)", color: "#42a5f5" }}>
                                      <ShieldCheck size={12} />
                                      <span className="font-bold">{player.clean_sheets}</span>
                                      <span style={{ color: "var(--text-muted)" }}>CS</span>
                                    </div>
                                  )}
                                  {player.minutes_played > 0 && (
                                    <div className="flex items-center gap-1 px-2 py-1 rounded-lg text-xs" style={{ background: "rgba(156, 39, 176, 0.08)", color: "#ab47bc" }}>
                                      <Timer size={12} />
                                      <span className="font-bold">{player.minutes_played}</span>
                                      <span style={{ color: "var(--text-muted)" }}>min</span>
                                    </div>
                                  )}
                                  {player.saves > 0 && (
                                    <div className="flex items-center gap-1 px-2 py-1 rounded-lg text-xs" style={{ background: "rgba(255, 152, 0, 0.08)", color: "#ffa726" }}>
                                      <Shield size={12} />
                                      <span className="font-bold">{player.saves}</span>
                                      <span style={{ color: "var(--text-muted)" }}>Save{player.saves !== 1 ? "s" : ""}</span>
                                    </div>
                                  )}
                                </div>
                              </motion.div>
                            );
                          })}
                        </div>
                      ) : (
                        <div className="text-center py-6">
                          <p className="text-sm" style={{ color: "var(--text-muted)" }}>No player stats for this gameweek</p>
                        </div>
                      )}
                    </motion.div>
                  )}
                </AnimatePresence>
              </motion.div>
            )}
          </motion.div>
        )}
      </div>

      {/* Lineup Modal */}
      <AnimatePresence>
        {lineupModal && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="fixed inset-0 z-50 flex items-center justify-center p-4"
            style={{ background: "rgba(0,0,0,0.7)", backdropFilter: "blur(4px)" }}
            onClick={() => setLineupModal(null)}
          >
            <motion.div
              initial={{ opacity: 0, scale: 0.9, y: 20 }}
              animate={{ opacity: 1, scale: 1, y: 0 }}
              exit={{ opacity: 0, scale: 0.9, y: 20 }}
              className="glass-card p-6 w-full max-w-md max-h-[90vh] overflow-y-auto"
              onClick={(e) => e.stopPropagation()}
            >
              <div className="flex items-center justify-between mb-4">
                <div>
                  <h3 className="text-lg font-bold" style={{ fontFamily: "var(--font-display)" }}>
                    {lineupModal.username}&apos;s Lineup
                  </h3>
                  <p className="text-xs" style={{ color: "var(--text-muted)" }}>
                    {lineupModal.team_name}
                  </p>
                </div>
                <button
                  onClick={() => setLineupModal(null)}
                  className="p-2 rounded-lg cursor-pointer bg-transparent border-none"
                  style={{ color: "var(--text-muted)" }}
                >
                  <X size={20} />
                </button>
              </div>

              <div className="mb-4">
                <Formation
                  players={lineupModal.starters.map((s): FormationPlayer => ({
                    player: s,
                    assignedPosition: s.assigned_position,
                  }))}
                  captainId={lineupModal.captain_id}
                />
              </div>

              <div className="space-y-2">
                <h4 className="text-xs uppercase tracking-wider" style={{ fontFamily: "var(--font-display)", color: "var(--text-muted)" }}>
                  Starting 6
                </h4>
                {lineupModal.starters.map((starter) => (
                  <div
                    key={starter.id}
                    className="flex items-center gap-3 p-2.5 rounded-lg"
                    style={{ background: "var(--bg-secondary)", border: "1px solid var(--border-color)" }}
                  >
                    <span
                      className={`text-[10px] font-bold px-2 py-0.5 rounded-full text-white ${
                        starter.assigned_position === "GK" ? "badge-gk" :
                        starter.assigned_position === "DEF" ? "badge-def" :
                        starter.assigned_position === "MID" ? "badge-mid" : "badge-fwd"
                      }`}
                    >
                      {starter.assigned_position}
                    </span>
                    <div className="flex-1 min-w-0">
                      <p className="text-sm font-medium truncate">
                        {starter.name}
                        {lineupModal.captain_id === starter.id && (
                          <span className="ml-1.5 text-[10px] font-bold px-1.5 py-0.5 rounded" style={{ background: "rgba(255,171,0,0.2)", color: "#ffab00" }}>C</span>
                        )}
                      </p>
                      <p className="text-xs truncate" style={{ color: "var(--text-muted)" }}>{starter.team_name}</p>
                    </div>
                    <span className="text-sm font-bold" style={{ fontFamily: "var(--font-display)", color: "var(--accent-amber)" }}>
                      {starter.total_points}
                    </span>
                  </div>
                ))}
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
