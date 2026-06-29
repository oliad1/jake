//General flow
//Scrape main job listings, keep sending channel messages (tx) until we reach a job we've already
//seen

use crate::{Error, Application, ScrapeEvent, City};
use scraper::{Html, Selector};
use tokio::sync::mpsc::Sender;
use std::collections::HashSet;

pub async fn main(
    company_id: i64,
    urls: HashSet<String>,
    tx: Sender<ScrapeEvent>
) -> Result<(), Error> {
    const JOB_LISTING_URL: &str = "https://www.google.com/about/careers/applications/jobs/results/?q&hl=en&target_level=INTERN_AND_APPRENTICE&degree=BACHELORS&employment_type=INTERN&sort_by=date&location=United%20States&location=Canada";
    
    let body = reqwest::get(JOB_LISTING_URL).await?.text().await?;

    let scraped_urls: Vec::<String> = {

        let document = Html::parse_document(&body);

        let jobs_selector = Selector::parse("#yDmH0d > c-wiz.zQTmif.SSPGKf > div > div.RRYLgd > div > div > div > div.BiNgOe.E2Mxid > main > div > c-wiz > div > ul").unwrap();

        let url_selector = Selector::parse("a").unwrap();

        let ul = document.select(&jobs_selector).next().unwrap();

        let mut temp_urls = Vec::new();

        for a in ul.select(&url_selector) {
            let mut base_url: String = "https://www.google.com/about/careers/applications/".to_owned();
            let href = String::from(a.value().attr("href").unwrap().split('?').next().unwrap());
            base_url.push_str(&href);

            if !urls.contains(&base_url) {
                temp_urls.push(base_url);
            }
        }

        temp_urls
    };

    for url in scraped_urls {
        let tx_clone = tx.clone();

        single_job(company_id, tx_clone, url).await?;
    }

    drop(tx);

    Ok(())
}

async fn single_job(
    company_id: i64,
    tx: Sender<ScrapeEvent>,
    url: String
) -> Result<(), Error> {
    let body = reqwest::get(&url).await?.text().await?;
    
    let event: ScrapeEvent = {
        let document = Html::parse_document(&body);

        let card_selector = Selector::parse("#yDmH0d > c-wiz.zQTmif.SSPGKf > div > div.RRYLgd > div > div > div > div.BiNgOe.E2Mxid > main > div > c-wiz > div > div > div > span > div").unwrap();

        let card = document.select(&card_selector).next().unwrap();

        let title_selector = Selector::parse("h2").unwrap();

        let compensation_selector = Selector::parse("div.aG5W3 > p:nth-child(7)").unwrap();

        let location_selector = Selector::parse("div.KwJkGe > div > div > span.MyVLbf > b").unwrap();

        let mut page_content = "".to_owned();

        // format is usually: Job Title, BS/MS/PHD, Term
        let header_html = card.select(&title_selector).next().unwrap().inner_html();
        page_content.push_str(&header_html);
        let mut header = header_html.split(",");

        let title = header.next().unwrap();
        header.next();

        // US: $98000 - $131000 (USD)
        let compensation_html = card.select(&compensation_selector).next().unwrap().inner_html();
        let uncleaned_compensation_text = compensation_html.replace(&['$', '(', ')'][..],"");
        let mut compensation_text = uncleaned_compensation_text.split(" ");

        // Skip the country 'US:'
        compensation_text.next();

        let lower: i16 = (compensation_text.next()
            .unwrap()
            .to_string()
            .parse::<i64>()
            .unwrap() * 100 / (40 * 52)
        ).try_into().unwrap();

        let upper: Option<i16> = if compensation_html.contains("-") {
            // skip the dash
            compensation_text.next();

            Some((compensation_text.next()
                 .unwrap()
                 .to_string()
                 .parse::<i64>()
                 .unwrap() * 100 / (40 * 52)
            ).try_into().unwrap())
        } else {
            None
        };

        //currency
        let currency = compensation_text.next().unwrap().to_string();

        let location_text = card.select(&location_selector).next().unwrap().inner_html();
        let locations: Vec<String> = location_text.split("; ").map(String::from).collect();
    
        //Winter/Summer 2026
        let total_term = header.next().unwrap();
        
        //[Winter/Summer] [2026]
        let mut term = total_term.split(" ");
        
        //skip the empty space at the start
        term.next().unwrap();

        //W/S
        let mut abbr_term = term.next().unwrap()
            .replace("Winter", "W")
            .replace("Summer", "S")
            .replace("Spring", "P")
            .replace("Fall", "F");

        //26
        let year = term.next().unwrap().replace("20", "");

        //W/S26
        abbr_term.push_str(&year);

        //W/F 2026 => W/F 26, W26, F26
        let final_term = abbr_term.replace("/", format!("{}, ", &year).as_str());

        //need to match term like Winter/Summer 2026 => [Winter, Summer, 2026?]
        let mut cities: Vec<City> = Vec::new();
        
        for location_str in locations {
            let mut loc_str = location_str.split(",");
            cities.push(City {
                display_name: loc_str.next().unwrap().trim().to_string(),
                region: loc_str.next().unwrap().trim().to_string(),
                country: loc_str.next().unwrap().trim().to_uppercase().chars().take(2).collect(),
            });
        }

        let application = Application {
            term_id: 1,
            company_id,
            job_title: title.to_string(),
            url: url,
            page_content: "".to_string(),
            lower_wage_cents: lower,
            upper_wage_cents: upper,
            state: "ACTIVE".to_string(),
            currency: currency
        };

        ScrapeEvent {
            cities,
            application
        }
    };

    if let Err(e) = tx.send(event).await {
        eprintln!("Failed to send scrape event over tx {:?}", e);
    }

    Ok(())
}
