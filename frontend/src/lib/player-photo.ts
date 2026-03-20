const NON_ALPHANUMERIC = /[^a-z0-9]+/g;

/** Normalize names so "Rajeev Lamichhaney" matches "Rajeev-Lamichhaney.jpeg". */
export function normalizePlayerName(name: string): string {
  return name.trim().toLowerCase().replace(NON_ALPHANUMERIC, "-").replace(/^-+|-+$/g, "");
}

/** Build API URL that resolves a player photo by name. */
export function getPlayerPhotoUrl(playerName: string): string {
  return `/api/player-photo/${encodeURIComponent(playerName)}`;
}

export function getPlayerInitials(name: string): string {
  const parts = name.trim().split(/\s+/).filter(Boolean);
  if (parts.length === 0) return "?";
  if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase();
  return `${parts[0][0] ?? ""}${parts[1][0] ?? ""}`.toUpperCase();
}
