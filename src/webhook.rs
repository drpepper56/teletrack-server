use actix_web::{post, web, HttpResponse, Responder};
use chrono::Utc;
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::env;

// Webhook payload structure from 17Track
#[derive(Debug, Serialize, Deserialize)]
pub struct TrackingUpdate {
    pub tracking_number: String,
    pub status: String,
    pub events: Vec<Event>,
    pub timestamp: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Event {
    pub event_time: String,
    pub location: String,
    pub status_description: String,
}

// Webhook verification structure
#[derive(Debug, Serialize, Deserialize)]
struct WebhookVerification {
    challenge: String,
}

// App State for shared data
pub struct AppState {
    pub webhook_secret: String,
}

#[post("/webhook/17track")]
pub async fn handle_webhook(
    data: web::Data<AppState>,
    payload: web::Json<TrackingUpdate>,
) -> impl Responder {
    info!("Received tracking update for: {}", payload.tracking_number);

    // Here you would:
    // 1. Verify the webhook signature (if using)
    // 2. Process the tracking update
    // 3. Store in database or trigger actions

    // Example processing:
    let latest_event = payload.events.last().map(|e| &e.status_description);
    info!("Latest status: {:?}", latest_event);

    HttpResponse::Ok().json(serde_json::json!({
        "status": "processed",
        "tracking_number": payload.tracking_number,
        "processed_at": Utc::now().timestamp()
    }))
}

#[post("/webhook/17track/verify")]
pub async fn verify_webhook(
    data: web::Data<AppState>,
    challenge: web::Json<WebhookVerification>,
) -> impl Responder {
    info!("Received webhook verification challenge");
    HttpResponse::Ok().json(serde_json::json!({
        "challenge": challenge.challenge
    }))
}
