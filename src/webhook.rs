use crate::{
    app_state,
    my_structs::tracking_data_formats::tracking_data_webhook_update::TrackingResponse as tracking_data_webhook_update,
};
use actix_web::{post, web, HttpRequest, HttpResponse, Responder};
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
    request: HttpRequest,
    payload: web::Json<serde_json::Value>,
) -> impl Responder {
    println!(
        "human written console message: webhook received, my secret key is {}",
        data.webhook_secret
    );

    println!("Received headers:");
    for (name, value) in request.headers().iter() {
        println!("  {}: {:?}", name, value);
    }

    // print the whole boomboclat thing
    println!("Full Webhook Payload: {:?}", payload);

    // Here you would:
    // 1. Verify the webhook signature (if using) - skipped for now
    // 2. Process the tracking update
    // 3. Store in database or trigger actions
    HttpResponse::Ok().body("OK")
    // Process the payload generically
    // HttpResponse::Ok().json(serde_json::json!({
    //     "status": "processed",
    //     "processed_at": chrono::Utc::now().timestamp()
    // }))
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
