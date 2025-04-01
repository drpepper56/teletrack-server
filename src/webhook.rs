use crate::{
    app_state,
    my_structs::tracking_data_formats::tracking_data_webhook_update::TrackingResponse as tracking_data_webhook_update,
};
use actix_web::{post, web, HttpResponse, Responder};
use chrono::Utc;

/*

    Structs

*/

#[derive(thiserror::Error, Debug)]
pub enum webhook_error {
    #[error("Serde error: {0}")]
    SerdeError(#[from] serde_json::Error),
}

#[post("/webhook/17track")]
pub async fn handle_webhook(
    data: web::Data<crate::app_state>,
    payload: web::Json<serde_json::Value>,
) -> impl Responder {
    println!(
        "human written console message: webhook received, my secret key is {}",
        data.webhook_secret
    );

    // print the whole boomboclat thing
    println!("Full Webhook Payload: {:?}", payload);

    // Here you would:
    // 1. Verify the webhook signature (if using) - skipped for now
    // 2. Process the tracking update
    // 3. Store in database or trigger actions

    // Process the payload generically
    let response = match process_any_payload(&payload) {
        Ok(msg) => HttpResponse::Ok().json(serde_json::json!({
            "status": "success",
            "message": msg
        })),
        Err(msg) => HttpResponse::BadRequest().json(serde_json::json!({
            "status": "error",
            "message": msg
        })),
    };

    response
}

fn process_any_payload(payload: &serde_json::Value) -> Result<String, String> {
    // Example: Check if payload is an object
    if !payload.is_object() {
        return Err("Payload must be a JSON object".to_string());
    }

    // You can add any generic validation here

    // Example: Extract some common fields if they exist
    let event_type = payload
        .get("event")
        .or_else(|| payload.get("event_type"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    println!("Processing event type: {}", event_type);

    // In a real application, you would:
    // 1. Verify the webhook signature
    // 2. Route to different handlers based on content
    // 3. Queue for background processing if needed

    Ok(format!("Processed event: {}", event_type))
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
