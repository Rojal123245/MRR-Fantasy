"use client";

import { useEffect, useMemo, useState } from "react";
import { useRouter } from "next/navigation";
import { motion } from "framer-motion";
import { Shuffle, Users, ShieldCheck, AlertCircle } from "lucide-react";
import Nav from "@/components/nav";
import { getPlayers, type Player } from "@/lib/api";
import { isAuthenticated } from "@/lib/auth";

const ADVANCE_PLAYERS = [
  "Aashish Tangnami",
  "Rajeev Lamichhaney",
  "Razz Kumar Basnet",
  "Aashis Bhattarai",
  "Dip Kc",
] as const;

const GOALKEEPERS = [
  "Anod Shrestha",
  "Bishnu Raj Tamang",
  "Himal Puri",
  "Nitesh Das",
] as const;

type TeamSlot = {
  player: {
    id: string;
    name: string;
    team_name: string;
  };
  category: "Advance" | "GK" | "Regular";
};

type TeamCategory = "Advance" | "GK" | "Regular";

type SelectablePlayer = {
  id: string;
  name: string;
  team_name: string;
  source: "listed" | "other";
  forcedCategory?: TeamCategory;
};

function normalizeName(name: string): string {
  return name.trim().toLowerCase();
}

function shuffleArray<T>(items: T[]): T[] {
  const next = [...items];
  for (let i = next.length - 1; i > 0; i -= 1) {
    const j = Math.floor(Math.random() * (i + 1));
    [next[i], next[j]] = [next[j], next[i]];
  }
  return next;
}

export default function RandomizerPage() {
  const router = useRouter();
  const [players, setPlayers] = useState<Player[]>([]);
  const [otherPlayers, setOtherPlayers] = useState<Array<{ id: string; name: string; category: TeamCategory }>>([]);
  const [selectedPlayerIds, setSelectedPlayerIds] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [teamCount, setTeamCount] = useState(2);
  const [error, setError] = useState("");
  const [teams, setTeams] = useState<TeamSlot[][]>([]);
  const [otherName, setOtherName] = useState("");
  const [otherCategory, setOtherCategory] = useState<TeamCategory>("Regular");

  useEffect(() => {
    if (!isAuthenticated()) {
      router.push("/login");
      return;
    }

    getPlayers()
      .then((allPlayers) => {
        setPlayers(allPlayers);
        setSelectedPlayerIds(allPlayers.map((player) => player.id));
      })
      .catch((err) => setError(err instanceof Error ? err.message : "Failed to load players"))
      .finally(() => setLoading(false));
  }, [router]);

  const allSelectablePlayers = useMemo<SelectablePlayer[]>(() => {
    const listed = players.map((player) => ({
      id: player.id,
      name: player.name,
      team_name: player.team_name,
      source: "listed" as const,
    }));
    const others = otherPlayers.map((player) => ({
      id: player.id,
      name: player.name,
      team_name: "Others",
      source: "other" as const,
      forcedCategory: player.category,
    }));
    return [...listed, ...others];
  }, [players, otherPlayers]);

  const selectedPlayers = useMemo(() => {
    const selectedSet = new Set(selectedPlayerIds);
    return allSelectablePlayers.filter((player) => selectedSet.has(player.id));
  }, [allSelectablePlayers, selectedPlayerIds]);

  const categorizedPools = useMemo(() => {
    const advanceSet = new Set(ADVANCE_PLAYERS.map(normalizeName));
    const gkSet = new Set(GOALKEEPERS.map(normalizeName));

    const advancePool = selectedPlayers.filter((player) => {
      if (player.source === "other") return player.forcedCategory === "Advance";
      return advanceSet.has(normalizeName(player.name));
    });
    const gkPool = selectedPlayers.filter((player) => {
      if (player.source === "other") return player.forcedCategory === "GK";
      return gkSet.has(normalizeName(player.name));
    });
    const regularPool = selectedPlayers.filter((player) => {
      if (player.source === "other") return player.forcedCategory === "Regular";
      return !advanceSet.has(normalizeName(player.name)) && !gkSet.has(normalizeName(player.name));
    });

    const availableNames = new Set(players.map((player) => normalizeName(player.name)));
    const missingAdvance = ADVANCE_PLAYERS.filter((name) => !availableNames.has(normalizeName(name)));
    const missingGk = GOALKEEPERS.filter((name) => !availableNames.has(normalizeName(name)));

    return { advancePool, gkPool, regularPool, missingAdvance, missingGk };
  }, [players, selectedPlayers]);

  const maxTeamsPossible = Math.min(categorizedPools.advancePool.length, categorizedPools.gkPool.length);

  const togglePlayer = (playerId: string) => {
    setSelectedPlayerIds((prev) =>
      prev.includes(playerId) ? prev.filter((id) => id !== playerId) : [...prev, playerId]
    );
    setTeams([]);
  };

  const handleAddOtherPlayer = () => {
    const trimmedName = otherName.trim();
    if (!trimmedName) {
      setError("Enter a name for the player.");
      return;
    }

    const normalized = normalizeName(trimmedName);
    const alreadyExists = allSelectablePlayers.some((player) => normalizeName(player.name) === normalized);
    if (alreadyExists) {
      setError("This player name already exists in the list.");
      return;
    }

    const newPlayerId = `other-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
    setOtherPlayers((prev) => [...prev, { id: newPlayerId, name: trimmedName, category: otherCategory }]);
    setSelectedPlayerIds((prev) => [...prev, newPlayerId]);
    setOtherName("");
    setOtherCategory("Regular");
    setError("");
    setTeams([]);
  };

  const handleRemoveOtherPlayer = (playerId: string) => {
    setOtherPlayers((prev) => prev.filter((player) => player.id !== playerId));
    setSelectedPlayerIds((prev) => prev.filter((id) => id !== playerId));
    setTeams([]);
  };

  const handleRandomize = () => {
    setError("");

    if (selectedPlayers.length === 0) {
      setError("Select players who came to play first.");
      return;
    }

    if (!Number.isInteger(teamCount) || teamCount <= 0) {
      setError("Please enter a valid team count.");
      return;
    }

    if (categorizedPools.advancePool.length < teamCount) {
      setError(
        `Not enough advance players for ${teamCount} teams. Available: ${categorizedPools.advancePool.length}.`
      );
      return;
    }

    if (categorizedPools.gkPool.length < teamCount) {
      setError(
        `Not enough goalkeepers for ${teamCount} teams. Available: ${categorizedPools.gkPool.length}.`
      );
      return;
    }

    const nextTeams: TeamSlot[][] = Array.from({ length: teamCount }, () => []);
    const shuffledAdvance = shuffleArray(categorizedPools.advancePool).slice(0, teamCount);
    const shuffledGk = shuffleArray(categorizedPools.gkPool).slice(0, teamCount);
    const shuffledRegular = shuffleArray(categorizedPools.regularPool);

    for (let i = 0; i < teamCount; i += 1) {
      nextTeams[i].push({ player: shuffledAdvance[i], category: "Advance" });
      nextTeams[i].push({ player: shuffledGk[i], category: "GK" });
    }

    shuffledRegular.forEach((player, idx) => {
      const targetTeam = idx % teamCount;
      nextTeams[targetTeam].push({ player, category: "Regular" });
    });

    setTeams(nextTeams.map((team) => shuffleArray(team)));
  };

  return (
    <div className="min-h-screen pitch-pattern">
      <Nav />
      <div className="pt-24 pb-12 max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        <motion.div initial={{ opacity: 0, y: 16 }} animate={{ opacity: 1, y: 0 }} className="mb-8">
          <h1 className="text-3xl sm:text-4xl font-bold" style={{ fontFamily: "var(--font-display)" }}>
            FUTSAL <span style={{ color: "var(--accent-green)" }}>RANDOMIZER</span>
          </h1>
          <p className="mt-2 text-sm" style={{ color: "var(--text-muted)" }}>
            Select the players who came to play, then create balanced teams: exactly 1 Advance and 1 GK
            per team, and remaining players spread evenly.
          </p>
        </motion.div>

        <div className="glass-card p-5 sm:p-6 mb-6">
          <div className="mb-5">
            <h2 className="text-lg font-bold mb-2" style={{ fontFamily: "var(--font-display)" }}>
              Add Other Player
            </h2>
            <p className="text-xs mb-3" style={{ color: "var(--text-muted)" }}>
              If someone is not listed in MRR Fantasy, add them here and choose their category.
            </p>
            <div className="flex flex-col lg:flex-row gap-3">
              <input
                type="text"
                value={otherName}
                onChange={(e) => setOtherName(e.target.value)}
                placeholder="Enter player name..."
                className="input-field"
              />
              <select
                value={otherCategory}
                onChange={(e) => setOtherCategory(e.target.value as TeamCategory)}
                className="input-field lg:max-w-[180px]"
              >
                <option value="Advance">Advance</option>
                <option value="GK">GK</option>
                <option value="Regular">Regular</option>
              </select>
              <button onClick={handleAddOtherPlayer} className="btn-primary text-sm px-5 py-3">
                Add Other
              </button>
            </div>
            {otherPlayers.length > 0 && (
              <div className="mt-3 flex flex-wrap gap-2">
                {otherPlayers.map((player) => (
                  <div
                    key={player.id}
                    className="flex items-center gap-2 rounded-full px-3 py-1.5"
                    style={{ background: "rgba(59,130,246,0.12)", border: "1px solid rgba(59,130,246,0.25)" }}
                  >
                    <span className="text-xs font-medium">
                      {player.name} ({player.category})
                    </span>
                    <button
                      onClick={() => handleRemoveOtherPlayer(player.id)}
                      className="text-xs bg-transparent border-none cursor-pointer"
                      style={{ color: "var(--danger)" }}
                    >
                      x
                    </button>
                  </div>
                ))}
              </div>
            )}
          </div>

          <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3 mb-4">
            <h2 className="text-lg font-bold" style={{ fontFamily: "var(--font-display)" }}>
              Players Who Came To Play
            </h2>
            <div className="flex items-center gap-2">
              <button
                onClick={() => {
                  setSelectedPlayerIds(allSelectablePlayers.map((player) => player.id));
                  setTeams([]);
                }}
                className="btn-secondary text-[11px] py-2 px-3"
              >
                Select All
              </button>
              <button
                onClick={() => {
                  setSelectedPlayerIds([]);
                  setTeams([]);
                }}
                className="btn-secondary text-[11px] py-2 px-3"
              >
                Clear
              </button>
            </div>
          </div>

          <p className="text-xs mb-4" style={{ color: "var(--text-muted)" }}>
            Selected: {selectedPlayers.length} / {allSelectablePlayers.length}
          </p>

          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-2.5 max-h-[320px] overflow-y-auto pr-1">
            {allSelectablePlayers.map((player) => {
              const checked = selectedPlayerIds.includes(player.id);
              return (
                <button
                  key={player.id}
                  onClick={() => togglePlayer(player.id)}
                  className="w-full text-left rounded-lg px-3 py-2 transition-all border cursor-pointer"
                  style={{
                    background: checked ? "rgba(0,230,118,0.08)" : "var(--bg-elevated)",
                    borderColor: checked ? "rgba(0,230,118,0.35)" : "var(--border-color)",
                  }}
                >
                  <p className="text-sm font-medium">{player.name}</p>
                  <p className="text-[11px]" style={{ color: "var(--text-muted)" }}>
                    {player.team_name}
                    {player.source === "other" && player.forcedCategory ? ` • ${player.forcedCategory}` : ""}
                  </p>
                </button>
              );
            })}
          </div>
        </div>

        <div className="glass-card p-5 sm:p-6 mb-6">
          <div className="flex flex-col md:flex-row md:items-end gap-4">
            <div className="w-full md:max-w-xs">
              <label
                htmlFor="team-count"
                className="block text-xs uppercase tracking-[0.14em] mb-2"
                style={{ color: "var(--text-muted)", fontFamily: "var(--font-display)" }}
              >
                Number of Teams
              </label>
              <input
                id="team-count"
                type="number"
                min={1}
                value={teamCount}
                onChange={(e) => setTeamCount(Number(e.target.value))}
                className="input-field"
              />
              <p className="text-[11px] mt-2" style={{ color: "var(--text-muted)" }}>
                Max possible from selected players: {maxTeamsPossible}
              </p>
            </div>
            <button
              onClick={handleRandomize}
              disabled={loading || selectedPlayers.length === 0}
              className="btn-primary text-sm inline-flex items-center justify-center gap-2 disabled:opacity-50"
            >
              <Shuffle size={16} />
              Randomize Teams
            </button>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-3 gap-3 mt-5 text-xs">
            <div className="rounded-xl px-3 py-2" style={{ background: "rgba(0,230,118,0.08)" }}>
              <strong style={{ color: "var(--accent-green)" }}>Advance:</strong> {categorizedPools.advancePool.length}
            </div>
            <div className="rounded-xl px-3 py-2" style={{ background: "rgba(245,158,11,0.08)" }}>
              <strong style={{ color: "var(--accent-amber)" }}>GK:</strong> {categorizedPools.gkPool.length}
            </div>
            <div className="rounded-xl px-3 py-2" style={{ background: "rgba(59,130,246,0.08)" }}>
              <strong style={{ color: "#60a5fa" }}>Regular:</strong> {categorizedPools.regularPool.length}
            </div>
          </div>
        </div>

        {(error || categorizedPools.missingAdvance.length > 0 || categorizedPools.missingGk.length > 0) && (
          <div
            className="flex items-start gap-2 p-3 rounded-lg mb-6 text-sm"
            style={{
              background: "rgba(255, 82, 82, 0.1)",
              border: "1px solid rgba(255, 82, 82, 0.3)",
              color: "var(--danger)",
            }}
          >
            <AlertCircle size={16} className="mt-0.5 shrink-0" />
            <div>
              {error && <p>{error}</p>}
              {categorizedPools.missingAdvance.length > 0 && (
                <p>Missing advance players: {categorizedPools.missingAdvance.join(", ")}</p>
              )}
              {categorizedPools.missingGk.length > 0 && (
                <p>Missing goalkeepers: {categorizedPools.missingGk.join(", ")}</p>
              )}
            </div>
          </div>
        )}

        {loading ? (
          <div className="flex items-center justify-center py-20">
            <div
              className="w-8 h-8 border-2 border-t-transparent rounded-full animate-spin"
              style={{ borderColor: "var(--accent-green)", borderTopColor: "transparent" }}
            />
          </div>
        ) : teams.length > 0 ? (
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-5">
            {teams.map((team, idx) => {
              const advance = team.filter((slot) => slot.category === "Advance").length;
              const gk = team.filter((slot) => slot.category === "GK").length;
              const regular = team.filter((slot) => slot.category === "Regular").length;

              return (
                <motion.div
                  key={`team-${idx + 1}`}
                  initial={{ opacity: 0, y: 12 }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{ delay: idx * 0.05 }}
                  className="glass-card p-5"
                >
                  <div className="flex items-center justify-between mb-4">
                    <h2 className="text-xl font-bold" style={{ fontFamily: "var(--font-display)" }}>
                      Team {idx + 1}
                    </h2>
                    <div className="flex items-center gap-3 text-xs" style={{ color: "var(--text-muted)" }}>
                      <span className="inline-flex items-center gap-1">
                        <ShieldCheck size={13} style={{ color: "var(--accent-green)" }} /> A:{advance}
                      </span>
                      <span className="inline-flex items-center gap-1">
                        <Users size={13} style={{ color: "var(--accent-amber)" }} /> GK:{gk}
                      </span>
                      <span>R:{regular}</span>
                    </div>
                  </div>

                  <div className="space-y-2">
                    {team.map((slot) => (
                      <div
                        key={`${slot.player.id}-${slot.category}`}
                        className="flex items-center justify-between rounded-lg px-3 py-2"
                        style={{ background: "var(--bg-elevated)", border: "1px solid var(--border-color)" }}
                      >
                        <div>
                          <p className="text-sm font-medium">{slot.player.name}</p>
                          <p className="text-xs" style={{ color: "var(--text-muted)" }}>
                            {slot.player.team_name}
                          </p>
                        </div>
                        <span
                          className="text-[10px] font-bold px-2 py-1 rounded-full"
                          style={{
                            fontFamily: "var(--font-display)",
                            background:
                              slot.category === "Advance"
                                ? "rgba(0,230,118,0.18)"
                                : slot.category === "GK"
                                ? "rgba(245,158,11,0.18)"
                                : "rgba(59,130,246,0.18)",
                            color:
                              slot.category === "Advance"
                                ? "var(--accent-green)"
                                : slot.category === "GK"
                                ? "var(--accent-amber)"
                                : "#60a5fa",
                          }}
                        >
                          {slot.category}
                        </span>
                      </div>
                    ))}
                  </div>
                </motion.div>
              );
            })}
          </div>
        ) : (
          <div
            className="glass-card p-10 text-center text-sm"
            style={{ color: "var(--text-muted)", fontFamily: "var(--font-body)" }}
          >
            Enter team count and click <strong style={{ color: "var(--text-secondary)" }}>Randomize Teams</strong>.
          </div>
        )}
      </div>
    </div>
  );
}
