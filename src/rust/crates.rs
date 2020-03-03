use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Deserialize)]
struct CrateResponse {
    #[serde(rename = "crate")]
    summary: CrateSummary,
    versions: Vec<CrateVersion>,
    keywords: Vec<CrateKeyword>,
    categories: Vec<CrateCategory>,
}

#[derive(Deserialize)]
struct CrateSummary {
    name: String,
    updated_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
    downloads: usize,
    recent_downloads: usize,
    newest_version: String,
    description: String,
    homepage: Option<String>,
    documentation: Option<String>,
    repository: Option<String>,
}

#[derive(Deserialize)]
struct CrateOwnerResponse {
    users: Vec<CrateUser>,
}

#[derive(Deserialize)]
struct CrateVersion {
    #[serde(rename = "num")]
    version: String,
    crate_size: Option<usize>,
    license: Option<String>,
}

#[derive(Deserialize)]
pub struct CrateUser {
    pub name: Option<String>,
    pub url: String,
}

#[derive(Deserialize)]
pub struct CrateDependencies {
    dependencies: Vec<CrateDependency>,
}

#[derive(Deserialize)]
pub struct CrateDependency {
    #[serde(default = "String::default")]
    kind: String,
}

#[derive(Deserialize)]
pub struct CrateKeyword {
    keyword: String,
}

#[derive(Deserialize)]
pub struct CrateCategory {
    category: String,
}

pub struct Information {
    pub name: String,
    pub updated_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub downloads: usize,
    pub recent_downloads: usize,
    pub newest_version: String,
    pub crate_size: usize,
    pub description: String,
    pub homepage: Option<String>,
    pub documentation: Option<String>,
    pub repository: Option<String>,
    pub owner: Vec<CrateUser>,
    pub dependency_count: usize,
    pub dev_dependency_count: usize,
    pub license: Option<String>,
    pub keywords: Vec<String>,
    pub categories: Vec<String>,
}

pub async fn get_information(crate_name: &str) -> reqwest::Result<Option<Information>> {
    use crate::util::WEB_CLIENT;
    let summary_url = format!("https://crates.io/api/v1/crates/{}", crate_name);
    let summary_response = WEB_CLIENT.get(&summary_url).send();
    let owner_url = format!("https://crates.io/api/v1/crates/{}/owner_user", crate_name);
    let owner_response = WEB_CLIENT.get(&owner_url).send();

    let (summary_response, owner_response) = tokio::try_join!(summary_response, owner_response)?;

    if summary_response.status().is_client_error() {
        return Ok(None);
    }

    if owner_response.status().is_client_error() {
        return Ok(None);
    }
    let owner: CrateOwnerResponse = owner_response.json().await?;

    let CrateResponse {
        summary,
        versions,
        mut keywords,
        mut categories,
    } = summary_response.json().await?;

    let newest_version = versions
        .iter()
        .find(|v| v.version == summary.newest_version);
    if let Some(newest_version) = newest_version {
        let dependency_url = format!(
            "https://crates.io/api/v1/crates/{}/{}/dependencies",
            crate_name, summary.newest_version
        );
        let dependency: CrateDependencies =
            WEB_CLIENT.get(&dependency_url).send().await?.json().await?;
        Ok(Some(Information {
            name: summary.name,
            updated_at: summary.updated_at,
            created_at: summary.created_at,
            downloads: summary.downloads,
            recent_downloads: summary.recent_downloads,
            newest_version: summary.newest_version,
            crate_size: newest_version.crate_size.unwrap_or(0),
            description: summary.description,
            homepage: summary.homepage,
            documentation: summary.documentation,
            repository: summary.repository,
            owner: owner.users,
            dependency_count: dependency.dependencies.len(),
            dev_dependency_count: dependency
                .dependencies
                .iter()
                .filter(|d| d.kind == "dev")
                .count(),
            license: newest_version.license.clone(),
            keywords: keywords.drain(..).map(|k| k.keyword).collect(),
            categories: categories.drain(..).map(|c| c.category).collect(),
        }))
    } else {
        Ok(None)
    }
}
