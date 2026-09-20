"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import { useRouter } from "next/navigation";
import { AnimatePresence, motion, useReducedMotion } from "framer-motion";
import { Anchor, AlertCircle, Minus, Plus, RotateCcw, Search, Shuffle, Star, X } from "lucide-react";
import Nav from "@/components/nav";
import { getPlayers, type Player } from "@/lib/api";
import { isAuthenticated } from "@/lib/auth";
import { POSITION_BADGE } from "@/lib/lineup";
import {
  conservationError,
  effectiveTeamCount,
  generateTeams,
  teamCapacities,
  type BalancePlayer,
  type GenerateResult,
} from "./balance";
import { DEFAULT_PRESET_ID, PRESETS, type SquadPreset } from "./presets";
import {
  emptyDeltas,
  loadState,
  saveDeltas,
  savePresetId,
  saveShape,
  type MarkDeltas,
  type TeamShape,
} from "./storage";

/**
 * Two marks are the organizer's to give: `star` (top player) and `prime` (holds
 * position, does not track back). They are independent — somebody may be both,
 * either, or neither — so they are two separate toggles rather than one chip
 * that cycles, which could never express "both" in a single tap.
 *
 * The third mark, `keeper`, is not tappable for roster players: it is read from
 * the roster's own position data. Making it tappable would recreate the very
 * defect this page was rewritten to remove.
 */
type MarkKey = "star" | "prime";

/** A mark's value plus where it came from, which is what drives outlined vs solid. */
type MarkState = {
  on: boolean;
  /** True when the value is the preset's; false when the organizer set it. */
  fromPreset: boolean;
};

type OtherPlayer = {
  id: string;
  name: string;
  canKeepGoal: boolean;
};

type Row = {
  id: string;
  name: string;
  /** Null for manual entries: they have no roster position, and none is invented. */
  position: Player["position"] | null;
  keeper: boolean;
  isOther: boolean;
};

const FOCUS_RING =
  "focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-green)]";

const DEFAULT_SHAPE: TeamShape = { teamCount: 2, playersPerTeam: 6 };

const PRESET_IDS: readonly string[] = PRESETS.map((entry) => entry.id);

const MARK_STYLE: Record<MarkKey, { color: string; label: string }> = {
  star: { color: "var(--accent-green)", label: "top player" },
  prime: { color: "var(--accent-amber)", label: "prime position" },
};

/**
 * No `nanoid`, no `uuid` — the platform already has this, and the fallback
 * covers the browsers that have `crypto` but not `randomUUID`.
 */
function newOtherId(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return `other:${crypto.randomUUID()}`;
  }
  return `other:${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;
}

/** "5, 4 and 4" */
function listNumbers(values: readonly number[]): string {
  if (values.length === 0) return "";
  if (values.length === 1) return String(values[0]);
  return `${values.slice(0, -1).join(", ")} and ${values[values.length - 1]}`;
}

/**
 * Resolve one mark for one player. Precedence is deltas, then the preset, then
 * unmarked — and the provenance travels with the value so the UI can show which
 * marks are the preset's defaults rather than leaving them to look like magic.
 */
function resolveMark(
  playerId: string,
  presetIds: ReadonlySet<string>,
  delta: { add: readonly string[]; remove: readonly string[] },
): MarkState {
  if (delta.add.includes(playerId)) return { on: true, fromPreset: false };
  if (delta.remove.includes(playerId)) return { on: false, fromPreset: false };
  return { on: presetIds.has(playerId), fromPreset: true };
}

export default function RandomizerPage() {
  const router = useRouter();
  const reduceMotion = useReducedMotion();

  const [players, setPlayers] = useState<Player[]>([]);
  /**
   * Set only when the roster actually arrives. `loading` is not a substitute:
   * it also clears on a failed fetch, and an empty roster would then make every
   * preset id look unresolved — the page would accuse its own preset of being
   * broken because the network was.
   */
  const [rosterLoaded, setRosterLoaded] = useState(false);
  const [otherPlayers, setOtherPlayers] = useState<OtherPlayer[]>([]);
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);

  const [presetId, setPresetId] = useState<string>(DEFAULT_PRESET_ID);
  const [deltasByPreset, setDeltasByPreset] = useState<Record<string, MarkDeltas>>({});
  const [teamCount, setTeamCount] = useState(DEFAULT_SHAPE.teamCount);
  const [playersPerTeam, setPlayersPerTeam] = useState(DEFAULT_SHAPE.playersPerTeam);

  const [search, setSearch] = useState("");
  const [otherName, setOtherName] = useState("");
  const [otherCanKeep, setOtherCanKeep] = useState(false);
  const [sheetOpen, setSheetOpen] = useState(false);

  const [result, setResult] = useState<GenerateResult | null>(null);

  /**
   * Exactly two message channels, and never a third.
   *
   * `error` is red, assertive, and reserved for blocking validation — the two
   * cases where there is nothing to generate, plus a roster that failed to load
   * and a conservation failure. A normal outcome never comes through here; the
   * old page announced its own success in red, which is what this separation
   * exists to prevent.
   *
   * `notice` is muted, polite and non-blocking: uneven sizes, unresolved preset
   * ids, storage trouble, fewer top players than teams.
   */
  const [error, setError] = useState("");
  const [systemNotices, setSystemNotices] = useState<string[]>([]);
  const [runNotices, setRunNotices] = useState<string[]>([]);

  useEffect(() => {
    if (!isAuthenticated()) {
      router.push("/login");
      return;
    }
    let cancelled = false;

    getPlayers()
      .then((all) => {
        if (cancelled) return;
        setPlayers(all);
        setSelectedIds(all.map((player) => player.id));
        setRosterLoaded(true);
      })
      .catch((err) => {
        if (cancelled) return;
        setError(err instanceof Error ? err.message : "Failed to load players");
      })
      .finally(() => {
        if (cancelled) return;
        // Read persisted state here, inside the promise callback, rather than in
        // the effect body: localStorage is client-only, and a synchronous
        // setState in an effect body is an error under this repo's lint config.
        // storage.ts owns validating the saved preset id, so that the legacy
        // star migration inside it keys its adopted marks under a preset that
        // still exists rather than stranding them under a dropped one.
        const loaded = loadState({
          presetId: DEFAULT_PRESET_ID,
          knownPresetIds: PRESET_IDS,
          shape: DEFAULT_SHAPE,
        });
        setPresetId(loaded.presetId);
        setDeltasByPreset(loaded.deltasByPreset);
        setTeamCount(loaded.shape.teamCount);
        setPlayersPerTeam(loaded.shape.playersPerTeam);
        setSystemNotices(loaded.notices);
        setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [router]);

  const preset: SquadPreset = useMemo(
    () => PRESETS.find((entry) => entry.id === presetId) ?? PRESETS[0],
    [presetId],
  );

  const deltas = useMemo(
    () => deltasByPreset[preset.id] ?? emptyDeltas(),
    [deltasByPreset, preset.id],
  );

  const presetStarIds = useMemo(() => new Set(preset.star.map((entry) => entry.id)), [preset]);
  const presetPrimeIds = useMemo(() => new Set(preset.prime.map((entry) => entry.id)), [preset]);

  /**
   * Preset entries whose id matches nobody on the roster.
   *
   * Surfaced, never swallowed. A preset that silently marks nobody is the
   * original defect: it looks exactly like a preset that is working.
   */
  const unresolved = useMemo(() => {
    // Nothing can be called missing until there is a roster to be missing from.
    if (!rosterLoaded) return [];
    const rosterIds = new Set(players.map((player) => player.id));
    const seen = new Set<string>();
    const missing: string[] = [];
    for (const entry of [...preset.star, ...preset.prime]) {
      if (rosterIds.has(entry.id) || seen.has(entry.id)) continue;
      seen.add(entry.id);
      // The label is shown here, and only here — a human cannot act on a UUID.
      missing.push(entry.name);
    }
    return missing;
  }, [players, preset, rosterLoaded]);

  /**
   * Development-only guard against a correct name pasted beside a wrong UUID.
   * The UUID still wins — the mark is applied either way — but somebody gets
   * told, which is the cheapest check available on a wall of opaque ids.
   */
  useEffect(() => {
    if (process.env.NODE_ENV === "production" || players.length === 0) return;
    const rosterNames = new Map(players.map((player) => [player.id, player.name]));
    for (const entry of [...preset.star, ...preset.prime]) {
      const rosterName = rosterNames.get(entry.id);
      if (rosterName !== undefined && rosterName !== entry.name) {
        console.warn(
          `[randomizer] preset "${preset.id}" labels ${entry.id} as "${entry.name}", but the roster calls them "${rosterName}". The id wins; fix the label in presets.ts.`,
        );
      }
    }
  }, [players, preset]);

  const rows = useMemo<Row[]>(() => {
    const listed: Row[] = players.map((player) => ({
      id: player.id,
      name: player.name,
      position: player.position,
      // Nullable, and narrowed rather than asserted.
      // secondary_position is nullable; narrowed by comparison, never asserted.
      keeper: player.position === "GK" || player.secondary_position === "GK",
      isOther: false,
    }));
    const manual: Row[] = otherPlayers.map((player) => ({
      id: player.id,
      name: player.name,
      position: null,
      keeper: player.canKeepGoal,
      isOther: true,
    }));
    return [...listed, ...manual];
  }, [players, otherPlayers]);

  const markOf = useCallback(
    (playerId: string, mark: MarkKey): MarkState =>
      resolveMark(playerId, mark === "star" ? presetStarIds : presetPrimeIds, deltas[mark]),
    [deltas, presetStarIds, presetPrimeIds],
  );

  const selectedSet = useMemo(() => new Set(selectedIds), [selectedIds]);

  const selected = useMemo<BalancePlayer[]>(
    () =>
      rows
        .filter((row) => selectedSet.has(row.id))
        .map((row) => ({
          id: row.id,
          name: row.name,
          position: row.position,
          star: markOf(row.id, "star").on,
          prime: markOf(row.id, "prime").on,
          keeper: row.keeper,
        })),
    [rows, selectedSet, markOf],
  );

  const counts = useMemo(
    () => ({
      selected: selected.length,
      star: selected.filter((player) => player.star).length,
      prime: selected.filter((player) => player.prime).length,
      keeper: selected.filter((player) => player.keeper).length,
    }),
    [selected],
  );

  const capacities = useMemo(
    () => teamCapacities(selected.length, teamCount),
    [selected.length, teamCount],
  );

  const visibleRows = useMemo(() => {
    const query = search.trim().toLowerCase();
    if (!query) return rows;
    // Substring search on a name the organizer is typing is a filter, not an
    // identity decision. Nothing is ever resolved to a person by name here.
    return rows.filter((row) => row.name.toLowerCase().includes(query));
  }, [rows, search]);

  const notices = useMemo(() => {
    const all: string[] = [];
    if (unresolved.length > 0) {
      all.push(
        `${unresolved.length} preset ${unresolved.length === 1 ? "player is" : "players are"} not in the roster — ${unresolved.join(", ")}. They will not be marked.`,
      );
    }
    return [...all, ...systemNotices, ...runNotices];
  }, [unresolved, systemNotices, runNotices]);

  // ── persistence writes, done in handlers so no effect races the initial load ──

  const noteStorageProblem = useCallback((problem: string | null) => {
    if (!problem) return;
    setSystemNotices((prev) => (prev.includes(problem) ? prev : [...prev, problem]));
  }, []);

  /**
   * Any change to who is playing, how they are marked, or the shape makes an
   * already-drawn set of teams wrong. Clearing it is the honest response:
   * leaving stale teams on screen beside a changed selection is exactly the
   * kind of quiet disagreement this rewrite exists to remove.
   */
  const invalidateTeams = useCallback(() => {
    setResult(null);
    setRunNotices([]);
  }, []);

  const commitDeltas = useCallback(
    (next: Record<string, MarkDeltas>) => {
      setDeltasByPreset(next);
      noteStorageProblem(saveDeltas(next));
      invalidateTeams();
    },
    [noteStorageProblem, invalidateTeams],
  );

  const commitShape = useCallback(
    (shape: TeamShape) => {
      setTeamCount(shape.teamCount);
      setPlayersPerTeam(shape.playersPerTeam);
      noteStorageProblem(saveShape(shape));
      invalidateTeams();
    },
    [noteStorageProblem, invalidateTeams],
  );

  /**
   * Toggling a mark records the difference from the preset, not the resulting
   * value. Tapping a mark back to whatever the preset says clears the delta
   * entirely rather than storing a contradicting pair — so the preset stays
   * live for that player, and a future edit to presets.ts still reaches them.
   */
  const toggleMark = (playerId: string, mark: MarkKey) => {
    const presetIds = mark === "star" ? presetStarIds : presetPrimeIds;
    const next = !markOf(playerId, mark).on;
    const side = deltas[mark];
    const add = side.add.filter((id) => id !== playerId);
    const remove = side.remove.filter((id) => id !== playerId);
    if (next !== presetIds.has(playerId)) {
      if (next) add.push(playerId);
      else remove.push(playerId);
    }
    commitDeltas({ ...deltasByPreset, [preset.id]: { ...deltas, [mark]: { add, remove } } });
  };

  const choosePreset = (nextId: string) => {
    setPresetId(nextId);
    noteStorageProblem(savePresetId(nextId));
    invalidateTeams();
    setSheetOpen(false);
  };

  /** Back to the preset's own marks: drop this preset's overrides. */
  const resetToPreset = () => {
    const next = { ...deltasByPreset };
    delete next[preset.id];
    commitDeltas(next);
    setSheetOpen(false);
  };

  /**
   * Everybody unmarked, without changing which preset is active — removing each
   * preset id is what "clear" means here, rather than switching to the blank
   * preset, which would swap the list out from under the organizer.
   */
  const clearAllMarks = () => {
    commitDeltas({
      ...deltasByPreset,
      [preset.id]: {
        star: { add: [], remove: [...presetStarIds] },
        prime: { add: [], remove: [...presetPrimeIds] },
      },
    });
    setSheetOpen(false);
  };

  const toggleSelected = (playerId: string) => {
    setSelectedIds((prev) =>
      prev.includes(playerId) ? prev.filter((id) => id !== playerId) : [...prev, playerId],
    );
    invalidateTeams();
  };

  const addOtherPlayer = () => {
    const name = otherName.trim();
    if (!name) return;
    // Deliberately no duplicate-name check. Two different people really do turn
    // up sharing a name — the roster has several such pairs — so refusing the
    // second one would be the same name-as-identity mistake this page bans.
    const player: OtherPlayer = { id: newOtherId(), name, canKeepGoal: otherCanKeep };
    setOtherPlayers((prev) => [...prev, player]);
    setSelectedIds((prev) => [...prev, player.id]);
    setOtherName("");
    setOtherCanKeep(false);
    invalidateTeams();
  };

  const removeOtherPlayer = (playerId: string) => {
    setOtherPlayers((prev) => prev.filter((player) => player.id !== playerId));
    setSelectedIds((prev) => prev.filter((id) => id !== playerId));
    invalidateTeams();
  };

  const handleGenerate = () => {
    if (selected.length === 0) {
      setError("Pick the players who turned up first.");
      invalidateTeams();
      return;
    }
    if (teamCount < 1) {
      setError("You need at least one team.");
      invalidateTeams();
      return;
    }

    // Math.random lives here, at the React boundary, and nowhere in the core.
    const generated = generateTeams(selected, teamCount, Math.random);

    const problem = conservationError(selected, generated.teams);
    if (problem) {
      // A real failure: somebody would have been lost or cloned. Never render it.
      setError(`Could not build teams safely — ${problem}`);
      invalidateTeams();
      return;
    }

    setError("");
    setResult(generated);

    const sizes = generated.teams.map((team) => team.length);
    const next: string[] = [`Teams of ${listNumbers(sizes)} — everyone plays.`];
    const madeTeams = generated.teams.length;
    if (madeTeams < teamCount) {
      next.push(
        `Only ${selected.length} ${selected.length === 1 ? "player" : "players"} selected, so ${madeTeams} ${madeTeams === 1 ? "team" : "teams"} were made instead of ${teamCount}.`,
      );
    }
    if (counts.star < madeTeams) {
      next.push(
        `${counts.star === 0 ? "No top players" : `Only ${counts.star} top ${counts.star === 1 ? "player" : "players"}`} for ${madeTeams} teams, so some teams have none.`,
      );
    }
    if (counts.keeper === 0) {
      next.push("Nobody selected can play in goal.");
    } else if (counts.keeper < madeTeams) {
      next.push(
        `Only ${counts.keeper} ${counts.keeper === 1 ? "keeper" : "keepers"} for ${madeTeams} teams.`,
      );
    }
    if (counts.prime === 0) {
      next.push("No prime-position players are marked, so that axis is even by default.");
    }
    if (selected.length !== teamCount * playersPerTeam) {
      next.push(
        `Your target of ${playersPerTeam} per team does not divide ${selected.length} players, so sizes were adjusted. Nobody is benched.`,
      );
    }
    setRunNotices(next);
  };

  useEffect(() => {
    if (!sheetOpen) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setSheetOpen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [sheetOpen]);

  const feasibility =
    selected.length === 0
      ? "Pick players to see the split"
      : `${selected.length} selected · ${effectiveTeamCount(selected.length, teamCount)} ${effectiveTeamCount(selected.length, teamCount) === 1 ? "team" : "teams"} → ${capacities.join(" / ")}`;

  return (
    <div className="min-h-screen pitch-pattern">
      <Nav />
      <div className="pt-24 pb-12 max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        <motion.div
          initial={reduceMotion ? false : { opacity: 0, y: 16 }}
          animate={{ opacity: 1, y: 0 }}
          className="mb-6"
        >
          <div className="flex flex-wrap items-center justify-between gap-3">
            <h1
              className="text-3xl sm:text-4xl font-bold"
              style={{ fontFamily: "var(--font-display)" }}
            >
              FUTSAL <span style={{ color: "var(--accent-green)" }}>RANDOMIZER</span>
            </h1>
            <button
              type="button"
              onClick={() => setSheetOpen(true)}
              className={`inline-flex items-center gap-2 rounded-full px-3 py-2 text-xs cursor-pointer ${FOCUS_RING}`}
              style={{
                background: "color-mix(in srgb, var(--accent-green) 10%, transparent)",
                border: "1px solid color-mix(in srgb, var(--accent-green) 30%, transparent)",
                color: "var(--accent-green)",
                fontFamily: "var(--font-display)",
              }}
              aria-haspopup="dialog"
              aria-expanded={sheetOpen}
            >
              <Star size={13} aria-hidden />
              {preset.label}
            </button>
          </div>
          <p className="mt-2 text-sm" style={{ color: "var(--text-muted)" }}>
            Pick who turned up, mark the top and prime-position players, then split them into
            balanced teams. Everybody selected gets a team.
          </p>
        </motion.div>

        {error && (
          <div
            role="alert"
            className="flex items-start gap-2 p-3 rounded-lg mb-4 text-sm"
            style={{
              background: "rgba(255, 82, 82, 0.1)",
              border: "1px solid rgba(255, 82, 82, 0.3)",
              color: "var(--danger)",
            }}
          >
            <AlertCircle size={16} className="mt-0.5 shrink-0" aria-hidden />
            <p>{error}</p>
          </div>
        )}

        {notices.length > 0 && (
          <div
            role="status"
            className="rounded-lg mb-6 px-3 py-2.5 text-xs space-y-1"
            style={{
              background: "var(--bg-elevated)",
              border: "1px solid var(--border-color)",
              color: "var(--text-muted)",
            }}
          >
            {notices.map((notice) => (
              <p key={notice}>{notice}</p>
            ))}
          </div>
        )}

        {loading ? (
          <div className="flex items-center justify-center py-20">
            <div
              className="w-8 h-8 border-2 border-t-transparent rounded-full animate-spin"
              style={{ borderColor: "var(--accent-green)", borderTopColor: "transparent" }}
            />
          </div>
        ) : (
          <>
            <div className="glass-card no-hover p-4 sm:p-6 mb-6">
              <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3 mb-4">
                <h2 className="text-lg font-bold" style={{ fontFamily: "var(--font-display)" }}>
                  Who turned up
                </h2>
                <p className="text-xs" style={{ color: "var(--text-muted)" }}>
                  {counts.selected} selected · {counts.star} top · {counts.prime} prime ·{" "}
                  {counts.keeper} {counts.keeper === 1 ? "keeper" : "keepers"}
                </p>
              </div>

              <div className="flex flex-col sm:flex-row gap-2 mb-4">
                <div className="relative flex-1">
                  <Search
                    size={15}
                    aria-hidden
                    className="absolute left-3 top-1/2 -translate-y-1/2"
                    style={{ color: "var(--text-muted)" }}
                  />
                  <input
                    type="search"
                    value={search}
                    onChange={(event) => setSearch(event.target.value)}
                    placeholder="Search by name..."
                    aria-label="Search players by name"
                    className="input-field pl-9"
                  />
                </div>
                <div className="flex items-center gap-2">
                  <button
                    type="button"
                    onClick={() => {
                      const shown = visibleRows.map((row) => row.id);
                      setSelectedIds((prev) => Array.from(new Set([...prev, ...shown])));
                      invalidateTeams();
                    }}
                    className={`btn-secondary text-[11px] py-2 px-3 ${FOCUS_RING}`}
                  >
                    Select all {search.trim() ? `(${visibleRows.length})` : ""}
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      const shown = new Set(visibleRows.map((row) => row.id));
                      setSelectedIds((prev) => prev.filter((id) => !shown.has(id)));
                      invalidateTeams();
                    }}
                    className={`btn-secondary text-[11px] py-2 px-3 ${FOCUS_RING}`}
                  >
                    Clear {search.trim() ? `(${visibleRows.length})` : ""}
                  </button>
                </div>
              </div>

              <div className="grid grid-cols-1 sm:grid-cols-2 gap-2 max-h-[380px] overflow-y-auto pr-1">
                {visibleRows.map((row) => {
                  const chosen = selectedSet.has(row.id);
                  const star = markOf(row.id, "star");
                  const prime = markOf(row.id, "prime");
                  return (
                    <div
                      key={row.id}
                      className="flex items-center gap-1 rounded-lg pl-1 pr-1"
                      style={{
                        background: chosen ? "rgba(0,230,118,0.08)" : "var(--bg-elevated)",
                        border: `1px solid ${chosen ? "rgba(0,230,118,0.35)" : "var(--border-color)"}`,
                      }}
                    >
                      <button
                        type="button"
                        onClick={() => toggleSelected(row.id)}
                        aria-pressed={chosen}
                        className={`flex flex-1 min-w-0 items-center gap-1.5 rounded-lg px-2 py-2.5 text-left cursor-pointer bg-transparent border-none ${FOCUS_RING}`}
                      >
                        {/*
                          A floor on the name, because the badges beside it are
                          shrink-0: without it a manual entry who can keep goal
                          renders as [GK²][star][prime] with zero characters of
                          name on a 320px screen. A manual entry needs no
                          "added" badge — having no position badge, and having a
                          remove button, already says so.
                        */}
                        <span
                          className="text-sm font-medium truncate min-w-[3.5rem]"
                          style={{ color: chosen ? "var(--text-primary)" : "var(--text-muted)" }}
                        >
                          {row.name}
                        </span>
                        {row.position ? (
                          <span
                            className={`text-[9px] font-bold px-1.5 py-0.5 rounded-full text-white shrink-0 ${POSITION_BADGE[row.position]}`}
                          >
                            {row.position}
                          </span>
                        ) : null}
                        {/*
                          GK² marks a non-specialist who can still go in goal.
                          For a roster player `keeper && position !== "GK"` is
                          exactly `secondary_position === "GK"`, and it extends
                          to a manual entry whose "Can keep goal" box is ticked.
                          A primary keeper is excluded — their own badge already
                          says GK.
                        */}
                        {row.keeper && row.position !== "GK" && (
                          <span
                            className="text-[9px] font-bold px-1.5 py-0.5 rounded-full shrink-0"
                            title="Can also play in goal"
                            style={{
                              color: "var(--accent-amber)",
                              border:
                                "1px dashed color-mix(in srgb, var(--accent-amber) 45%, transparent)",
                            }}
                          >
                            GK²
                          </span>
                        )}
                      </button>

                      <MarkToggle
                        mark="star"
                        state={star}
                        playerName={row.name}
                        onToggle={() => toggleMark(row.id, "star")}
                      />
                      <MarkToggle
                        mark="prime"
                        state={prime}
                        playerName={row.name}
                        onToggle={() => toggleMark(row.id, "prime")}
                      />
                      {row.isOther && (
                        <button
                          type="button"
                          onClick={() => removeOtherPlayer(row.id)}
                          aria-label={`Remove ${row.name}`}
                          className={`shrink-0 w-7 h-11 flex items-center justify-center rounded-lg cursor-pointer bg-transparent border-none ${FOCUS_RING}`}
                          style={{ color: "var(--text-muted)" }}
                        >
                          <X size={13} aria-hidden />
                        </button>
                      )}
                    </div>
                  );
                })}
                {visibleRows.length === 0 && (
                  <p className="text-xs py-4" style={{ color: "var(--text-muted)" }}>
                    Nobody matches that search.
                  </p>
                )}
              </div>

              <div
                className="mt-4 pt-4 flex flex-col sm:flex-row gap-2"
                style={{ borderTop: "1px solid var(--border-color)" }}
              >
                <input
                  type="text"
                  value={otherName}
                  onChange={(event) => setOtherName(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") addOtherPlayer();
                  }}
                  placeholder="Add someone not on the list..."
                  aria-label="Name of a player who is not on the roster"
                  className="input-field flex-1"
                />
                <label
                  className="inline-flex items-center gap-2 text-xs px-3 cursor-pointer select-none"
                  style={{ color: "var(--text-secondary)" }}
                >
                  <input
                    type="checkbox"
                    checked={otherCanKeep}
                    onChange={(event) => setOtherCanKeep(event.target.checked)}
                    className="w-4 h-4 accent-[var(--accent-green)]"
                  />
                  Can keep goal
                </label>
                <button
                  type="button"
                  onClick={addOtherPlayer}
                  disabled={!otherName.trim()}
                  className={`btn-secondary text-[11px] py-2 px-4 disabled:opacity-40 ${FOCUS_RING}`}
                >
                  Add
                </button>
              </div>
            </div>

            <div className="glass-card no-hover p-4 sm:p-6 mb-6">
              <div className="flex flex-col sm:flex-row sm:items-end gap-4">
                <Stepper
                  label="Teams"
                  value={teamCount}
                  min={1}
                  onChange={(value) => commitShape({ teamCount: value, playersPerTeam })}
                />
                <Stepper
                  label="Players per team"
                  value={playersPerTeam}
                  min={1}
                  onChange={(value) => commitShape({ teamCount, playersPerTeam: value })}
                />
                <div className="flex-1">
                  <p className="text-xs mb-2" style={{ color: "var(--text-muted)" }}>
                    {feasibility}
                  </p>
                  <div className="flex gap-2">
                    <button
                      type="button"
                      onClick={handleGenerate}
                      className={`btn-primary text-sm inline-flex items-center justify-center gap-2 px-5 py-3 ${FOCUS_RING}`}
                    >
                      <Shuffle size={16} aria-hidden />
                      {result ? "Regenerate" : "Make teams"}
                    </button>
                  </div>
                </div>
              </div>
              <p className="text-[11px] mt-3" style={{ color: "var(--text-muted)" }}>
                Players per team is a target, not a cap — everyone selected gets a team even when
                the numbers do not divide evenly.
              </p>
            </div>

            {result && result.teams.length > 0 && (
              <>
                <div className="grid grid-cols-1 lg:grid-cols-2 gap-4 mb-6">
                  {result.teams.map((team, index) => (
                    <motion.div
                      key={`team-${index + 1}`}
                      initial={reduceMotion ? false : { opacity: 0, y: 12 }}
                      animate={{ opacity: 1, y: 0 }}
                      transition={reduceMotion ? { duration: 0 } : { delay: index * 0.05 }}
                      className="glass-card no-hover p-4 sm:p-5"
                    >
                      <div className="flex items-baseline justify-between gap-2 mb-3">
                        <h2
                          className="text-xl font-bold"
                          style={{ fontFamily: "var(--font-display)" }}
                        >
                          Team {index + 1}
                        </h2>
                        <p className="text-[11px]" style={{ color: "var(--text-muted)" }}>
                          {team.length} players · {team.filter((p) => p.star).length} top ·{" "}
                          {team.filter((p) => p.prime).length} prime ·{" "}
                          {team.filter((p) => p.keeper).length} GK
                        </p>
                      </div>
                      <div className="space-y-1.5">
                        {team.map((player) => (
                          <div
                            key={player.id}
                            className="flex items-center gap-2 rounded-lg px-3 py-2"
                            style={{
                              background: "var(--bg-elevated)",
                              border: "1px solid var(--border-color)",
                            }}
                          >
                            <span className="text-sm font-medium truncate flex-1 min-w-0">
                              {player.name}
                            </span>
                            {player.position && (
                              <span
                                className={`text-[9px] font-bold px-1.5 py-0.5 rounded-full text-white shrink-0 ${POSITION_BADGE[player.position]}`}
                              >
                                {player.position}
                              </span>
                            )}
                            {player.keeper && player.position !== "GK" && (
                              <span
                                className="text-[9px] font-bold px-1.5 py-0.5 rounded-full shrink-0"
                                style={{
                                  color: "var(--accent-amber)",
                                  border:
                                    "1px dashed color-mix(in srgb, var(--accent-amber) 45%, transparent)",
                                }}
                              >
                                GK²
                              </span>
                            )}
                            {player.star && (
                              <Star
                                size={13}
                                role="img"
                                aria-label="Top player"
                                className="shrink-0"
                                style={{ color: "var(--accent-green)" }}
                                fill="currentColor"
                              />
                            )}
                            {player.prime && (
                              <Anchor
                                size={13}
                                role="img"
                                aria-label="Prime position"
                                className="shrink-0"
                                style={{ color: "var(--accent-amber)" }}
                              />
                            )}
                          </div>
                        ))}
                      </div>
                    </motion.div>
                  ))}
                </div>

                <details
                  className="glass-card no-hover p-4 sm:p-5 mb-6 text-sm"
                  style={{ color: "var(--text-secondary)" }}
                >
                  <summary className="cursor-pointer text-xs" style={{ color: "var(--text-muted)" }}>
                    Balance breakdown — total penalty {result.score.total} across{" "}
                    {result.candidatesTried} tried line-ups (lower is more even)
                  </summary>
                  <table className="w-full mt-3 text-xs">
                    <thead>
                      <tr style={{ color: "var(--text-muted)" }}>
                        <th className="text-left font-normal pb-1">Axis</th>
                        <th className="text-left font-normal pb-1">Per team</th>
                        <th className="text-right font-normal pb-1">Spread</th>
                        <th className="text-right font-normal pb-1">Weight</th>
                        <th className="text-right font-normal pb-1">Penalty</th>
                      </tr>
                    </thead>
                    <tbody>
                      {result.score.axes.map((axis) => (
                        <tr key={axis.key} style={{ borderTop: "1px solid var(--border-color)" }}>
                          <td className="py-1.5 pr-2">{axis.label}</td>
                          <td className="py-1.5 pr-2" style={{ color: "var(--text-muted)" }}>
                            {axis.parts.length > 0
                              ? axis.parts
                                  .map((part) => `${part.label} ${part.perTeam.join("/")}`)
                                  .join("  ")
                              : axis.perTeam.join(" / ")}
                          </td>
                          <td className="py-1.5 text-right">{axis.spread}</td>
                          <td className="py-1.5 text-right" style={{ color: "var(--text-muted)" }}>
                            ×{axis.weight}
                          </td>
                          <td
                            className="py-1.5 text-right font-bold"
                            style={{
                              color:
                                axis.penalty > 0 ? "var(--accent-amber)" : "var(--text-muted)",
                            }}
                          >
                            {axis.penalty}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                  <p className="mt-3 text-[11px]" style={{ color: "var(--text-muted)" }}>
                    Each number is a <em>spread</em> — the gap between the fullest and emptiest
                    team on that axis — not a score out of anything. Team size is usually a floor
                    rather than a fault: whenever the headcount does not divide evenly, a spread of
                    1 is the best any split can do. Position mix is a single term over the sum of
                    the DEF, MID and FWD spreads. The axes are also correlated — top players skew
                    midfield, prime players skew defence — so a non-zero total normally means one
                    axis gave way to another, not that something went wrong.
                  </p>
                </details>
              </>
            )}
          </>
        )}
      </div>

      <AnimatePresence>
        {sheetOpen && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="fixed inset-0 z-50 flex items-end sm:items-center justify-center sm:p-4"
            style={{ background: "rgba(0,0,0,0.7)", backdropFilter: "blur(4px)" }}
            onClick={() => setSheetOpen(false)}
          >
            <motion.div
              role="dialog"
              aria-label="Squad list"
              initial={reduceMotion ? false : { y: "100%", opacity: 0.6 }}
              animate={{ y: 0, opacity: 1 }}
              exit={reduceMotion ? { opacity: 0 } : { y: "100%", opacity: 0.6 }}
              transition={reduceMotion ? { duration: 0 } : { type: "spring", damping: 30, stiffness: 300 }}
              className="w-full sm:max-w-lg flex flex-col rounded-t-2xl sm:rounded-2xl overflow-hidden"
              style={{
                background: "var(--bg-card)",
                border: "1px solid var(--border-color)",
                maxHeight: "92vh",
              }}
              onClick={(event) => event.stopPropagation()}
            >
              <div
                className="flex items-center justify-between p-4"
                style={{ borderBottom: "1px solid var(--border-color)" }}
              >
                <h2 className="text-base font-bold" style={{ fontFamily: "var(--font-display)" }}>
                  Squad list
                </h2>
                <button
                  type="button"
                  onClick={() => setSheetOpen(false)}
                  aria-label="Close squad list"
                  className={`p-1.5 rounded-lg cursor-pointer bg-transparent border-none ${FOCUS_RING}`}
                  style={{ color: "var(--text-muted)" }}
                >
                  <X size={18} aria-hidden />
                </button>
              </div>

              <div className="flex-1 overflow-y-auto p-3 space-y-2">
                {PRESETS.map((entry) => {
                  const active = entry.id === preset.id;
                  return (
                    <button
                      key={entry.id}
                      type="button"
                      onClick={() => choosePreset(entry.id)}
                      aria-pressed={active}
                      className={`w-full text-left rounded-xl px-3 py-3 cursor-pointer ${FOCUS_RING}`}
                      style={{
                        background: active
                          ? "color-mix(in srgb, var(--accent-green) 10%, transparent)"
                          : "var(--bg-elevated)",
                        border: `1px solid ${active ? "color-mix(in srgb, var(--accent-green) 35%, transparent)" : "var(--border-color)"}`,
                      }}
                    >
                      <p className="text-sm font-medium">{entry.label}</p>
                      <p className="text-[11px] mt-0.5" style={{ color: "var(--text-muted)" }}>
                        {entry.blurb}
                      </p>
                    </button>
                  );
                })}
              </div>

              <div
                className="p-4 space-y-2"
                style={{ borderTop: "1px solid var(--border-color)" }}
              >
                <button
                  type="button"
                  onClick={resetToPreset}
                  className={`btn-secondary w-full text-[11px] py-2.5 inline-flex items-center justify-center gap-2 ${FOCUS_RING}`}
                >
                  <RotateCcw size={13} aria-hidden />
                  Reset to preset
                </button>
                <button
                  type="button"
                  onClick={clearAllMarks}
                  className={`w-full text-[11px] py-2.5 rounded-[14px] cursor-pointer inline-flex items-center justify-center gap-2 ${FOCUS_RING}`}
                  style={{
                    background: "transparent",
                    border: "1px solid var(--border-color)",
                    color: "var(--text-muted)",
                    fontFamily: "var(--font-display)",
                    textTransform: "uppercase",
                    letterSpacing: "0.08em",
                  }}
                >
                  <X size={13} aria-hidden />
                  Clear all marks
                </button>
                <p className="text-[11px] pt-1" style={{ color: "var(--text-muted)" }}>
                  Outlined marks come from the list. Solid marks are yours.
                </p>
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

/**
 * One mark, three states, 44x44 so it can be hit with a thumb on a cold
 * touchline: off, on-from-the-preset (outlined), on-because-you-tapped-it
 * (solid). That last distinction is the whole point — it makes the defaults
 * visible as defaults instead of looking like magic.
 */
function MarkToggle({
  mark,
  state,
  playerName,
  onToggle,
}: {
  mark: MarkKey;
  state: MarkState;
  playerName: string;
  onToggle: () => void;
}) {
  const { color, label } = MARK_STYLE[mark];
  const Icon = mark === "star" ? Star : Anchor;
  const solid = state.on && !state.fromPreset;

  return (
    <button
      type="button"
      onClick={onToggle}
      aria-pressed={state.on}
      aria-label={`${state.on ? "Unmark" : "Mark"} ${playerName} as ${label}${
        state.on && state.fromPreset ? " (currently set by the squad list)" : ""
      }`}
      className="shrink-0 w-11 h-11 flex items-center justify-center rounded-lg cursor-pointer focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-green)]"
      style={{
        background: solid ? color : "transparent",
        border: state.on
          ? `1px ${solid ? "solid" : "dashed"} color-mix(in srgb, ${color} ${solid ? 100 : 55}%, transparent)`
          : "1px solid transparent",
        color: solid ? "var(--bg-primary)" : state.on ? color : "var(--text-muted)",
      }}
    >
      <Icon size={17} aria-hidden fill={solid ? "currentColor" : "none"} />
    </button>
  );
}

function Stepper({
  label,
  value,
  min,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  onChange: (value: number) => void;
}) {
  return (
    <div>
      <p
        className="text-xs uppercase tracking-[0.14em] mb-2"
        style={{ color: "var(--text-muted)", fontFamily: "var(--font-display)" }}
      >
        {label}
      </p>
      <div
        className="inline-flex items-center rounded-[14px]"
        style={{ border: "1px solid var(--border-color)", background: "rgba(255,255,255,0.03)" }}
      >
        <button
          type="button"
          onClick={() => onChange(Math.max(min, value - 1))}
          disabled={value <= min}
          aria-label={`Fewer ${label.toLowerCase()}`}
          className={`w-11 h-11 flex items-center justify-center rounded-l-[14px] cursor-pointer bg-transparent border-none disabled:opacity-30 ${FOCUS_RING}`}
          style={{ color: "var(--text-secondary)" }}
        >
          <Minus size={15} aria-hidden />
        </button>
        <span
          aria-live="polite"
          className="w-10 text-center text-base font-bold"
          style={{ fontFamily: "var(--font-display)" }}
        >
          {value}
        </span>
        <button
          type="button"
          onClick={() => onChange(value + 1)}
          aria-label={`More ${label.toLowerCase()}`}
          className={`w-11 h-11 flex items-center justify-center rounded-r-[14px] cursor-pointer bg-transparent border-none ${FOCUS_RING}`}
          style={{ color: "var(--text-secondary)" }}
        >
          <Plus size={15} aria-hidden />
        </button>
      </div>
    </div>
  );
}
