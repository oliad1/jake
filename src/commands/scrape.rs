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

    for company in companies {
        let tx_clone = tx.clone();

        match company.display_name.as_str() {
            "Google" => {
                scrapers::google::main(
                    company.id,
                    company_url_map.get(&company.id).unwrap_or(&HashSet::<String>::new()).clone(),
                    tx_clone
                ).await?;
            }
            _ => { panic!("Unknown company"); }
        };
    }

    while let Some(ScrapeEvent { terms, cities, application }) = rx.recv().await {
        //first insert to DB then send on Discord
        let _ = sqlx::query!(r#"
                WITH application_row AS (
                    INSERT INTO applications (
                      company_id,
                      job_title,
                      url,
                      page_content,
                      lower_wage_cents,
                      upper_wage_cents,
                      currency
                    )
                    VALUES (
                      $1,
                      $2,
                      $3,
                      $4,
                      $5,
                      $6,
                      $7
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
                serde_json::to_value(&terms)?
                    )
                    .execute(&ctx.data().pool).await?;

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
        
        if let Some(terms_list) = terms {
            term_text = terms_list.into_iter()
                .map(|t| t.display_name)
                .collect::<Vec<String>>()
                .join(", ");
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
                .title(format!("{} - {}", &term_text, application.job_title)) // need to fix this, add field
                .url(application.url)
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
                    serenity::CreateButton::new("applied")
                        .label("Applied")
                        .style(serenity::ButtonStyle::Secondary),
                    serenity::CreateButton::new("ignored")
                        .label("Ignored")
                        .style(serenity::ButtonStyle::Secondary),
                ])
            ]);

        let reply_handle = ctx.send(response).await?;
        reply_handle.message().await?;
    }

    Ok(())
}
