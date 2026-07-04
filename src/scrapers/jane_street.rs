use crate::{Error, Application, ScrapeEvent, City, Term};
use tokio::sync::mpsc::Sender;
use std::collections::HashSet;
use serde::Deserialize;
use reqwest::Client;

#[derive(Deserialize, Debug)]
struct Job {
    id: u64, //8573523002
    position: String, //Quantitative Trader
    category: String, //Trading, Research, and Machine Learning
    availability: String, //Full-Time: New Grad
    city: String, //NYC
    overview: String, //Long job description
    team: String, //Quantitative Trading
    duration: String, //Permanent
    min_salary: Option<String>, //300,000
    max_salary: Option<String>, //300,000
}

pub async fn main(
    company_id: i64,
    urls: HashSet<String>,
    tx: Sender<ScrapeEvent>,
    client: Client
) -> Result<(), Error> {
    const JOB_LISTING_URL: &str = "https://www.janestreet.com/jobs/main.json";

    let body = client.get(JOB_LISTING_URL)
        .send()
        .await?
        .text()
        .await?;

    let jobs: Vec<Job> = serde_json::from_str(&body).unwrap();

    let allowed_cities = vec!["NYC", "CHI", "PHL", "SF", "ATX"];

    for job in jobs {
        let url = format!("{}{}", "https://www.janestreet.com/join-jane-street/position/", job.id);

        if urls.contains(&url) || !job.availability.contains("Internship") || !allowed_cities.contains(&job.city.as_str()) {
            continue;
        }

        let mut terms: Vec<Term> = Vec::new();

        let year = 26;

        let seasons = [("Winter", "W"), ("Summer", "S"), ("Fall", "F"), ("Spring", "P")];

        for (season, prefix) in seasons {
            if job.availability.contains(season) {
                terms.push(Term {
                    display_name: format!("{}{}", prefix, &year)
                });
            }
        }

        let city: City = match job.city.as_str() {
            "NYC" => City { display_name: "New York".to_string(), region: "NY".to_string(), country: "US".to_string() },
            "CHI" => City { display_name: "Chicago".to_string(), region: "IL".to_string(), country: "US".to_string() },
            "PHL" => City { display_name: "Philadelphia".to_string(), region: "PA".to_string(), country: "US".to_string() },
            "SF" => City { display_name: "San Francisco".to_string(), region: "CA".to_string(), country: "US".to_string() },
            "ATX" => City { display_name: "Austin".to_string(), region: "TX".to_string(), country: "US".to_string() },
            _ => { panic!("This job should've been filtered by its location. Id: {} City: {}", job.id, job.city) }
        };

        let lower: i16 = (job.min_salary
            .unwrap_or("0".to_string())
            .replace(",", "")
            .parse::<i64>()
            .unwrap() * 100 / (40 * 52)
        ).try_into().unwrap();

        let mut upper: Option<i16> = Some((job.max_salary
            .unwrap_or("0".to_string())
            .replace(",", "")
            .to_string()
            .parse::<i64>()
            .unwrap() * 100 / (40 * 52)
        ).try_into().unwrap());

        if upper.is_some() && lower == upper.unwrap() {
            upper = None;
        }

        let application = Application {
            company_id,
            job_title: job.position,
            url,
            page_content: job.overview,
            lower_wage_cents: lower,
            upper_wage_cents: upper,
            state: "ACTIVE".to_string(),
            currency: "USD".to_string(),
            thread_id: None
        };

        let event = ScrapeEvent {
            terms: Some(terms),
            cities: vec![city],
            application
        };


        if let Err(e) = tx.send(event).await {
            eprintln!("Failed to send scrape event over tx {:?}", e);
        }
    }

    drop(tx);

    Ok(())
}
