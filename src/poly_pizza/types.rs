use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Creator {
    #[serde(rename = "Username")]
    pub username: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Orbit {
    pub phi: String,
    pub radius: String,
    pub theta: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PizzaModel {
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(rename = "Title")]
    pub title: String,
    #[serde(rename = "Description")]
    pub description: Option<String>,
    #[serde(rename = "Attribution")]
    pub attribution: String,
    #[serde(rename = "Thumbnail")]
    pub thumbnail_url: String,
    #[serde(rename = "Download")]
    pub download_url: String,
    #[serde(rename = "Tri Count")]
    pub tri_count: u32,
    #[serde(rename = "Creator")]
    pub creator: Creator,
    #[serde(rename = "Category")]
    pub category: Option<serde_json::Value>,
    #[serde(rename = "Licence")]
    pub licence: Option<String>,
    #[serde(rename = "Animated")]
    pub animated: Option<bool>,
    #[serde(rename = "Orbit")]
    pub orbit: Option<Orbit>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchResponse {
    pub total: u32,
    pub results: Vec<PizzaModel>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListResponse {
    #[serde(rename = "Models")]
    pub models: Vec<PizzaModel>,
    #[serde(rename = "Name")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserResponse {
    pub username: String,
    pub models: Vec<PizzaModel>,
}
