use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use serde::{Serialize, Deserialize};
use log::{warn, error};

use crate::balance;

#[derive(Serialize, Deserialize)]
pub struct BalanceResponse {
    pub balance: f64,
}

#[derive(serde::Deserialize)]
struct ProfileQuery {
    user_id: String,
}

#[get("/profile")]
async fn profile(query: web::Query<ProfileQuery>) -> impl Responder {
    let user_id = &query.user_id;

    if !balance::is_user_bound(user_id) {
        warn!("🛑 Attempting to access the profile of an unregistered user: {}", user_id);
        return HttpResponse::BadRequest().body("🛑 Your account is not bound. please use /bind first.");
    }

    let user_data = balance::load_user_data(user_id);
    let response = BalanceResponse {
        balance: user_data.balance,
    };
    HttpResponse::Ok().json(response)
}

pub fn init_api(cfg: &mut web::ServiceConfig) {
    cfg.service(profile);
}

pub async fn start_api_server() -> std::io::Result<()> {
    HttpServer::new(|| App::new().configure(init_api))
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App, http::StatusCode};
    use serde_json;

    #[actix_web::test]
    async fn test_profile_unbound() {
        let app = test::init_service(App::new().configure(init_api)).await;
        let req = test::TestRequest::get()
            .uri("/profile?user_id=test")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[actix_web::test]
    async fn test_profile_bound() {
        let user_id = "!testApiBackport";

        let _ = crate::balance::bind_user(user_id);

        let mut user_data = crate::balance::load_user_data(user_id);
        user_data.balance = 42.0;
        let _ = crate::balance::save_user_data(user_id, &user_data);

        let app = test::init_service(App::new().configure(init_api)).await;
        let req = test::TestRequest::get()
            .uri(&format!("/profile?user_id={}", user_id))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let body = test::read_body(resp).await;
        let response: BalanceResponse = serde_json::from_slice(&body)
            .expect("🛑 Error deserializing response");
        assert_eq!(response.balance, 42.0);
    }
}