mod auth;
mod config;
mod db;
mod error;
mod handlers;
mod models;
mod services;

use axum::{
    middleware,
    routing::{delete, get, post, put},
    Extension, Router,
};
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::EnvFilter;

use auth::handler::AppState;

async fn health_check() -> &'static str {
    "ok"
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    // Load environment
    dotenvy::dotenv().ok();
    let config = config::AppConfig::from_env();

    // Connect to database
    let pool = db::create_pool(&config.database_url)
        .await
        .expect("Failed to connect to database");

    tracing::info!("Connected to database");

    // Run migrations
    sqlx::migrate!("../migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    tracing::info!("Migrations applied");

    // Seed data
    if let Err(e) = services::seed::seed_players(&pool).await {
        tracing::warn!("Failed to seed players: {e}");
    }
    if let Err(e) = services::seed::seed_match_weeks(&pool).await {
        tracing::warn!("Failed to seed match weeks: {e}");
    }

    let state = AppState {
        pool,
        jwt_secret: config.jwt_secret.clone(),
    };

    // CORS layer
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Auth routes (public)
    let auth_routes = Router::new()
        .route("/register", post(auth::handler::register))
        .route("/login", post(auth::handler::login));

    // Player routes (public)
    let player_routes = Router::new()
        .route("/", get(handlers::players::list_players))
        .route("/leaderboard", get(handlers::players::leaderboard))
        .route("/:id", get(handlers::players::get_player));

    // Points routes (public)
    let points_routes = Router::new()
        .route("/week/:week", get(handlers::points::get_week_points))
        .route("/player/:id", get(handlers::points::get_player_points));

    // Team routes (mixed: lock-status is public, rest protected)
    let team_public_routes = Router::new()
        .route("/lock-status", get(handlers::teams::lock_status));

    let team_protected_routes = Router::new()
        .route("/", post(handlers::teams::create_team))
        .route("/my", get(handlers::teams::get_my_team))
        .route("/:id/players", put(handlers::teams::set_team_players))
        .route("/:id/points", get(handlers::teams::get_team_points))
        .route(
            "/:id/transfer",
            get(handlers::teams::get_transfer_status).post(handlers::teams::transfer_player),
        )
        .route(
            "/:id/chips",
            get(handlers::chips::get_chip_status).post(handlers::chips::activate_chip),
        )
        .route(
            "/:id/chips/:chip_type",
            delete(handlers::chips::deactivate_chip),
        )
        .layer(middleware::from_fn(auth::middleware::auth_middleware))
        .layer(Extension(config.jwt_secret.clone()));

    let team_routes = Router::new()
        .merge(team_public_routes)
        .merge(team_protected_routes);

    // League routes (mixed: some public, some protected)
    let league_public_routes = Router::new()
        .route("/:id", get(handlers::leagues::get_league))
        .route("/:id/leaderboard", get(handlers::leagues::get_leaderboard));

    let league_protected_routes = Router::new()
        .route("/", post(handlers::leagues::create_league))
        .route("/join", post(handlers::leagues::join_league))
        .route("/my", get(handlers::leagues::get_my_leagues))
        .route(
            "/:league_id/members/:user_id/lineup",
            get(handlers::leagues::get_member_lineup),
        )
        .layer(middleware::from_fn(auth::middleware::auth_middleware))
        .layer(Extension(config.jwt_secret.clone()));

    let league_routes = Router::new()
        .merge(league_public_routes)
        .merge(league_protected_routes);

    // Admin routes (protected by both auth + admin middleware)
    let admin_routes = Router::new()
        .route("/gameweek", post(handlers::admin::create_gameweek))
        .route("/gameweek/:week/stats", get(handlers::admin::get_week_stats))
        .route("/gameweek/:week/stats", post(handlers::admin::submit_week_stats))
        .layer(middleware::from_fn(auth::admin::admin_middleware))
        .layer(middleware::from_fn(auth::middleware::auth_middleware))
        .layer(Extension(state.pool.clone()))
        .layer(Extension(config.jwt_secret.clone()));

    // Compose all routes under /api
    let app = Router::new()
        .route("/healthz", get(health_check))
        .nest("/api/auth", auth_routes)
        .nest("/api/players", player_routes)
        .nest("/api/points", points_routes)
        .nest("/api/teams", team_routes)
        .nest("/api/leagues", league_routes)
        .nest("/api/admin", admin_routes)
        .layer(cors)
        .with_state(state);

    let addr = format!("{}:{}", config.server_host, config.server_port);
    tracing::info!("MRR Fantasy backend starting on {addr}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind address");

    axum::serve(listener, app).await.expect("Server error");
}
