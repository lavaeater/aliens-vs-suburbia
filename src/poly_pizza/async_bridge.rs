use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender, channel};
use bevy::prelude::Resource;
use crate::poly_pizza::client::{self, SearchFilters};
use crate::poly_pizza::types::{ListResponse, PizzaModel, SearchResponse, UserResponse};

#[derive(Resource)]
pub struct PolyPizzaConfig {
    pub api_key: String,
}

pub enum ApiRequest {
    SearchKeyword { keyword: String, filters: SearchFilters },
    SearchFilters { filters: SearchFilters },
    GetList(String),
    GetUser(String),
    DownloadGlb { id: String, url: String, dest: PathBuf },
    DownloadThumbnail { id: String, url: String, dest: PathBuf },
}

pub enum ApiResponse {
    SearchResults(SearchResponse),
    ListResults(ListResponse),
    UserResults(UserResponse),
    DownloadComplete { id: String },
    ThumbnailComplete { id: String },
    Error(String),
}

#[derive(Resource)]
pub struct ApiChannels {
    pub tx: Sender<ApiRequest>,
    pub rx: Mutex<Receiver<ApiResponse>>,
}

pub fn spawn_api_thread(api_key: String) -> ApiChannels {
    let (req_tx, req_rx) = channel::<ApiRequest>();
    let (resp_tx, resp_rx) = channel::<ApiResponse>();

    std::thread::spawn(move || {
        loop {
            match req_rx.recv() {
                Err(_) => break,
                Ok(request) => {
                    let response = match request {
                        ApiRequest::SearchKeyword { keyword, filters } => {
                            match client::search_keyword(&api_key, &keyword, &filters) {
                                Ok(r) => ApiResponse::SearchResults(r),
                                Err(e) => ApiResponse::Error(e.to_string()),
                            }
                        }
                        ApiRequest::SearchFilters { filters } => {
                            match client::search_filters(&api_key, &filters) {
                                Ok(r) => ApiResponse::SearchResults(r),
                                Err(e) => ApiResponse::Error(e.to_string()),
                            }
                        }
                        ApiRequest::GetList(id) => {
                            match client::get_list(&api_key, &id) {
                                Ok(r) => ApiResponse::ListResults(r),
                                Err(e) => ApiResponse::Error(e.to_string()),
                            }
                        }
                        ApiRequest::GetUser(username) => {
                            match client::get_user(&api_key, &username) {
                                Ok(r) => ApiResponse::UserResults(r),
                                Err(e) => ApiResponse::Error(e.to_string()),
                            }
                        }
                        ApiRequest::DownloadGlb { id, url, dest } => {
                            match client::download_glb(&url, &dest) {
                                Ok(()) => ApiResponse::DownloadComplete { id },
                                Err(e) => ApiResponse::Error(e.to_string()),
                            }
                        }
                        ApiRequest::DownloadThumbnail { id, url, dest } => {
                            match client::download_thumbnail(&url, &dest) {
                                Ok(()) => ApiResponse::ThumbnailComplete { id },
                                Err(e) => ApiResponse::Error(e.to_string()),
                            }
                        }
                    };
                    if resp_tx.send(response).is_err() {
                        break;
                    }
                }
            }
        }
    });

    ApiChannels { tx: req_tx, rx: Mutex::new(resp_rx) }
}
