import type { Player, Position } from "@/lib/api";

/** A starter and the position they are picked to play. */
export interface LineupSlot {
  player: Player;
  assignedPosition: Position;
}

export const POSITIONS: Position[] = ["GK", "DEF", "MID", "FWD"];

export const POSITION_BADGE: Record<Position, string> = {
  GK: "badge-gk",
  DEF: "badge-def",
  MID: "badge-mid",
  FWD: "badge-fwd",
};

/** The positions a player may be picked at: their primary, then their secondary. */
export function playablePositions(player: Player): Position[] {
  const result: Position[] = [player.position];
  if (player.secondary_position && player.secondary_position !== player.position) {
    result.push(player.secondary_position);
  }
  return result;
}

/** The position a two-position player would switch to, or null if they only play one. */
export function otherPosition(player: Player, current: Position): Position | null {
  return playablePositions(player).find((pos) => pos !== current) ?? null;
}

/** Formation label from assigned positions (e.g. "2-2-1"). */
export function getFormationLabel(slots: LineupSlot[]): string {
  const count = (pos: Position) => slots.filter((s) => s.assignedPosition === pos).length;
  const def = count("DEF");
  const mid = count("MID");
  const fwd = count("FWD");
  if (def + mid + fwd === 0) return "";
  return `${def}-${mid}-${fwd}`;
}

/** Positions with nobody assigned to them. */
export function getMissingPositions(slots: LineupSlot[]): Position[] {
  return POSITIONS.filter((pos) => !slots.some((s) => s.assignedPosition === pos));
}

/**
 * Everything that stops a lineup from being saved on position grounds: a
 * position nobody plays, or more than the one goalkeeper the rules allow.
 */
export interface LineupIssues {
  missing: Position[];
  extraGk: boolean;
}

export function getLineupIssues(slots: LineupSlot[]): LineupIssues {
  return {
    missing: getMissingPositions(slots),
    extraGk: slots.filter((s) => s.assignedPosition === "GK").length > 1,
  };
}

export function hasLineupIssues(issues: LineupIssues): boolean {
  return issues.missing.length > 0 || issues.extraGk;
}

export function moveStarter<T extends LineupSlot>(slots: T[], playerId: string, to: Position): T[] {
  return slots.map((s) => (s.player.id === playerId ? { ...s, assignedPosition: to } : s));
}

/** Two starters trade positions. */
export function swapSpots<T extends LineupSlot>(slots: T[], aId: string, bId: string): T[] {
  const a = slots.find((s) => s.player.id === aId);
  const b = slots.find((s) => s.player.id === bId);
  if (!a || !b) return slots;
  return slots.map((s) =>
    s.player.id === aId
      ? { ...s, assignedPosition: b.assignedPosition }
      : s.player.id === bId
        ? { ...s, assignedPosition: a.assignedPosition }
        : s
  );
}

/**
 * A starter already playing `to` who can take over the mover's current
 * position, so the two can trade without the lineup ever going invalid.
 */
export function findSwapPartner<T extends LineupSlot>(slots: T[], playerId: string, to: Position): T | null {
  const mover = slots.find((s) => s.player.id === playerId);
  if (!mover) return null;
  return (
    slots.find(
      (s) =>
        s.player.id !== playerId &&
        s.assignedPosition === to &&
        playablePositions(s.player).includes(mover.assignedPosition)
    ) ?? null
  );
}

export type MovePreview = { tone: "ok" | "fix" | "warn"; text: string };

/** What moving one starter to `to` would do to the lineup, in a few words. */
export function previewMove(slots: LineupSlot[], playerId: string, to: Position): MovePreview {
  const next = moveStarter(slots, playerId, to);
  const before = getLineupIssues(slots);
  const after = getLineupIssues(next);
  const opened = after.missing.filter((pos) => !before.missing.includes(pos));
  const closed = before.missing.filter((pos) => !after.missing.includes(pos));

  if (opened.length > 0) return { tone: "warn", text: `Leaves no ${opened[0]}` };
  if (after.extraGk && !before.extraGk) return { tone: "warn", text: "2 GKs — only 1 can start" };
  if (closed.length > 0) return { tone: "fix", text: `Fills your ${closed[0]} gap` };
  if (before.extraGk && !after.extraGk) return { tone: "fix", text: "Leaves 1 GK in goal" };
  return { tone: "ok", text: `${getFormationLabel(slots)} → ${getFormationLabel(next)}` };
}
