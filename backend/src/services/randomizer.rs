use std::collections::HashMap;
use std::sync::Arc;

use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

const TEAM_SIZE: usize = 6;

const ADVANCE_PLAYERS: &[&str] = &[
    "Aashish Tangnami",
    "Rajeev Lamichhaney",
    "Razz Kumar Basnet",
    "Aashis Bhattarai",
    "Dip Kc",
];

const GOALKEEPERS: &[&str] = &["Anod Shrestha", "Bishnu Raj Tamang", "Himal Puri", "Nitesh Das"];

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum TeamCategory {
    #[serde(rename = "Advance")]
    Advance,
    #[serde(rename = "GK")]
    Gk,
    #[serde(rename = "Regular")]
    Regular,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerIdentity {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRandomizerPlayer {
    pub player_id: Uuid,
    pub player_name: String,
    pub category: TeamCategory,
}

#[derive(Debug, Clone)]
pub struct ConnectedRandomizerUser {
    pub user_id: Uuid,
    pub username: String,
    pub full_name: String,
}

#[derive(Debug, Clone)]
pub struct JoinEligibility {
    pub resolved_player: Option<ResolvedRandomizerPlayer>,
    pub join_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RandomizerParticipant {
    pub user_id: Uuid,
    pub username: String,
    pub player_id: Uuid,
    pub player_name: String,
    pub category: TeamCategory,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RandomizerTeamPlayer {
    pub player_id: Uuid,
    pub player_name: String,
    pub category: TeamCategory,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RandomizerTeam {
    pub team_number: usize,
    pub players: Vec<RandomizerTeamPlayer>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RandomizerRoomStateMessage {
    #[serde(rename = "type")]
    pub message_type: &'static str,
    pub joined: bool,
    pub can_join: bool,
    pub join_error: Option<String>,
    pub participants: Vec<RandomizerParticipant>,
    pub teams: Vec<RandomizerTeam>,
}

#[derive(Debug, Serialize)]
pub struct RandomizerErrorMessage {
    #[serde(rename = "type")]
    pub message_type: &'static str,
    pub message: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RandomizerClientMessage {
    JoinRoom,
    LeaveRoom,
    Randomize { team_count: usize },
}

#[derive(Clone, Default)]
pub struct RandomizerHub {
    inner: Arc<Mutex<RandomizerRoom>>,
}

#[derive(Default)]
struct RandomizerRoom {
    connections: HashMap<Uuid, ConnectionEntry>,
    active_connection_by_user: HashMap<Uuid, Uuid>,
    participants: HashMap<Uuid, RandomizerParticipant>,
    teams: Vec<RandomizerTeam>,
}

struct ConnectionEntry {
    user: ConnectedRandomizerUser,
    eligibility: JoinEligibility,
    sender: mpsc::UnboundedSender<String>,
}

struct PendingMessage {
    sender: mpsc::UnboundedSender<String>,
    payload: String,
}

#[derive(Debug, Clone)]
struct TeamSlot {
    player_id: Uuid,
    player_name: String,
    category: TeamCategory,
}

impl RandomizerHub {
    pub async fn register_connection(
        &self,
        connection_id: Uuid,
        user: ConnectedRandomizerUser,
        eligibility: JoinEligibility,
        sender: mpsc::UnboundedSender<String>,
    ) {
        let mut room = self.inner.lock().await;

        if let Some(old_connection_id) = room.active_connection_by_user.insert(user.user_id, connection_id) {
            room.connections.remove(&old_connection_id);
        }

        room.connections.insert(
            connection_id,
            ConnectionEntry {
                user,
                eligibility,
                sender,
            },
        );
    }

    pub async fn unregister_connection(&self, connection_id: Uuid) {
        let pending = {
            let mut room = self.inner.lock().await;
            let Some(connection) = room.connections.remove(&connection_id) else {
                return;
            };

            let is_active_connection = room
                .active_connection_by_user
                .get(&connection.user.user_id)
                .is_some_and(|active_connection_id| *active_connection_id == connection_id);

            if !is_active_connection {
                Vec::new()
            } else {
                room.active_connection_by_user.remove(&connection.user.user_id);
                if room.participants.remove(&connection.user.user_id).is_some() {
                    room.teams.clear();
                    build_room_state_messages(&room, room.connections.keys().copied())
                } else {
                    Vec::new()
                }
            }
        };

        dispatch_pending_messages(pending);
    }

    pub async fn send_state_to_connection(&self, connection_id: Uuid) {
        let pending = {
            let room = self.inner.lock().await;
            build_room_state_messages(&room, std::iter::once(connection_id))
        };

        dispatch_pending_messages(pending);
    }

    pub async fn send_error_to_connection(&self, connection_id: Uuid, message: impl Into<String>) {
        let pending = {
            let room = self.inner.lock().await;
            room.connections
                .get(&connection_id)
                .and_then(|connection| serialize_error_message(message.into()).map(|payload| PendingMessage {
                    sender: connection.sender.clone(),
                    payload,
                }))
        };

        if let Some(message) = pending {
            let _ = message.sender.send(message.payload);
        }
    }

    pub async fn join(&self, connection_id: Uuid) -> Result<(), String> {
        let pending = {
            let mut room = self.inner.lock().await;
            let Some(connection) = room.connections.get(&connection_id) else {
                return Err("Realtime connection is no longer active.".to_string());
            };
            let user_id = connection.user.user_id;
            let username = connection.user.username.clone();
            let eligibility = connection.eligibility.clone();

            let Some(player) = eligibility.resolved_player else {
                return Err(
                    eligibility
                        .join_error
                        .unwrap_or_else(|| "You cannot join the current match.".to_string()),
                );
            };

            if let Some(existing_participant) = room
                .participants
                .values()
                .find(|participant| participant.player_id == player.player_id && participant.user_id != user_id)
            {
                return Err(format!(
                    "{} is already joined by {}.",
                    existing_participant.player_name, existing_participant.username
                ));
            }

            let next_participant = RandomizerParticipant {
                user_id,
                username,
                player_id: player.player_id,
                player_name: player.player_name,
                category: player.category,
            };

            let changed = match room.participants.insert(user_id, next_participant.clone()) {
                Some(existing) => existing != next_participant,
                None => true,
            };

            if changed {
                room.teams.clear();
            }

            build_room_state_messages(&room, room.connections.keys().copied())
        };

        dispatch_pending_messages(pending);
        Ok(())
    }

    pub async fn leave(&self, connection_id: Uuid) -> Result<(), String> {
        let pending = {
            let mut room = self.inner.lock().await;
            let Some(connection) = room.connections.get(&connection_id) else {
                return Err("Realtime connection is no longer active.".to_string());
            };
            let user_id = connection.user.user_id;

            if room.participants.remove(&user_id).is_some() {
                room.teams.clear();
                build_room_state_messages(&room, room.connections.keys().copied())
            } else {
                build_room_state_messages(&room, std::iter::once(connection_id))
            }
        };

        dispatch_pending_messages(pending);
        Ok(())
    }

    pub async fn randomize(&self, connection_id: Uuid, team_count: usize) -> Result<(), String> {
        let pending = {
            let mut room = self.inner.lock().await;
            let Some(connection) = room.connections.get(&connection_id) else {
                return Err("Realtime connection is no longer active.".to_string());
            };
            let user_id = connection.user.user_id;

            if !room.participants.contains_key(&user_id) {
                return Err("Join the current match before randomizing teams.".to_string());
            }

            if team_count == 0 {
                return Err("Please enter a valid team count.".to_string());
            }

            let participants: Vec<RandomizerParticipant> = room.participants.values().cloned().collect();
            if participants.len() < team_count {
                return Err("Joined players are fewer than number of teams.".to_string());
            }

            room.teams = randomize_participants(&participants, team_count)?;
            build_room_state_messages(&room, room.connections.keys().copied())
        };

        dispatch_pending_messages(pending);
        Ok(())
    }
}

pub fn resolve_player_for_full_name(
    full_name: &str,
    matching_players: &[PlayerIdentity],
) -> Result<ResolvedRandomizerPlayer, String> {
    let trimmed_name = full_name.trim();
    if trimmed_name.is_empty() {
        return Err("Your account is missing a full name. Update it before joining the current match.".to_string());
    }

    match matching_players {
        [] => Err(format!(
            "No futsal player matches your full name \"{trimmed_name}\". Ask an admin to align your account name with the player list."
        )),
        [player] => Ok(ResolvedRandomizerPlayer {
            player_id: player.id,
            player_name: player.name.clone(),
            category: categorize_name(&player.name),
        }),
        _ => Err(format!(
            "Multiple futsal players match your full name \"{trimmed_name}\". Ask an admin to remove the duplicate before joining."
        )),
    }
}

pub fn categorize_name(name: &str) -> TeamCategory {
    let normalized_name = normalize_name(name);

    if ADVANCE_PLAYERS
        .iter()
        .any(|candidate| normalize_name(candidate) == normalized_name)
    {
        TeamCategory::Advance
    } else if GOALKEEPERS
        .iter()
        .any(|candidate| normalize_name(candidate) == normalized_name)
    {
        TeamCategory::Gk
    } else {
        TeamCategory::Regular
    }
}

pub fn randomize_participants(
    participants: &[RandomizerParticipant],
    team_count: usize,
) -> Result<Vec<RandomizerTeam>, String> {
    let mut rng = rand::thread_rng();
    randomize_participants_with_rng(participants, team_count, &mut rng)
}

pub fn randomize_participants_with_rng<R: rand::Rng + ?Sized>(
    participants: &[RandomizerParticipant],
    team_count: usize,
    rng: &mut R,
) -> Result<Vec<RandomizerTeam>, String> {
    if team_count == 0 {
        return Err("Please enter a valid team count.".to_string());
    }

    if participants.len() < team_count {
        return Err("Joined players are fewer than number of teams.".to_string());
    }

    let advance_pool: Vec<_> = participants
        .iter()
        .filter(|participant| participant.category == TeamCategory::Advance)
        .cloned()
        .collect();
    let gk_pool: Vec<_> = participants
        .iter()
        .filter(|participant| participant.category == TeamCategory::Gk)
        .cloned()
        .collect();
    let regular_pool: Vec<_> = participants
        .iter()
        .filter(|participant| participant.category == TeamCategory::Regular)
        .cloned()
        .collect();

    let mut next_teams: Vec<Vec<TeamSlot>> = (0..team_count).map(|_| Vec::new()).collect();
    let mut shuffled_advance = advance_pool;
    let mut shuffled_gk = gk_pool;

    shuffled_advance.shuffle(rng);
    shuffled_gk.shuffle(rng);

    let balanced_team_count = team_count
        .min(participants.len() / TEAM_SIZE)
        .min(shuffled_advance.len());

    for idx in 0..balanced_team_count {
        next_teams[idx].push(team_slot_from_participant(&shuffled_advance[idx]));
    }

    let mut team_order_for_gk: Vec<_> = (0..balanced_team_count).collect();
    team_order_for_gk.shuffle(rng);

    let guaranteed_gk_count = balanced_team_count.min(shuffled_gk.len());
    for idx in 0..guaranteed_gk_count {
        next_teams[team_order_for_gk[idx]].push(team_slot_from_participant(&shuffled_gk[idx]));
    }

    let mut remaining_pool: Vec<TeamSlot> = shuffled_advance
        .iter()
        .skip(balanced_team_count)
        .map(team_slot_from_participant)
        .collect();
    remaining_pool.extend(
        shuffled_gk
            .iter()
            .skip(guaranteed_gk_count)
            .map(team_slot_from_participant),
    );
    remaining_pool.extend(regular_pool.iter().map(team_slot_from_participant));
    remaining_pool.shuffle(rng);

    let mut remaining_idx = 0usize;
    while remaining_idx < remaining_pool.len() {
        let mut fill_order: Vec<_> = (0..balanced_team_count).collect();
        fill_order.shuffle(rng);

        let mut filled_any = false;
        for team_idx in fill_order {
            if remaining_idx >= remaining_pool.len() {
                break;
            }
            if next_teams[team_idx].len() >= TEAM_SIZE {
                continue;
            }

            next_teams[team_idx].push(remaining_pool[remaining_idx].clone());
            remaining_idx += 1;
            filled_any = true;
        }

        if !filled_any {
            break;
        }
    }

    let remaining_team_indices: Vec<_> = (balanced_team_count..team_count).collect();
    let mut team_cycle = 0usize;
    while remaining_idx < remaining_pool.len() && !remaining_team_indices.is_empty() {
        let team_idx = remaining_team_indices[team_cycle % remaining_team_indices.len()];
        next_teams[team_idx].push(remaining_pool[remaining_idx].clone());
        remaining_idx += 1;
        team_cycle += 1;
    }

    if next_teams.iter().all(Vec::is_empty) {
        return Err("Could not create teams from the joined players.".to_string());
    }

    for team in &mut next_teams {
        team.shuffle(rng);
    }

    Ok(next_teams
        .into_iter()
        .enumerate()
        .filter(|(_, team)| !team.is_empty())
        .map(|(idx, team)| RandomizerTeam {
            team_number: idx + 1,
            players: team
                .into_iter()
                .map(|slot| RandomizerTeamPlayer {
                    player_id: slot.player_id,
                    player_name: slot.player_name,
                    category: slot.category,
                })
                .collect(),
        })
        .collect())
}

fn team_slot_from_participant(participant: &RandomizerParticipant) -> TeamSlot {
    TeamSlot {
        player_id: participant.player_id,
        player_name: participant.player_name.clone(),
        category: participant.category,
    }
}

fn normalize_name(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

fn build_room_state_messages(
    room: &RandomizerRoom,
    connection_ids: impl IntoIterator<Item = Uuid>,
) -> Vec<PendingMessage> {
    connection_ids
        .into_iter()
        .filter_map(|connection_id| {
            let connection = room.connections.get(&connection_id)?;
            let payload = serialize_room_state(room, connection)?;

            Some(PendingMessage {
                sender: connection.sender.clone(),
                payload,
            })
        })
        .collect()
}

fn serialize_room_state(
    room: &RandomizerRoom,
    connection: &ConnectionEntry,
) -> Option<String> {
    let message = RandomizerRoomStateMessage {
        message_type: "room_state",
        joined: room.participants.contains_key(&connection.user.user_id),
        can_join: compute_can_join(room, connection),
        join_error: compute_join_error(room, connection),
        participants: sorted_participants(room),
        teams: room.teams.clone(),
    };

    serde_json::to_string(&message)
        .map_err(|err| tracing::error!("Failed to serialize room state: {err}"))
        .ok()
}

fn serialize_error_message(message: String) -> Option<String> {
    serde_json::to_string(&RandomizerErrorMessage {
        message_type: "error",
        message,
    })
    .map_err(|err| tracing::error!("Failed to serialize randomizer error message: {err}"))
    .ok()
}

fn compute_can_join(room: &RandomizerRoom, connection: &ConnectionEntry) -> bool {
    if room.participants.contains_key(&connection.user.user_id) {
        return true;
    }

    match &connection.eligibility.resolved_player {
        Some(player) => room
            .participants
            .values()
            .all(|participant| participant.player_id != player.player_id),
        None => false,
    }
}

fn compute_join_error(room: &RandomizerRoom, connection: &ConnectionEntry) -> Option<String> {
    if room.participants.contains_key(&connection.user.user_id) {
        return None;
    }

    if let Some(base_error) = &connection.eligibility.join_error {
        return Some(base_error.clone());
    }

    connection
        .eligibility
        .resolved_player
        .as_ref()
        .and_then(|player| {
            room.participants
                .values()
                .find(|participant| participant.player_id == player.player_id)
        })
        .map(|participant| format!("{} is already joined by {}.", participant.player_name, participant.username))
}

fn sorted_participants(room: &RandomizerRoom) -> Vec<RandomizerParticipant> {
    let mut participants: Vec<_> = room.participants.values().cloned().collect();
    participants.sort_by(|left, right| {
        left.player_name
            .to_ascii_lowercase()
            .cmp(&right.player_name.to_ascii_lowercase())
            .then_with(|| left.username.to_ascii_lowercase().cmp(&right.username.to_ascii_lowercase()))
    });
    participants
}

fn dispatch_pending_messages(pending_messages: Vec<PendingMessage>) {
    for message in pending_messages {
        let _ = message.sender.send(message.payload);
    }
}

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};
    use tokio::sync::mpsc;

    use super::*;

    fn participant(
        username: &str,
        player_name: &str,
        category: TeamCategory,
    ) -> RandomizerParticipant {
        RandomizerParticipant {
            user_id: Uuid::new_v4(),
            username: username.to_string(),
            player_id: Uuid::new_v4(),
            player_name: player_name.to_string(),
            category,
        }
    }

    fn connection(
        user_id: Uuid,
        username: &str,
        full_name: &str,
        eligibility: JoinEligibility,
    ) -> (
        Uuid,
        ConnectedRandomizerUser,
        JoinEligibility,
        mpsc::UnboundedReceiver<String>,
        mpsc::UnboundedSender<String>,
    ) {
        let connection_id = Uuid::new_v4();
        let user = ConnectedRandomizerUser {
            user_id,
            username: username.to_string(),
            full_name: full_name.to_string(),
        };
        let (tx, rx) = mpsc::unbounded_channel();
        (connection_id, user, eligibility, rx, tx)
    }

    #[test]
    fn resolves_player_by_full_name_case_insensitively() {
        let player_id = Uuid::new_v4();
        let players = vec![PlayerIdentity {
            id: player_id,
            name: "Aashish Tangnami".to_string(),
        }];

        let resolved = resolve_player_for_full_name("  aashish tangnami  ", &players).unwrap();

        assert_eq!(resolved.player_id, player_id);
        assert_eq!(resolved.player_name, "Aashish Tangnami");
        assert_eq!(resolved.category, TeamCategory::Advance);
    }

    #[test]
    fn rejects_missing_or_duplicate_player_matches() {
        let duplicate_name = "Shared Name".to_string();
        let duplicate_players = vec![
            PlayerIdentity {
                id: Uuid::new_v4(),
                name: duplicate_name.clone(),
            },
            PlayerIdentity {
                id: Uuid::new_v4(),
                name: duplicate_name,
            },
        ];

        let missing_error = resolve_player_for_full_name("Missing Player", &[]).unwrap_err();
        let duplicate_error = resolve_player_for_full_name("Shared Name", &duplicate_players).unwrap_err();

        assert!(missing_error.contains("No futsal player matches"));
        assert!(duplicate_error.contains("Multiple futsal players match"));
    }

    #[test]
    fn randomization_only_uses_joined_participants() {
        let joined = vec![
            participant("user-1", "Aashish Tangnami", TeamCategory::Advance),
            participant("user-2", "Anod Shrestha", TeamCategory::Gk),
            participant("user-3", "Bishal Das", TeamCategory::Regular),
        ];

        let mut rng = StdRng::seed_from_u64(7);
        let teams = randomize_participants_with_rng(&joined, 1, &mut rng).unwrap();

        assert_eq!(teams.len(), 1);
        assert_eq!(teams[0].players.len(), 3);
        assert!(teams[0]
            .players
            .iter()
            .all(|player| joined.iter().any(|joined_player| joined_player.player_id == player.player_id)));
    }

    #[test]
    fn randomization_preserves_advance_and_gk_distribution_when_available() {
        let joined = vec![
            participant("user-1", "Aashish Tangnami", TeamCategory::Advance),
            participant("user-2", "Rajeev Lamichhaney", TeamCategory::Advance),
            participant("user-3", "Anod Shrestha", TeamCategory::Gk),
            participant("user-4", "Nitesh Das", TeamCategory::Gk),
            participant("user-5", "Bishal Das", TeamCategory::Regular),
            participant("user-6", "Subin Gajurel", TeamCategory::Regular),
            participant("user-7", "Parbat Rokka", TeamCategory::Regular),
            participant("user-8", "Sumit Luitel", TeamCategory::Regular),
            participant("user-9", "Dipendra Adhikari", TeamCategory::Regular),
            participant("user-10", "Kishor Dhakal", TeamCategory::Regular),
            participant("user-11", "Sabin Regmi", TeamCategory::Regular),
            participant("user-12", "Anish Rana", TeamCategory::Regular),
        ];

        let mut rng = StdRng::seed_from_u64(9);
        let teams = randomize_participants_with_rng(&joined, 2, &mut rng).unwrap();

        assert_eq!(teams.len(), 2);
        for team in teams {
            assert_eq!(team.players.len(), 6);
            assert_eq!(
                team.players
                    .iter()
                    .filter(|player| player.category == TeamCategory::Advance)
                    .count(),
                1
            );
            assert_eq!(
                team.players
                    .iter()
                    .filter(|player| player.category == TeamCategory::Gk)
                    .count(),
                1
            );
        }
    }

    #[tokio::test]
    async fn membership_changes_clear_existing_teams() {
        let hub = RandomizerHub::default();

        let user_one_id = Uuid::new_v4();
        let user_two_id = Uuid::new_v4();

        let (conn_one, user_one, eligibility_one, _rx_one, tx_one) = connection(
            user_one_id,
            "user-one",
            "Aashish Tangnami",
            JoinEligibility {
                resolved_player: Some(ResolvedRandomizerPlayer {
                    player_id: Uuid::new_v4(),
                    player_name: "Aashish Tangnami".to_string(),
                    category: TeamCategory::Advance,
                }),
                join_error: None,
            },
        );
        let (conn_two, user_two, eligibility_two, _rx_two, tx_two) = connection(
            user_two_id,
            "user-two",
            "Anod Shrestha",
            JoinEligibility {
                resolved_player: Some(ResolvedRandomizerPlayer {
                    player_id: Uuid::new_v4(),
                    player_name: "Anod Shrestha".to_string(),
                    category: TeamCategory::Gk,
                }),
                join_error: None,
            },
        );

        hub.register_connection(conn_one, user_one, eligibility_one, tx_one).await;
        hub.register_connection(conn_two, user_two, eligibility_two, tx_two).await;
        hub.join(conn_one).await.unwrap();
        hub.join(conn_two).await.unwrap();
        hub.randomize(conn_one, 1).await.unwrap();

        {
            let room = hub.inner.lock().await;
            assert_eq!(room.teams.len(), 1);
        }

        hub.leave(conn_two).await.unwrap();

        let room = hub.inner.lock().await;
        assert!(room.teams.is_empty());
    }

    #[tokio::test]
    async fn reconnect_replaces_old_connection_without_duplicate_presence() {
        let hub = RandomizerHub::default();
        let user_id = Uuid::new_v4();
        let eligibility = JoinEligibility {
            resolved_player: Some(ResolvedRandomizerPlayer {
                player_id: Uuid::new_v4(),
                player_name: "Bishal Das".to_string(),
                category: TeamCategory::Regular,
            }),
            join_error: None,
        };

        let (conn_one, user_one, eligibility_one, _rx_one, tx_one) =
            connection(user_id, "same-user", "Bishal Das", eligibility.clone());
        let (conn_two, user_two, eligibility_two, _rx_two, tx_two) =
            connection(user_id, "same-user", "Bishal Das", eligibility);

        hub.register_connection(conn_one, user_one, eligibility_one, tx_one).await;
        hub.join(conn_one).await.unwrap();
        hub.register_connection(conn_two, user_two, eligibility_two, tx_two).await;

        {
            let room = hub.inner.lock().await;
            assert_eq!(room.participants.len(), 1);
            assert_eq!(room.active_connection_by_user.get(&user_id), Some(&conn_two));
        }

        hub.unregister_connection(conn_one).await;

        {
            let room = hub.inner.lock().await;
            assert_eq!(room.participants.len(), 1);
        }

        hub.unregister_connection(conn_two).await;

        let room = hub.inner.lock().await;
        assert!(room.participants.is_empty());
    }
}
