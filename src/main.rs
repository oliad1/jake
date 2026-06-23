use std::env;

use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    //Handler for the `message` event.
    //Handlers are dispatched through a threadpool allowing for concurrency
    
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.content == "!ping" {
            if let Err(why) = msg.channel_id.say(&ctx.http, "Pong!").await {
                println!("Error sending message: {why:?}");
            }
        }
    }

    //Set a handler for the `ready` event when the bot is booted
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[tokio::main]
async fn main() {
    //Initialize .env
    dotenvy::dotenv().ok();     

    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment.");

    let intents = GatewayIntents::GUILD_MESSAGES 
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    // Create a new instance of the Client, logged in as the bot.
    let mut client = Client::builder(&token, intents).event_handler(Handler).await.expect("Err creating client");

    //Start a single shard and listen to events
    if let Err(why) = client.start().await {
        println!("Client error: {why:?}");
    }
}
