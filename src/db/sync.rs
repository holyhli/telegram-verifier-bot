use sqlx::PgPool;

use crate::config::CommunityConfig;

pub async fn sync_config_to_db(
    pool: &PgPool,
    communities: &[CommunityConfig],
) -> Result<(), sqlx::Error> {
    for community in communities {
        let community_id = upsert_community(pool, community).await?;
        sync_questions(pool, community_id, community).await?;
    }
    Ok(())
}

async fn upsert_community(pool: &PgPool, community: &CommunityConfig) -> Result<i64, sqlx::Error> {
    let row = sqlx::query_scalar!(
        r#"
        INSERT INTO communities (telegram_chat_id, title, slug, is_active)
        VALUES ($1, $2, $3, TRUE)
        ON CONFLICT (telegram_chat_id) DO UPDATE
            SET title = EXCLUDED.title,
                slug = EXCLUDED.slug,
                is_active = TRUE,
                updated_at = NOW()
        RETURNING id
        "#,
        community.telegram_chat_id,
        community.title,
        community.slug,
    )
    .fetch_one(pool)
    .await?;

    Ok(row)
}

async fn sync_questions(
    pool: &PgPool,
    community_id: i64,
    community: &CommunityConfig,
) -> Result<(), sqlx::Error> {
    // Step 1: Deactivate ALL active questions for this community.
    // This clears the partial unique indexes on (community_id, position)
    // and (community_id, question_key) WHERE is_active = TRUE,
    // preventing conflicts when questions are added, removed, or reordered.
    sqlx::query!(
        r#"
        UPDATE community_questions
        SET is_active = FALSE, updated_at = NOW()
        WHERE community_id = $1 AND is_active = TRUE
        "#,
        community_id,
    )
    .execute(pool)
    .await?;

    // Step 2: For each question in config, reactivate existing row or insert new.
    // Since all rows are inactive, partial unique indexes are empty — no conflicts.
    for question in &community.questions {
        let result = sqlx::query!(
            r#"
            UPDATE community_questions
            SET question_text = $3,
                question_text_uk = $4,
                required = $5,
                position = $6,
                is_active = TRUE,
                updated_at = NOW()
            WHERE community_id = $1 AND question_key = $2
            "#,
            community_id,
            question.key,
            question.text_en,
            question.text_uk,
            question.required,
            question.position as i32,
        )
        .execute(pool)
        .await?;

        if result.rows_affected() == 0 {
            sqlx::query!(
                r#"
                INSERT INTO community_questions (community_id, question_key, question_text, question_text_uk, required, position, is_active)
                VALUES ($1, $2, $3, $4, $5, $6, TRUE)
                "#,
                community_id,
                question.key,
                question.text_en,
                question.text_uk,
                question.required,
                question.position as i32,
            )
            .execute(pool)
            .await?;
        }
    }

    Ok(())
}
