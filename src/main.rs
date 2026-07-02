
mod commands;
mod scrapers;

use poise::serenity_prelude as serenity;
use std::{env::var};

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct Company {
    id: i64,
    display_name: String,
    url: String,
    hex_code: String,
    icon_url: String,
}

#[derive(sqlx::FromRow, Debug, Clone, serde::Serialize)]
pub struct City {
  display_name: String,
  region: String,
  country: String,
}

#[derive(sqlx::FromRow, Debug, Clone, serde::Serialize)]
pub struct Term {
  display_name: String,
}

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct Application {
    company_id: i64,
    job_title: String,
    url: String,
    page_content: String,
    lower_wage_cents: i16,
    upper_wage_cents: Option<i16>, // There could be no range
    state: String, // ACTIVE, SUBMITTED, REJECTED, DELETED, IGNORED
    currency: String, 
    thread_id: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct ScrapeEvent {
    terms: Option<Vec<Term>>,
    cities: Vec<City>,
    application: Application,
}

pub struct Data {
    pool: sqlx::Pool<sqlx::Postgres>
}

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

async fn on_error(error: poise::FrameworkError<'_, Data, Error>) {
    //Custom error handler
    match error {
        poise::FrameworkError::Setup { error, .. } => panic!("Failed to start bot: {:?}", error),
        poise::FrameworkError::Command { error, ctx, .. } => {
            println!("Error in command: `{}`: {:?}", ctx.command().name, error,);
        }
        error => {
            if let Err(e) = poise::builtins::on_error(error).await {
                println!("Error while handling error: {}", e)
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    //Initialize .env
    dotenvy::dotenv().ok();

    //Set up DB conn
    let conn_str = var("DATABASE_URL").expect("Missing `DATABASE_URL` env var, see README for more information");
    let pool = sqlx::PgPool::connect(&conn_str).await?;

    let options = poise::FrameworkOptions {
        commands: vec![commands::scrape::scrape(), commands::update::update()],
        on_error: |error| Box::pin(on_error(error)),
        pre_command: |ctx| {
            Box::pin(async move {
                println!("Executing command {}...", ctx.command().qualified_name);
            })
        },
        post_command: |ctx| {
            Box::pin(async move {
                println!("Executed command {}!", ctx.command().qualified_name);
            })
        },
        event_handler: |ctx, event, _framework, shared_data| {
            Box::pin(async move {
                println!(
                    "Got an event in event handler: {:?}",
                    event.snake_case_name()
                );

                match event {
                    serenity::FullEvent::InteractionCreate { interaction: serenity::Interaction::Component(data) } => {
                        let action = data.data.custom_id.as_str();
                        let embed = data.message.embeds.first().unwrap();
                        let company_name = embed.author.clone().unwrap().name;
                        let title = format!("{} @ {}", embed.title.clone().unwrap(), company_name);
                        let message_id = data.message.id;

                        match action {
                            "ACTIVE" | "IGNORED"  => {
                                let _ = data.create_response(
                                    ctx,
                                    serenity::CreateInteractionResponse::UpdateMessage(
                                        serenity::CreateInteractionResponseMessage::new().components(vec![])
                                    )
                                ).await;

                                if action == "ACTIVE" {
                                    let thread_builder = serenity::CreateThread::new(&title)
                                        .kind(serenity::ChannelType::PublicThread);

                                    let thread_channel = data.channel.clone().unwrap().id
                                        .create_thread_from_message(
                                            ctx, 
                                            message_id,
                                            thread_builder.to_owned()
                                        )
                                        .await;

                                    if let Ok(thread_c) = thread_channel {
                                        let _ = thread_c.say(ctx, "Starting application... Good luck!").await;
                                    }
                                } else {
                                    let _ = commands::update::update_application_state(&shared_data.pool, data.message.id.get(), "IGNORED").await;
                                }
                            }
                            _ => println!("Unknown interaction type: `{:?}`", action)
                        }
                    }

                    _ => {}
                }

                Ok(())
            })
        },
        ..Default::default()
    };

    let framework = poise::Framework::builder()
        .setup(move |ctx, ready, framework| {
            Box::pin(async move {
                println!("Logged in as {}", ready.user.name);
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {
                    pool
                })
            })
        })
        .options(options)
        .build();

    let token = var("DISCORD_TOKEN").expect("Missing `DISCORD_TOKEN` env var, see README for more information.");
    let intents = serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT;

    let client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await;

    client.unwrap().start().await.unwrap();

    Ok(())
}
