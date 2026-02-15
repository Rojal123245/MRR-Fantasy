use sqlx::PgPool;

/// Seed the database with sample football players.
///
/// This populates the players table with well-known football players
/// across all positions for development and testing.
///
/// # Errors
/// Returns an error if database operations fail.
pub async fn seed_players(pool: &PgPool) -> Result<(), sqlx::Error> {
    // Check if players already exist
    let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM players")
        .fetch_one(pool)
        .await?;

    if count > 0 {
        tracing::info!("Players already seeded ({count} found), skipping");
        return Ok(());
    }

    tracing::info!("Seeding players...");

    // (name, primary_position, secondary_position or None, team, price, is_top_player)
    let players: Vec<(&str, &str, Option<&str>, &str, f64, bool)> = vec![
        // Goalkeepers
        ("Nitesh Das", "GK", Some("DEF"), "MRR Fantasy", 7.0, false),
        ("Bishnu Raj Tamang", "GK", Some("MID"), "MRR Fantasy", 8.0, false),
        ("Himal Puri", "GK", None, "MRR Fantasy", 5.0, false),
        ("Anod Shrestha", "GK", None, "MRR Fantasy", 10.0, true),            // TOP
        // Forwards
        ("Khagendra Kandel", "FWD", Some("DEF"), "MRR Fantasy", 7.0, false),
        ("Arun Lamichhane", "FWD", Some("MID"), "MRR Fantasy", 7.0, false),
        ("Aashish Tangnami", "FWD", Some("MID"), "MRR Fantasy", 10.0, true), // TOP
        ("Kaushal Niraula", "FWD", Some("MID"), "MRR Fantasy", 5.0, false),
        ("Rajeev Lamichhaney", "FWD", Some("MID"), "MRR Fantasy", 10.0, true), // TOP
        ("Aashis Bhattarai", "FWD", Some("DEF"), "MRR Fantasy", 10.0, true), // TOP
        ("Dip Kc", "FWD", Some("MID"), "MRR Fantasy", 10.0, true),           // TOP
        ("Sanish Maharjan", "FWD", Some("MID"), "MRR Fantasy", 6.0, false),
        ("Devendra Nepal", "FWD", Some("MID"), "MRR Fantasy", 7.0, false),
        ("Rojal Pradhan", "FWD", Some("DEF"), "MRR Fantasy", 8.0, false),
        ("Razz Kumar Basnet", "FWD", Some("MID"), "MRR Fantasy", 10.0, true), // TOP
        ("Sudarshan Sapkota", "FWD", None, "MRR Fantasy", 6.0, false),
        ("Siddhartha Shrestha", "FWD", Some("MID"), "MRR Fantasy", 8.0, false),
        ("Dipak Mahatara", "FWD", Some("DEF"), "MRR Fantasy", 7.0, false),
        // Midfielders
        ("Bishal Das", "MID", Some("DEF"), "MRR Fantasy", 6.0, false),
        ("Subin Gajurel", "MID", Some("DEF"), "MRR Fantasy", 7.0, false),
        ("Aayuush Rijal", "MID", Some("DEF"), "MRR Fantasy", 8.0, false),
        ("Parbat Rokka", "MID", Some("DEF"), "MRR Fantasy", 9.0, false),
        ("Sumit Luitel", "MID", Some("DEF"), "MRR Fantasy", 6.0, false),
        // Defenders
        ("Dipendra Adhikari", "DEF", None, "MRR Fantasy", 6.0, false),
        ("Kishor Dhakal", "DEF", None, "MRR Fantasy", 9.0, false),
        ("Suprab Rajbhandari", "DEF", None, "MRR Fantasy", 5.0, false),
        ("Sabin Regmi", "DEF", None, "MRR Fantasy", 7.0, false),
        ("Anish Rana", "DEF", None, "MRR Fantasy", 7.0, false),
        ("Sumit Basnet", "DEF", None, "MRR Fantasy", 5.0, false),
    ];

    for (name, position, secondary_position, team, price, is_top) in &players {
        sqlx::query(
            r#"INSERT INTO players (name, position, secondary_position, team_name, price, is_top_player)
               VALUES ($1, $2::player_position, $3::player_position, $4, $5, $6)"#,
        )
        .bind(name)
        .bind(position)
        .bind(secondary_position)
        .bind(team)
        .bind(price)
        .bind(is_top)
        .execute(pool)
        .await?;
    }

    tracing::info!("Seeded {} players", players.len());
    Ok(())
}

/// Seed sample match weeks for development.
///
/// # Errors
/// Returns an error if database operations fail.
pub async fn seed_match_weeks(pool: &PgPool) -> Result<(), sqlx::Error> {
    let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM match_weeks")
        .fetch_one(pool)
        .await?;

    if count > 0 {
        tracing::info!("Match weeks already seeded, skipping");
        return Ok(());
    }

    tracing::info!("Seeding match weeks...");

    for week in 1..=5 {
        let start = chrono::NaiveDate::from_ymd_opt(2026, 1, (week * 7 - 6) as u32)
            .unwrap_or_default();
        let end = chrono::NaiveDate::from_ymd_opt(2026, 1, (week * 7) as u32)
            .unwrap_or_default();

        sqlx::query(
            r#"INSERT INTO match_weeks (week_number, start_date, end_date, is_active)
               VALUES ($1, $2, $3, $4)"#,
        )
        .bind(week)
        .bind(start)
        .bind(end)
        .bind(week == 1) // First week is active
        .execute(pool)
        .await?;
    }

    tracing::info!("Seeded 5 match weeks");
    Ok(())
}
