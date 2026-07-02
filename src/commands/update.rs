use crate::{Context, Error};
use poise::serenity_prelude as serenity;
use poise::reply::CreateReply;
use futures::{Stream, StreamExt};
use serenity::Timestamp;
use serenity::ChannelType;
use sqlx::{Pool, Postgres};

async fn autocomplete_state<'a>(
    _ctx: Context<'_>,
    partial: &'a str,
) -> impl Stream<Item = String> + 'a {
    futures::stream::iter(&["ACTIVE", "SUBMITTED", "REJECTED", "DELETED", "IGNORED"])
        .filter(move |state| futures::future::ready(state.to_lowercase().starts_with(partial)))
        .map(|state| state.to_string())
}

pub async fn update_application_state(
    db_pool: &Pool<Postgres>,
    thread_id: u64,
    after_state: &str 
) -> Result<Option<String>, Error> {
    Ok(sqlx::query!(r#"
        WITH new_row AS (
            UPDATE applications
            SET state = $2
            WHERE thread_id = $1
            RETURNING id
        )

        INSERT INTO application_events (
            application_id,
            before_state,
            after_state
        )
        SELECT
            nr.id,
            (
                SELECT after_state
                FROM application_events AS ae
                WHERE ae.application_id = nr.id
                ORDER BY id DESC
                LIMIT 1
            ),
            $2
        FROM new_row AS nr
        RETURNING before_state;
        "#,
        thread_id as i64,
        after_state
    )
    .fetch_one(db_pool)
    .await?
    .before_state)
}

#[poise::command(slash_command)]
pub async fn update(
    ctx: Context<'_>,
    #[description = "Application's new state"] #[autocomplete = "autocomplete_state"] after_state: String
) -> Result<(), Error> {
    ctx.defer().await?;

    let Some(guild_c) = ctx.guild_channel().await else {
        ctx.say("Guild channel is None.").await?;
        return Ok(())
    };

    if guild_c.kind != ChannelType::PublicThread {
        ctx.say("Invalid use of /update command. Use this command in a public job thread.").await?;

        return Ok(())
    }

    let parent_id = guild_c.id.get();
    
    let Some(before_state) = update_application_state(&ctx.data().pool,
        parent_id,
        &after_state.as_str()
    ).await? else {
        ctx.say("No before_state found for this application.").await?;

        return Ok(())
    };

    let timestamp = Timestamp::now();

    //Might want to make this embed with a timestamp
    let response = CreateReply::default()
        .embed(
            serenity::CreateEmbed::new()
            .title("Application Updated")
            .description(
                format!("Application status changed from `{}` to `{}`.",
                    before_state,
                    after_state
                )
            )
            .timestamp(&timestamp)
        );

    let reply_handle = ctx.send(response).await?;
    reply_handle.message().await?;

    Ok(())
}
