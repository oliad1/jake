use crate::{Error, Application, ScrapeEvent, City, Term};
use tokio::sync::mpsc::Sender;
use std::collections::{HashSet, HashMap};
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
#[serde(rename_all="camelCase")]
struct Location {
    normalized_state_name: String, //British Columbia,
    normalized_country_code: String, //CAN,
    city: String, //Vancouver,
    country_iso_3a: String, //CAN,
    country_iso_2a: String, //CA,
    location_non_stemming: String, //Canada, BC, Vancouver,
    //coordinates: (f32, f32), //49.26038,-123.11336,
    normalized_county_name: String, //Metro Vancouver,
    //type: String, //ONSITE,
    normalized_country_name: String, //Canada,
    normalized_location: String, //Vancouver, British Columbia, CAN
    location: String, //CA, BC, Vancouver
    region: String, //BC
    building_code_list: Vec<String>,
    normalized_city_name: String
}

#[derive(Debug, Deserialize)]
#[serde(rename_all="camelCase")]
struct SearchHint {
    optional_search_labels: Option<Vec<String>>,
    country: Option<Vec<String>>,
    is_intern: Option<Vec<String>>,
    normalized_country_code: Option<Vec<String>>,
    art_job_id: Option<Vec<String>>,
    city: Option<Vec<String>>,
    country_iso_3a: Option<Vec<String>>,
    source_system: Option<Vec<String>>,
    company_name: Option<Vec<String>>,
    primary_search_label: Option<Vec<String>>,
    job_code: Option<Vec<String>>,
    description: Vec<String>,
    is_tech: Option<Vec<String>>,
    basic_qualifications: Option<Vec<String>>,
    updated_date: Option<Vec<String>>,
    title: Vec<String>,
    normalized_location: Option<Vec<String>>,
    job_function_id: Option<Vec<String>>,
    is_manager: Option<Vec<String>>,
    job_role: Option<Vec<String>>,
    job_family: Option<Vec<String>>,
    normalized_state_name: Option<Vec<String>>,
    schedule_type_id: Option<Vec<String>>,
    normalized_city_name: Option<Vec<String>>,
    employee_class: Option<Vec<String>>,
    preferred_qualifications: Vec<String>,
    is_confidential: Option<Vec<String>>,
    is_unsearchable: Option<Vec<String>>,
    short_description: Option<Vec<String>>,
    role_fungibility: Option<Vec<String>>,
    central_recruitment_team: Option<Vec<String>>,
    created_date: Option<Vec<String>>,
    team_category: Option<Vec<String>>,
    url_next_step: Option<Vec<String>>,
    business_category: Option<Vec<String>>,
    location: Option<Vec<String>>,
    locations: Option<Vec<String>>,
    region: Option<Vec<String>>,
    category: Option<Vec<String>>,
    hire_type_id: Option<Vec<String>>,
    icims_job_id: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct HintWrapper {
    fields: SearchHint
}

#[derive(Debug, Deserialize)]
#[serde(rename_all="camelCase")]
struct AmazonRes {
    _found: i64,
    _start: i64,
    search_hits: Vec<HintWrapper>
}

pub async fn main(
    company_id: i64,
    urls: HashSet<String>,
    tx: Sender<ScrapeEvent>
) -> Result<(), Error> {
    let client = reqwest::Client::new();

    let map = json!({
        "accessLevel": "EXTERNAL",
        "contentFilterFacets": [{
            "name": "primarySearchLabel",
            "requestedFacetCount": 9999,
            "values": [{ "name": "studentprograms.team-internships-for-students" }]
        }],
        "excludeFacets": [
            { "name": "isConfidential", "values": [{ "name": "1" }] },
            { "name": "businessCategory", "values": [{ "name": "a-confidential-job" }] }
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
    });

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

        //26
        //TODO: Handle year parsing, clean up job title (remove dates like Fall 2026 and places
        //like '(Canada)' or '(US)', remove the word 'Internship')
        let year = 26;

        let mut terms: Vec<Term> = Vec::new();
        
        if title.contains("Winter") {
            terms.push(Term {
                display_name: format!("W{}", &year)
            });
        }

        if title.contains("Summer") {
            terms.push(Term {
                display_name: format!("S{}", &year)
            });
        }

        if title.contains("Spring") {
            terms.push(Term {
                display_name: format!("P{}", &year)
            });
        }

        if title.contains("Fall") {
            terms.push(Term {
                display_name: format!("F{}", &year)
            });
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
            job_title: title.to_string(),
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
