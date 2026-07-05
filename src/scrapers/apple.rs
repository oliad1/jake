use crate::{Error, Application, ScrapeEvent, City, Term};
use tokio::sync::mpsc::Sender;
use std::collections::{HashSet};
use serde::Deserialize;
use serde_json::json;
use reqwest::Client;

#[derive(Debug, Deserialize)]
#[serde(rename_all="camelCase")]
struct Location {
    post_location_id: String, //postLocation-USA
    city: String,
    state_province: String,
    country_name: String, //United States of America
    metro: String,
    region: String,
    name: String, //United States
    countryID: String, //iso-country-USA
    level: i16, //1
}

#[derive(Debug, Deserialize)]
#[serde(rename_all="camelCase")]
struct SearchResult {
    id: String, //200664221-3810
    job_summary: String,
    locations: Vec<Location>,
    position_id: String, //200664221,
    posting_date: String, //May 22, 2026,
    posting_title: String, //Machine Learning and Artificial Intelligence Masters Internships,
    managed_pipeline_role: bool,
    home_office: bool,
    job_position_id: String, //PIPE-200664221,
    is_multi_location: bool,
    post_external: bool
}

#[derive(Debug, Deserialize)]
#[serde(rename_all="camelCase")]
struct AppleRes {
    search_results: Vec<SearchResult>
}

#[derive(Debug, Deserialize)]
struct AppleApiRes {
    res: AppleRes
}

fn get_req(
)->serde_json::Value {
    json!({
        "query": "",
        "filters": {
            "locations": [ "postLocation-CANC", "postLocation-USA" ],
            "teams": [ { "team": "teamsAndSubTeams-STDNT", "subTeam": "subTeam-INTRN" } ]
        },
        "page": 1,
        "locale": "en-ca",
        "sort": "",
        "format": {
            "longDate": "MMMM D, YYYY",
            "mediumDate": "MMM D, YYYY"
        }
    })
}

pub async fn main(
    company_id: i64,
    urls: HashSet<String>,
    tx: Sender<ScrapeEvent>,
    client: Client
) -> Result<(), Error> {
    let map = get_req();

    let res = client.post("https://jobs.apple.com/api/v1/search")
        .json(&map)
        .send()
        .await?
        .json::<AppleApiRes>()
        .await?;

    println!("BODY: {:?}", res);

    //Hard-coded values (cities, compensation) come from this source: https://simplify.jobs/blog/apple-internship-faq
    
    //hit the GET request
    //TODO: full page_content, parsing for locations, parsing for salary and wage text

    for job in res.res.search_results {
        let url = format!("{}{}", "https://jobs.apple.com/en-ca/details/", job.id);

        if urls.contains(&url) || job.posting_title.contains("Masters") || job.posting_title.contains("PhD") {
            continue;
        }

        let year = 26;

        let mut terms: Vec<Term> = Vec::new();
        
        let seasons = [("Winter", "W"), ("Summer", "S"), ("Fall", "F"), ("Spring", "P")];

        for (season, prefix) in seasons {
            if job.posting_title.contains(season) {
                terms.push(Term {
                    display_name: format!("{}{}", prefix, &year)
                });
            }
        }
        
        let cities: Vec<City> = vec![
            City { display_name: "Cupertino".to_string(), region: "California".to_string(), country: "US".to_string(), },
            City { display_name: "Boston".to_string(), region: "Massachusetts".to_string(), country: "US".to_string(), },
            City { display_name: "Austin".to_string(), region: "Texas".to_string(), country: "US".to_string(), },
            City { display_name: "Seattle".to_string(), region: "Washington".to_string(), country: "US".to_string(), },
            City { display_name: "San Diego".to_string(), region: "California".to_string(), country: "US".to_string(), },
            City { display_name: "San Francisco".to_string(), region: "California".to_string(), country: "US".to_string(), },
            City { display_name: "Washington".to_string(), region: "DC".to_string(), country: "US".to_string(), }
        ];

        let application = Application {
            company_id,
            job_title: job.posting_title,
            url: url,
            page_content: job.job_summary,
            lower_wage_cents: 4400,
            upper_wage_cents: None,
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
    }
    
    drop(tx);

    Ok(())
}
