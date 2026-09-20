/**
 * The randomizer's balance core.
 *
 * Pure, React-free, and deterministic given its injected RNG, so a partition can
 * be built, scored and brute-force-checked outside a browser. `Math.random` is
 * not referenced in this file; the only entropy is whatever the caller injects.
 */

import type { Position } from "@/lib/api";

/** Returns a float in [0, 1), like Math.random. Injected so runs are reproducible. */
export type Rng = () => number;

export type BalancePlayer = {
  id: string;
  name: string;
  /**
   * Nullable on purpose. Players the organizer types in by hand have no roster
   * entry and therefore no position, and inventing one for them would skew the
   * position-mix axis with data nobody supplied. They are excluded from that
   * axis alone — they still count for size, keeper, top and prime.
   */
  position: Position | null;
  /** Top / skilled player. Balance axis (a). From the preset, then organizer taps. */
  star: boolean;
  /** Holds position, does not track back. Balance axis (e). Preset, then taps. */
  prime: boolean;
  /**
   * Can play in goal. Balance axis (b). Derived from position data for roster
   * players, or from the "Can keep goal" checkbox for manual entries.
   *
   * Primary and secondary keepers are both simply `true` here: a secondary
   * keeper is a real keeper, and only the assignment ORDER prefers specialists.
   */
  keeper: boolean;
};

export type BalanceTeam = BalancePlayer[];

export type Weights = {
  size: number;
  star: number;
  keeper: number;
  prime: number;
  mix: number;
};

/**
 * All five axes weigh the same until somebody deliberately changes that.
 *
 * Equal weights are NOT equal influence, and that is measured rather than
 * assumed. In the group's own lists the two mark sets lean opposite ways:
 * the 13 top players are 7 MID and 6 FWD with no defender at all, while the 8
 * prime-position players are 4 DEF, 3 FWD and 1 MID — against a roster of
 * 11 MID, 14 DEF, 20 FWD and 4 GK. So spreading top players largely IS
 * spreading midfielders, and spreading prime players largely spreads
 * defenders: axes (a) and (e) are each correlated with axis (d), and they can
 * pull against each other.
 *
 * The response is not to invent a corrective weight — that would bake one
 * night's squad into the code, which is the failure mode this whole rewrite
 * exists to remove. The response is to keep the weights explicit and tunable
 * and to report the per-axis breakdown, so the organizer can see which axis
 * lost rather than guess.
 */
export const WEIGHTS: Weights = { size: 1, star: 1, keeper: 1, prime: 1, mix: 1 };

export const CANDIDATE_COUNT = 200;

export type AxisKey = keyof Weights;

/** One of the three position counts folded into the single `mix` term. */
export type AxisPart = {
  label: string;
  perTeam: number[];
  spread: number;
};

export type Axis = {
  key: AxisKey;
  label: string;
  /** Per-team counts. Empty for `mix`, which reports through `parts` instead. */
  perTeam: number[];
  /** Populated only for `mix`: DEF, MID and FWD. Empty for every other axis. */
  parts: AxisPart[];
  spread: number;
  weight: number;
  /** weight * spread — this axis's contribution to the total. */
  penalty: number;
};

export type TeamsScore = {
  total: number;
  axes: Axis[];
};

export type GenerateResult = {
  teams: BalanceTeam[];
  score: TeamsScore;
  candidatesTried: number;
};

/**
 * max - min. Empty input scores 0, not NaN.
 *
 * This guard is load-bearing: Math.max() of no arguments is -Infinity and
 * Math.min() is Infinity, so an unguarded spread of an empty array returns
 * -Infinity, every candidate ties at -Infinity, and "keep the lowest" silently
 * stops meaning anything. Zero teams and zero keepers both reach this.
 */
export function spread(counts: readonly number[]): number {
  if (counts.length === 0) return 0;
  let min = counts[0];
  let max = counts[0];
  for (const value of counts) {
    if (value < min) min = value;
    if (value > max) max = value;
  }
  return max - min;
}

/**
 * How many players each team actually gets, derived from the selection.
 *
 * Deliberately not the players-per-team stepper. That stepper is a target the
 * organizer is aiming at, and treating it as a capacity would bench everyone
 * past `teamCount * playersPerTeam`. These capacities always sum to exactly
 * `selected`, so everyone plays and sizes differ by at most one.
 */
export function teamCapacities(selected: number, teamCount: number): number[] {
  const teams = effectiveTeamCount(selected, teamCount);
  if (teams === 0) return [];
  const base = Math.floor(selected / teams);
  const remainder = selected % teams;
  return Array.from({ length: teams }, (_, index) => base + (index < remainder ? 1 : 0));
}

/**
 * More teams than players is a legitimate thing to type by accident pitchside.
 * Rather than emit empty team cards, the extra teams are dropped and the page
 * says so through the notice channel.
 */
export function effectiveTeamCount(selected: number, teamCount: number): number {
  if (selected <= 0 || teamCount <= 0) return 0;
  return Math.min(Math.floor(teamCount), selected);
}

/**
 * A player whose listed position is goalkeeper — the specialist tier.
 *
 * The `keeper` flag is required as well as the position. In the app the two
 * cannot disagree (page.tsx derives keeper from the position data, so every GK
 * has it), but this is an exported function on a public module: a caller who
 * built a GK with `keeper: false` would otherwise have them consume a team's
 * goalkeeping slot while axis (b) — which reads the flag — counted nobody, and
 * that team would end up with no counted keeper at all. Reading the same field
 * the scoring reads keeps seating and scoring from ever drifting apart.
 */
export function isPrimaryKeeper(player: BalancePlayer): boolean {
  return player.keeper && player.position === "GK";
}

/** Fisher-Yates on a copy, using the injected RNG so results are reproducible. */
function shuffled<T>(items: readonly T[], rng: Rng): T[] {
  const next = [...items];
  for (let i = next.length - 1; i > 0; i -= 1) {
    const j = Math.floor(rng() * (i + 1));
    [next[i], next[j]] = [next[j], next[i]];
  }
  return next;
}

/**
 * DEF, then MID, then FWD — the three groups axis (d) actually counts.
 *
 * GK and position-less players sort after them, in that order, so the ordering
 * is total and deterministic rather than dependent on the sort's stability.
 */
const POSITION_RANK: Record<string, number> = { DEF: 0, MID: 1, FWD: 2, GK: 3 };
const UNKNOWN_POSITION_RANK = 4;

function positionRank(player: BalancePlayer): number {
  if (player.position === null) return UNKNOWN_POSITION_RANK;
  return POSITION_RANK[player.position] ?? UNKNOWN_POSITION_RANK;
}

/**
 * Sub-order one stratum by primary position, shuffling within each position
 * group so repeated calls differ.
 *
 * This is what makes the position axis BUILT rather than merely scored. Without
 * it, the prime stratum deals its four defenders as one blind block and the top
 * stratum deals its seven midfielders as another, and axis (d) can only
 * complain about the result afterwards.
 */
function byPositionGroup(players: readonly BalancePlayer[], rng: Rng): BalancePlayer[] {
  const groups = new Map<number, BalancePlayer[]>();
  for (const player of players) {
    const rank = positionRank(player);
    const group = groups.get(rank);
    if (group) group.push(player);
    else groups.set(rank, [player]);
  }
  return [...groups.keys()]
    .sort((a, b) => a - b)
    .flatMap((rank) => shuffled(groups.get(rank) ?? [], rng));
}

/**
 * Build one candidate partition.
 *
 * Strata are dealt keeper, then prime, then top, then everyone else, and each
 * player is dealt in exactly one of them. Within a stratum players are
 * sub-ordered by position and handed round-robin to whichever team currently
 * has the fewest players, with RNG breaking the ties — so two calls with the
 * same players give two different, equally reasonable teams.
 */
export function buildCandidate(
  players: readonly BalancePlayer[],
  teamCount: number,
  rng: Rng,
): BalanceTeam[] {
  const count = effectiveTeamCount(players.length, teamCount);
  if (count === 0) return [];

  const capacities = teamCapacities(players.length, count);
  const teams: BalanceTeam[] = Array.from({ length: count }, () => []);
  const placed = new Set<string>();

  /**
   * The emptiest team that still has room, ties broken uniformly by RNG via
   * reservoir sampling.
   *
   * ── Do not "simplify" the tie-break to the lowest index. ──────────────────
   *
   * It looks tidier and every conservation, capacity, lint and type check still
   * passes, which is exactly what makes it dangerous. With a fixed slot-to-team
   * map the only remaining freedom is permuting players inside a single
   * (stratum, position) group — and every member of such a group carries the
   * same star/prime/keeper flags by construction, so those permutations change
   * WHO is on each team without changing a single count. All five axes are
   * count axes, so all 200 candidates collapse to one score and the search
   * becomes theatre: measured on the 13-player fixture, a lowest-index
   * tie-break yields 1 distinct candidate penalty instead of 8, and reaches the
   * true optimum in 0 runs out of 500 instead of 500 out of 500.
   *
   * That is this page's original defect wearing new clothes — silent, and
   * visible only in production. The regression hook is candidate diversity:
   * 200 draws on a realistic squad must produce more than one distinct penalty.
   */
  const openTeam = (): number => {
    let chosen = -1;
    let fewest = Number.POSITIVE_INFINITY;
    let tied = 0;
    for (let team = 0; team < count; team += 1) {
      if (teams[team].length >= capacities[team]) continue;
      const size = teams[team].length;
      if (size < fewest) {
        fewest = size;
        chosen = team;
        tied = 1;
      } else if (size === fewest) {
        tied += 1;
        if (rng() < 1 / tied) chosen = team;
      }
    }
    // Capacities sum to exactly players.length, so every player has a seat and
    // this cannot fire. If it ever did, conservation still wins over tidy
    // sizes: put them in the smallest team rather than drop a human.
    if (chosen === -1) {
      chosen = 0;
      for (let team = 1; team < count; team += 1) {
        if (teams[team].length < teams[chosen].length) chosen = team;
      }
    }
    return chosen;
  };

  const place = (player: BalancePlayer, team: number) => {
    teams[team].push(player);
    placed.add(player.id);
  };

  // ── Stratum 1: keepers ─────────────────────────────────────────────────────
  // Every primary keeper is seated before any secondary is considered, one per
  // team. Then secondaries fill the teams that still have nobody in goal. Both
  // tiers are equally real keepers; only this ordering prefers the specialists.
  const teamOrder = shuffled(
    Array.from({ length: count }, (_, index) => index),
    rng,
  );
  const hasKeeper = new Array<boolean>(count).fill(false);

  const seatKeepers = (pool: readonly BalancePlayer[]) => {
    for (const player of pool) {
      const team = teamOrder.find(
        (candidate) => !hasKeeper[candidate] && teams[candidate].length < capacities[candidate],
      );
      if (team === undefined) return;
      place(player, team);
      hasKeeper[team] = true;
    }
  };

  seatKeepers(shuffled(players.filter(isPrimaryKeeper), rng));
  seatKeepers(shuffled(players.filter((p) => p.keeper && !isPrimaryKeeper(p)), rng));

  // ── Strata 2-4: prime, then top, then the rest ────────────────────────────
  // A keeper left over once every team has one rejoins here as an ordinary
  // player, classified by their other marks — surplus keepers are never
  // dropped. A player who is both prime and top is dealt once, in the prime
  // stratum, and still counts on both axes when the partition is scored.
  const remaining = players.filter((player) => !placed.has(player.id));
  const strata: BalancePlayer[][] = [
    remaining.filter((player) => player.prime),
    remaining.filter((player) => player.star && !player.prime),
    remaining.filter((player) => !player.star && !player.prime),
  ];

  for (const stratum of strata) {
    for (const player of byPositionGroup(stratum, rng)) {
      place(player, openTeam());
    }
  }

  return teams;
}

function countAxis(
  key: AxisKey,
  label: string,
  perTeam: number[],
  weight: number,
): Axis {
  const value = spread(perTeam);
  return { key, label, perTeam, parts: [], spread: value, weight, penalty: weight * value };
}

function countPerTeam(
  teams: readonly BalanceTeam[],
  predicate: (player: BalancePlayer) => boolean,
): number[] {
  return teams.map((team) => team.reduce((total, player) => total + (predicate(player) ? 1 : 0), 0));
}

/**
 * Score a partition: lower is better, and 0 would be perfectly even on every
 * axis. Nothing here reads total_points or price — those measure fantasy form,
 * not whether someone can play futsal.
 */
export function scoreTeams(
  teams: readonly BalanceTeam[],
  weights: Weights = WEIGHTS,
): TeamsScore {
  const mixParts: AxisPart[] = (["DEF", "MID", "FWD"] as const).map((position) => {
    const perTeam = countPerTeam(teams, (player) => player.position === position);
    return { label: position, perTeam, spread: spread(perTeam) };
  });

  // One `mix` term over the SUM of the three position spreads, not three
  // separately weighted terms. Identical while mix is 1, and different the
  // moment anyone tunes it.
  const mixSpread = mixParts.reduce((total, part) => total + part.spread, 0);

  const axes: Axis[] = [
    countAxis("size", "Team size", teams.map((team) => team.length), weights.size),
    countAxis("star", "Top players", countPerTeam(teams, (p) => p.star), weights.star),
    countAxis("keeper", "Keepers", countPerTeam(teams, (p) => p.keeper), weights.keeper),
    countAxis("prime", "Prime position", countPerTeam(teams, (p) => p.prime), weights.prime),
    {
      key: "mix",
      label: "Position mix",
      perTeam: [],
      parts: mixParts,
      spread: mixSpread,
      weight: weights.mix,
      penalty: weights.mix * mixSpread,
    },
  ];

  return { total: axes.reduce((total, axis) => total + axis.penalty, 0), axes };
}

/**
 * Build many candidates, keep the best-scoring one.
 *
 * Equal-scoring candidates are chosen between uniformly at random (reservoir
 * sampling over the run of ties) rather than by keeping the first. On a small
 * even squad a large share of candidates tie at the optimum, and keeping the
 * first would make Regenerate look broken.
 */
export function generateTeams(
  players: readonly BalancePlayer[],
  teamCount: number,
  rng: Rng,
  options: { candidateCount?: number; weights?: Weights } = {},
): GenerateResult {
  const candidateCount = Math.max(1, options.candidateCount ?? CANDIDATE_COUNT);
  const weights = options.weights ?? WEIGHTS;

  let bestTeams: BalanceTeam[] = [];
  let bestScore: TeamsScore | null = null;
  let tied = 0;

  for (let attempt = 0; attempt < candidateCount; attempt += 1) {
    const candidate = buildCandidate(players, teamCount, rng);
    const score = scoreTeams(candidate, weights);
    if (bestScore === null || score.total < bestScore.total) {
      bestTeams = candidate;
      bestScore = score;
      tied = 1;
    } else if (score.total === bestScore.total) {
      tied += 1;
      if (rng() < 1 / tied) {
        bestTeams = candidate;
        bestScore = score;
      }
    }
  }

  return {
    teams: bestTeams,
    score: bestScore ?? scoreTeams([], weights),
    candidatesTried: candidateCount,
  };
}

/**
 * Conservation: every selected player lands in exactly one team.
 *
 * Compares id sets rather than totals, because a count check passes happily
 * when one player has been duplicated and another dropped. Returns null when
 * the partition is sound, or a human-readable reason when it is not — that
 * reason is a genuine error, not a notice.
 */
export function conservationError(
  players: readonly BalancePlayer[],
  teams: readonly BalanceTeam[],
): string | null {
  const expected = new Set(players.map((player) => player.id));
  const seen = new Set<string>();
  let dealt = 0;

  for (const team of teams) {
    for (const player of team) {
      dealt += 1;
      if (seen.has(player.id)) return `${player.name} was placed in more than one team.`;
      seen.add(player.id);
      if (!expected.has(player.id)) return `${player.name} was placed but was not selected.`;
    }
  }

  if (dealt !== expected.size) {
    const missing = [...expected].filter((id) => !seen.has(id)).length;
    return `${missing} selected ${missing === 1 ? "player was" : "players were"} left out of every team.`;
  }
  return null;
}
