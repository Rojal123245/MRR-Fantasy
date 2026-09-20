/**
 * Randomizer persistence.
 *
 * Every function here is total: storage can be missing, blocked, full, or hold
 * somebody else's garbage, and none of that is allowed to throw or to blank the
 * page. A failed read falls back to the preset and reports itself through the
 * returned notices so the organizer sees a muted line rather than nothing.
 *
 * ── Why deltas and not the marks themselves ──────────────────────────────────
 *
 * What is stored is the organizer's *edits* to a preset — which ids they added
 * and which they removed — never the resulting set of marked players.
 *
 * This is load-bearing, not stylistic. If the absolute mark set were persisted,
 * then a later edit to presets.ts would never reach any browser that had ever
 * tapped a star: that browser would restore its frozen snapshot and quietly
 * ignore the new list forever. That is the original "edit the array, redeploy,
 * hope" defect wearing a different hat. Storing deltas means a preset edit
 * always lands, and only the organizer's own deliberate overrides survive it.
 *
 * Deltas are scoped per preset id, so switching preset does not drag one
 * squad's overrides onto another's.
 */

/** Marks the organizer has added to, or removed from, one preset. */
export type MarkDeltas = {
  star: { add: string[]; remove: string[] };
  prime: { add: string[]; remove: string[] };
};

export type TeamShape = {
  teamCount: number;
  playersPerTeam: number;
};

export type LoadedState = {
  presetId: string;
  /** Keyed by preset id. */
  deltasByPreset: Record<string, MarkDeltas>;
  shape: TeamShape;
  /** Non-blocking messages for the notice channel. Never errors. */
  notices: string[];
};

export const STORAGE_KEYS = {
  presetId: "randomizer:presetId",
  deltas: "randomizer:deltas",
  shape: "randomizer:shape",
  /** Pre-preset key: a flat list of starred ids. Read once, then retired. */
  legacyStarred: "randomizer:starred",
} as const;

export function emptyDeltas(): MarkDeltas {
  return { star: { add: [], remove: [] }, prime: { add: [], remove: [] } };
}

const STORAGE_UNAVAILABLE =
  "Saved marks could not be read on this device, so the preset is showing as-is. Your taps will still work for this session.";

const STORAGE_WRITE_FAILED =
  "This device is not saving changes (private browsing or full storage). Everything still works, it just will not be remembered.";

const STORAGE_CORRUPT =
  "Some saved settings on this device were unreadable and have been reset. The squad list is showing as-is.";

const UNKNOWN_PRESET =
  "The squad list saved on this device no longer exists, so the default one is showing.";

/**
 * localStorage access that never throws. Safari in private mode and blocked
 * third-party storage both throw on plain property access, not only on write,
 * so even reaching for the object is wrapped.
 */
function readRaw(key: string): { raw: string | null; ok: boolean } {
  if (typeof window === "undefined") return { raw: null, ok: true };
  try {
    return { raw: window.localStorage.getItem(key), ok: true };
  } catch {
    return { raw: null, ok: false };
  }
}

function writeRaw(key: string, value: string): boolean {
  if (typeof window === "undefined") return true;
  try {
    window.localStorage.setItem(key, value);
    return true;
  } catch {
    return false;
  }
}

function removeRaw(key: string): boolean {
  if (typeof window === "undefined") return true;
  try {
    window.localStorage.removeItem(key);
    return true;
  } catch {
    return false;
  }
}

/**
 * `ok: false` means the key held something, and that something was not JSON.
 * The distinction matters: the caller must be able to tell "nothing saved yet"
 * (normal, silent) from "your saved marks were corrupted" (worth a notice).
 * Swallowing the second as the first is the same silent-failure shape this
 * rewrite exists to remove.
 */
function parseJson(raw: string): { value: unknown; ok: boolean } {
  try {
    return { value: JSON.parse(raw) as unknown, ok: true };
  } catch {
    return { value: undefined, ok: false };
  }
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/**
 * Well-formed JSON of the wrong shape is as likely as malformed JSON — an old
 * version of this app, a different app on the same origin, a half-finished
 * hand-edit — so every field is narrowed rather than asserted. Unknown ids are
 * kept: an id that resolves to nobody today may be a player who is re-added
 * tomorrow, and dropping it would silently discard the organizer's tap.
 */
function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item) => typeof item === "string");
}

function toIdList(value: unknown): string[] {
  return isStringArray(value) ? Array.from(new Set(value)) : [];
}

function toMarkDeltas(value: unknown): MarkDeltas {
  const result = emptyDeltas();
  if (typeof value !== "object" || value === null) return result;
  const record = value as Record<string, unknown>;

  for (const mark of ["star", "prime"] as const) {
    const side = record[mark];
    if (typeof side !== "object" || side === null) continue;
    const sideRecord = side as Record<string, unknown>;
    const add = toIdList(sideRecord.add);
    const removeSet = new Set(toIdList(sideRecord.remove));
    // An id on both sides is incoherent. `add` wins, because the only way to
    // land on both is a bug here, and a spurious extra mark is visible and
    // one tap from fixed, whereas a spurious removal looks like the preset
    // simply forgot someone.
    result[mark].add = add;
    result[mark].remove = Array.from(removeSet).filter((id) => !add.includes(id));
  }
  return result;
}

function toDeltaMap(value: unknown): Record<string, MarkDeltas> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return {};
  const out: Record<string, MarkDeltas> = {};
  for (const [presetId, deltas] of Object.entries(value as Record<string, unknown>)) {
    out[presetId] = toMarkDeltas(deltas);
  }
  return out;
}

function toShape(value: unknown, fallback: TeamShape): TeamShape {
  if (typeof value !== "object" || value === null) return fallback;
  const record = value as Record<string, unknown>;
  const teamCount = record.teamCount;
  const playersPerTeam = record.playersPerTeam;
  return {
    teamCount: isPositiveInt(teamCount) ? teamCount : fallback.teamCount,
    playersPerTeam: isPositiveInt(playersPerTeam) ? playersPerTeam : fallback.playersPerTeam,
  };
}

function isPositiveInt(value: unknown): value is number {
  return typeof value === "number" && Number.isInteger(value) && value > 0;
}

/**
 * Read everything in one pass, because the legacy migration needs the preset id
 * to know which preset the adopted stars belong to.
 */
export function loadState(defaults: {
  presetId: string;
  /** Every preset id the app recognises, so an unknown saved id is caught here. */
  knownPresetIds: readonly string[];
  shape: TeamShape;
}): LoadedState {
  const notices: string[] = [];
  /** An access threw — storage is blocked or unavailable entirely. */
  let unreadable = false;
  /** A key held a value that could not be used — present, but corrupt. */
  let corrupt = false;

  const presetRead = readRaw(STORAGE_KEYS.presetId);
  if (!presetRead.ok) unreadable = true;
  const storedPresetId = presetRead.raw;
  // Validated here rather than by the caller, so that the migration below keys
  // the adopted stars under a preset that actually exists. Keying them under a
  // dropped id would strand them where nothing can reach them again.
  const presetId =
    storedPresetId !== null && defaults.knownPresetIds.includes(storedPresetId)
      ? storedPresetId
      : defaults.presetId;
  if (storedPresetId !== null && storedPresetId !== presetId) notices.push(UNKNOWN_PRESET);

  const deltasRead = readRaw(STORAGE_KEYS.deltas);
  if (!deltasRead.ok) unreadable = true;
  let deltasByPreset: Record<string, MarkDeltas> = {};
  if (deltasRead.raw !== null) {
    const parsed = parseJson(deltasRead.raw);
    if (parsed.ok && isPlainObject(parsed.value)) deltasByPreset = toDeltaMap(parsed.value);
    else corrupt = true;
  }

  const shapeRead = readRaw(STORAGE_KEYS.shape);
  if (!shapeRead.ok) unreadable = true;
  let shape = defaults.shape;
  if (shapeRead.raw !== null) {
    const parsed = parseJson(shapeRead.raw);
    if (parsed.ok && isPlainObject(parsed.value)) shape = toShape(parsed.value, defaults.shape);
    else corrupt = true;
  }

  const migration = migrateLegacyStarred(presetId, deltasByPreset);
  if (migration.corrupt) corrupt = true;
  if (migration.adopted > 0) {
    notices.push(
      `Carried ${migration.adopted} starred ${migration.adopted === 1 ? "player" : "players"} over from the previous version.`,
    );
  }

  if (unreadable) notices.push(STORAGE_UNAVAILABLE);
  else if (corrupt) notices.push(STORAGE_CORRUPT);

  return { presetId, deltasByPreset, shape, notices };
}

/**
 * Adopt the pre-preset `randomizer:starred` list as star additions on whichever
 * preset is active, then retire the key.
 *
 * Retiring it is the point: left in place it would be re-adopted on every load,
 * resurrecting a star the organizer had since removed. Migration has to be a
 * one-time event, so the evidence of it is consumed.
 */
function migrateLegacyStarred(
  presetId: string,
  deltasByPreset: Record<string, MarkDeltas>,
): { adopted: number; corrupt: boolean } {
  const legacy = readRaw(STORAGE_KEYS.legacyStarred);
  if (!legacy.ok || legacy.raw === null) return { adopted: 0, corrupt: false };

  const parsed = parseJson(legacy.raw);
  if (!parsed.ok) {
    // The key holds something, and that something is not JSON. Retiring it here
    // would destroy the organizer's old star list permanently and silently, to
    // save them a muted line of text. Leave it where it is and say so instead;
    // a later version, or a human, may still be able to read it.
    return { adopted: 0, corrupt: true };
  }

  const ids = toIdList(parsed.value);
  if (ids.length === 0) {
    // Parsed cleanly and genuinely holds nothing: safe to retire.
    removeRaw(STORAGE_KEYS.legacyStarred);
    return { adopted: 0, corrupt: false };
  }

  const current = deltasByPreset[presetId] ?? emptyDeltas();
  const add = Array.from(new Set([...current.star.add, ...ids]));
  deltasByPreset[presetId] = {
    star: { add, remove: current.star.remove.filter((id) => !add.includes(id)) },
    prime: current.prime,
  };

  // Only retire the key once the adopted value is safely written forward.
  if (writeRaw(STORAGE_KEYS.deltas, JSON.stringify(deltasByPreset))) {
    removeRaw(STORAGE_KEYS.legacyStarred);
  }
  return { adopted: ids.length, corrupt: false };
}

/** Returns a notice if the write failed, so the caller can surface it once. */
export function savePresetId(presetId: string): string | null {
  return writeRaw(STORAGE_KEYS.presetId, presetId) ? null : STORAGE_WRITE_FAILED;
}

export function saveDeltas(deltasByPreset: Record<string, MarkDeltas>): string | null {
  return writeRaw(STORAGE_KEYS.deltas, JSON.stringify(deltasByPreset))
    ? null
    : STORAGE_WRITE_FAILED;
}

export function saveShape(shape: TeamShape): string | null {
  return writeRaw(STORAGE_KEYS.shape, JSON.stringify(shape)) ? null : STORAGE_WRITE_FAILED;
}
