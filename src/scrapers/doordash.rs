use crate::{Error, Application, ScrapeEvent, City, Term};
use scraper::{Html, Selector};
use tokio::sync::mpsc::Sender;
use std::collections::HashSet;
use futures::future::join_all;
use serde::Deserialize;
use serde_json::json;
use reqwest::Client;
use html_escape::decode_html_entities;

#[derive(Debug, Deserialize)]
struct Location {
    name: String
}

#[derive(Debug, Deserialize)]
struct Metadata {
    id: u32,
    name: String,
    //value: Option<>, Could be map or string
    value_type: String,
}

#[derive(Debug, Deserialize)]
struct PayInputRange {
    min_cents: u32,
    max_cents: u32,
    title: String,
}

#[derive(Debug, Deserialize)]
struct Job {
    absolute_url: String,
    education: Option<String>,
    internal_job_id: u32,
    location: Location,
    metadata: Vec<Metadata>,
    id: u32,
    updated_at: String,
    title: String,
    content: Option<String>,
    pay_input_ranges: Option<Vec<PayInputRange>>,
}

#[derive(Debug, Deserialize)]
struct GreenHouseRes {
    jobs: Vec<Job>
}

pub async fn main(
    company_id: i64,
    urls: HashSet<String>,
    tx: Sender<ScrapeEvent>,
    client: Client
) -> Result<(), Error> {
    const JOB_LISTING_URL: &str = "https://boards-api.greenhouse.io/v1/boards/doordashusa/jobs";

    let res = client.get(JOB_LISTING_URL)
        .send()
        .await
        .expect("Network request failed")
        .json::<GreenHouseRes>()
        .await
        .expect("Parsing failed");

    let mut scraped_urls: Vec::<String> = Vec::new();

    for job in res.jobs {
        if job.title.contains("Internship") || job.title.contains("Fellowship") {
            scraped_urls.push(format!("{}/{}", JOB_LISTING_URL, job.id));
        }
    }

    println!("scraped urls: {:?}", scraped_urls);
    
    let tasks = scraped_urls.into_iter().map(|url| {
        let tx_clone = tx.clone();
        let client_clone = client.clone();

        single_job(company_id, url, tx_clone, client_clone)
    });

    let _ = join_all(tasks).await;

    drop(tx);

    Ok(())
}

async fn single_job(
    company_id: i64,
    url: String,
    tx: Sender<ScrapeEvent>,
    client: Client
) -> Result<(), Error> {
    let job = client.get(url)
        .query(&[("pay_transparency", "true")])
        .send()
        .await
        .expect("Network request failed")
        .json::<Job>()
        .await
        .expect("Parsing failed");

    let title = job.title;
    let cleaned_title = title
        .replace("Summer", "")
        .replace("Winter", "")
        .replace("Fall", "")
        .replace("Spring", "")
        .replace("2026", "")
        .replace("2027", "")
        .replace(" and ", "")
        .replace("( )", "")
        .replace(",", "");
    
    let mut lowest: Option<i16> = None;
    let mut highest: Option<i16> = None;

    for range in job.pay_input_ranges.unwrap() {
        let total_hours = if range.title.contains("Month") { 40 * 4 } else { 40 * 52 };

        if lowest.is_none() || range.min_cents < lowest.unwrap().try_into().unwrap() {
            lowest = Some((range.min_cents / total_hours).try_into().unwrap());
        }

        if highest.is_none() || range.max_cents < highest.unwrap().try_into().unwrap() {
            highest = Some((range.max_cents / total_hours).try_into().unwrap());
        }
    }

    let cities: Vec<City> = job.location.name.split(";").map(|i| {
        let mut loc_parts = i.split(",");
        City {
            display_name: loc_parts.next().unwrap().trim().to_string(),
            region: loc_parts.next().unwrap().trim().to_string(),
            country: "US".to_string(),
        }
    }).collect();

    let mut terms: Vec<Term> = Vec::new();

    let year = if title.contains("2027") { 27 } else { 26 };

    let seasons = [("Winter", "W"), ("Summer", "S"), ("Fall", "F"), ("Spring", "P")];

    for (season, prefix) in seasons {
        if title.contains(season) {
            terms.push(Term {
                display_name: format!("{}{}", prefix, &year)
            });
        }
    }

    let page_content = decode_html_entities(&job.content.unwrap())
        .split("</div>")
        .nth(1)
        .unwrap()
        .to_string();

    let application = Application {
        company_id,
        job_title: cleaned_title.trim().to_string(),
        url: job.absolute_url,
        page_content,
        lower_wage_cents: lowest.unwrap(),
        upper_wage_cents: highest,
        state: "ACTIVE".to_string(),
        currency: "USD".to_string(),
        thread_id: None
    };

    let event = ScrapeEvent {
        terms: Some(terms),
        cities,
        application
    };

    if let Err(e) = tx.send(event).await {
        eprintln!("Failed to send scrape event over tx {:?}", e);
    }

    Ok(())
}
