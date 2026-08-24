const API_BASE =
  process.env.NEXT_PUBLIC_API_URL ??
  (process.env.NODE_ENV === "development" ? "http://localhost:8080" : "");

interface FetchOptions {
  method?: string;
  body?: unknown;
  token?: string;
}

/**
 * A failed request, carrying the status so callers can tell an expired session
 * from a rejected one. Without it every failure is just a string, and the only
 * way to react to a 401 is to match on its text.
 */
export class ApiError extends Error {
  constructor(
    message: string,
    readonly status: number,
  ) {
    super(message);
    this.name = "ApiError";
  }

  /** The session is gone or no longer valid; the user has to sign in again. */
  get isExpiredSession() {
    return this.status === 401;
  }
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
    // Not every failure has a JSON body. An unmatched route returns an empty
    // 404, and a proxy or gateway error returns HTML — both used to land here
    // as a bare "Unknown error" with the status thrown away, which says nothing
    // about whether the server is down, the route is missing, or the request
    // was refused. Fall back to the status so the failure is always identifiable.
    const payload: unknown = await res.json().catch(() => null);
    const serverMessage =
      payload && typeof payload === "object" && "error" in payload
        ? (payload as { error?: unknown }).error
        : undefined;

    throw new ApiError(
      typeof serverMessage === "string" && serverMessage.length > 0
        ? serverMessage
        : `Request failed (${res.status}${res.statusText ? ` ${res.statusText}` : ""})`,
      res.status,
    );
  }

  return res.json();
}

// Auth
export interface User {
  id: string;
  username: string;
  full_name: string;
  email: string;
  is_admin: boolean;
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

export function resetPassword(
  email: string,
  newPassword: string,
  currentPassword: string,
) {
  return apiFetch<{ message: string }>("/api/auth/reset-password", {
    method: "POST",
    body: { email, new_password: newPassword, current_password: currentPassword },
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
  budget_limit: string;
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
  full_name: string;
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

export interface MemberLineup {
  user_id: string;
  username: string;
  team_name: string;
  captain_id: string | null;
  starters: StarterPlayer[];
}

export function getMemberLineup(leagueId: string, userId: string, token: string) {
  return apiFetch<MemberLineup>(`/api/leagues/${leagueId}/members/${userId}/lineup`, { token });
}

export interface MyLeague {
  id: string;
  name: string;
  invite_code: string;
  member_count: number;
  created_at: string;
}

export function getMyLeagues(token: string) {
  return apiFetch<MyLeague[]>("/api/leagues/my", { token });
}

export interface LeagueGameweekMember {
  user_id: string;
  username: string;
  full_name: string;
  team_name: string | null;
  week_number: number;
  gameweek_points: number;
}

export interface LeagueGameweekDetail {
  league_id: string;
  week_number: number;
  members: LeagueGameweekMember[];
}

export function getLeagueGameweek(leagueId: string, week: number) {
  return apiFetch<LeagueGameweekDetail>(`/api/leagues/${leagueId}/gameweek/${week}`);
}

// Completed gameweeks: snapshot lineups and the scoreboard

/// One squad member's line in a completed gameweek. The six `*_points` fields
/// are the arithmetic behind `base_points` and add up to it exactly, so the UI
/// can show a total the manager can check by eye.
export interface GameweekPlayerLine {
  id: string;
  name: string;
  position: Position;
  secondary_position: Position | null;
  team_name: string;
  photo_url: string | null;
  /** The role they were played in that week, which set their rates. */
  played_as: Position;
  is_bench: boolean;
  is_captain: boolean;

  goals: number;
  assists: number;
  clean_sheets: number;
  saves: number;
  penalty_saves: number;
  own_goals: number;
  penalty_misses: number;
  regular_fouls: number;
  serious_fouls: number;
  minutes_played: number;

  goal_points: number;
  assist_points: number;
  clean_sheet_points: number;
  save_points: number;
  minutes_points: number;
  deduction_points: number;

  base_points: number;
  /** 1, 2 as captain, or 3 under Triple Captain. */
  multiplier: number;
  /** Whether this line reached the team total. */
  counted: boolean;
  total_points: number;
}

export type ChipType = "triple_captain" | "bench_boost";

export interface MemberGameweek {
  user_id: string;
  username: string;
  team_name: string;
  week_number: number;
  /** False for gameweeks that predate lineup snapshots. The arrays are then
   *  empty and the UI must say the lineup is unknown, not show today's squad. */
  has_snapshot: boolean;
  captain_id: string | null;
  /** Every chip played that week. Usually none or one, but a Triple Captain and
   *  a Bench Boost can land in the same week and for one manager they did. */
  chips_played: ChipType[];
  gross_points: number | null;
  transfer_points_hit: number | null;
  total_points: number | null;
  starters: GameweekPlayerLine[];
  bench: GameweekPlayerLine[];
}

export function getMemberGameweek(
  leagueId: string,
  userId: string,
  week: number,
  token: string,
) {
  return apiFetch<MemberGameweek>(
    `/api/leagues/${leagueId}/members/${userId}/gameweek/${week}`,
    { token },
  );
}

export interface GameweekScoreboardEntry {
  user_id: string;
  username: string;
  full_name: string;
  team_name: string | null;
  gross_points: number | null;
  transfer_points_hit: number | null;
  total_points: number | null;
  chips_played: ChipType[];
  has_snapshot: boolean;
}

export interface GameweekScoreboard {
  league_id: string;
  week_number: number;
  is_complete: boolean;
  entries: GameweekScoreboardEntry[];
}

export function getGameweekScoreboard(leagueId: string, week: number, token: string) {
  return apiFetch<GameweekScoreboard>(
    `/api/leagues/${leagueId}/gameweek/${week}/scoreboard`,
    { token },
  );
}

// Chips
export interface ChipInfo {
  available: boolean;
  used_in_week: number | null;
  can_deactivate: boolean;
}

export interface ActiveGameweek {
  id: string;
  week_number: number;
}

export interface ChipStatus {
  triple_captain: ChipInfo;
  bench_boost: ChipInfo;
  active_gameweek: ActiveGameweek | null;
}

export function getChipStatus(teamId: string, token: string) {
  return apiFetch<ChipStatus>(`/api/teams/${teamId}/chips`, { token });
}

export function activateChip(
  teamId: string,
  chipType: "triple_captain" | "bench_boost",
  token: string
) {
  return apiFetch<ChipStatus>(`/api/teams/${teamId}/chips`, {
    method: "POST",
    body: { chip_type: chipType },
    token,
  });
}

export function deactivateChip(
  teamId: string,
  chipType: "triple_captain" | "bench_boost",
  token: string
) {
  return apiFetch<ChipStatus>(`/api/teams/${teamId}/chips/${chipType}`, {
    method: "DELETE",
    token,
  });
}

// Transfers
export interface TransferStatus {
  transfer_available: boolean;
  active_gameweek: number | null;
  transfers_used: number;
  free_transfers: number;
  extra_transfers: number;
  points_hit: number;
  transferred_out: string | null;
  transferred_in: string | null;
}

export function getTransferStatus(teamId: string, token: string) {
  return apiFetch<TransferStatus>(`/api/teams/${teamId}/transfer`, { token });
}

export function transferPlayer(
  teamId: string,
  playerOutId: string,
  playerInId: string,
  assignedPosition: Position | null,
  token: string
) {
  return apiFetch<FantasyTeam>(`/api/teams/${teamId}/transfer`, {
    method: "POST",
    body: {
      player_out_id: playerOutId,
      player_in_id: playerInId,
      ...(assignedPosition ? { assigned_position: assignedPosition } : {}),
    },
    token,
  });
}

// Lock status
export interface LockStatus {
  locked: boolean;
  unlock_at: string | null;
  manually_unlocked: boolean;
  active_gameweek: number | null;
}

export function getLockStatus() {
  return apiFetch<LockStatus>("/api/teams/lock-status");
}

export interface AdminLineupLockStatus {
  force_unlock: boolean;
  effective_locked: boolean;
  unlock_at: string | null;
}

export function getLineupLockControl(token: string) {
  return apiFetch<AdminLineupLockStatus>("/api/admin/lineup-lock", { token });
}

export function setLineupLockControl(forceUnlock: boolean, token: string) {
  return apiFetch<AdminLineupLockStatus>("/api/admin/lineup-lock", {
    method: "PUT",
    body: { force_unlock: forceUnlock },
    token,
  });
}

// Player leaderboard with aggregated stats
export interface PlayerLeaderboard {
  id: string;
  name: string;
  position: "GK" | "DEF" | "MID" | "FWD";
  team_name: string;
  photo_url: string | null;
  is_top_player: boolean;
  goals: number;
  assists: number;
  clean_sheets: number;
  saves: number;
  total_points: number;
  chosen_by_percent: number;
}

export function getPlayerLeaderboard(position?: string) {
  const params = new URLSearchParams();
  if (position) params.set("position", position);
  const qs = params.toString();
  return apiFetch<PlayerLeaderboard[]>(`/api/players/leaderboard${qs ? `?${qs}` : ""}`);
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
  minutes_played: number;
  total_points: number;
  week_number: number;
}

export function getWeekPoints(week: number) {
  return apiFetch<PlayerPointsDisplay[]>(`/api/points/week/${week}`);
}

export function getPlayerPoints(playerId: string) {
  return apiFetch<PlayerPointsDisplay[]>(`/api/points/player/${playerId}`);
}

// Admin
export interface MatchWeek {
  id: string;
  week_number: number;
  start_date: string;
  end_date: string;
  is_active: boolean;
}

export interface AdminPlayerStats {
  player_id: string;
  player_name: string;
  position: string;
  goals: number;
  assists: number;
  clean_sheets: number;
  saves: number;
  penalty_saves: number;
  own_goals: number;
  penalty_misses: number;
  regular_fouls: number;
  serious_fouls: number;
  minutes_played: number;
  total_points: number;
}

export interface PlayerStatInput {
  player_id: string;
  goals: number;
  assists: number;
  clean_sheets: number;
  saves: number;
  penalty_saves: number;
  own_goals: number;
  penalty_misses: number;
  regular_fouls: number;
  serious_fouls: number;
  minutes_played: number;
}

export function createGameweek(
  weekNumber: number,
  startDate: string,
  endDate: string,
  token: string
) {
  return apiFetch<MatchWeek>("/api/admin/gameweek", {
    method: "POST",
    body: { week_number: weekNumber, start_date: startDate, end_date: endDate },
    token,
  });
}

export function getGameweeks(token: string) {
  return apiFetch<MatchWeek[]>("/api/admin/gameweeks", { token });
}

export function toggleGameweek(weekNumber: number, token: string) {
  return apiFetch<MatchWeek>(`/api/admin/gameweek/${weekNumber}/toggle`, {
    method: "PUT",
    token,
  });
}

export function getWeekStatsAdmin(week: number, token: string) {
  return apiFetch<AdminPlayerStats[]>(`/api/admin/gameweek/${week}/stats`, {
    token,
  });
}

export function submitWeekStats(
  week: number,
  stats: PlayerStatInput[],
  token: string
) {
  return apiFetch<{ ok: boolean; players_updated: number; week: number }>(
    `/api/admin/gameweek/${week}/stats`,
    { method: "POST", body: stats, token }
  );
}

// Accounting
export interface FutsalSession {
  id: string;
  title: string;
  total_amount: string;
  player_count: number;
  paid_count: number;
  created_at: string;
}

export interface SessionPlayer {
  id: string;
  session_id: string;
  user_id: string | null;
  player_name: string;
  amount_due: string;
  is_paid: boolean;
  paid_at: string | null;
}

export interface SessionDetail {
  session: FutsalSession;
  players: SessionPlayer[];
}

export interface AccountingUser {
  id: string;
  username: string;
  full_name: string;
}

export interface UserDue {
  session_id: string;
  session_title: string;
  player_entry_id: string;
  amount_due: string;
  is_paid: boolean;
}

export interface UserSummary {
  user_id: string | null;
  player_name: string;
  total_due: string;
  total_paid: string;
  total_unpaid: string;
  sessions_count: number;
}

export function createFutsalSession(title: string, totalAmount: number, token: string) {
  return apiFetch<FutsalSession>("/api/accounting/sessions", {
    method: "POST",
    body: { title, total_amount: totalAmount },
    token,
  });
}

export function getFutsalSessions(token: string) {
  return apiFetch<FutsalSession[]>("/api/accounting/sessions", { token });
}

export function getFutsalSession(id: string, token: string) {
  return apiFetch<SessionDetail>(`/api/accounting/sessions/${id}`, { token });
}

export function deleteFutsalSession(id: string, token: string) {
  return apiFetch<{ ok: boolean }>(`/api/accounting/sessions/${id}`, {
    method: "DELETE",
    token,
  });
}

export function addSessionPlayer(
  sessionId: string,
  playerName: string,
  userId: string | null,
  token: string
) {
  return apiFetch<SessionDetail>(`/api/accounting/sessions/${sessionId}/players`, {
    method: "POST",
    body: { player_name: playerName, user_id: userId },
    token,
  });
}

export function removeSessionPlayer(sessionId: string, playerId: string, token: string) {
  return apiFetch<SessionDetail>(
    `/api/accounting/sessions/${sessionId}/players/${playerId}`,
    { method: "DELETE", token }
  );
}

export function togglePlayerPaid(sessionId: string, playerId: string, token: string) {
  return apiFetch<SessionDetail>(
    `/api/accounting/sessions/${sessionId}/players/${playerId}/pay`,
    { method: "PUT", token }
  );
}

export function getAccountingUsers(token: string) {
  return apiFetch<AccountingUser[]>("/api/accounting/users", { token });
}

export function getUserSummary(token: string) {
  return apiFetch<UserSummary[]>("/api/accounting/user-summary", { token });
}

export function getMyDues(token: string) {
  return apiFetch<UserDue[]>("/api/accounting/my-dues", { token });
}
