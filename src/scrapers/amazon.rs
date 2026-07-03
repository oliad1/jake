use crate::{Error, Application, ScrapeEvent, City, Term};
use tokio::sync::mpsc::Sender;
use std::collections::{HashSet};
use serde::Deserialize;
use serde_json::json;
use reqwest::Client;

#[derive(Debug, Deserialize)]
#[serde(rename_all="camelCase")]
struct Location {
    city: String, //Vancouver,
    country_iso_2a: String, //CA
    region: String, //BC
}

#[derive(Debug, Deserialize)]
#[serde(rename_all="camelCase")]
struct SearchHint {
    description: Vec<String>,
    basic_qualifications: Option<Vec<String>>,
    title: Vec<String>,
    preferred_qualifications: Vec<String>,
    location: Option<Vec<String>>,
    locations: Option<Vec<String>>,
    icims_job_id: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct HintWrapper {
    fields: SearchHint
}

#[derive(Debug, Deserialize)]
#[serde(rename_all="camelCase")]
struct AmazonRes {
    search_hits: Vec<HintWrapper>
}

fn get_req(
)->serde_json::Value {
    json!({
        "accessLevel": "EXTERNAL",
        "contentFilterFacets": [{
            "name": "primarySearchLabel",
            "requestedFacetCount": 9999,
            "values": [{ "name": "studentprograms.team-internships-for-students" }]
        }],
        "excludeFacets": [
            { "name": "isConfidential", "values": [{ "name": "1" }] },
            { "name": "businessCategory", "values": [{ "name": "a-confidential-job" }] },
            { "name": "optionalSearchLabels", "values": [{ "name": "military-spouse" }, { "name": "military-tech" }, { "name": "military-na" }, { "name": "ops.lander-military-skillbridge" }, { "name": "military" }, { "name": "military-skillbridge" }, { "name": "military-student" }, { "name": "aws.team-clearedvets" }] }
        ],
        "filterFacets": [{
            "name": "category",
            "requestedFacetCount": 9999,
            "values": [{ "name": "Software Development" }]
        }],
        "locationFacets": [
            [{ "name": "country", "requestedFacetCount": 9999, "values": [{ "name": "US" }] }],
            [{ "name": "country", "requestedFacetCount": 9999, "values": [{ "name": "CA" }] }]
        ],
        "query": "",
        "size": 10,
        "start": 0,
        "treatment": "OM",
        "sort": {
            "sortOrder": "DESCENDING",
            "sortType": "CREATED_DATE"
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

    let res = client.post("https://amazon.jobs/api/jobs/search?is_als=true")
        .json(&map)
        .send()
        .await?
        .json::<AmazonRes>()
        .await?;
    
    for hit in res.search_hits {
        let url = format!("{}{}", "https://amazon.jobs/jobs/", hit.fields.icims_job_id.first().unwrap());
        
        if urls.contains(&url) {
            continue;
        }

        let title = hit.fields.title.first().unwrap();

        //if 2027/2026 is contained then use that, else fallback to nearest cycle
        let year = if title.contains("2026") { 26 } else { 27 };

        let cleaned_title = title
            .replace(" Internship", "")
            .replace(" Intern", "")
            .replace("US", "")
            .replace("Canada", "")
            .replace(" - ", "")
            .replace("Fall", "")
            .replace("Winter", "")
            .replace("Summer", "")
            .replace("Spring", "")
            .replace("Spring", "")
            .replace(&format!(" 20{}", year), "")
            .replace("()", "");
    

        let mut terms: Vec<Term> = Vec::new();
        
        let seasons = [("Winter", "W"), ("Summer", "S"), ("Fall", "F"), ("Spring", "P")];

        for (season, prefix) in seasons {
            if title.contains(season) {
                terms.push(Term {
                    display_name: format!("{}{}", prefix, &year)
                });
            }
        }

        let cities: Vec<City> = if hit.fields.locations.is_some() {
            let mut cities: Vec<City> = Vec::new();
            let mut locations: Vec<Location> = Vec::new();

            for location in hit.fields.locations.unwrap() {
                let parsed_loc: Location = serde_json::from_str(&location).unwrap();
                locations.push(parsed_loc);
            }
            
            for location in locations {
                cities.push(City {
                    country: location.country_iso_2a,
                    region: location.region,
                    display_name: location.city,
                });
            }

            cities 
        } else {
            let location_val = hit.fields.location.unwrap();
            let mut location = location_val.first().unwrap().trim().split(",");

            vec![City {
                display_name: location.next().unwrap().to_string(),
                region: location.next().unwrap().to_string(),
                country: location.next().unwrap().to_string(),
            }]
        };

        //Wage
        let wage_lines = hit.fields.preferred_qualifications.first().unwrap().split("<br/>");

        let mut lowest: Option<i16> = None;
        let mut highest: Option<i16> = None;
        let mut currency: Option<String> = None;

        for wage in wage_lines {
            if !wage.contains("annually") {
                continue;
            }

            let mut line = wage.split(" - ");
            
            //skip the location
            line.next();

            let lower_text = line.next();

            let lower: Option<i16> = Some((lower_text
                .unwrap()
                .to_string()
                .replace(".00", "")
                .replace(",", "")
                .parse::<i64>()
                .unwrap() * 100 / (40 * 52)
            ).try_into().unwrap());

            if lowest.is_none() || lower < lowest {
                lowest = lower;
            }

            let mut upper_text = line.next().unwrap().split(" "); //includes {num} {currency} annually
            
            let upper: Option<i16> = Some((upper_text.next()
                .unwrap()
                .to_string()
                .replace(".00", "")
                .replace(",", "")
                .parse::<i64>()
                .unwrap() * 100 / (40 * 52)
            ).try_into().unwrap());

            if highest.is_none() || upper > highest {
                highest = upper;
            }
            
            if currency.is_none() {
                currency = Some(upper_text.next().unwrap().to_string());
            }
        }

        let application = Application {
            company_id,
            job_title: cleaned_title.trim().to_string(),
            url: url,
            page_content: format!("{}\n{}\n{}",
                hit.fields.description.first().unwrap(),
                hit.fields.basic_qualifications.unwrap().first().unwrap(),
                hit.fields.preferred_qualifications.first().unwrap()
            ),
            lower_wage_cents: lowest.unwrap(),
            upper_wage_cents: highest,
            state: "ACTIVE".to_string(),
            currency: currency.unwrap(),
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
