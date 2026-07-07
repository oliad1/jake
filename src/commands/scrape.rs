use crate::{Context, Error, Company, ScrapeEvent};
use crate::scrapers;
use std::{collections::{HashMap, HashSet}};
use tokio::sync::mpsc;
use poise::serenity_prelude as serenity;
use poise::reply::CreateReply;

#[poise::command(slash_command)]
pub async fn scrape(
    ctx: Context<'_>
) -> Result<(), Error> {
    //Send an immediate 'pending' response so Discord doesn't timeout
    ctx.defer().await?; 

    let companies: Vec<Company> = sqlx::query_as(r#"
            SELECT
                id,
                display_name,
                url,
                hex_code,
                icon_url
            FROM companies
            WHERE state = 'ACTIVE'
        "#)
        .fetch_all(&ctx.data().pool)
        .await?;

    let company_id_map: HashMap<i64, Company> = companies.iter().map(|c| (c.id, c.clone())).collect();

    let application_rows: Vec<(i64, Vec<String>)> = sqlx::query_as(r#"
            SELECT
                c.id,
                COALESCE(ARRAY_AGG(a.url), ARRAY[]::text[]) AS urls
            FROM applications AS a
            JOIN companies AS c
            ON c.id = a.company_id
            GROUP BY c.id
        "#)
        .fetch_all(&ctx.data().pool)
        .await?;

    let company_url_map: HashMap<i64, HashSet<String>> = application_rows.into_iter()
        .map(|(id, urls)| (id, HashSet::from_iter(urls.into_iter())))
        .collect();

    let (tx, mut rx) = mpsc::channel::<ScrapeEvent>(10);

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36")
        .build()?;

    for company in companies {
        let tx_clone = tx.clone();
        let client_clone = client.clone();
        let company_url_map_clone = company_url_map.get(&company.id).unwrap_or(&HashSet::<String>::new()).clone();
        
        tokio::spawn(async move {
            match company.display_name.as_str() {
                "Google" => {
                    scrapers::google::main(
                        company.id,
                        company_url_map_clone,
                        tx_clone
                    ).await
                }
                "Amazon" => {
                    scrapers::amazon::main(
                        company.id,
                        company_url_map_clone,
                        tx_clone,
                        client_clone
                    ).await
                }
                "Jane Street" => {
                    scrapers::jane_street::main(
                        company.id,
                        company_url_map_clone,
                        tx_clone,
                        client_clone
                    ).await
                }
                "DoorDash" => {
                    scrapers::doordash::main(
                        company.id,
                        company_url_map_clone,
                        tx_clone,
                        client_clone
                    ).await
                }
                "Stripe" => {
                    scrapers::stripe::main(
                        company.id,
                        company_url_map_clone,
                        tx_clone,
                        client_clone
                    ).await
                }
                _ => { panic!("Unknown company"); }
            }
        });
    }

    drop(tx);

    let mut processed_anything = false;

    while let Some(ScrapeEvent { terms, cities, application }) = rx.recv().await {
        processed_anything = true;

        let company = company_id_map.get(&application.company_id).unwrap().to_owned();

        let mut cities_text = format!("{}, {}, {}",
            cities[0].display_name,
            cities[0].region,
            cities[0].country,
        );

        if cities.len() > 1 {
            cities_text.push_str(format!(" (+{} more)", cities.len() - 1).as_str());
        }

        let mut term_text = String::new();
        
        if let Some(terms_list) = &terms {
            term_text = terms_list.into_iter()
                .map(|t| t.display_name.clone())
                .collect::<Vec<String>>()
                .join(", ");

            if terms_list.len() > 0 {
                term_text = format!("{} - ", term_text);
            }
        };

        let description = if application.upper_wage_cents.is_some() {
            format!("${}-${}/hr ({})",
                (application.lower_wage_cents / 100),
                (application.upper_wage_cents.unwrap() / 100),
                application.currency
            )
        } else {
            format!("${}/hr ({})", 
                (application.lower_wage_cents / 100),
                application.currency
            )
        };

        let response = CreateReply::default()
            .embed(
                serenity::CreateEmbed::new()
                .title(format!("{}{}", &term_text, application.job_title))
                .url(&application.url)
                .author(
                    serenity::CreateEmbedAuthor::new(&company.display_name)
                    .url(company.url)
                    .icon_url(company.icon_url)
                )
                .colour(serenity::Colour::from(u32::from_str_radix(&company.hex_code.replace("#", ""), 16).unwrap_or(0)))
                .field("Compensation", &description, true)
                .field("Location(s)", &cities_text, true)
                .description(format!("{}...", &application.page_content.chars().take(214).collect::<String>()))
            ).components(vec![
                serenity::CreateActionRow::Buttons(vec![
                    serenity::CreateButton::new("ACTIVE")
                        .label("Apply")
                        .style(serenity::ButtonStyle::Secondary),
                    serenity::CreateButton::new("IGNORED")
                        .label("Ignore")
                        .style(serenity::ButtonStyle::Secondary),
                ])
            ]);

        let reply_handle = ctx.send(response).await?;
        let message_id = reply_handle.message().await?.id.get();

        let _ = sqlx::query!(r#"
            WITH application_row AS (
                INSERT INTO applications (
                  company_id,
                  job_title,
                  url,
                  page_content,
                  lower_wage_cents,
                  upper_wage_cents,
                  currency,
                  thread_id
                )
                VALUES (
                  $1,
                  $2,
                  $3,
                  $4,
                  $5,
                  $6,
                  $7,
                  $10
                )
                RETURNING id
            ),

            -- Inserting cities

            input_cities AS (
                SELECT display_name, region, country
                FROM jsonb_to_recordset($8::jsonb) AS t(display_name text, region text, country text)
            ),

            new_cities AS (
                INSERT INTO cities (
                    display_name,
                    region,
                    country
                )
                SELECT display_name, region, country
                FROM input_cities AS ic
                ON CONFLICT (display_name, region, country) DO UPDATE
                    SET display_name = EXCLUDED.display_name
                RETURNING id
            ),

            new_application_cities AS (
                INSERT INTO application_cities (
                    application_id,
                    city_id
                )
                SELECT
                    ar.id,
                    nc.id
                FROM application_row AS ar
                CROSS JOIN new_cities AS nc
            ),

            -- Inserting terms
            input_terms AS (
                SELECT display_name
                FROM jsonb_to_recordset($9::jsonb) AS t(display_name text)
            ),

            new_terms AS (
                INSERT INTO terms (display_name)
                SELECT display_name
                FROM input_terms AS it
                ON CONFLICT (display_name) DO UPDATE
                    SET display_name = EXCLUDED.display_name
                RETURNING id
            ),

            new_application_terms AS (
                INSERT INTO application_terms (
                    application_id,
                    term_id
                )
                SELECT
                    ar.id,
                    nt.id
                FROM application_row AS ar
                CROSS JOIN new_terms AS nt
            )
            
            INSERT INTO application_events (
              application_id,
              after_state
            )
            SELECT
                ar.id,
                'ACTIVE'
            FROM application_row AS ar
                "#,
                application.company_id,
                application.job_title,
                application.url,
                application.page_content,
                application.lower_wage_cents,
                application.upper_wage_cents,
                application.currency,
                serde_json::to_value(&cities)?,
                serde_json::to_value(&terms)?,
                message_id as i64
            )
            .execute(&ctx.data().pool).await?;
    }

    if !processed_anything {
        ctx.say("No new jobs were found.").await?;
    }

    Ok(())
}
