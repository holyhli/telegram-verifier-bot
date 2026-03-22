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
    let active_keys: Vec<String> = community.questions.iter().map(|q| q.key.clone()).collect();

    for question in &community.questions {
        sqlx::query!(
            r#"
            INSERT INTO community_questions (community_id, question_key, question_text, question_text_uk, required, position, is_active)
            VALUES ($1, $2, $3, $4, $5, $6, TRUE)
            ON CONFLICT (community_id, question_key) WHERE is_active = TRUE
            DO UPDATE
                SET question_text = EXCLUDED.question_text,
                    question_text_uk = EXCLUDED.question_text_uk,
                    required = EXCLUDED.required,
                    position = EXCLUDED.position,
                    is_active = TRUE,
                    updated_at = NOW()
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

    deactivate_removed_questions(pool, community_id, &active_keys).await?;

    Ok(())
}

async fn deactivate_removed_questions(
    pool: &PgPool,
    community_id: i64,
    active_keys: &[String],
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        UPDATE community_questions
        SET is_active = FALSE, updated_at = NOW()
        WHERE community_id = $1
          AND is_active = TRUE
          AND question_key != ALL($2)
        "#,
        community_id,
        active_keys,
    )
    .execute(pool)
    .await?;

    Ok(())
}
