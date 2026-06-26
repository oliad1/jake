//General flow
//Scrape main job listings, keep sending channel messages (tx) until we reach a job we've already
//seen

use crate::{Context, Error};
use scraper::{Html, Selector};
use poise::serenity_prelude as serenity;
use poise::reply::CreateReply;

pub async fn main(
    ctx: Context<'_>
) -> Result<(), Error> {

    const JOB_LISTING_URL: &str = "https://www.google.com/about/careers/applications/jobs/results/?q&hl=en&target_level=INTERN_AND_APPRENTICE&degree=BACHELORS&employment_type=INTERN&sort_by=date&location=United%20States&location=Canada";
    
    let body = reqwest::get(JOB_LISTING_URL).await?.text().await?;

    let urls: Vec<String> = {
        let document = Html::parse_document(&body);

        let jobs_selector = Selector::parse("#yDmH0d > c-wiz.zQTmif.SSPGKf > div > div.RRYLgd > div > div > div > div.BiNgOe.E2Mxid > main > div > c-wiz > div > ul").unwrap();

        let url_selector = Selector::parse("a").unwrap();

        let ul = document.select(&jobs_selector).next().unwrap();

        let mut temp_urls = Vec::new();
        
        for a in ul.select(&url_selector) {
            let mut base_url: String = "https://www.google.com/about/careers/applications/".to_owned();
            let href = String::from(a.value().attr("href").unwrap().split('?').next().unwrap());
            base_url.push_str(&href);
            temp_urls.push(base_url);
        }
        
        temp_urls
    };

    for url in urls {
        single_job(ctx, url).await?;
    }

    Ok(())
}

async fn single_job(
    ctx: Context<'_>,
    url: String
) -> Result<(), Error> {
    let body = reqwest::get(&url).await?.text().await?;
    
    let (term, desc) = {
        let document = Html::parse_document(&body);

        let card_selector = Selector::parse("#yDmH0d > c-wiz.zQTmif.SSPGKf > div > div.RRYLgd > div > div > div > div.BiNgOe.E2Mxid > main > div > c-wiz > div > div > div > span > div").unwrap();

        let card = document.select(&card_selector).next().unwrap();

        let title_selector = Selector::parse("h2").unwrap();
    
        let compensation_selector = Selector::parse("div.aG5W3 > p:nth-child(7)").unwrap();

        let location_selector = Selector::parse("div.op1BBf > span.pwO9Dc.vo5qdf > span:nth-child(2)").unwrap();

        // format is usually: Job Title, BS/MS/PHD, Term
        let header_html = card.select(&title_selector).next().unwrap().inner_html();
        let mut header = header_html.split(",");

        let title = header.next().unwrap();
        header.next();

        // US: $98000 - $131000 (USD)
        let compensation_html = card.select(&compensation_selector).next().unwrap().inner_html();
        let mut compensation_text = compensation_html.split(" ");
        
        compensation_text.next();
        
        let lower = compensation_text.next().unwrap().to_string().chars().skip(1).collect::<String>().parse::<i32>().unwrap() / (40 * 52);

        // avoid the dash
        // compensation_text.next();

        //let upper = compensation_text.next().unwrap() {

        let location = card.select(&location_selector).next().unwrap().inner_html();

        (format!("{} - {}", header.next().unwrap(), title), format!("${}/hr • {}", lower, location))
    };

    let response = CreateReply::default()
        .embed(
            serenity::CreateEmbed::new()
            .title(term)
            .url(url)
            .author(
                serenity::CreateEmbedAuthor::new("Google")
                    .url("https://about.google/")
                    .icon_url("https://www.google.com/favicon.ico")
            )
            .colour(serenity::Colour::from(0xFFFFFF))
            .description(desc)
        ).components(vec![
            serenity::CreateActionRow::Buttons(
                vec![
                    serenity::CreateButton::new("applied")
                        .label("Applied")
                        .style(
                            serenity::ButtonStyle::Secondary
                        ),
                    serenity::CreateButton::new("ignored")
                        .label("Ignored")
                        .style(
                            serenity::ButtonStyle::Secondary
                        ),
                ]
            )
        ]
    );
    
    let reply_handle = ctx.send(response).await?;
    let message = reply_handle.message().await?;

    let title = format!("{} @ Google",message.embeds.first().unwrap().title.clone().unwrap());

    while let Some(press) = message.await_component_interaction(&ctx)
        .author_id(ctx.author().id)
        .next()
        .await
    {
        let action = press.data.custom_id.as_str();

        match action {
            "applied" | "ignored"  => {
                press.create_response(
                    &ctx.serenity_context().http,
                    serenity::CreateInteractionResponse::UpdateMessage(
                        serenity::CreateInteractionResponseMessage::new().components(vec![])
                    )
                ).await?;

                let thread_builder = serenity::CreateThread::new(&title)
                    .kind(serenity::ChannelType::PublicThread);

                let thread_channel = ctx.channel_id()
                    .create_thread_from_message(ctx.serenity_context(), message.id, thread_builder.to_owned())
                    .await?;

                thread_channel.say(ctx.serenity_context(), action).await?;
            }
            _ => println!("Unknown interaction type")
        };

    }

    Ok(())
}
