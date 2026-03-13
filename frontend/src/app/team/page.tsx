"use client";

import { useEffect, useState, useCallback } from "react";
import { useRouter } from "next/navigation";
import { motion, AnimatePresence } from "framer-motion";
import { Search, Filter, Save, AlertCircle, Check, Crown, DollarSign, Users, Armchair, ChevronDown, Shield, Lock, Zap, Flame, ArrowLeftRight, X } from "lucide-react";
import Nav from "@/components/nav";
import PlayerCard from "@/components/player-card";
import Formation, { type FormationPlayer, getFormationLabel, getMissingPositions } from "@/components/formation";
import {
  getPlayers,
  getMyTeam,
  createTeam,
  setTeamPlayers,
  getLockStatus,
  getChipStatus,
  activateChip,
  deactivateChip,
  getTransferStatus,
  transferPlayer,
  type Player,
  type FantasyTeam,
  type Position,
  type StarterAssignment,
  type LockStatus,
  type ChipStatus,
  type TransferStatus,
} from "@/lib/api";
import { getToken, getUser, isAuthenticated } from "@/lib/auth";

const positions = ["ALL", "GK", "DEF", "MID", "FWD"] as const;
type AddMode = "starter" | "bench";

/** Determine which positions a player can play. */
function getPlayablePositions(player: Player): Position[] {
  const result: Position[] = [player.position];
  if (player.secondary_position && player.secondary_position !== player.position) {
    result.push(player.secondary_position);
  }
  return result;
}

export default function TeamBuilderPage() {
  const router = useRouter();
  const [players, setPlayers] = useState<Player[]>([]);
  const [selected, setSelected] = useState<FormationPlayer[]>([]);
  const [bench, setBench] = useState<Player[]>([]);
  const [team, setTeam] = useState<FantasyTeam | null>(null);
  const [filter, setFilter] = useState<string>("ALL");
  const [search, setSearch] = useState("");
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const [success, setSuccess] = useState("");
  const [addMode, setAddMode] = useState<AddMode>("starter");

  // Position picker state for dual-position players
  const [pendingPlayer, setPendingPlayer] = useState<Player | null>(null);
  // Captain state
  const [captainId, setCaptainId] = useState<string | null>(null);
  // Lineup lock state
  const [lockStatus, setLockStatus] = useState<LockStatus | null>(null);
  // Chip state
  const [chipStatus, setChipStatus] = useState<ChipStatus | null>(null);
  const [activatingChip, setActivatingChip] = useState<string | null>(null);
  // Transfer state
  const [transferStatus, setTransferStatus] = useState<TransferStatus | null>(null);
  const [transferOutPlayer, setTransferOutPlayer] = useState<Player | null>(null);
  const [transferOutIsBench, setTransferOutIsBench] = useState(false);
  const [transferPending, setTransferPending] = useState(false);

  const loadData = useCallback(async () => {
    const token = getToken();
    if (!token) return;

    try {
      const allPlayers = await getPlayers();
      setPlayers(allPlayers);

      try {
        const myTeam = await getMyTeam(token);
        setTeam(myTeam);
        // Convert StarterPlayer[] to FormationPlayer[] using assigned_position from API
        setSelected(
          (myTeam.players || []).map((sp) => ({
            player: sp,
            assignedPosition: sp.assigned_position ?? sp.position,
          }))
        );
        setBench(myTeam.bench || []);
        setCaptainId(myTeam.captain_id || null);
        // Load chip status
        try {
          const chips = await getChipStatus(myTeam.id, token);
          setChipStatus(chips);
        } catch {
          // Chips not loaded, that's fine
        }
        // Load transfer status
        try {
          const ts = await getTransferStatus(myTeam.id, token);
          setTransferStatus(ts);
        } catch {
          // Transfer status not loaded, that's fine
        }
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
    getLockStatus().then(setLockStatus).catch(() => {});
  }, [router, loadData]);

  const filteredPlayers = players.filter((p) => {
    const matchesPosition = filter === "ALL" || p.position === filter || p.secondary_position === filter;
    const matchesSearch =
      !search ||
      p.name.toLowerCase().includes(search.toLowerCase()) ||
      p.team_name.toLowerCase().includes(search.toLowerCase());
    return matchesPosition && matchesSearch;
  });

  // Combined stats across starters + bench
  const allSquadPlayers = [...selected.map((fp) => fp.player), ...bench];
  const topPlayerCount = allSquadPlayers.filter((p) => p.is_top_player).length;
  const BUDGET = 70;
  const totalCost = allSquadPlayers.reduce((sum, p) => sum + parseFloat(p.price), 0);
  const remainingBudget = BUDGET - totalCost;
  const isOverBudget = totalCost > BUDGET;
  const benchGkCount = bench.filter((p) => p.position === "GK" || p.secondary_position === "GK").length;

  const isPlayerInSquad = (player: Player) =>
    selected.some((fp) => fp.player.id === player.id) || bench.some((p) => p.id === player.id);

  // Formation info
  const formationLabel = getFormationLabel(selected);
  const missingPositions = getMissingPositions(selected);

  /** Add a player as a starter with the chosen position. */
  const addStarter = (player: Player, position: Position) => {
    setSelected((prev) => [...prev, { player, assignedPosition: position }]);
    setError("");
    setPendingPlayer(null);
  };

  const handleSelect = (player: Player) => {
    if (lockStatus?.locked) {
      setError("Lineup changes are locked until Sunday 12:00 PM ET");
      return;
    }

    // Transfer mode: clicking a catalog player completes the swap
    if (transferMode && transferOutPlayer) {
      if (isPlayerInSquad(player)) {
        setError("Player already in your squad");
        return;
      }
      if (!transferOutIsBench) {
        const playable = getPlayablePositions(player);
        if (playable.length > 1) {
          setPendingPlayer(player);
          return;
        }
        handleCompleteTransfer(player, playable[0]);
      } else {
        handleCompleteTransfer(player);
      }
      return;
    }

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
      setError(
        `Adding ${player.name} ($${player.price}) would exceed the $${BUDGET} budget. Remaining: $${remainingBudget.toFixed(0)}`
      );
      return;
    }

    if (addMode === "starter") {
      if (selected.length >= 6) {
        setError("Starting XI is full (6/6). Switch to Bench mode or remove a starter.");
        return;
      }

      const playable = getPlayablePositions(player);

      if (playable.length === 1) {
        // Single-position player: auto-assign
        addStarter(player, playable[0]);
      } else {
        // Dual-position player: show position picker
        setPendingPlayer(player);
      }
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
    setSelected((prev) => prev.filter((fp) => fp.player.id !== player.id));
    if (captainId === player.id) setCaptainId(null);
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
    if (missingPositions.length > 0) {
      setError(`Formation requires at least 1 player in each position. Missing: ${missingPositions.join(", ")}`);
      return;
    }

    const gkCount = selected.filter((fp) => fp.assignedPosition === "GK").length;
    if (gkCount !== 1) {
      setError("Starting lineup must have exactly 1 GK");
      return;
    }

    if (!captainId) {
      setError("You must choose a captain from your starting 6");
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
        currentTeam = (await createTeam("My Squad", token)) as unknown as FantasyTeam;
        setTeam(currentTeam);
      }

      const starters: StarterAssignment[] = selected.map((fp) => ({
        player_id: fp.player.id,
        assigned_position: fp.assignedPosition,
      }));
      const benchIds = bench.map((p) => p.id);
      const updated = await setTeamPlayers(currentTeam.id, starters, benchIds, captainId, token);
      setTeam(updated);
      setSelected(
        (updated.players || []).map((sp) => ({
          player: sp,
          assignedPosition: sp.assigned_position ?? sp.position,
        }))
      );
      setBench(updated.bench);
      setCaptainId(updated.captain_id || null);
      // Load chip status after saving (especially for newly created teams)
      try {
        const chips = await getChipStatus(updated.id, token);
        setChipStatus(chips);
      } catch {
        // Chips not loaded, that's fine
      }
      setSuccess("Team saved successfully!");
      setTimeout(() => setSuccess(""), 3000);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to save team");
    } finally {
      setSaving(false);
    }
  };

  /** Check if a player can be captain (name must not match user's full name). */
  const canBeCaptain = (player: Player): boolean => {
    const user = getUser();
    if (!user?.full_name) return true;
    return player.name.trim().toLowerCase() !== user.full_name.trim().toLowerCase();
  };

  const handleSetCaptain = (playerId: string) => {
    const fp = selected.find((fp) => fp.player.id === playerId);
    if (!fp) return;
    if (!canBeCaptain(fp.player)) {
      setError(`You cannot captain ${fp.player.name} because they share your name. Choose a different captain.`);
      return;
    }
    setCaptainId(playerId);
    setError("");
  };

  const handleActivateChip = async (chipType: "triple_captain" | "bench_boost") => {
    if (!team) {
      setError("Save your team first before activating chips");
      return;
    }
    const token = getToken();
    if (!token) return;

    setActivatingChip(chipType);
    setError("");
    try {
      const updated = await activateChip(team.id, chipType, token);
      setChipStatus(updated);
      const chipLabel = chipType === "triple_captain" ? "Triple Captain" : "Bench Boost";
      setSuccess(`${chipLabel} activated for Gameweek ${updated.active_gameweek?.week_number ?? "?"}!`);
      setTimeout(() => setSuccess(""), 4000);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to activate chip");
    } finally {
      setActivatingChip(null);
    }
  };

  const handleDeactivateChip = async (chipType: "triple_captain" | "bench_boost") => {
    if (!team) return;
    const token = getToken();
    if (!token) return;

    setActivatingChip(chipType);
    setError("");
    try {
      const updated = await deactivateChip(team.id, chipType, token);
      setChipStatus(updated);
      const chipLabel = chipType === "triple_captain" ? "Triple Captain" : "Bench Boost";
      setSuccess(`${chipLabel} deactivated. You can use it in a future gameweek.`);
      setTimeout(() => setSuccess(""), 4000);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to deactivate chip");
    } finally {
      setActivatingChip(null);
    }
  };

  const isGameweekActive = transferStatus?.active_gameweek != null;
  const hasExistingSquad = selected.length === 6 && bench.length === 3;
  const transferMode = isGameweekActive && hasExistingSquad;

  const handleStartTransfer = (player: Player, isBench: boolean) => {
    if (!transferStatus?.transfer_available) {
      setError("You have already used your 1 free transfer this gameweek");
      return;
    }
    if (lockStatus?.locked) {
      setError("Transfers are locked until Sunday 12:00 PM ET");
      return;
    }
    if (captainId === player.id) {
      setError("Cannot transfer out your captain. Change your captain first.");
      return;
    }
    setTransferOutPlayer(player);
    setTransferOutIsBench(isBench);
    setError("");
  };

  const handleCompleteTransfer = async (playerIn: Player, assignedPos?: Position) => {
    if (!transferOutPlayer || !team) return;
    const token = getToken();
    if (!token) return;

    setTransferPending(true);
    setError("");
    setSuccess("");

    try {
      const posToSend = transferOutIsBench ? null : (assignedPos ?? null);
      const updated = await transferPlayer(team.id, transferOutPlayer.id, playerIn.id, posToSend, token);
      setTeam(updated);
      setSelected(
        (updated.players || []).map((sp) => ({
          player: sp,
          assignedPosition: sp.assigned_position ?? sp.position,
        }))
      );
      setBench(updated.bench);
      setCaptainId(updated.captain_id || null);
      setTransferOutPlayer(null);
      setSuccess(`Transfer complete: ${transferOutPlayer.name} out, ${playerIn.name} in!`);
      setTimeout(() => setSuccess(""), 4000);
      // Reload transfer status
      try {
        const ts = await getTransferStatus(updated.id, token);
        setTransferStatus(ts);
      } catch { /* ignore */ }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Transfer failed");
    } finally {
      setTransferPending(false);
    }
  };

  /** Find the assigned position for a player if they're in the starting 6. */
  const getAssignedPosition = (playerId: string): Position | undefined => {
    return selected.find((fp) => fp.player.id === playerId)?.assignedPosition;
  };

  const isSquadComplete = selected.length === 6 && bench.length === 3 && missingPositions.length === 0 && !!captainId;

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
              Build your squad: 6 starters + 3 bench ({allSquadPlayers.length}/9)
              {formationLabel && (
                <span className="ml-2 font-bold" style={{ color: "var(--accent-green)", fontFamily: "var(--font-display)" }}>
                  Formation: {formationLabel}
                </span>
              )}
            </p>
            <div className="flex items-center gap-4 mt-1 flex-wrap">
              <p
                className="text-sm flex items-center gap-1.5"
                style={{ color: topPlayerCount >= 2 ? "#f59e0b" : "var(--text-muted)" }}
              >
                <Crown size={14} style={{ color: "#fbbf24" }} />
                Top players: {topPlayerCount}/2
              </p>
              <p
                className="text-sm flex items-center gap-1.5"
                style={{
                  color: isOverBudget ? "var(--danger)" : remainingBudget < 10 ? "#f59e0b" : "var(--text-muted)",
                }}
              >
                <DollarSign size={14} style={{ color: isOverBudget ? "var(--danger)" : "var(--accent-green)" }} />
                Budget: ${totalCost.toFixed(0)}/${BUDGET} (${remainingBudget.toFixed(0)} left)
              </p>
            </div>
          </div>

          <button
            onClick={handleSave}
            disabled={saving || !isSquadComplete || isOverBudget || lockStatus?.locked}
            className="btn-primary flex items-center gap-2 text-sm disabled:opacity-50"
          >
            {lockStatus?.locked ? <Lock size={16} /> : <Save size={16} />}
            {lockStatus?.locked ? "Locked" : saving ? "Saving..." : "Save Team"}
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
              style={{
                background: "rgba(255, 82, 82, 0.1)",
                border: "1px solid rgba(255, 82, 82, 0.3)",
                color: "var(--danger)",
              }}
            >
              <AlertCircle size={16} />
              {error}
              <button
                onClick={() => setError("")}
                className="ml-auto bg-transparent border-none cursor-pointer"
                style={{ color: "var(--danger)" }}
              >
                x
              </button>
            </motion.div>
          )}
          {success && (
            <motion.div
              initial={{ opacity: 0, y: -10 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0 }}
              className="flex items-center gap-2 p-3 rounded-lg mb-6 text-sm"
              style={{
                background: "rgba(0, 230, 118, 0.1)",
                border: "1px solid rgba(0, 230, 118, 0.3)",
                color: "var(--accent-green)",
              }}
            >
              <Check size={16} />
              {success}
            </motion.div>
          )}
        </AnimatePresence>

        {/* Lineup Lock Banner */}
        {lockStatus?.locked && (
          <motion.div
            initial={{ opacity: 0, y: -10 }}
            animate={{ opacity: 1, y: 0 }}
            className="flex items-center gap-3 p-4 rounded-lg mb-6"
            style={{
              background: "rgba(255, 171, 0, 0.1)",
              border: "1px solid rgba(255, 171, 0, 0.3)",
            }}
          >
            <Lock size={20} style={{ color: "var(--accent-amber)", flexShrink: 0 }} />
            <div>
              <p className="text-sm font-bold" style={{ fontFamily: "var(--font-display)", color: "var(--accent-amber)" }}>
                LINEUP LOCKED
              </p>
              <p className="text-xs" style={{ color: "var(--text-muted)" }}>
                Lineup changes are locked from Saturday midnight to Sunday 12:00 PM ET
              </p>
            </div>
          </motion.div>
        )}

        {/* Transfer Status Banner */}
        {transferMode && !lockStatus?.locked && (
          <motion.div
            initial={{ opacity: 0, y: -10 }}
            animate={{ opacity: 1, y: 0 }}
            className="flex items-center gap-3 p-4 rounded-lg mb-6"
            style={{
              background: transferStatus?.transfer_available
                ? "rgba(0, 230, 118, 0.08)"
                : "rgba(255, 171, 0, 0.08)",
              border: transferStatus?.transfer_available
                ? "1px solid rgba(0, 230, 118, 0.3)"
                : "1px solid rgba(255, 171, 0, 0.3)",
            }}
          >
            <ArrowLeftRight
              size={20}
              style={{
                color: transferStatus?.transfer_available ? "var(--accent-green)" : "var(--accent-amber)",
                flexShrink: 0,
              }}
            />
            <div className="flex-1">
              <p className="text-sm font-bold" style={{ fontFamily: "var(--font-display)", color: transferStatus?.transfer_available ? "var(--accent-green)" : "var(--accent-amber)" }}>
                {transferStatus?.transfer_available
                  ? `GAMEWEEK ${transferStatus.active_gameweek} — 1 FREE TRANSFER`
                  : `GAMEWEEK ${transferStatus?.active_gameweek} — TRANSFER USED`}
              </p>
              {transferStatus?.transfer_available ? (
                <p className="text-xs" style={{ color: "var(--text-muted)" }}>
                  {transferOutPlayer
                    ? `Select a replacement for ${transferOutPlayer.name} from the player list`
                    : "Tap a squad player below to start a transfer"}
                </p>
              ) : (
                <p className="text-xs" style={{ color: "var(--text-muted)" }}>
                  {transferStatus?.transferred_out} out, {transferStatus?.transferred_in} in
                </p>
              )}
            </div>
            {transferOutPlayer && (
              <button
                onClick={() => { setTransferOutPlayer(null); setError(""); }}
                className="p-1.5 rounded-lg cursor-pointer bg-transparent border-none"
                style={{ color: "var(--text-muted)" }}
                title="Cancel transfer"
              >
                <X size={16} />
              </button>
            )}
          </motion.div>
        )}

        {/* Transfer Out Selection Indicator */}
        <AnimatePresence>
          {transferOutPlayer && (
            <motion.div
              initial={{ opacity: 0, y: -10 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0 }}
              className="flex items-center gap-3 p-3 rounded-lg mb-6 text-sm"
              style={{
                background: "rgba(239, 68, 68, 0.08)",
                border: "1px solid rgba(239, 68, 68, 0.3)",
                color: "var(--danger)",
              }}
            >
              <ArrowLeftRight size={16} />
              <span>
                Transferring out <strong>{transferOutPlayer.name}</strong> — now pick a replacement from the player list
              </span>
              {transferPending && (
                <div className="ml-auto w-4 h-4 border-2 border-t-transparent rounded-full animate-spin" style={{ borderColor: "var(--danger)", borderTopColor: "transparent" }} />
              )}
            </motion.div>
          )}
        </AnimatePresence>

        {/* Position Picker Modal */}
        <AnimatePresence>
          {pendingPlayer && (
            <motion.div
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              className="fixed inset-0 z-50 flex items-center justify-center"
              style={{ background: "rgba(0,0,0,0.7)", backdropFilter: "blur(4px)" }}
              onClick={() => setPendingPlayer(null)}
            >
              <motion.div
                initial={{ scale: 0.8, opacity: 0 }}
                animate={{ scale: 1, opacity: 1 }}
                exit={{ scale: 0.8, opacity: 0 }}
                transition={{ type: "spring", stiffness: 300, damping: 25 }}
                className="glass-card p-6 max-w-sm w-full mx-4"
                onClick={(e) => e.stopPropagation()}
              >
                <h3
                  className="text-lg font-bold mb-1"
                  style={{ fontFamily: "var(--font-display)", color: "var(--text-primary)" }}
                >
                  CHOOSE POSITION
                </h3>
                <p className="text-sm mb-5" style={{ color: "var(--text-muted)" }}>
                  <span className="font-semibold" style={{ color: "var(--accent-green)" }}>
                    {pendingPlayer.name}
                  </span>{" "}
                  can play multiple positions. Where should they play?
                </p>

                <div className="flex gap-3">
                  {getPlayablePositions(pendingPlayer).map((pos) => {
                    const badgeClass =
                      pos === "GK"
                        ? "badge-gk"
                        : pos === "DEF"
                        ? "badge-def"
                        : pos === "MID"
                        ? "badge-mid"
                        : "badge-fwd";
                    return (
                      <button
                        key={pos}
                        onClick={() => {
                          if (transferOutPlayer) {
                            handleCompleteTransfer(pendingPlayer, pos);
                            setPendingPlayer(null);
                          } else {
                            addStarter(pendingPlayer, pos);
                          }
                        }}
                        className={`flex-1 ${badgeClass} text-white font-bold py-3 rounded-xl text-sm transition-all hover:scale-105 cursor-pointer border-none`}
                        style={{ fontFamily: "var(--font-display)", letterSpacing: "0.05em" }}
                      >
                        <ChevronDown size={14} className="inline mr-1 opacity-60" />
                        {pos}
                      </button>
                    );
                  })}
                </div>

                <button
                  onClick={() => setPendingPlayer(null)}
                  className="w-full mt-3 py-2 rounded-lg text-sm font-medium cursor-pointer bg-transparent"
                  style={{ color: "var(--text-muted)", border: "1px solid var(--border-color)" }}
                >
                  Cancel
                </button>
              </motion.div>
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
              <h3
                className="text-sm uppercase tracking-wider mb-4 flex items-center gap-2"
                style={{ fontFamily: "var(--font-display)", color: "var(--text-muted)" }}
              >
                <Users size={14} />
                Starting XI ({selected.length}/6)
                {formationLabel && (
                  <span
                    className="ml-auto text-xs px-2 py-0.5 rounded-full"
                    style={{
                      background: "rgba(0,230,118,0.15)",
                      color: "var(--accent-green)",
                      fontFamily: "var(--font-display)",
                    }}
                  >
                    {formationLabel}
                  </span>
                )}
              </h3>
              <Formation players={selected} captainId={captainId} onRemove={handleRemoveStarter} />

              {/* Captain prompt */}
              {selected.length > 0 && !captainId && (
                <div
                  className="mt-3 p-2 rounded-lg text-xs text-center animate-pulse"
                  style={{
                    background: "rgba(255,171,0,0.1)",
                    border: "1px solid rgba(255,171,0,0.3)",
                    color: "#ffab00",
                    fontFamily: "var(--font-display)",
                  }}
                >
                  <Shield size={12} className="inline mr-1" />
                  TAP A STARTER TO SET CAPTAIN
                </div>
              )}

              {/* Starter list */}
              <div className="mt-4 space-y-2">
                {selected.map((fp, i) => {
                  const isCaptain = captainId === fp.player.id;
                  const canCaptain = canBeCaptain(fp.player);
                  return (
                    <motion.div
                      key={fp.player.id}
                      initial={{ opacity: 0, x: -10 }}
                      animate={{ opacity: 1, x: 0 }}
                      transition={{ delay: i * 0.05 }}
                      className="flex items-center gap-2 p-2 rounded-lg text-sm"
                      style={{
                        background: isCaptain ? "rgba(255,171,0,0.08)" : "var(--bg-secondary)",
                        border: isCaptain ? "1px solid rgba(255,171,0,0.4)" : "1px solid var(--border-color)",
                      }}
                    >
                      {/* Captain toggle button */}
                      <button
                        onClick={(e) => { e.stopPropagation(); handleSetCaptain(fp.player.id); }}
                        className="flex items-center justify-center w-5 h-5 rounded-full text-[8px] font-black border-none cursor-pointer transition-all"
                        title={!canCaptain ? "Cannot captain — shares your name" : isCaptain ? "Captain" : "Set as captain"}
                        style={{
                          background: isCaptain
                            ? "linear-gradient(135deg, #fbbf24, #f59e0b)"
                            : canCaptain
                            ? "rgba(255,255,255,0.1)"
                            : "rgba(255,82,82,0.15)",
                          color: isCaptain ? "#1a1a2e" : canCaptain ? "var(--text-muted)" : "rgba(255,82,82,0.5)",
                          boxShadow: isCaptain ? "0 0 8px rgba(251,191,36,0.5)" : "none",
                          opacity: canCaptain ? 1 : 0.5,
                        }}
                      >
                        C
                      </button>
                      <span
                        className={`badge-${fp.assignedPosition.toLowerCase()} text-[10px] font-bold px-1.5 py-0.5 rounded text-white`}
                      >
                        {fp.assignedPosition}
                      </span>
                      {fp.assignedPosition !== fp.player.position && (
                        <span
                          className="text-[9px] font-bold px-1 rounded"
                          style={{ background: "rgba(255,171,0,0.2)", color: "#ffab00" }}
                        >
                          FLEX
                        </span>
                      )}
                      {fp.player.is_top_player && (
                        <Crown size={12} style={{ color: "#fbbf24", flexShrink: 0 }} />
                      )}
                      <span className="flex-1 truncate">{fp.player.name}</span>
                      <span className="text-[10px] font-bold" style={{ color: "var(--text-muted)" }}>
                        ${fp.player.price}
                      </span>
                      {transferMode ? (
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            handleStartTransfer(fp.player, false);
                          }}
                          disabled={!transferStatus?.transfer_available || transferOutPlayer?.id === fp.player.id}
                          className="flex items-center gap-1 px-2 py-1 rounded text-[10px] font-bold cursor-pointer border-none transition-all disabled:opacity-30"
                          style={{
                            fontFamily: "var(--font-display)",
                            background: transferOutPlayer?.id === fp.player.id
                              ? "rgba(239,68,68,0.2)"
                              : "rgba(0,230,118,0.1)",
                            color: transferOutPlayer?.id === fp.player.id
                              ? "var(--danger)"
                              : "var(--accent-green)",
                          }}
                        >
                          <ArrowLeftRight size={10} />
                          {transferOutPlayer?.id === fp.player.id ? "OUT" : "SWAP"}
                        </button>
                      ) : (
                        <button
                          onClick={() => handleRemoveStarter(fp.player)}
                          className="text-xs bg-transparent border-none cursor-pointer"
                          style={{ color: "var(--danger)" }}
                        >
                          x
                        </button>
                      )}
                    </motion.div>
                  );
                })}
              </div>

              {/* Position requirement indicators */}
              {selected.length > 0 && selected.length < 6 && missingPositions.length > 0 && (
                <div className="mt-3 flex flex-wrap gap-1.5">
                  {missingPositions.map((pos) => (
                    <span
                      key={pos}
                      className="text-[10px] font-bold px-2 py-1 rounded-full animate-pulse"
                      style={{
                        background: "rgba(255,82,82,0.1)",
                        border: "1px solid rgba(255,82,82,0.3)",
                        color: "#ff8a80",
                        fontFamily: "var(--font-display)",
                      }}
                    >
                      Need 1 {pos}
                    </span>
                  ))}
                </div>
              )}

              {/* Chips section */}
              {team && (
                <>
                  <h3
                    className="text-sm uppercase tracking-wider mt-6 mb-3 flex items-center gap-2"
                    style={{ fontFamily: "var(--font-display)", color: "var(--text-muted)" }}
                  >
                    <Zap size={14} style={{ color: "var(--accent-amber)" }} />
                    Chips
                    {chipStatus?.active_gameweek && (
                      <span
                        className="ml-auto text-[10px] px-2 py-0.5 rounded-full"
                        style={{
                          background: "rgba(255,171,0,0.12)",
                          color: "var(--accent-amber)",
                          fontFamily: "var(--font-display)",
                        }}
                      >
                        GW {chipStatus.active_gameweek.week_number}
                      </span>
                    )}
                  </h3>
                  <div className="space-y-2">
                    {/* Triple Captain */}
                    <div
                      className="p-3 rounded-xl transition-all"
                      style={{
                        background: chipStatus?.triple_captain.available
                          ? "rgba(255,171,0,0.06)"
                          : chipStatus?.triple_captain.can_deactivate
                          ? "rgba(255,171,0,0.04)"
                          : "rgba(255,255,255,0.02)",
                        border: chipStatus?.triple_captain.available
                          ? "1px solid rgba(255,171,0,0.25)"
                          : chipStatus?.triple_captain.can_deactivate
                          ? "1px solid rgba(255,171,0,0.15)"
                          : "1px solid var(--border-color)",
                        opacity: chipStatus?.triple_captain.available || chipStatus?.triple_captain.can_deactivate ? 1 : 0.6,
                      }}
                    >
                      <div className="flex items-center gap-2 mb-1.5">
                        <div
                          className="w-6 h-6 rounded-lg flex items-center justify-center"
                          style={{
                            background: chipStatus?.triple_captain.available
                              ? "linear-gradient(135deg, #fbbf24, #f59e0b)"
                              : "rgba(255,255,255,0.05)",
                          }}
                        >
                          <Crown size={12} style={{ color: chipStatus?.triple_captain.available ? "#1a1a2e" : "var(--text-muted)" }} />
                        </div>
                        <div className="flex-1">
                          <p className="text-xs font-bold" style={{ fontFamily: "var(--font-display)" }}>
                            TRIPLE CAPTAIN
                          </p>
                          <p className="text-[10px]" style={{ color: "var(--text-muted)" }}>
                            Captain scores 3x points
                          </p>
                        </div>
                      </div>
                      {chipStatus?.triple_captain.available ? (
                        <button
                          onClick={() => handleActivateChip("triple_captain")}
                          disabled={activatingChip === "triple_captain" || lockStatus?.locked || !chipStatus?.active_gameweek}
                          className="w-full py-1.5 rounded-lg text-[11px] font-bold cursor-pointer border-none transition-all disabled:opacity-40 disabled:cursor-not-allowed"
                          style={{
                            fontFamily: "var(--font-display)",
                            background: "linear-gradient(135deg, rgba(251,191,36,0.2), rgba(245,158,11,0.2))",
                            color: "#fbbf24",
                            letterSpacing: "0.05em",
                          }}
                        >
                          {activatingChip === "triple_captain" ? "ACTIVATING..." : "ACTIVATE"}
                        </button>
                      ) : chipStatus?.triple_captain.can_deactivate ? (
                        <div className="flex items-center gap-2">
                          <p
                            className="text-[10px] font-bold flex items-center gap-1 flex-1"
                            style={{ color: "#fbbf24", fontFamily: "var(--font-display)" }}
                          >
                            <Zap size={10} />
                            ACTIVE — GW {chipStatus?.triple_captain.used_in_week}
                          </p>
                          <button
                            onClick={() => handleDeactivateChip("triple_captain")}
                            disabled={activatingChip === "triple_captain"}
                            className="px-2 py-1 rounded-md text-[10px] font-bold cursor-pointer border-none transition-all disabled:opacity-40"
                            style={{
                              fontFamily: "var(--font-display)",
                              background: "rgba(255,82,82,0.15)",
                              color: "var(--danger)",
                            }}
                          >
                            {activatingChip === "triple_captain" ? "..." : "CANCEL"}
                          </button>
                        </div>
                      ) : (
                        <p
                          className="text-[10px] font-bold flex items-center gap-1"
                          style={{ color: "var(--accent-green)", fontFamily: "var(--font-display)" }}
                        >
                          <Check size={10} />
                          USED IN GW {chipStatus?.triple_captain.used_in_week}
                        </p>
                      )}
                    </div>

                    {/* Bench Boost */}
                    <div
                      className="p-3 rounded-xl transition-all"
                      style={{
                        background: chipStatus?.bench_boost.available
                          ? "rgba(99,102,241,0.06)"
                          : chipStatus?.bench_boost.can_deactivate
                          ? "rgba(99,102,241,0.04)"
                          : "rgba(255,255,255,0.02)",
                        border: chipStatus?.bench_boost.available
                          ? "1px solid rgba(99,102,241,0.25)"
                          : chipStatus?.bench_boost.can_deactivate
                          ? "1px solid rgba(99,102,241,0.15)"
                          : "1px solid var(--border-color)",
                        opacity: chipStatus?.bench_boost.available || chipStatus?.bench_boost.can_deactivate ? 1 : 0.6,
                      }}
                    >
                      <div className="flex items-center gap-2 mb-1.5">
                        <div
                          className="w-6 h-6 rounded-lg flex items-center justify-center"
                          style={{
                            background: chipStatus?.bench_boost.available
                              ? "linear-gradient(135deg, #818cf8, #6366f1)"
                              : "rgba(255,255,255,0.05)",
                          }}
                        >
                          <Flame size={12} style={{ color: chipStatus?.bench_boost.available ? "white" : "var(--text-muted)" }} />
                        </div>
                        <div className="flex-1">
                          <p className="text-xs font-bold" style={{ fontFamily: "var(--font-display)" }}>
                            BENCH BOOST
                          </p>
                          <p className="text-[10px]" style={{ color: "var(--text-muted)" }}>
                            Bench players also score
                          </p>
                        </div>
                      </div>
                      {chipStatus?.bench_boost.available ? (
                        <button
                          onClick={() => handleActivateChip("bench_boost")}
                          disabled={activatingChip === "bench_boost" || lockStatus?.locked || !chipStatus?.active_gameweek}
                          className="w-full py-1.5 rounded-lg text-[11px] font-bold cursor-pointer border-none transition-all disabled:opacity-40 disabled:cursor-not-allowed"
                          style={{
                            fontFamily: "var(--font-display)",
                            background: "linear-gradient(135deg, rgba(129,140,248,0.2), rgba(99,102,241,0.2))",
                            color: "#818cf8",
                            letterSpacing: "0.05em",
                          }}
                        >
                          {activatingChip === "bench_boost" ? "ACTIVATING..." : "ACTIVATE"}
                        </button>
                      ) : chipStatus?.bench_boost.can_deactivate ? (
                        <div className="flex items-center gap-2">
                          <p
                            className="text-[10px] font-bold flex items-center gap-1 flex-1"
                            style={{ color: "#818cf8", fontFamily: "var(--font-display)" }}
                          >
                            <Zap size={10} />
                            ACTIVE — GW {chipStatus?.bench_boost.used_in_week}
                          </p>
                          <button
                            onClick={() => handleDeactivateChip("bench_boost")}
                            disabled={activatingChip === "bench_boost"}
                            className="px-2 py-1 rounded-md text-[10px] font-bold cursor-pointer border-none transition-all disabled:opacity-40"
                            style={{
                              fontFamily: "var(--font-display)",
                              background: "rgba(255,82,82,0.15)",
                              color: "var(--danger)",
                            }}
                          >
                            {activatingChip === "bench_boost" ? "..." : "CANCEL"}
                          </button>
                        </div>
                      ) : (
                        <p
                          className="text-[10px] font-bold flex items-center gap-1"
                          style={{ color: "var(--accent-green)", fontFamily: "var(--font-display)" }}
                        >
                          <Check size={10} />
                          USED IN GW {chipStatus?.bench_boost.used_in_week}
                        </p>
                      )}
                    </div>
                  </div>
                </>
              )}

              {/* Bench section */}
              <h3
                className="text-sm uppercase tracking-wider mt-6 mb-3 flex items-center gap-2"
                style={{ fontFamily: "var(--font-display)", color: "var(--text-muted)" }}
              >
                <Armchair size={14} />
                Bench ({bench.length}/3)
                <span className="text-[10px] font-normal" style={{ color: "var(--text-muted)" }}>
                  — 1 GK + 2 outfield
                </span>
              </h3>
              <div className="space-y-2">
                {bench.length === 0 && (
                  <div
                    className="p-3 rounded-lg text-center text-xs"
                    style={{
                      background: "var(--bg-secondary)",
                      border: "1px dashed var(--border-color)",
                      color: "var(--text-muted)",
                    }}
                  >
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
                    <span
                      className={`badge-${player.position.toLowerCase()} text-[10px] font-bold px-1.5 py-0.5 rounded text-white`}
                    >
                      {player.position}
                    </span>
                    {player.is_top_player && (
                      <Crown size={12} style={{ color: "#fbbf24", flexShrink: 0 }} />
                    )}
                    <span className="flex-1 truncate">{player.name}</span>
                    <span className="text-[10px] font-bold" style={{ color: "var(--text-muted)" }}>
                      ${player.price}
                    </span>
                    {transferMode ? (
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          handleStartTransfer(player, true);
                        }}
                        disabled={!transferStatus?.transfer_available || transferOutPlayer?.id === player.id}
                        className="flex items-center gap-1 px-2 py-1 rounded text-[10px] font-bold cursor-pointer border-none transition-all disabled:opacity-30"
                        style={{
                          fontFamily: "var(--font-display)",
                          background: transferOutPlayer?.id === player.id
                            ? "rgba(239,68,68,0.2)"
                            : "rgba(0,230,118,0.1)",
                          color: transferOutPlayer?.id === player.id
                            ? "var(--danger)"
                            : "var(--accent-green)",
                        }}
                      >
                        <ArrowLeftRight size={10} />
                        {transferOutPlayer?.id === player.id ? "OUT" : "SWAP"}
                      </button>
                    ) : (
                      <button
                        onClick={() => handleRemoveBench(player)}
                        className="text-xs bg-transparent border-none cursor-pointer"
                        style={{ color: "var(--danger)" }}
                      >
                        x
                      </button>
                    )}
                  </motion.div>
                ))}
              </div>

              {/* Budget total */}
              {allSquadPlayers.length > 0 && (
                <div
                  className="flex items-center justify-between p-2 rounded-lg text-sm font-bold mt-4"
                  style={{
                    background: "var(--bg-elevated)",
                    border: `1px solid ${isOverBudget ? "rgba(255,82,82,0.4)" : "var(--border-color)"}`,
                  }}
                >
                  <span style={{ color: "var(--text-muted)" }}>Total ({allSquadPlayers.length}/9)</span>
                  <span
                    style={{
                      color: isOverBudget ? "var(--danger)" : "var(--accent-green)",
                      fontFamily: "var(--font-display)",
                    }}
                  >
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
                <Search
                  size={16}
                  className="absolute left-4 top-1/2 -translate-y-1/2"
                  style={{ color: "var(--text-muted)" }}
                />
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
                <div
                  className="w-8 h-8 border-2 border-t-transparent rounded-full animate-spin"
                  style={{ borderColor: "var(--accent-green)", borderTopColor: "transparent" }}
                />
              </div>
            ) : (
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                {filteredPlayers.map((player, i) => (
                  <PlayerCard
                    key={player.id}
                    player={player}
                    selected={isPlayerInSquad(player)}
                    assignedPosition={getAssignedPosition(player.id)}
                    onSelect={handleSelect}
                    onRemove={(p) => {
                      if (selected.some((fp) => fp.player.id === p.id)) handleRemoveStarter(p);
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
