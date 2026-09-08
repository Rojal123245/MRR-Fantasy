//! Shared scaffolding for the tests that need a real database.
//!
//! Most of this project's behaviour is SQL, so most of its tests are worthless
//! without Postgres to run against.

/// Connect to `DATABASE_URL`, or fail loudly.
///
/// This used to hand back an `Option` that every caller turned into an early
/// `return`, and an early return from a `#[tokio::test]` is a **pass**. With no
/// `DATABASE_URL` the suite reported "64 passed" in 0.01s — the same count it
/// reports in 5.8s with a database, and nothing in the output told the two
/// apart. Thirty-nine tests, essentially all of the scoring and money
/// regression coverage, were reported green without executing.
///
/// The connection error was swallowed too, so a CI job that sets the variable
/// and points it at a Postgres still starting up passed just as quietly.
///
/// So silence now has to be asked for: set `MRR_SKIP_DB_TESTS=1` to skip, and
/// the run says so. Anything else — variable unset, database unreachable — is a
/// failure, because that is what it is.
pub async fn pool() -> Option<sqlx::PgPool> {
    if std::env::var("MRR_SKIP_DB_TESTS").is_ok_and(|v| v == "1") {
        eprintln!(
            "SKIPPING a database test: MRR_SKIP_DB_TESTS=1. \
             This run does NOT cover scoring, points or budget behaviour."
        );
        return None;
    }

    let url = std::env::var("DATABASE_URL").expect(
        "DATABASE_URL is not set, so this test would pass without checking anything. \
         Point it at a Postgres with `migrations/` applied, or set MRR_SKIP_DB_TESTS=1 \
         to skip database tests deliberately.",
    );

    match sqlx::PgPool::connect(&url).await {
        Ok(pool) => Some(pool),
        Err(e) => panic!(
            "DATABASE_URL is set but unreachable, so this test would otherwise pass \
             without checking anything: {e}"
        ),
    }
}
