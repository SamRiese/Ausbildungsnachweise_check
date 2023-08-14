use std::fs;
use std::fs::{create_dir_all, File};
use std::io::Write;

use chrono::{Datelike, Days, Duration, Local, NaiveDate};
use dialoguer::Input;
use directories::ProjectDirs;
use octocrab::GitHubError;
use octocrab::models::repos::ContentItems;
use octocrab::Octocrab;
use octocrab::OctocrabBuilder;
use serde::Deserialize;
use thiserror::Error;

#[derive(Deserialize, Debug)]
struct Configuration {
    github_token: String,
    start_of_apprenticeship: String,
    organization: String,
    branch: String,
    file_dir: String,
    apprentices: Vec<String>,
}

#[derive(Error, Debug)]
enum AusbildungsnachweiseCheckError {
    #[error("File not found")]
    FileNotFound
}

#[tokio::main]
async fn main() -> Result<(), AusbildungsnachweiseCheckError> {
    let project_dirs= ProjectDirs::from(
        "",
        "NGITL",
        "Ausbildungsnachweise_Check"
    ).expect("no valid home directory found");

    let directory = project_dirs.config_dir();
    let path = directory.join("configuration.json");

    if !path.exists() {
        let _ = create_dir_all(directory);
        let mut file = File::create(&path).expect("Unable to create file");
        file.write_all(include_bytes!("../configuration_sample.json"))
            .expect("Failed to create sample configuration.json");
        println!("Created sample Configuration.json at: {}", path.display());
        return Ok(())
    }

    let data = fs::read_to_string(&path)
        .expect(format!("Unable to read file at: {}", path.display()).as_str());

    let configuration: Configuration = serde_json::from_str(&data)
        .expect("Json conversion failed");

    let octocrab: Octocrab = OctocrabBuilder::default().personal_token(configuration.github_token)
        .build().expect("Github token builder failed");

    let week: i64 = get_current_week(configuration.start_of_apprenticeship);

    let week= Input::new()
        .with_prompt(format!("Enter number of report or press enter to continue with current week"))
        .default(week)
        .interact_text()
        .expect("Could not display prompt");

    for apprentice in configuration.apprentices.into_iter() {
        let repository: String = apprentice.replace(" ", "_");
        let file_path: String = get_file_path(&apprentice, &configuration.file_dir, week);

        let file = get_file(
            &octocrab,
            &configuration.organization,
            repository,
            file_path,
            &configuration.branch);

        if let Err(_) = file.await {
            println!("{}: file non-existing", apprentice);
        }
    }
    Ok(())
}

fn get_current_week(start_of_apprenticeship: String) -> i64 {
    let start_date: NaiveDate = NaiveDate::parse_from_str(&start_of_apprenticeship,"%Y-%m-%d")
        .expect(format!("Unexpected date format: {start_of_apprenticeship}").as_str());
    let today: NaiveDate = Local::now().date_naive();

    let today_days: Days = Days::new(today.weekday().num_days_from_monday() as u64);
    let today_monday: NaiveDate = today.checked_sub_days(today_days).unwrap();

    let start_days: Days = Days::new(start_date.weekday().num_days_from_monday() as u64);
    let start_monday: NaiveDate = start_date.checked_sub_days(start_days).unwrap();

    let duration: Duration = today_monday.signed_duration_since(start_monday);

    duration.num_weeks()
}

fn get_file_path(apprentice: &String, file_dir: &String, week: i64) -> String {
    let name: Vec<&str> = apprentice.split_whitespace().collect();
    if name.len() >= 2 {
         format!("{}/AN_{}_{}_{:03}.pdf",
                 file_dir,
                 name.last().unwrap(),
                 name.first().unwrap(),
                 week)
    } else {
        panic!("The apprentice's name is not in the correct format!");
    }
}

async fn get_file(octocrab: &Octocrab, organization: &String, repository: String,
                  file_path: String, branch: &String) -> Result<ContentItems, AusbildungsnachweiseCheckError> {
    let content = octocrab
        .repos(organization, repository)
        .get_content()
        .path(file_path)
        .r#ref(branch)
        .send()
        .await;

    if let Err(
        octocrab::Error::GitHub {
            source: GitHubError { message: msg, .. },
            ..
        }
    ) = &content {
        if msg == "Not Found" {
            return Err(AusbildungsnachweiseCheckError::FileNotFound);
        }
    }

    Ok(content.expect("Unexpected failure"))
}
