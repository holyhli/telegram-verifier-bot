pub mod answer_repo;
pub mod applicant_repo;
pub mod blacklist_repo;
pub mod community_repo;
pub mod join_request_repo;
pub mod moderation_repo;
pub mod session_repo;
pub mod sync;

pub use answer_repo::AnswerRepo;
pub use applicant_repo::ApplicantRepo;
pub use blacklist_repo::BlacklistRepo;
pub use community_repo::CommunityRepo;
pub use join_request_repo::JoinRequestRepo;
pub use moderation_repo::ModerationActionRepo;
pub use session_repo::SessionRepo;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

pub async fn create_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await
}

pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}
