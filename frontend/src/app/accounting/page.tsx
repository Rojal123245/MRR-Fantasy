"use client";

import { useEffect, useState, useCallback } from "react";
import { useRouter } from "next/navigation";
import { motion } from "framer-motion";
import {
  Calculator,
  Plus,
  Trash2,
  CheckCircle2,
  Circle,
  ChevronDown,
  ChevronUp,
  Users,
  AlertCircle,
  X,
  BarChart3,
} from "lucide-react";
import Nav from "@/components/nav";
import {
  createFutsalSession,
  getFutsalSessions,
  getFutsalSession,
  deleteFutsalSession,
  addSessionPlayer,
  removeSessionPlayer,
  togglePlayerPaid,
  getAccountingUsers,
  getUserSummary,
  type FutsalSession,
  type SessionDetail,
  type AccountingUser,
  type UserSummary,
} from "@/lib/api";
import { getToken, getUser, isAuthenticated } from "@/lib/auth";

type Tab = "sessions" | "summary";

export default function AccountingPage() {
  const router = useRouter();
  const [sessions, setSessions] = useState<FutsalSession[]>([]);
  const [expandedSession, setExpandedSession] = useState<string | null>(null);
  const [sessionDetail, setSessionDetail] = useState<SessionDetail | null>(null);
  const [allUsers, setAllUsers] = useState<AccountingUser[]>([]);
  const [summaries, setSummaries] = useState<UserSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  const [activeTab, setActiveTab] = useState<Tab>("sessions");

  // Create session form
  const [newTitle, setNewTitle] = useState("");
  const [newAmount, setNewAmount] = useState("");
  const [showCreateForm, setShowCreateForm] = useState(false);

  // Add player form
  const [playerName, setPlayerName] = useState("");
  const [selectedUserId, setSelectedUserId] = useState<string>("");

  const token = getToken();
  const user = getUser();

  const loadSessions = useCallback(async () => {
    if (!token) return;
    try {
      const data = await getFutsalSessions(token);
      setSessions(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load sessions");
    }
  }, [token]);

  const loadSessionDetail = useCallback(
    async (sessionId: string) => {
      if (!token) return;
      try {
        const data = await getFutsalSession(sessionId, token);
        setSessionDetail(data);
      } catch (err) {
        setError(err instanceof Error ? err.message : "Failed to load session");
      }
    },
    [token]
  );

  const loadSummaries = useCallback(async () => {
    if (!token) return;
    try {
      const data = await getUserSummary(token);
      setSummaries(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load summaries");
    }
  }, [token]);

  useEffect(() => {
    if (!isAuthenticated()) {
      router.push("/login");
      return;
    }
    if (!user?.is_admin) {
      router.push("/dashboard");
      return;
    }

    Promise.all([loadSessions(), getAccountingUsers(token!).then(setAllUsers)])
      .catch(() => {})
      .finally(() => setLoading(false));
  }, [router, token, user?.is_admin, loadSessions]);

  useEffect(() => {
    if (activeTab === "summary") {
      loadSummaries();
    }
  }, [activeTab, loadSummaries]);

  const handleCreateSession = async () => {
    if (!token) return;
    setError("");
    const title = newTitle.trim();
    const amount = parseFloat(newAmount);
    if (!title) {
      setError("Enter a session title.");
      return;
    }
    if (isNaN(amount) || amount <= 0) {
      setError("Enter a valid positive amount.");
      return;
    }
    try {
      await createFutsalSession(title, amount, token);
      setNewTitle("");
      setNewAmount("");
      setShowCreateForm(false);
      await loadSessions();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to create session");
    }
  };

  const handleDeleteSession = async (sessionId: string) => {
    if (!token) return;
    try {
      await deleteFutsalSession(sessionId, token);
      if (expandedSession === sessionId) {
        setExpandedSession(null);
        setSessionDetail(null);
      }
      await loadSessions();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to delete session");
    }
  };

  const handleToggleExpand = async (sessionId: string) => {
    if (expandedSession === sessionId) {
      setExpandedSession(null);
      setSessionDetail(null);
    } else {
      setExpandedSession(sessionId);
      await loadSessionDetail(sessionId);
    }
  };

  const handleAddPlayer = async () => {
    if (!token || !expandedSession) return;
    setError("");
    const name = playerName.trim();
    if (!name) {
      setError("Enter a player name.");
      return;
    }
    try {
      const detail = await addSessionPlayer(
        expandedSession,
        name,
        selectedUserId || null,
        token
      );
      setSessionDetail(detail);
      setPlayerName("");
      setSelectedUserId("");
      await loadSessions();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to add player");
    }
  };

  const handleRemovePlayer = async (playerId: string) => {
    if (!token || !expandedSession) return;
    try {
      const detail = await removeSessionPlayer(expandedSession, playerId, token);
      setSessionDetail(detail);
      await loadSessions();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to remove player");
    }
  };

  const handleTogglePaid = async (sessionId: string, playerId: string) => {
    if (!token) return;
    try {
      const detail = await togglePlayerPaid(sessionId, playerId, token);
      setSessionDetail(detail);
      await loadSessions();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to update payment");
    }
  };

  const handleUserSelect = (userId: string) => {
    setSelectedUserId(userId);
    const matchedUser = allUsers.find((u) => u.id === userId);
    if (matchedUser) {
      setPlayerName(matchedUser.full_name || matchedUser.username);
    }
  };

  return (
    <div className="min-h-screen pitch-pattern">
      <Nav />
      <div className="pt-24 pb-12 max-w-5xl mx-auto px-4 sm:px-6 lg:px-8">
        {/* Header */}
        <motion.div initial={{ opacity: 0, y: 16 }} animate={{ opacity: 1, y: 0 }} className="mb-8">
          <h1 className="text-3xl sm:text-4xl font-bold" style={{ fontFamily: "var(--font-display)" }}>
            FUTSAL <span style={{ color: "var(--accent-green)" }}>ACCOUNTING</span>
          </h1>
          <p className="mt-2 text-sm" style={{ color: "var(--text-muted)" }}>
            Create sessions, add players, split costs, and track payments.
          </p>
        </motion.div>

        {/* Tabs */}
        <div className="flex gap-2 mb-6">
          <button
            onClick={() => setActiveTab("sessions")}
            className="flex items-center gap-2 px-4 py-2 rounded-xl text-sm font-semibold transition-all cursor-pointer border-none"
            style={{
              background: activeTab === "sessions" ? "rgba(0, 230, 118, 0.1)" : "transparent",
              color: activeTab === "sessions" ? "var(--accent-green)" : "var(--text-muted)",
              border: activeTab === "sessions" ? "1px solid rgba(0, 230, 118, 0.2)" : "1px solid transparent",
            }}
          >
            <Calculator size={16} />
            Sessions
          </button>
          <button
            onClick={() => setActiveTab("summary")}
            className="flex items-center gap-2 px-4 py-2 rounded-xl text-sm font-semibold transition-all cursor-pointer border-none"
            style={{
              background: activeTab === "summary" ? "rgba(0, 230, 118, 0.1)" : "transparent",
              color: activeTab === "summary" ? "var(--accent-green)" : "var(--text-muted)",
              border: activeTab === "summary" ? "1px solid rgba(0, 230, 118, 0.2)" : "1px solid transparent",
            }}
          >
            <BarChart3 size={16} />
            Player Summary
          </button>
        </div>

        {/* Error */}
        {error && (
          <div
            className="flex items-center gap-2 p-3 rounded-lg mb-6 text-sm"
            style={{
              background: "rgba(255, 82, 82, 0.1)",
              border: "1px solid rgba(255, 82, 82, 0.3)",
              color: "var(--danger)",
            }}
          >
            <AlertCircle size={16} className="shrink-0" />
            <span className="flex-1">{error}</span>
            <button onClick={() => setError("")} className="bg-transparent border-none cursor-pointer" style={{ color: "var(--danger)" }}>
              <X size={14} />
            </button>
          </div>
        )}

        {loading ? (
          <div className="flex items-center justify-center py-20">
            <div className="w-8 h-8 border-2 border-t-transparent rounded-full animate-spin" style={{ borderColor: "var(--accent-green)", borderTopColor: "transparent" }} />
          </div>
        ) : activeTab === "sessions" ? (
          <>
            {/* Create Session */}
            <motion.div initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }} className="mb-6">
              {showCreateForm ? (
                <div className="glass-card p-5">
                  <h2 className="text-lg font-bold mb-4" style={{ fontFamily: "var(--font-display)" }}>
                    New Futsal Session
                  </h2>
                  <div className="flex flex-col sm:flex-row gap-3">
                    <input
                      type="text"
                      value={newTitle}
                      onChange={(e) => setNewTitle(e.target.value)}
                      placeholder="Session title (e.g. Futsal Week 5)"
                      className="input-field flex-1"
                    />
                    <input
                      type="number"
                      value={newAmount}
                      onChange={(e) => setNewAmount(e.target.value)}
                      placeholder="Total amount"
                      className="input-field sm:max-w-[160px]"
                      min="0"
                      step="0.01"
                    />
                  </div>
                  <div className="flex gap-2 mt-4">
                    <button onClick={handleCreateSession} className="btn-primary text-sm px-5 py-2.5 inline-flex items-center gap-2">
                      <Plus size={16} />
                      Create Session
                    </button>
                    <button
                      onClick={() => { setShowCreateForm(false); setNewTitle(""); setNewAmount(""); }}
                      className="btn-secondary text-sm px-4 py-2.5"
                    >
                      Cancel
                    </button>
                  </div>
                </div>
              ) : (
                <button
                  onClick={() => setShowCreateForm(true)}
                  className="btn-primary text-sm inline-flex items-center gap-2"
                >
                  <Plus size={16} />
                  New Session
                </button>
              )}
            </motion.div>

            {/* Sessions List */}
            {sessions.length === 0 ? (
              <div className="glass-card p-10 text-center text-sm" style={{ color: "var(--text-muted)" }}>
                No futsal sessions yet. Create one to get started.
              </div>
            ) : (
              <div className="space-y-4">
                {sessions.map((session, idx) => {
                  const isExpanded = expandedSession === session.id;
                  return (
                    <motion.div
                      key={session.id}
                      initial={{ opacity: 0, y: 12 }}
                      animate={{ opacity: 1, y: 0 }}
                      transition={{ delay: idx * 0.03 }}
                      className="glass-card overflow-hidden"
                    >
                      {/* Session header row */}
                      <div
                        className="flex items-center justify-between p-5 cursor-pointer"
                        onClick={() => handleToggleExpand(session.id)}
                      >
                        <div className="flex-1 min-w-0">
                          <h3 className="text-base font-bold truncate" style={{ fontFamily: "var(--font-display)" }}>
                            {session.title}
                          </h3>
                          <div className="flex flex-wrap items-center gap-3 mt-1.5">
                            <span className="text-xs" style={{ color: "var(--text-muted)" }}>
                              Total: <strong style={{ color: "var(--accent-amber)" }}>Rs. {parseFloat(session.total_amount).toLocaleString()}</strong>
                            </span>
                            <span className="text-xs" style={{ color: "var(--text-muted)" }}>
                              Players: <strong>{session.player_count}</strong>
                            </span>
                            {session.player_count > 0 && (
                              <span className="text-xs" style={{ color: "var(--text-muted)" }}>
                                Per person: <strong style={{ color: "var(--accent-green)" }}>
                                  Rs. {(parseFloat(session.total_amount) / session.player_count).toFixed(2)}
                                </strong>
                              </span>
                            )}
                            <span className="text-xs" style={{ color: "var(--text-muted)" }}>
                              Paid: <strong style={{ color: session.paid_count === session.player_count && session.player_count > 0 ? "var(--accent-green)" : "var(--accent-amber)" }}>
                                {session.paid_count}/{session.player_count}
                              </strong>
                            </span>
                          </div>
                        </div>
                        <div className="flex items-center gap-2 ml-4">
                          <button
                            onClick={(e) => {
                              e.stopPropagation();
                              handleDeleteSession(session.id);
                            }}
                            className="p-2 rounded-lg bg-transparent border-none cursor-pointer transition-colors"
                            style={{ color: "var(--text-muted)" }}
                            title="Delete session"
                          >
                            <Trash2 size={16} />
                          </button>
                          {isExpanded ? <ChevronUp size={18} style={{ color: "var(--text-muted)" }} /> : <ChevronDown size={18} style={{ color: "var(--text-muted)" }} />}
                        </div>
                      </div>

                      {/* Expanded detail */}
                      {isExpanded && sessionDetail && sessionDetail.session.id === session.id && (
                        <div className="px-5 pb-5" style={{ borderTop: "1px solid var(--border-color)" }}>
                          {/* Add player */}
                          <div className="pt-4 mb-4">
                            <h4 className="text-sm font-bold mb-3" style={{ fontFamily: "var(--font-display)" }}>
                              Add Player
                            </h4>
                            <div className="flex flex-col sm:flex-row gap-3">
                              <select
                                value={selectedUserId}
                                onChange={(e) => handleUserSelect(e.target.value)}
                                className="input-field sm:max-w-[220px]"
                              >
                                <option value="">-- Link to user (optional) --</option>
                                {allUsers.map((u) => (
                                  <option key={u.id} value={u.id}>
                                    {u.full_name || u.username}
                                  </option>
                                ))}
                              </select>
                              <input
                                type="text"
                                value={playerName}
                                onChange={(e) => setPlayerName(e.target.value)}
                                placeholder="Player name..."
                                className="input-field flex-1"
                              />
                              <button onClick={handleAddPlayer} className="btn-primary text-sm px-4 py-2.5 inline-flex items-center gap-1.5">
                                <Plus size={14} />
                                Add
                              </button>
                            </div>
                          </div>

                          {/* Player list */}
                          {sessionDetail.players.length === 0 ? (
                            <p className="text-sm py-4 text-center" style={{ color: "var(--text-muted)" }}>
                              No players added yet. Add players to split the cost.
                            </p>
                          ) : (
                            <div className="space-y-2">
                              <div className="flex items-center justify-between px-3 py-1.5">
                                <span className="text-[11px] uppercase tracking-wider font-semibold" style={{ color: "var(--text-muted)", fontFamily: "var(--font-display)" }}>
                                  Player
                                </span>
                                <div className="flex items-center gap-8">
                                  <span className="text-[11px] uppercase tracking-wider font-semibold w-20 text-right" style={{ color: "var(--text-muted)", fontFamily: "var(--font-display)" }}>
                                    Amount
                                  </span>
                                  <span className="text-[11px] uppercase tracking-wider font-semibold w-16 text-center" style={{ color: "var(--text-muted)", fontFamily: "var(--font-display)" }}>
                                    Status
                                  </span>
                                  <span className="w-8" />
                                </div>
                              </div>
                              {sessionDetail.players.map((player) => (
                                <div
                                  key={player.id}
                                  className="flex items-center justify-between rounded-xl px-3 py-2.5 transition-all"
                                  style={{
                                    background: player.is_paid ? "rgba(0, 230, 118, 0.06)" : "var(--bg-elevated)",
                                    border: `1px solid ${player.is_paid ? "rgba(0, 230, 118, 0.15)" : "var(--border-color)"}`,
                                  }}
                                >
                                  <div className="flex items-center gap-3 min-w-0 flex-1">
                                    <button
                                      onClick={() => handleTogglePaid(session.id, player.id)}
                                      className="bg-transparent border-none cursor-pointer p-0 shrink-0"
                                      title={player.is_paid ? "Mark as unpaid" : "Mark as paid"}
                                    >
                                      {player.is_paid ? (
                                        <CheckCircle2 size={20} style={{ color: "var(--accent-green)" }} />
                                      ) : (
                                        <Circle size={20} style={{ color: "var(--text-muted)" }} />
                                      )}
                                    </button>
                                    <div className="min-w-0">
                                      <p className={`text-sm font-medium truncate ${player.is_paid ? "line-through opacity-60" : ""}`}>
                                        {player.player_name}
                                      </p>
                                      {player.user_id && (
                                        <p className="text-[10px]" style={{ color: "var(--accent-green)" }}>
                                          Linked account
                                        </p>
                                      )}
                                    </div>
                                  </div>
                                  <div className="flex items-center gap-8">
                                    <span className="text-sm font-bold w-20 text-right" style={{ fontFamily: "var(--font-display)", color: player.is_paid ? "var(--accent-green)" : "var(--accent-amber)" }}>
                                      Rs. {parseFloat(player.amount_due).toFixed(2)}
                                    </span>
                                    <span
                                      className="text-[10px] font-bold px-2.5 py-1 rounded-full w-16 text-center"
                                      style={{
                                        fontFamily: "var(--font-display)",
                                        background: player.is_paid ? "rgba(0, 230, 118, 0.15)" : "rgba(255, 171, 0, 0.15)",
                                        color: player.is_paid ? "var(--accent-green)" : "var(--accent-amber)",
                                      }}
                                    >
                                      {player.is_paid ? "PAID" : "DUE"}
                                    </span>
                                    <button
                                      onClick={() => handleRemovePlayer(player.id)}
                                      className="p-1.5 rounded-lg bg-transparent border-none cursor-pointer transition-colors w-8 flex items-center justify-center"
                                      style={{ color: "var(--text-muted)" }}
                                      title="Remove player"
                                    >
                                      <X size={14} />
                                    </button>
                                  </div>
                                </div>
                              ))}
                            </div>
                          )}
                        </div>
                      )}
                    </motion.div>
                  );
                })}
              </div>
            )}
          </>
        ) : (
          /* Player Summary Tab */
          <motion.div initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }}>
            {summaries.length === 0 ? (
              <div className="glass-card p-10 text-center text-sm" style={{ color: "var(--text-muted)" }}>
                No accounting data yet. Create sessions and add players first.
              </div>
            ) : (
              <div className="glass-card overflow-hidden">
                {/* Table header */}
                <div
                  className="grid grid-cols-5 gap-4 px-5 py-3 text-[11px] uppercase tracking-wider font-semibold"
                  style={{
                    color: "var(--text-muted)",
                    fontFamily: "var(--font-display)",
                    borderBottom: "1px solid var(--border-color)",
                    background: "rgba(0, 0, 0, 0.2)",
                  }}
                >
                  <span className="col-span-2">Player</span>
                  <span className="text-right">Total Due</span>
                  <span className="text-right">Paid</span>
                  <span className="text-right">Unpaid</span>
                </div>
                {/* Rows */}
                {summaries.map((s, idx) => (
                  <div
                    key={`${s.user_id}-${s.player_name}-${idx}`}
                    className="grid grid-cols-5 gap-4 px-5 py-3 items-center transition-colors"
                    style={{
                      borderBottom: "1px solid var(--border-color)",
                      background: idx % 2 === 0 ? "transparent" : "rgba(0, 0, 0, 0.08)",
                    }}
                  >
                    <div className="col-span-2 flex items-center gap-2 min-w-0">
                      <div
                        className="w-7 h-7 rounded-full flex items-center justify-center text-[10px] font-bold shrink-0"
                        style={{
                          background: s.user_id ? "rgba(0, 230, 118, 0.1)" : "rgba(255, 171, 0, 0.1)",
                          color: s.user_id ? "var(--accent-green)" : "var(--accent-amber)",
                          fontFamily: "var(--font-display)",
                        }}
                      >
                        {s.player_name.charAt(0).toUpperCase()}
                      </div>
                      <div className="min-w-0">
                        <p className="text-sm font-medium truncate">{s.player_name}</p>
                        <p className="text-[10px]" style={{ color: "var(--text-muted)" }}>
                          {s.sessions_count} session{s.sessions_count !== 1 ? "s" : ""}
                        </p>
                      </div>
                    </div>
                    <span className="text-sm font-bold text-right" style={{ fontFamily: "var(--font-display)" }}>
                      Rs. {parseFloat(s.total_due).toFixed(2)}
                    </span>
                    <span className="text-sm font-bold text-right" style={{ fontFamily: "var(--font-display)", color: "var(--accent-green)" }}>
                      Rs. {parseFloat(s.total_paid).toFixed(2)}
                    </span>
                    <span
                      className="text-sm font-bold text-right"
                      style={{
                        fontFamily: "var(--font-display)",
                        color: parseFloat(s.total_unpaid) > 0 ? "var(--danger)" : "var(--accent-green)",
                      }}
                    >
                      Rs. {parseFloat(s.total_unpaid).toFixed(2)}
                    </span>
                  </div>
                ))}
              </div>
            )}
          </motion.div>
        )}
      </div>
    </div>
  );
}
