"use client";

import { motion } from "framer-motion";
import { AlertCircle, ArrowDown, ArrowUp, Loader2, Search, X } from "lucide-react";
import { useMemo, useState } from "react";

import type { Player, Position } from "@/lib/api";

const POSITION_BADGE: Record<Position, string> = {
  GK: "badge-gk",
  DEF: "badge-def",
  MID: "badge-mid",
  FWD: "badge-fwd",
};

function playablePositions(player: Player): Position[] {
  const result: Position[] = [player.position];
  if (player.secondary_position && player.secondary_position !== player.position) {
    result.push(player.secondary_position);
  }
  return result;
}

const money = (n: number) => n.toFixed(2);

/** Why a player cannot be brought in. `null` means they can. */
type Blocked = string | null;

export interface TransferSheetProps {
  outgoing: Player;
  outgoingIsBench: boolean;
  /** The starting slot being vacated, if the outgoing player was a starter. */
  outgoingSlot?: Position;
  players: Player[];
  squadPlayerIds: string[];
  /** Bench goalkeepers the squad keeps once the outgoing player has left. */
  benchGksAfterOut: number;
  /** Top players the squad keeps once the outgoing player has left. */
  topPlayersAfterOut: number;
  budgetLimit: number;
  /** Squad cost with the outgoing player already taken out. */
  squadCostAfterOut: number;
  freeTransfersLeft: number;
  /** What this transfer costs in points: 0 while a free one remains, else 4. */
  pointsCost: number;
  pending: boolean;
  error: string | null;
  onConfirm: (playerIn: Player, assignedPosition?: Position) => void;
  onClose: () => void;
}

/**
 * The whole transfer, in one surface.
 *
 * The flow this replaces asked a manager to tap a button, scroll to a list
 * shared with squad building, and commit on a single tap — with the budget, the
 * points cost and the eligibility rules all discovered afterwards, as red
 * errors. Everything needed to make the decision is here, and nothing is
 * written until Confirm.
 */
export function TransferSheet({
  outgoing,
  outgoingIsBench,
  outgoingSlot,
  players,
  squadPlayerIds,
  benchGksAfterOut,
  topPlayersAfterOut,
  budgetLimit,
  squadCostAfterOut,
  freeTransfersLeft,
  pointsCost,
  pending,
  error,
  onConfirm,
  onClose,
}: TransferSheetProps) {
  const [search, setSearch] = useState("");
  const [sortKey, setSortKey] = useState<"points" | "price" | "name">("points");
  const [sortDir, setSortDir] = useState<"asc" | "desc">("desc");
  const [showBlocked, setShowBlocked] = useState(false);
  const [picked, setPicked] = useState<Player | null>(null);
  const [playAs, setPlayAs] = useState<Position | undefined>(undefined);

  const budgetToSpend = budgetLimit - squadCostAfterOut;
  const outgoingPrice = parseFloat(outgoing.price);

  /**
   * The same five rules the server enforces, so the list tells the truth rather
   * than letting a manager pick something that will be rejected. The server
   * stays the authority — anything it refuses is shown against Confirm.
   */
  const blockedReason = (candidate: Player): Blocked => {
    if (parseFloat(candidate.price) > budgetToSpend) {
      return `$${money(parseFloat(candidate.price) - budgetToSpend)} over budget`;
    }
    if (candidate.is_top_player && topPlayersAfterOut >= 2) {
      return "Would be a third top player";
    }
    if (outgoingIsBench) {
      const incomingIsGk = candidate.position === "GK";
      const outgoingIsGk = outgoing.position === "GK";
      if (outgoingIsGk && !incomingIsGk && benchGksAfterOut === 0) {
        return "Bench must keep 1 GK";
      }
      if (!outgoingIsGk && incomingIsGk && benchGksAfterOut >= 1) {
        return "Bench already has a GK";
      }
    }
    return null;
  };

  const candidates = useMemo(() => {
    const inSquad = new Set(squadPlayerIds);
    const term = search.trim().toLowerCase();

    return players
      .filter((p) => !inSquad.has(p.id))
      .filter(
        (p) =>
          !term ||
          p.name.toLowerCase().includes(term) ||
          p.team_name.toLowerCase().includes(term)
      )
      .map((p) => ({ player: p, blocked: blockedReason(p) }))
      .sort((a, b) => {
        // Available players first, whatever the sort — an unaffordable player at
        // the top of the list is noise.
        if (!a.blocked !== !b.blocked) return a.blocked ? 1 : -1;
        const dir = sortDir === "asc" ? 1 : -1;
        const by =
          sortKey === "price"
            ? parseFloat(a.player.price) - parseFloat(b.player.price)
            : sortKey === "name"
              ? a.player.name.localeCompare(b.player.name)
              : a.player.total_points - b.player.total_points;
        return by !== 0 ? by * dir : a.player.name.localeCompare(b.player.name);
      });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [players, squadPlayerIds, search, sortKey, sortDir, budgetToSpend, topPlayersAfterOut, benchGksAfterOut]);

  const available = candidates.filter((c) => !c.blocked);
  const blocked = candidates.filter((c) => c.blocked);
  const visible = showBlocked ? candidates : available;

  const choosePlayer = (player: Player) => {
    setPicked(player);
    const options = playablePositions(player);
    // Default to the slot being vacated when they can fill it, so the common
    // case needs no decision at all.
    setPlayAs(
      outgoingIsBench
        ? undefined
        : outgoingSlot && options.includes(outgoingSlot)
          ? outgoingSlot
          : options[0]
    );
  };

  const applySort = (key: "points" | "price" | "name") => {
    if (key === sortKey) {
      setSortDir((d) => (d === "asc" ? "desc" : "asc"));
      return;
    }
    setSortKey(key);
    setSortDir(key === "name" ? "asc" : "desc");
  };

  const budgetAfter = picked
    ? budgetLimit - (squadCostAfterOut + parseFloat(picked.price))
    : budgetToSpend;

  return (
    <motion.div
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      className="fixed inset-0 z-50 flex items-end sm:items-center justify-center sm:p-4"
      style={{ background: "rgba(0,0,0,0.7)", backdropFilter: "blur(4px)" }}
      onClick={onClose}
    >
      <motion.div
        initial={{ y: "100%", opacity: 0.6 }}
        animate={{ y: 0, opacity: 1 }}
        exit={{ y: "100%", opacity: 0.6 }}
        transition={{ type: "spring", damping: 30, stiffness: 300 }}
        className="w-full sm:max-w-lg flex flex-col rounded-t-2xl sm:rounded-2xl overflow-hidden"
        style={{
          background: "var(--bg-card)",
          border: "1px solid var(--border-color)",
          maxHeight: "92vh",
        }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* ---- Header: who is leaving, and what that buys ---- */}
        <div className="p-4" style={{ borderBottom: "1px solid var(--border-color)" }}>
          <div className="flex items-center justify-between mb-3">
            <h3
              className="text-base font-bold"
              style={{ fontFamily: "var(--font-display)" }}
            >
              Transfer
            </h3>
            <button
              onClick={onClose}
              className="p-1.5 rounded-lg cursor-pointer bg-transparent border-none"
              style={{ color: "var(--text-muted)" }}
              aria-label="Cancel transfer"
            >
              <X size={18} />
            </button>
          </div>

          <div className="space-y-1.5 text-sm">
            <div className="flex items-center gap-2">
              <span
                className="text-[10px] font-bold w-8 shrink-0"
                style={{ color: "var(--text-muted)", fontFamily: "var(--font-display)" }}
              >
                OUT
              </span>
              <span
                className={`text-[10px] font-bold px-1.5 py-0.5 rounded-full text-white ${
                  POSITION_BADGE[outgoingSlot ?? outgoing.position]
                }`}
              >
                {outgoingSlot ?? outgoing.position}
              </span>
              <span className="flex-1 truncate">{outgoing.name}</span>
              <span style={{ color: "var(--text-muted)" }}>${outgoing.price}</span>
            </div>

            <div className="flex items-center gap-2">
              <span
                className="text-[10px] font-bold w-8 shrink-0"
                style={{ color: "var(--text-muted)", fontFamily: "var(--font-display)" }}
              >
                IN
              </span>
              {picked ? (
                <>
                  <span
                    className={`text-[10px] font-bold px-1.5 py-0.5 rounded-full text-white ${
                      POSITION_BADGE[playAs ?? picked.position]
                    }`}
                  >
                    {playAs ?? picked.position}
                  </span>
                  <span className="flex-1 truncate">{picked.name}</span>
                  <span style={{ color: "var(--accent-green)" }}>${picked.price}</span>
                </>
              ) : (
                <span className="flex-1" style={{ color: "var(--text-muted)" }}>
                  pick a replacement below
                </span>
              )}
            </div>
          </div>

          <div
            className="mt-3 pt-3 flex flex-wrap items-center gap-x-4 gap-y-1 text-xs"
            style={{ borderTop: "1px solid var(--border-color)" }}
          >
            <span style={{ color: "var(--text-muted)" }}>
              <strong style={{ color: "var(--text-secondary)" }}>
                ${money(budgetToSpend)}
              </strong>{" "}
              to spend
              <span className="opacity-60">
                {" "}
                (${money(Math.max(0, budgetToSpend - outgoingPrice))} free + ${outgoing.price}{" "}
                from the sale)
              </span>
            </span>
            <span
              className="font-bold"
              style={{ color: pointsCost > 0 ? "var(--accent-amber)" : "var(--accent-green)" }}
            >
              {pointsCost > 0
                ? `Costs −${pointsCost} pts`
                : `Free (${freeTransfersLeft} left)`}
            </span>
          </div>
        </div>

        {/* ---- Body: replacements that will actually be accepted ---- */}
        <div className="p-3" style={{ borderBottom: "1px solid var(--border-color)" }}>
          <div className="relative mb-2">
            <Search
              size={14}
              className="absolute left-3 top-1/2 -translate-y-1/2"
              style={{ color: "var(--text-muted)" }}
            />
            <input
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Search players or teams..."
              className="w-full pl-9 pr-3 py-2 rounded-lg text-sm outline-none border-none"
              style={{ background: "var(--bg-secondary)", color: "var(--text-primary)" }}
            />
          </div>
          <div className="flex items-center gap-1.5 flex-wrap">
            <span
              className="text-[10px] uppercase tracking-wider"
              style={{ fontFamily: "var(--font-display)", color: "var(--text-muted)" }}
            >
              Sort
            </span>
            {(["points", "price", "name"] as const).map((key) => {
              const active = sortKey === key;
              return (
                <button
                  key={key}
                  onClick={() => applySort(key)}
                  className="flex items-center gap-1 px-2.5 py-1 rounded-lg text-[11px] font-bold cursor-pointer"
                  style={{
                    fontFamily: "var(--font-display)",
                    background: active ? "var(--accent-green)" : "var(--bg-secondary)",
                    color: active ? "var(--bg-primary)" : "var(--text-muted)",
                    border: active ? "none" : "1px solid var(--border-color)",
                  }}
                >
                  {key === "points" ? "Points" : key === "price" ? "Price" : "Name"}
                  {active &&
                    (sortDir === "asc" ? <ArrowUp size={11} /> : <ArrowDown size={11} />)}
                </button>
              );
            })}
          </div>
        </div>

        <div className="flex-1 overflow-y-auto p-3 space-y-1.5">
          {visible.map(({ player, blocked: why }) => {
            const isPicked = picked?.id === player.id;
            const priceDelta = parseFloat(player.price) - outgoingPrice;
            const pointsDelta = player.total_points - outgoing.total_points;

            return (
              <button
                key={player.id}
                onClick={() => !why && choosePlayer(player)}
                disabled={!!why}
                className="w-full flex items-center gap-2.5 p-2.5 rounded-lg text-left cursor-pointer transition-all disabled:cursor-not-allowed"
                style={{
                  background: isPicked ? "rgba(0,230,118,0.10)" : "var(--bg-secondary)",
                  border: `1px solid ${isPicked ? "var(--accent-green)" : "var(--border-color)"}`,
                  opacity: why ? 0.4 : 1,
                }}
              >
                <div className="flex gap-1 shrink-0">
                  {playablePositions(player).map((pos) => (
                    <span
                      key={pos}
                      className={`text-[9px] font-bold px-1.5 py-0.5 rounded-full text-white ${POSITION_BADGE[pos]}`}
                    >
                      {pos}
                    </span>
                  ))}
                </div>

                <div className="flex-1 min-w-0">
                  <p className="text-sm font-medium truncate">{player.name}</p>
                  <p className="text-[11px] truncate" style={{ color: "var(--text-muted)" }}>
                    {why ?? player.team_name}
                  </p>
                </div>

                <div className="text-right shrink-0">
                  <p className="text-sm font-bold" style={{ fontFamily: "var(--font-display)" }}>
                    ${player.price}
                  </p>
                  {!why && (
                    <p className="text-[10px]" style={{ color: "var(--text-muted)" }}>
                      <span
                        style={{
                          color: priceDelta > 0 ? "var(--accent-amber)" : "var(--accent-green)",
                        }}
                      >
                        {priceDelta >= 0 ? "+" : "−"}${money(Math.abs(priceDelta))}
                      </span>
                      {"  "}
                      <span
                        style={{
                          color: pointsDelta >= 0 ? "var(--accent-green)" : "var(--danger)",
                        }}
                      >
                        {pointsDelta >= 0 ? "+" : "−"}
                        {Math.abs(pointsDelta)} pts
                      </span>
                    </p>
                  )}
                </div>
              </button>
            );
          })}

          {available.length === 0 && !showBlocked && (
            <p className="text-xs text-center py-8" style={{ color: "var(--text-muted)" }}>
              No player you can afford fits this slot.
            </p>
          )}

          {blocked.length > 0 && (
            <button
              onClick={() => setShowBlocked((v) => !v)}
              className="w-full text-center text-[11px] py-2 cursor-pointer bg-transparent border-none"
              style={{ color: "var(--text-muted)" }}
            >
              {showBlocked
                ? "Hide unavailable players"
                : `Show all (${blocked.length} unavailable)`}
            </button>
          )}
        </div>

        {/* ---- Footer: the only thing that commits ---- */}
        <div className="p-4 space-y-2.5" style={{ borderTop: "1px solid var(--border-color)" }}>
          {picked && !outgoingIsBench && playablePositions(picked).length > 1 && (
            <div className="flex items-center gap-2">
              <span className="text-xs" style={{ color: "var(--text-muted)" }}>
                Play as
              </span>
              {playablePositions(picked).map((pos) => (
                <button
                  key={pos}
                  onClick={() => setPlayAs(pos)}
                  className="px-2.5 py-1 rounded-lg text-[11px] font-bold cursor-pointer border-none"
                  style={{
                    fontFamily: "var(--font-display)",
                    background: playAs === pos ? "var(--accent-green)" : "var(--bg-secondary)",
                    color: playAs === pos ? "var(--bg-primary)" : "var(--text-muted)",
                  }}
                >
                  {pos}
                </button>
              ))}
            </div>
          )}

          {error && (
            <div
              className="flex items-start gap-2 px-3 py-2 rounded-lg text-xs"
              style={{
                background: "rgba(239,68,68,0.1)",
                border: "1px solid rgba(239,68,68,0.25)",
                color: "var(--danger)",
              }}
            >
              <AlertCircle size={14} className="shrink-0 mt-0.5" />
              <span>{error}</span>
            </div>
          )}

          <button
            onClick={() => picked && onConfirm(picked, outgoingIsBench ? undefined : playAs)}
            disabled={!picked || pending}
            className="w-full flex items-center justify-center gap-2 py-3 rounded-xl text-sm font-bold cursor-pointer border-none disabled:cursor-not-allowed"
            style={{
              fontFamily: "var(--font-display)",
              background: picked ? "var(--accent-green)" : "var(--bg-secondary)",
              color: picked ? "var(--bg-primary)" : "var(--text-muted)",
              opacity: pending ? 0.7 : 1,
            }}
          >
            {pending && <Loader2 size={15} className="animate-spin" />}
            {pending ? "Confirming..." : "Confirm transfer"}
          </button>

          <p className="text-[11px] text-center" style={{ color: "var(--text-muted)" }}>
            {picked ? (
              <>
                {outgoing.name} → {picked.name} · budget after ${money(budgetAfter)} ·{" "}
                {pointsCost > 0 ? `−${pointsCost} pts` : "free"}
              </>
            ) : (
              "Nothing is changed until you confirm."
            )}
          </p>
        </div>
      </motion.div>
    </motion.div>
  );
}
