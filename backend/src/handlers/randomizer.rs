use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::Response,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use sqlx::FromRow;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{
    auth::{handler::AppState, jwt::validate_token},
    error::{AppError, AppResult},
    services::randomizer::{
        resolve_player_for_full_name, ConnectedRandomizerUser, JoinEligibility, PlayerIdentity,
        RandomizerClientMessage,
    },
};

#[derive(Debug, Deserialize)]
pub struct RandomizerWsQuery {
    token: String,
}

#[derive(Debug, FromRow)]
struct RandomizerUserRow {
    id: Uuid,
    username: String,
    full_name: String,
}

#[derive(Debug, FromRow)]
struct RandomizerPlayerRow {
    id: Uuid,
    name: String,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(query): Query<RandomizerWsQuery>,
) -> AppResult<Response> {
    let claims = validate_token(&query.token, &state.jwt_secret)?;
    let user = fetch_randomizer_user(&state, claims.sub).await?;
    let eligibility = fetch_join_eligibility(&state, &user.full_name).await?;
    let randomizer_hub = state.randomizer.clone();

    Ok(ws.on_upgrade(move |socket| async move {
        handle_socket(socket, randomizer_hub, user, eligibility).await;
    }))
}

async fn handle_socket(
    socket: WebSocket,
    randomizer_hub: crate::services::randomizer::RandomizerHub,
    user: ConnectedRandomizerUser,
    eligibility: JoinEligibility,
) {
    let connection_id = Uuid::new_v4();
    let (mut sender, mut receiver) = socket.split();
    let (outbound_tx, mut outbound_rx) = mpsc::unbounded_channel::<String>();

    randomizer_hub
        .register_connection(connection_id, user, eligibility, outbound_tx)
        .await;
    randomizer_hub.send_state_to_connection(connection_id).await;

    let writer = tokio::spawn(async move {
        while let Some(message) = outbound_rx.recv().await {
            if sender.send(Message::Text(message.into())).await.is_err() {
                break;
            }
        }
    });

    while let Some(result) = receiver.next().await {
        match result {
            Ok(Message::Text(text)) => {
                handle_client_message(&randomizer_hub, connection_id, &text).await;
            }
            Ok(Message::Close(_)) => {
                break;
            }
            Ok(Message::Ping(_)) => {}
            Ok(_) => {}
            Err(err) => {
                tracing::warn!("Randomizer websocket receive error: {err}");
                break;
            }
        }
    }

    randomizer_hub.unregister_connection(connection_id).await;
    writer.abort();
}

async fn handle_client_message(
    randomizer_hub: &crate::services::randomizer::RandomizerHub,
    connection_id: Uuid,
    payload: &str,
) {
    match serde_json::from_str::<RandomizerClientMessage>(payload) {
        Ok(RandomizerClientMessage::JoinRoom) => {
            if let Err(message) = randomizer_hub.join(connection_id).await {
                randomizer_hub.send_state_to_connection(connection_id).await;
                randomizer_hub.send_error_to_connection(connection_id, message).await;
            }
        }
        Ok(RandomizerClientMessage::LeaveRoom) => {
            if let Err(message) = randomizer_hub.leave(connection_id).await {
                randomizer_hub.send_error_to_connection(connection_id, message).await;
            }
        }
        Ok(RandomizerClientMessage::Randomize { team_count }) => {
            if let Err(message) = randomizer_hub.randomize(connection_id, team_count).await {
                randomizer_hub.send_error_to_connection(connection_id, message).await;
            }
        }
        Err(err) => {
            tracing::warn!("Failed to parse randomizer websocket message: {err}");
            randomizer_hub
                .send_error_to_connection(connection_id, "Invalid realtime randomizer message.".to_string())
                .await;
        }
    }
}

async fn fetch_randomizer_user(state: &AppState, user_id: Uuid) -> AppResult<ConnectedRandomizerUser> {
    let row = sqlx::query_as::<_, RandomizerUserRow>(
        "SELECT id, username, full_name FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::Auth("Authenticated user no longer exists".to_string()))?;

    Ok(ConnectedRandomizerUser {
        user_id: row.id,
        username: row.username,
        full_name: row.full_name,
    })
}

async fn fetch_join_eligibility(state: &AppState, full_name: &str) -> AppResult<JoinEligibility> {
    let matching_players = sqlx::query_as::<_, RandomizerPlayerRow>(
        r#"SELECT id, name
           FROM players
           WHERE LOWER(BTRIM(name)) = LOWER(BTRIM($1))
           ORDER BY name ASC"#,
    )
    .bind(full_name)
    .fetch_all(&state.pool)
    .await?;

    let players: Vec<PlayerIdentity> = matching_players
        .into_iter()
        .map(|player| PlayerIdentity {
            id: player.id,
            name: player.name,
        })
        .collect();

    match resolve_player_for_full_name(full_name, &players) {
        Ok(player) => Ok(JoinEligibility {
            resolved_player: Some(player),
            join_error: None,
        }),
        Err(join_error) => Ok(JoinEligibility {
            resolved_player: None,
            join_error: Some(join_error),
        }),
    }
}
