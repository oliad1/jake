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
struct StripeRes {
    html: Option<String>
}

pub async fn main(
    company_id: i64,
    urls: HashSet<String>,
    tx: Sender<ScrapeEvent>,
    client: Client
) -> Result<(), Error> {
    const JOB_LISTING_URL: &str = "https://stripe.com/jobs/search?tags=University&view_type=list";

    let res = client.get(JOB_LISTING_URL)
        .query(&[
            ("office_locations", "North+America--Atlanta"),
            ("office_locations", "North+America--Chicago"),
            ("office_locations", "North+America--New+York"),
            ("office_locations", "North+America--San+Francisco+Bridge+HQ"),
            ("office_locations", "North+America--Seattle"),
            ("office_locations", "North+America--South+San+Francisco"),
            ("office_locations", "North+America--Toronto"),
            ("office_locations", "North+America--Washington+DC"),
            ("tags", "University"),
            ("view_type", "list")
        ])
        .send()
        .await
        .expect("Network request failed")
        .json::<StripeRes>()
        .await
        .expect("Parsing failed");

    let scraped_urls: Vec::<String> = {
        let mut temp_urls = Vec::new();

        let html = res.html.unwrap();

        //let body = decode_html_entities(&html);

        let document = Html::parse_document(&html);

        let url_selector = Selector::parse("a").unwrap();

        for a in document.select(&url_selector) {
            let href = String::from(a.value().attr("href").unwrap());
            let url = format!("{}{}", "https://stripe.com", href);

            if !urls.contains(&url) {
                temp_urls.push(url);
            }
        }

        temp_urls
    };
    
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
    let body = reqwest::get(&url).await?.text().await?;
    
    let event: ScrapeEvent = {
        let document = Html::parse_document(&body);

        let card_selector = Selector::parse("#MktContent > section.Section.JobsBodySection.Section--paddingNormal.Section--hasGuides > div > div.Section__container > div > div > div > div.RowLayout").unwrap();

        let card = document.select(&card_selector).next().unwrap();

        let title_selector = Selector::parse("#MktContent > section.Section.JobsDetail__heroSection.Section--paddingNormal.Section--paddingBottomNone.Section--hasGuides > div > div.Section__container > div > div > section > header > h1").unwrap();

        let compensation_selector = Selector::parse("section:nth-child(3) > div > p:nth-child(1)").unwrap();

        let locations_selector = Selector::parse("#MktContent > section.Section.JobsBodySection.Section--paddingNormal.Section--hasGuides > div > div.Section__container > div > div > div > div.Card.Card--border.JobsDetailCard > div:nth-child(1) > div > p:nth-child(2)").unwrap();

        let title = document.select(&title_selector).next().unwrap().inner_html();
        let page_content = card.inner_html();

        let compensation_html = card.select(&compensation_selector).next().unwrap().inner_html();
        
        let compensation_sentence = compensation_html.split('.').next().unwrap();

        let split_comp = compensation_sentence.split(' ');

        let mut lower: Option<i16> = None;
        let mut upper: Option<i16> = None;

        for word in split_comp {
            if word.contains('$') {
                let cleaned_word = word.replace(&['$', ',', '.'][..], "").replace("CA", "").replace("US", "");

                if lower.is_none() {
                    lower = Some((cleaned_word.parse::<i64>().unwrap() * 100 / (40 * 52)).try_into().unwrap());
                } else {
                    upper = Some((cleaned_word.parse::<i64>().unwrap() * 100 / (40 * 52)).try_into().unwrap());
                }
            }
        }

        //currency
        let currency = if compensation_sentence.contains("US") { "USD".to_string() } else { "CAD".to_string() };
        
        let year = if title.contains("2027") { 27 } else { 26 };

        let mut terms: Vec<Term> = Vec::new();
        
        let seasons = [("Winter", "W"), ("Summer", "S"), ("Fall", "F"), ("Spring", "P")];

        for (season, prefix) in seasons {
            if title.contains(season) {
                terms.push(Term {
                    display_name: format!("{}{}", prefix, &year)
                });
            }
        }

        //need to match term like Winter/Summer 2026 => [Winter, Summer, 2026?]
        let mut cities: Vec<City> = Vec::new();
    
        //they have an oxford-comma-esque " or": "Toronto, New York, or Dublin"
        let clean_city_text = document.select(&locations_selector).next().unwrap().inner_html().replace(" or", "");
        let city_names = clean_city_text.split(',');
        
        for name in city_names {
            match name.trim() {
                "Atlanta" => { cities.push(City { display_name: "Atlanta".to_string(), region: "GA".to_string(), country: "US".to_string() }) }
                "Chicago" => { cities.push(City { display_name: "Chicago".to_string(), region: "IL".to_string(), country: "US".to_string() }) }
                "New York" => { cities.push(City { display_name: "New York".to_string(), region: "NY".to_string(), country: "US".to_string() }) }
                "Seattle" => { cities.push(City { display_name: "Seattle".to_string(), region: "WA".to_string(), country: "US".to_string() }) }
                "San Francisco Bridge HQ" | "South San Francisco HQ" => { cities.push(City { display_name: "San Francisco".to_string(), region: "CA".to_string(), country: "US".to_string() }) }
                "Toronto" => { cities.push(City { display_name: "Toronto".to_string(), region: "ON".to_string(), country: "CA".to_string() }) }
                "Washington DC" => { cities.push(City { display_name: "Washington D.C.".to_string(), region: "DC".to_string(), country: "US".to_string() }) }
                _ => { panic!("This city should've been filtered out by the url params. City: {}", name); }
            }
        }

        let application = Application {
            company_id,
            job_title: decode_html_entities(title.trim()).to_string(),
            url,
            page_content,
            lower_wage_cents: lower.unwrap(),
            upper_wage_cents: upper,
            state: "ACTIVE".to_string(),
            currency: currency,
            thread_id: None
        };

        ScrapeEvent {
            terms: Some(terms),
            cities,
            application
        }
    };

    if let Err(e) = tx.send(event).await {
        eprintln!("Failed to send scrape event over tx {:?}", e);
    }

    Ok(())
}
