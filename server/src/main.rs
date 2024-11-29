use axum::{
    routing::{get, post},
    Router,
    Json,
    response::IntoResponse,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;

#[derive(Debug, Serialize, Deserialize)]
struct UserCredentials {
    username: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct BrowserSettings {
    start_url: String,
    incognito: bool,
    max_navigation_timeout: u64,
    allowed_domains: Vec<String>,   
}

#[axum::debug_handler]
async fn authenticate(Json(credentials): Json<UserCredentials>) -> impl IntoResponse {
    // Hardcoded credentials check
    if credentials.username == "admin_user" && credentials.password == "secure_password" {
        StatusCode::OK
    } else {
        StatusCode::UNAUTHORIZED
    }
}

#[axum::debug_handler]
async fn get_browser_settings() -> Json<BrowserSettings> {
    Json(BrowserSettings {
        start_url: "https://www.google.com".to_string(),
        incognito: false,
        max_navigation_timeout: 30000,
        allowed_domains: vec!["google.com".to_string(), "github.com".to_string()],
    })
}

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() {
    // Initialize logging
    env_logger::init();

    let cors = CorsLayer::permissive();

    let app = Router::new()
        .route("/authenticate", post(authenticate))
        .route("/browser-settings", get(get_browser_settings))
        .layer(cors);

    let addr = "0.0.0.0:8080";
    println!("Server running on http://{}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app.into_make_service()).await.unwrap();
}