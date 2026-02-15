"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { motion, AnimatePresence } from "framer-motion";
import { Users, Plus, ArrowRight, Copy, Check, AlertCircle, Trophy, Crown } from "lucide-react";
import Nav from "@/components/nav";
import { createLeague, joinLeague, getLeague, type League, type LeagueDetail } from "@/lib/api";
import { getToken, isAuthenticated } from "@/lib/auth";

export default function LeaguePage() {
  const router = useRouter();
  const [tab, setTab] = useState<"create" | "join" | "view">("create");
  const [leagueName, setLeagueName] = useState("");
  const [inviteCode, setInviteCode] = useState("");
  const [createdLeague, setCreatedLeague] = useState<League | null>(null);
  const [joinedLeague, setJoinedLeague] = useState<League | null>(null);
  const [leagueDetail, setLeagueDetail] = useState<LeagueDetail | null>(null);
  const [viewLeagueId, setViewLeagueId] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!isAuthenticated()) {
      router.push("/login");
    }
  }, [router]);

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
        <div className="flex gap-2 mb-8">
          {[
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
          <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} className="glass-card p-6">
            <div className="flex items-center justify-between mb-6">
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

            <h3 className="text-sm uppercase tracking-wider mb-4" style={{ fontFamily: "var(--font-display)", color: "var(--text-muted)" }}>
              Members ({leagueDetail.members.length})
            </h3>

            <div className="space-y-2">
              {leagueDetail.members.map((member, i) => (
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
                    <p className="text-xs" style={{ color: "var(--text-muted)" }}>{member.team_name || "No team"}</p>
                  </div>
                  <span className="text-lg font-bold" style={{ fontFamily: "var(--font-display)", color: "var(--accent-green)" }}>
                    {member.total_points}
                  </span>
                </motion.div>
              ))}
            </div>
          </motion.div>
        )}
      </div>
    </div>
  );
}
