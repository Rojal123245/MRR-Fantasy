const API_BASE = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080";

interface FetchOptions {
  method?: string;
  body?: unknown;
  token?: string;
}

async function apiFetch<T>(endpoint: string, options: FetchOptions = {}): Promise<T> {
  const { method = "GET", body, token } = options;

  const headers: Record<string, string> = {
    "Content-Type": "application/json",
  };

  if (token) {
    headers["Authorization"] = `Bearer ${token}`;
  }

  const res = await fetch(`${API_BASE}${endpoint}`, {
    method,
    headers,
    body: body ? JSON.stringify(body) : undefined,
  });

  if (!res.ok) {
    const error = await res.json().catch(() => ({ error: "Unknown error" }));
    throw new Error(error.error || `Request failed with status ${res.status}`);
  }

  return res.json();
}

// Auth
export interface User {
  id: string;
  username: string;
  full_name: string;
  email: string;
  created_at: string;
}

export interface AuthResponse {
  token: string;
  user: User;
}

export function register(username: string, fullName: string, email: string, password: string) {
  return apiFetch<AuthResponse>("/api/auth/register", {
    method: "POST",
    body: { username, full_name: fullName, email, password },
  });
}

export function login(email: string, password: string) {
  return apiFetch<AuthResponse>("/api/auth/login", {
    method: "POST",
    body: { email, password },
  });
}

// Players
export interface Player {
  id: string;
  name: string;
  position: "GK" | "DEF" | "MID" | "FWD";
  secondary_position: "GK" | "DEF" | "MID" | "FWD" | null;
  is_top_player: boolean;
  team_name: string;
  photo_url: string | null;
  price: string;
  total_points: number;
}

export function getPlayers(position?: string, search?: string) {
  const params = new URLSearchParams();
  if (position) params.set("position", position);
  if (search) params.set("search", search);
  const qs = params.toString();
  return apiFetch<Player[]>(`/api/players${qs ? `?${qs}` : ""}`);
}

export function getPlayer(id: string) {
  return apiFetch<Player>(`/api/players/${id}`);
}

// Teams
export type Position = "GK" | "DEF" | "MID" | "FWD";

export interface StarterAssignment {
  player_id: string;
  assigned_position: Position;
}

export interface StarterPlayer extends Player {
  assigned_position: Position;
}

export interface FantasyTeam {
  id: string;
  user_id: string;
  name: string;
  captain_id: string | null;
  created_at: string;
  players: StarterPlayer[];
  bench: Player[];
  total_points: number;
}

export function createTeam(name: string, token: string) {
  return apiFetch<FantasyTeam>("/api/teams", {
    method: "POST",
    body: { name },
    token,
  });
}

export function getMyTeam(token: string) {
  return apiFetch<FantasyTeam>("/api/teams/my", { token });
}

export function setTeamPlayers(
  teamId: string,
  starters: StarterAssignment[],
  benchPlayerIds: string[],
  captainId: string,
  token: string
) {
  return apiFetch<FantasyTeam>(`/api/teams/${teamId}/players`, {
    method: "PUT",
    body: { starters, bench_player_ids: benchPlayerIds, captain_id: captainId },
    token,
  });
}

// Leagues
export interface League {
  id: string;
  name: string;
  invite_code: string;
  created_by: string;
  created_at: string;
}

export interface LeagueMember {
  user_id: string;
  username: string;
  team_name: string | null;
  total_points: number;
}

export interface LeagueDetail {
  league: League;
  members: LeagueMember[];
}

export function createLeague(name: string, token: string) {
  return apiFetch<League>("/api/leagues", {
    method: "POST",
    body: { name },
    token,
  });
}

export function joinLeague(inviteCode: string, token: string) {
  return apiFetch<League>("/api/leagues/join", {
    method: "POST",
    body: { invite_code: inviteCode },
    token,
  });
}

export function getLeague(id: string) {
  return apiFetch<LeagueDetail>(`/api/leagues/${id}`);
}

export function getLeaderboard(leagueId: string) {
  return apiFetch<LeagueMember[]>(`/api/leagues/${leagueId}/leaderboard`);
}

// Points
export interface PlayerPointsDisplay {
  player_id: string;
  player_name: string;
  position: string;
  goals: number;
  assists: number;
  clean_sheets: number;
  saves: number;
  tackles: number;
  total_points: number;
  week_number: number;
}

export function getWeekPoints(week: number) {
  return apiFetch<PlayerPointsDisplay[]>(`/api/points/week/${week}`);
}

export function getPlayerPoints(playerId: string) {
  return apiFetch<PlayerPointsDisplay[]>(`/api/points/player/${playerId}`);
}
