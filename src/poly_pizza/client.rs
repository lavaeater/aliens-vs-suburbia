use std::path::Path;
use crate::poly_pizza::types::{ListResponse, PizzaModel, SearchResponse, UserResponse};

const BASE: &str = "https://api.poly.pizza/v1.1";

type BoxError = Box<dyn std::error::Error + Send + Sync>;

pub struct SearchFilters {
    pub category: Option<u32>,
    pub license: Option<u32>,
    pub animated_only: bool,
    pub page: u32,
}

fn build_filter_params(filters: &SearchFilters) -> Vec<(&'static str, String)> {
    let mut params: Vec<(&str, String)> = Vec::new();
    if let Some(cat) = filters.category {
        params.push(("Category", cat.to_string()));
    }
    if let Some(lic) = filters.license {
        params.push(("License", lic.to_string()));
    }
    if filters.animated_only {
        params.push(("Animated", "1".to_string()));
    }
    if filters.page > 0 {
        params.push(("Page", filters.page.to_string()));
    }
    params
}

pub fn search_keyword(api_key: &str, keyword: &str, filters: &SearchFilters) -> Result<SearchResponse, BoxError> {
    let encoded = urlencoding::encode(keyword);
    let url = format!("{BASE}/search/{encoded}");
    let mut req = ureq::get(&url).set("x-auth-token", api_key);
    for (k, v) in build_filter_params(filters) {
        req = req.query(k, &v);
    }
    let resp: SearchResponse = req.call()?.into_json()?;
    Ok(resp)
}

pub fn search_filters(api_key: &str, filters: &SearchFilters) -> Result<SearchResponse, BoxError> {
    let url = format!("{BASE}/search");
    let mut req = ureq::get(&url).set("x-auth-token", api_key);
    for (k, v) in build_filter_params(filters) {
        req = req.query(k, &v);
    }
    let resp: SearchResponse = req.call()?.into_json()?;
    Ok(resp)
}

pub fn get_model(api_key: &str, id: &str) -> Result<PizzaModel, BoxError> {
    let url = format!("{BASE}/model/{id}");
    let resp: PizzaModel = ureq::get(&url)
        .set("x-auth-token", api_key)
        .call()?
        .into_json()?;
    Ok(resp)
}

pub fn get_list(api_key: &str, list_id: &str) -> Result<ListResponse, BoxError> {
    let url = format!("{BASE}/list/{list_id}");
    let resp: ListResponse = ureq::get(&url)
        .set("x-auth-token", api_key)
        .call()?
        .into_json()?;
    Ok(resp)
}

pub fn get_user(api_key: &str, username: &str) -> Result<UserResponse, BoxError> {
    let url = format!("{BASE}/user/{username}");
    let resp: UserResponse = ureq::get(&url)
        .set("x-auth-token", api_key)
        .call()?
        .into_json()?;
    Ok(resp)
}

pub fn download_glb(url: &str, dest: &Path) -> Result<(), BoxError> {
    download_file_with_auth(url, dest, None)
}

pub fn download_thumbnail(url: &str, dest: &Path) -> Result<(), BoxError> {
    download_file_with_auth(url, dest, None)
}

fn download_file_with_auth(url: &str, dest: &Path, auth: Option<&str>) -> Result<(), BoxError> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut req = ureq::get(url);
    if let Some(key) = auth {
        req = req.set("x-auth-token", key);
    }
    let mut reader = req.call()?.into_reader();
    let mut file = std::fs::File::create(dest)?;
    std::io::copy(&mut reader, &mut file)?;
    Ok(())
}
