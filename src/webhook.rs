use crate::{
    app_state,
    my_structs::tracking_data_formats::tracking_data_database_form::TrackingData_DBF as tracking_data_database_form,
    my_structs::tracking_data_formats::tracking_data_webhook_update::TrackingResponse as webhook_update,
};
use actix_web::{body, post, web, FromRequest, HttpMessage, HttpRequest, HttpResponse, Responder};
use chrono::Utc;
use hex::encode;
use mongodb::{
    bson::{de, doc},
    options::{ClientOptions, FindOneAndUpdateOptions, FindOptions},
    Client,
};
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
    #[error("failed to get the raw body of request")]
    FailedToGetRawBody,
    #[error("error converting webhook update to db format")]
    ErrorConvertingWebhookUpdate,
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

/*
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
    HELPER FUNCTIONS

-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
*/

/// consume the body and return whatever the text is as a @String, skip the extractor for consistency with the API sign
async fn get_raw_body_as_string(body: web::Bytes) -> Result<String, webhook_error> {
    // get a string from the bytes
    match String::from_utf8(body.to_vec()) {
        Ok(body_str) => Ok(body_str),
        Err(e) => {
            println!("{}", e);
            Err(webhook_error::FailedToGetRawBody)
        }
    }
}

/// Function to verify 17Crack origin
async fn verify_origin_body(
    data: web::Data<crate::app_state>,
    request: HttpRequest,
    body: web::Bytes,
) -> Result<webhook_update, HttpResponse> {
    // check if the the header contains a sign value
    let digest_from_api = match request.headers().get("sign") {
        Some(header) => match header.to_str() {
            Ok(s) => Ok::<String, actix_web::Error>(s.to_string()),
            Err(_) => {
                println!("sign header bad format");
                return Err(HttpResponse::BadRequest().finish());
            }
        },
        None => {
            println!("sign header missing");
            return Err(HttpResponse::BadRequest().finish());
        }
    }
    .unwrap();
    //

    // get the request body as a string, not using payload, it's parsed as a expected format and it's semantically incompatible because of diffs in nulls
    let body_string = match get_raw_body_as_string(body).await {
        Ok(s) => Ok::<String, actix_web::Error>(s),
        Err(e) => {
            println!("problem with getting the raw request body string: {}", e);
            return Err(HttpResponse::BadRequest().finish());
        }
    }
    .unwrap();
    //

    // concat payload + / + security key as strings
    let digest_raw = body_string.clone() + "/" + &data.webhook_secret;
    // get a sha256 digest of the string
    let hash = Sha256::digest(&digest_raw);
    // compare it to the sign value from the header
    if digest_from_api != encode(&hash) {
        println!("{}", webhook_error::SignFailedNoMatch);
        return Err(HttpResponse::BadRequest().finish());
    }
    //

    // // print all headers
    // println!("Received headers:");
    // for (name, value) in request.headers().iter() {
    //     println!("  {}: {:?}", name, value);
    // }

    // extract the webhook_update struct from the body bytes
    match serde_json::from_str::<webhook_update>(&body_string) {
        Ok(payload) => Ok(payload),
        Err(e) => {
            println!("error parsing the body to the webhook_update struct: {}", e);
            return Err(HttpResponse::InternalServerError().finish());
        }
    }
}

/// Function used by webhook, takes the webhook update format of tracking update, converts to database form and refreshed the entry in the database
async fn refresh_tracking_info_from_webhook_update(
    client: web::Data<Client>,
    tracking_info_update: webhook_update,
) -> Result<(), HttpResponse> {
    // convert the webhook_update to tracking_data_database_form
    let tracking_data_database_form = match tracking_info_update.convert_to_TrackingData_DBF() {
        Some(data) => Ok(data),
        None => Err(webhook_error::ErrorConvertingWebhookUpdate),
    }
    .unwrap();
    //

    // set database
    let db = client.database("teletrack");
    // set collection
    let collection_tracking_data: mongodb::Collection<tracking_data_database_form> =
        db.collection("tracking_data");
    // set filter to search for old data
    let filter = doc! {"data.number": &tracking_data_database_form.data.number};
    // delete any previous info with that tracking number
    match collection_tracking_data.delete_many(filter, None).await {
        Ok(delete_result) => {
            println!("deleted {} tracking data docs", delete_result.deleted_count);
            Ok(())
        }
        Err(e) => {
            println!(
                "@REFRESH_TRACKING_DATA: non fatal error deleting old tracking data: {}",
                e
            );
            Err(e)
        }
    };
    //

    // insert the fresh tracking info into the database
    match collection_tracking_data
        .insert_one(tracking_data_database_form.clone(), None)
        .await
    {
        Ok(_) => {
            // returning the fresh tracking info to the user
            println!("tracking data inserted");
            Ok(())
        }
        Err(e) => {
            println!(
                "@WEBHOOK_UPDATE_DATABASE: error inserting tracking data: {}",
                e
            );
            Err(HttpResponse::InternalServerError().body(e.to_string()))
        }
    }
}
/*
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
    WEBHOOK

-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
*/

#[post("/webhook_17track")]
pub async fn handle_webhook(
    data: web::Data<crate::app_state>,
    client: web::Data<Client>,
    request: HttpRequest,
    body: web::Bytes,
) -> impl Responder {
    let payload = match verify_origin_body(data.clone(), request.clone(), body.clone()).await {
        Ok(payload) => payload,
        Err(e) => return e,
    };

    println!("webhook received payload and extracted");
    // print the whole boomboclat thing
    // println!("  {:?}", payload);
    println!("  {:?}", payload.event);

    // split to two paths based on event, only test UPDATE rn

    // save the update in database in format
    match refresh_tracking_info_from_webhook_update(client.clone(), payload.clone()).await {
        Ok(_) => {}
        Err(response) => {
            println!("unknown error trying to refresh database tracking info from update");
            return response;
        }
    };
    //

    // get list of users to notify of the update

    // call the update function

    HttpResponse::Ok().body(format!("{}", payload.event))
}
