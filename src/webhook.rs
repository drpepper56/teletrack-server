use crate::{
    main,
    my_structs::tracking_data_formats::{
        tracking_data_database_form::TrackingData_DBF as tracking_data_database_form,
        tracking_data_webhook_update::{
            PackageDataWebhook, TrackingData, TrackingResponse as webhook_update,
        },
    },
    AppState,
};
use actix_web::{post, web, HttpRequest, HttpResponse, Responder};
use futures::{StreamExt, TryStreamExt};
use hex::encode;
use mongodb::{bson::doc, Client};
use serde::{Deserialize, Serialize};
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

// struct for saving tracking number + carrier (optional) + user id hash as a relation record in the database
// this also holds a bool that decides if the user is getting updates for the number or not
// TODO: redundant with main
#[derive(Serialize, Deserialize, Debug)]
pub struct tracking_number_user_relation {
    tracking_number: String,
    carrier: Option<i32>,
    user_id_hash: String,
    is_subscribed: bool,
}
/// User structure
// TODO: redundant with main
#[derive(Debug, Deserialize, Serialize)]
struct user {
    user_id: i64,
    user_id_hash: String,
    user_name: String,
}

/*
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
    NOTIFICATION FUNCTIONS

-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
*/

/// this function will send an update to the Bot API in telegram that will (hopefully) show a popup notification through the
/// telegram environment and pass the data to be resolved in the mini app
pub async fn notify_of_tracking_event_update(
    data: web::Data<AppState>,
    user_id: i64,
    message: &str,
    tracking_number_that_was_updated: &str,
) -> Result<(), HttpResponse> {
    // access the service and deal with validation checks from the errors
    match &*data.notification_service {
        Ok(service) => {
            match service
                .send_ma_notification(user_id, message, tracking_number_that_was_updated)
                .await
            {
                Ok(_) => Ok(()),
                Err(e) => Err(HttpResponse::InternalServerError().json(e.to_string())),
            }
        }
        Err(e) => Err(HttpResponse::InternalServerError().json(e.to_string())),
    }
}

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
    data: web::Data<crate::AppState>,
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
    tracking_info_update: PackageDataWebhook,
) -> Result<(), HttpResponse> {
    // convert the webhook_update_accepted_package to tracking_data_database_form
    let tracking_data_database_form = match tracking_info_update.convert_to_tracking_data_dbf() {
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
    let _ = match collection_tracking_data.delete_many(filter, None).await {
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

/// Function to get all users related to the tracking number from the database
// TODO: faf around and find out @$lookup doc joint search actual SQL
async fn get_user_ids_related_to_tracking_number(
    client: web::Data<Client>,
    tracking_number: String,
) -> Result<Vec<i64>, HttpResponse> {
    // set database, collection and filter
    let db = client.database("teletrack");
    let collection_relations: mongodb::Collection<tracking_number_user_relation> =
        db.collection("tracking_number_user_relation");
    let filter_find_hash = doc! {"tracking_number": &tracking_number, "is_subscribed": true};

    // get the result of the search
    let relation_cursor = match collection_relations.find(filter_find_hash, None).await {
        Ok(cursor) => Ok(cursor),
        Err(e) => {
            println!("database error 1: {}", e);
            Err(HttpResponse::InternalServerError().body(e.to_string()))
        }
    }
    .unwrap();
    //

    // convert the result of the search into a vector of user id hashes
    let user_id_hashes: Vec<String> = match relation_cursor
        .try_collect::<Vec<tracking_number_user_relation>>()
        .await
    {
        Ok(relations) => Ok(relations.into_iter().map(|r| r.user_id_hash).collect()),
        Err(e) => {
            println!("database error 2: {}", e);
            Err(HttpResponse::InternalServerError().body(e.to_string()))
        }
    }
    .unwrap();
    //

    // set new collection and filter to look for true user IDs
    let collection_users: mongodb::Collection<user> = db.collection("users");
    let filter_find_id = doc! { "user_id_hash": { "$in": &user_id_hashes } };

    // get the result of the search
    let user_cursor = match collection_users.find(filter_find_id, None).await {
        Ok(cursor) => Ok(cursor),
        Err(e) => {
            println!("database error 3: {}", e);
            Err(HttpResponse::InternalServerError().body(e.to_string()))
        }
    }
    .unwrap();
    //

    // convert the result of the search into a vector of user IDs and return
    match user_cursor.try_collect::<Vec<user>>().await {
        Ok(users) => Ok(users.into_iter().map(|u| u.user_id).collect::<Vec<i64>>()),
        Err(e) => {
            println!("database error 4: {}", e);
            Err(HttpResponse::InternalServerError().body(e.to_string()))
        }
    }
    //
}

/// Function to send notifications to all users from a vector of user ids
async fn send_notifications_to_users(
    data: web::Data<AppState>,
    user_ids: Vec<i64>,
    message: &str,
    tracking_number_that_was_updated: &str,
) -> Vec<(i64, Result<(), HttpResponse>)> {
    futures::stream::iter(user_ids.clone().into_iter().map(|user_id| {
        // one for each C:
        let data = data.clone();
        async move {
            // call the notification function and save the outcome of each one
            let response = notify_of_tracking_event_update(
                data,
                user_id,
                message,
                tracking_number_that_was_updated,
            )
            .await;
            (user_id, response)
        }
    }))
    // run them all in parallel (None = unlimited concurrency)
    .buffer_unordered(user_ids.len())
    .collect()
    .await
}

/*
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
    WEBHOOK

    TODO: handle stopped update

-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
*/

/*
    Since the miniapp is some hot garbage and it doesn't have any sensible way to transport a payload
    through their notification (which would be really nice), we keep cheating them by sending more than
    one parameters just out of spite (send the tracking number that will open the client tracking page
    directly), the way the client will get the tracking data is through a https request
*/

#[post("/webhook_17track")]
pub async fn handle_webhook(
    data: web::Data<crate::AppState>,
    client: web::Data<Client>,
    request: HttpRequest,
    body: web::Bytes,
) -> impl Responder {
    // check the sign to verify it's from the api
    let payload = match verify_origin_body(data.clone(), request.clone(), body.clone()).await {
        Ok(payload) => payload,
        Err(e) => return e,
    };

    // println!("webhook received payload and extracted");
    // print the whole boomboclat thing
    // println!("  {:?}", payload);
    // println!("  {:?}", payload.event);

    /*
        Split to two paths based on the enum value of PackageData
    */
    if let TrackingData::PackageData(package_update) = payload.data {
        // save the update in database in format
        match refresh_tracking_info_from_webhook_update(client.clone(), package_update.clone())
            .await
        {
            Ok(_) => {}
            Err(response) => {
                println!("unknown error trying to refresh database tracking info from update");
                return response;
            }
        };
        //

        // get list of users to notify of the update
        let user_ids_to_notify = match get_user_ids_related_to_tracking_number(
            client.clone(),
            package_update.number.clone(),
        )
        .await
        {
            Ok(user_ids) => Ok(user_ids),
            Err(response) => {
                println!("failed to get user ID from the tracking number of the update");
                Err(response)
            }
        }
        .unwrap();
        //

        if user_ids_to_notify.len() == 0 {
            println!("no user to notify")
        }

        // build the message that will be displayed in the chat window and notification banner
        // dump the description
        let message = "Update on your order tracking: ".to_string()
            + package_update.number.as_str()
            + "\n"
            + package_update
                .track_info
                .latest_event
                .description
                .unwrap()
                .as_str();

        // call the update function on all IDs from the vector
        let notifications_results = send_notifications_to_users(
            data.clone(),
            user_ids_to_notify,
            &message,
            &package_update.number,
        )
        .await;

        // open the results of sending notifications
        for each_result in notifications_results {
            // if there was an error, log it, but don't tell the API
            if let Err(_response) = each_result.1 {
                println!("notification to {} failed", each_result.0);
            } else if let Ok(_) = each_result.1 {
                println!("notification to {} succeeded", each_result.0);
            }
        }
    } else if let TrackingData::TrackingStopped(tracking_stopped) = payload.data {
        println!("tracking stopped for package {}", tracking_stopped.number);
    } else {
        return HttpResponse::BadRequest().finish();
    }

    HttpResponse::Ok().body(format!("{}", payload.event))
}
