use crate::my_structs::tracking_data_formats::tracking_data_webhook_update::TrackingResponse as tracking_data_webhook_update;
use actix_web::{post, web, HttpResponse, Responder};
use chrono::Utc;
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::env;

#[post("/webhook/17track")]
pub async fn handle_webhook(
    data: web::Data<crate::app_state>,
    payload: web::Json<tracking_data_webhook_update>,
) -> impl Responder {
    info!("Received tracking update from 17track webhook");

    // print the whole boomboclat thing
    info!("Full Webhook Payload: {:?}", payload);

    // Here you would:
    // 1. Verify the webhook signature (if using) - skipped for now
    // 2. Process the tracking update
    // 3. Store in database or trigger actions

    HttpResponse::Ok().json(serde_json::json!({
        "status": "processed",
        "processed_at": Utc::now().timestamp()
    }))
}

// #[post("/webhook/17track/verify")]
// pub async fn verify_webhook(
//     data: web::Data<AppState>,
//     challenge: web::Json<WebhookVerification>,
// ) -> impl Responder {
//     info!("Received webhook verification challenge");
//     HttpResponse::Ok().json(serde_json::json!({
//         "challenge": challenge.challenge
//     }))
// }

// // Webhook verification structure
// #[derive(Debug, Serialize, Deserialize)]
// struct WebhookVerification {
//     challenge: String,
// }
