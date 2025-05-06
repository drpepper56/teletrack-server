use crate::{
    app_state,
    my_structs::tracking_data_formats::tracking_data_webhook_update::TrackingResponse as tracking_data_webhook_update,
};
use actix_web::{post, web, HttpRequest, HttpResponse, Responder};
use chrono::Utc;
use hex::encode;
use sha2::{Digest, Sha256};

/*

    Structs

*/

#[derive(thiserror::Error, Debug)]
pub enum webhook_error {
    #[error("Serde error: {0}")]
    SerdeError(#[from] serde_json::Error),
    #[error("missing header with the signed digest")]
    MissingHeaderSign,
    #[error("sign failed - no match, abort")]
    SignFailedNoMatch,
}

/*
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
    NOTIFICATION FUNCTIONS

-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
*/

/// this function will send an update to the Bot API in telegram that will (hopefully) show a popup notification through the
/// telegram environment and pass the data to be resolved in the mini app
async fn notify_of_tracking_event_update(
    data: web::Data<app_state>,
    // to what user //TODO: verify to which user using database
    user_id: web::Path<i64>,
) -> impl Responder {
    // access the service and deal with validation checks from the errors
    match &*data.notification_service {
        Ok(service) => {
            match service
                .send_ma_notification(
                    *user_id,
                    "Update on your order tracking.",
                    Some(vec![
                        ("balls", "new new params"),
                        ("balls2", "properly handled"),
                    ]),
                )
                .await
            {
                Ok(_) => HttpResponse::Ok().json("Notification sent successfully"),
                Err(e) => HttpResponse::InternalServerError().json(e.to_string()),
            }
        }
        Err(e) => HttpResponse::InternalServerError().json(e.to_string()),
    }
}

// TODO: implement logic for notifying the right user of the update on their package
// TODO: move to webhook

/*
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
    WEBHOOK

-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
*/

#[post("/webhook_17track")]
pub async fn handle_webhook(
    data: web::Data<crate::app_state>,
    request: HttpRequest,
    payload: web::Json<tracking_data_webhook_update>,
) -> impl Responder {
    // hello
    println!("human written console message: webhook received, my secret key is 123fuckyou456");

    // check if the the header contains a sign value
    let digest_from_api = match request.headers().get("sign") {
        Some(header) => match header.to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return HttpResponse::BadRequest().finish(),
        },
        None => return HttpResponse::BadRequest().finish(),
    };
    // concat payload + / + security key as strings
    let digest_raw = payload.to_string() + "/" + &data.webhook_secret;
    let digest_raw_test = "{\"event\":\"TRACKING_UPDATED\",\"data\":{\"number\":\"RR123456789CN\",\"carrier\":3011,\"tag\":null}}/".to_string() + &data.webhook_secret;
    // get a sha256 digest of the string
    let hash = Sha256::digest(&digest_raw);
    let hash_test = Sha256::digest(&digest_raw_test);
    // compare it to the sign value from the header
    println!(" test {}", digest_raw_test);
    println!(" test {}", encode(&hash_test));
    println!("      {}", digest_raw);
    println!("      {}", encode(&hash));
    println!("");
    println!("      {}", digest_from_api);
    if digest_from_api != encode(&hash) {
        println!("{}", webhook_error::SignFailedNoMatch);
        return HttpResponse::BadRequest().finish();
    }
    //

    // print all headers
    println!("Received headers:");
    for (name, value) in request.headers().iter() {
        println!("  {}: {:?}", name, value);
    }

    // print the whole boomboclat thing
    println!("webhook received payload: {:?}", payload);

    HttpResponse::Ok().body("OK")
}
