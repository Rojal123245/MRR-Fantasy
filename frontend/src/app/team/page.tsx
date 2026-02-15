"use client";

import { useEffect, useState, useCallback } from "react";
import { useRouter } from "next/navigation";
import { motion, AnimatePresence } from "framer-motion";
import { Search, Filter, Save, AlertCircle, Check, Crown, DollarSign, Users, Armchair } from "lucide-react";
import Nav from "@/components/nav";
import PlayerCard from "@/components/player-card";
import Formation from "@/components/formation";
import { getPlayers, getMyTeam, createTeam, setTeamPlayers, type Player, type FantasyTeam } from "@/lib/api";
import { getToken, isAuthenticated } from "@/lib/auth";

const positions = ["ALL", "GK", "DEF", "MID", "FWD"] as const;
type AddMode = "starter" | "bench";

export default function TeamBuilderPage() {
  const router = useRouter();
  const [players, setPlayers] = useState<Player[]>([]);
  const [selected, setSelected] = useState<Player[]>([]);
  const [bench, setBench] = useState<Player[]>([]);
  const [team, setTeam] = useState<FantasyTeam | null>(null);
  const [filter, setFilter] = useState<string>("ALL");
  const [search, setSearch] = useState("");
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const [success, setSuccess] = useState("");
  const [addMode, setAddMode] = useState<AddMode>("starter");

  const loadData = useCallback(async () => {
    const token = getToken();
    if (!token) return;

    try {
      const allPlayers = await getPlayers();
      setPlayers(allPlayers);

      try {
        const myTeam = await getMyTeam(token);
        setTeam(myTeam);
        setSelected(myTeam.players || []);
        setBench(myTeam.bench || []);
      } catch {
        // No team yet, that's fine
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load data");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (!isAuthenticated()) {
      router.push("/login");
      return;
    }
    loadData();
  }, [router, loadData]);

  const filteredPlayers = players.filter((p) => {
    const matchesPosition = filter === "ALL" || p.position === filter || p.secondary_position === filter;
    const matchesSearch = !search || p.name.toLowerCase().includes(search.toLowerCase()) || p.team_name.toLowerCase().includes(search.toLowerCase());
    return matchesPosition && matchesSearch;
  });

  // Combined stats across starters + bench
  const allSquad = [...selected, ...bench];
  const topPlayerCount = allSquad.filter((p) => p.is_top_player).length;
  const BUDGET = 70;
  const totalCost = allSquad.reduce((sum, p) => sum + parseFloat(p.price), 0);
  const remainingBudget = BUDGET - totalCost;
  const isOverBudget = totalCost > BUDGET;
  const benchGkCount = bench.filter((p) => p.position === "GK" || p.secondary_position === "GK").length;

  const isPlayerInSquad = (player: Player) =>
    selected.some((p) => p.id === player.id) || bench.some((p) => p.id === player.id);

  const handleSelect = (player: Player) => {
    if (isPlayerInSquad(player)) {
      setError("Player already in your squad");
      return;
    }

    if (player.is_top_player && topPlayerCount >= 2) {
      setError("Maximum 2 top players allowed per team (starters + bench combined)");
      return;
    }

    const newCost = totalCost + parseFloat(player.price);
    if (newCost > BUDGET) {
      setError(`Adding ${player.name} ($${player.price}) would exceed the $${BUDGET} budget. Remaining: $${remainingBudget.toFixed(0)}`);
      return;
    }

    if (addMode === "starter") {
      if (selected.length >= 6) {
        setError("Starting XI is full (6/6). Switch to Bench mode or remove a starter.");
        return;
      }
      setError("");
      setSelected((prev) => [...prev, player]);
    } else {
      if (bench.length >= 3) {
        setError("Bench is full (3/3). Remove a bench player first.");
        return;
      }
      // Check bench GK rule: need exactly 1 GK on bench
      const isGk = player.position === "GK";
      const currentBenchGks = bench.filter((p) => p.position === "GK").length;
      if (!isGk && currentBenchGks === 0 && bench.length === 2) {
        setError("Your last bench slot must be a GK. Bench requires exactly 1 goalkeeper.");
        return;
      }
      if (isGk && currentBenchGks >= 1) {
        setError("Bench already has 1 GK. The other 2 bench slots must be DEF/MID/FWD.");
        return;
      }
      setError("");
      setBench((prev) => [...prev, player]);
    }
  };

  const handleRemoveStarter = (player: Player) => {
    setSelected((prev) => prev.filter((p) => p.id !== player.id));
  };

  const handleRemoveBench = (player: Player) => {
    setBench((prev) => prev.filter((p) => p.id !== player.id));
  };

  const handleSave = async () => {
    if (selected.length !== 6) {
      setError("You must select exactly 6 starting players");
      return;
    }
    if (bench.length !== 3) {
      setError("You must select exactly 3 bench players");
      return;
    }
    const benchGks = bench.filter((p) => p.position === "GK").length;
    if (benchGks !== 1) {
      setError("Bench must include exactly 1 goalkeeper (GK)");
      return;
    }

    const token = getToken();
    if (!token) return;

    setSaving(true);
    setError("");
    setSuccess("");

    try {
      let currentTeam = team;
      if (!currentTeam) {
        currentTeam = await createTeam("My Squad", token) as unknown as FantasyTeam;
        setTeam(currentTeam);
      }

      const playerIds = selected.map((p) => p.id);
      const benchIds = bench.map((p) => p.id);
      const updated = await setTeamPlayers(currentTeam.id, playerIds, benchIds, token);
      setTeam(updated);
      setSelected(updated.players);
      setBench(updated.bench);
      setSuccess("Team saved successfully!");
      setTimeout(() => setSuccess(""), 3000);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to save team");
    } finally {
      setSaving(false);
    }
  };

  const isSquadComplete = selected.length === 6 && bench.length === 3;

  return (
    <div className="min-h-screen pitch-pattern">
      <Nav />
      <div className="pt-24 pb-12 max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        {/* Header */}
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          className="flex flex-col sm:flex-row items-start sm:items-center justify-between mb-8 gap-4"
        >
          <div>
            <h1 className="text-3xl font-bold" style={{ fontFamily: "var(--font-display)" }}>
              TEAM <span style={{ color: "var(--accent-green)" }}>BUILDER</span>
            </h1>
            <p className="text-sm" style={{ color: "var(--text-muted)" }}>
              Build your squad: 6 starters + 3 bench ({allSquad.length}/9)
            </p>
            <div className="flex items-center gap-4 mt-1 flex-wrap">
              <p className="text-sm flex items-center gap-1.5" style={{ color: topPlayerCount >= 2 ? "#f59e0b" : "var(--text-muted)" }}>
                <Crown size={14} style={{ color: "#fbbf24" }} />
                Top players: {topPlayerCount}/2
              </p>
              <p className="text-sm flex items-center gap-1.5" style={{ color: isOverBudget ? "var(--danger)" : remainingBudget < 10 ? "#f59e0b" : "var(--text-muted)" }}>
                <DollarSign size={14} style={{ color: isOverBudget ? "var(--danger)" : "var(--accent-green)" }} />
                Budget: ${totalCost.toFixed(0)}/${BUDGET} (${remainingBudget.toFixed(0)} left)
              </p>
            </div>
          </div>

          <button
            onClick={handleSave}
            disabled={saving || !isSquadComplete || isOverBudget}
            className="btn-primary flex items-center gap-2 text-sm disabled:opacity-50"
          >
            <Save size={16} />
            {saving ? "Saving..." : "Save Team"}
          </button>
        </motion.div>

        {/* Notifications */}
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
              <button onClick={() => setError("")} className="ml-auto bg-transparent border-none cursor-pointer" style={{ color: "var(--danger)" }}>×</button>
            </motion.div>
          )}
          {success && (
            <motion.div
              initial={{ opacity: 0, y: -10 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0 }}
              className="flex items-center gap-2 p-3 rounded-lg mb-6 text-sm"
              style={{ background: "rgba(0, 230, 118, 0.1)", border: "1px solid rgba(0, 230, 118, 0.3)", color: "var(--accent-green)" }}
            >
              <Check size={16} />
              {success}
            </motion.div>
          )}
        </AnimatePresence>

        <div className="grid grid-cols-1 lg:grid-cols-3 gap-8">
          {/* Formation + Bench preview */}
          <motion.div
            initial={{ opacity: 0, x: -20 }}
            animate={{ opacity: 1, x: 0 }}
            transition={{ delay: 0.2 }}
            className="order-2 lg:order-1"
          >
            <div className="sticky top-24">
              {/* Starting XI */}
              <h3 className="text-sm uppercase tracking-wider mb-4 flex items-center gap-2" style={{ fontFamily: "var(--font-display)", color: "var(--text-muted)" }}>
                <Users size={14} />
                Starting XI ({selected.length}/6)
              </h3>
              <Formation players={selected} onRemove={handleRemoveStarter} />

              {/* Starter list */}
              <div className="mt-4 space-y-2">
                {selected.map((player, i) => (
                  <motion.div
                    key={player.id}
                    initial={{ opacity: 0, x: -10 }}
                    animate={{ opacity: 1, x: 0 }}
                    transition={{ delay: i * 0.05 }}
                    className="flex items-center gap-2 p-2 rounded-lg text-sm"
                    style={{ background: "var(--bg-secondary)", border: "1px solid var(--border-color)" }}
                  >
                    <span className={`badge-${player.position.toLowerCase()} text-[10px] font-bold px-1.5 py-0.5 rounded text-white`}>
                      {player.position}
                    </span>
                    {player.is_top_player && (
                      <Crown size={12} style={{ color: "#fbbf24", flexShrink: 0 }} />
                    )}
                    <span className="flex-1 truncate">{player.name}</span>
                    <span className="text-[10px] font-bold" style={{ color: "var(--text-muted)" }}>${player.price}</span>
                    <button
                      onClick={() => handleRemoveStarter(player)}
                      className="text-xs bg-transparent border-none cursor-pointer"
                      style={{ color: "var(--danger)" }}
                    >
                      ×
                    </button>
                  </motion.div>
                ))}
              </div>

              {/* Bench section */}
              <h3 className="text-sm uppercase tracking-wider mt-6 mb-3 flex items-center gap-2" style={{ fontFamily: "var(--font-display)", color: "var(--text-muted)" }}>
                <Armchair size={14} />
                Bench ({bench.length}/3)
                <span className="text-[10px] font-normal" style={{ color: "var(--text-muted)" }}>
                  — 1 GK + 2 outfield
                </span>
              </h3>
              <div className="space-y-2">
                {bench.length === 0 && (
                  <div className="p-3 rounded-lg text-center text-xs" style={{ background: "var(--bg-secondary)", border: "1px dashed var(--border-color)", color: "var(--text-muted)" }}>
                    Switch to &quot;Bench&quot; mode below to add bench players
                  </div>
                )}
                {bench.map((player, i) => (
                  <motion.div
                    key={player.id}
                    initial={{ opacity: 0, x: -10 }}
                    animate={{ opacity: 1, x: 0 }}
                    transition={{ delay: i * 0.05 }}
                    className="flex items-center gap-2 p-2 rounded-lg text-sm"
                    style={{ background: "rgba(99,102,241,0.08)", border: "1px solid rgba(99,102,241,0.2)" }}
                  >
                    <span className={`badge-${player.position.toLowerCase()} text-[10px] font-bold px-1.5 py-0.5 rounded text-white`}>
                      {player.position}
                    </span>
                    {player.is_top_player && (
                      <Crown size={12} style={{ color: "#fbbf24", flexShrink: 0 }} />
                    )}
                    <span className="flex-1 truncate">{player.name}</span>
                    <span className="text-[10px] font-bold" style={{ color: "var(--text-muted)" }}>${player.price}</span>
                    <button
                      onClick={() => handleRemoveBench(player)}
                      className="text-xs bg-transparent border-none cursor-pointer"
                      style={{ color: "var(--danger)" }}
                    >
                      ×
                    </button>
                  </motion.div>
                ))}
              </div>

              {/* Budget total */}
              {allSquad.length > 0 && (
                <div className="flex items-center justify-between p-2 rounded-lg text-sm font-bold mt-4" style={{ background: "var(--bg-elevated)", border: `1px solid ${isOverBudget ? "rgba(255,82,82,0.4)" : "var(--border-color)"}` }}>
                  <span style={{ color: "var(--text-muted)" }}>Total ({allSquad.length}/9)</span>
                  <span style={{ color: isOverBudget ? "var(--danger)" : "var(--accent-green)", fontFamily: "var(--font-display)" }}>
                    ${totalCost.toFixed(0)} / ${BUDGET}
                  </span>
                </div>
              )}
            </div>
          </motion.div>

          {/* Player catalog */}
          <div className="lg:col-span-2 order-1 lg:order-2">
            {/* Add Mode Toggle */}
            <div className="flex gap-2 mb-4">
              <button
                onClick={() => setAddMode("starter")}
                className="flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-bold transition-all cursor-pointer border-none"
                style={{
                  fontFamily: "var(--font-display)",
                  background: addMode === "starter" ? "var(--accent-green)" : "var(--bg-secondary)",
                  color: addMode === "starter" ? "var(--bg-primary)" : "var(--text-muted)",
                  border: addMode === "starter" ? "none" : "1px solid var(--border-color)",
                }}
              >
                <Users size={14} />
                STARTERS ({selected.length}/6)
              </button>
              <button
                onClick={() => setAddMode("bench")}
                className="flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-bold transition-all cursor-pointer border-none"
                style={{
                  fontFamily: "var(--font-display)",
                  background: addMode === "bench" ? "rgba(99,102,241,0.8)" : "var(--bg-secondary)",
                  color: addMode === "bench" ? "white" : "var(--text-muted)",
                  border: addMode === "bench" ? "none" : "1px solid var(--border-color)",
                }}
              >
                <Armchair size={14} />
                BENCH ({bench.length}/3)
                {bench.length < 3 && benchGkCount === 0 && (
                  <span className="text-[10px] font-normal opacity-80">needs 1 GK</span>
                )}
              </button>
            </div>

            {/* Search & Filter */}
            <div className="flex flex-col sm:flex-row gap-3 mb-6">
              <div className="relative flex-1">
                <Search size={16} className="absolute left-4 top-1/2 -translate-y-1/2" style={{ color: "var(--text-muted)" }} />
                <input
                  type="text"
                  value={search}
                  onChange={(e) => setSearch(e.target.value)}
                  className="input-field pl-11"
                  placeholder="Search players or teams..."
                />
              </div>
              <div className="flex gap-1">
                {positions.map((pos) => (
                  <button
                    key={pos}
                    onClick={() => setFilter(pos)}
                    className="px-3 py-2 rounded-lg text-xs font-bold transition-all cursor-pointer border-none"
                    style={{
                      fontFamily: "var(--font-display)",
                      background: filter === pos ? "var(--accent-green)" : "var(--bg-secondary)",
                      color: filter === pos ? "var(--bg-primary)" : "var(--text-muted)",
                      border: filter === pos ? "none" : "1px solid var(--border-color)",
                    }}
                  >
                    {pos}
                  </button>
                ))}
              </div>
            </div>

            {/* Player list */}
            {loading ? (
              <div className="flex items-center justify-center py-20">
                <div className="w-8 h-8 border-2 border-t-transparent rounded-full animate-spin" style={{ borderColor: "var(--accent-green)", borderTopColor: "transparent" }} />
              </div>
            ) : (
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                {filteredPlayers.map((player, i) => (
                  <PlayerCard
                    key={player.id}
                    player={player}
                    selected={isPlayerInSquad(player)}
                    onSelect={handleSelect}
                    onRemove={(p) => {
                      if (selected.some((s) => s.id === p.id)) handleRemoveStarter(p);
                      else handleRemoveBench(p);
                    }}
                    delay={i * 0.03}
                  />
                ))}
                {filteredPlayers.length === 0 && (
                  <div className="col-span-2 text-center py-12">
                    <Filter size={32} className="mx-auto mb-3" style={{ color: "var(--text-muted)" }} />
                    <p style={{ color: "var(--text-muted)" }}>No players found matching your filters</p>
                  </div>
                )}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
