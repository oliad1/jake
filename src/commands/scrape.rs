use crate::{Context, Error};
use crate::scrapers;

#[poise::command(slash_command)]
pub async fn scrape(
    ctx: Context<'_>,
    #[description = "Scrape company job boards"]
    company: String
) -> Result<(), Error> {
    _ = match company.as_str() {
        "google" => {
            scrapers::google::main(ctx).await?; //?
        }
        _ => { ctx.say("Unknown job").await?; }
    };
    Ok(())
}
