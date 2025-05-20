/*
    Cargo stuff
*/

mod my_structs;
mod notifications;
mod trackingapi;
//TODO: CHANGE THE WEBHOOK.LEMONCARDBOARD.UK ROOT TO SOMETHING BETTER THAN WEBHOOK (LIKE TELETRACK)
mod webhook;

use crate::{
    my_structs::tracking_data_formats::delete_tracking_number_response::DeleteTrackingResponseNumber as delete_tracking_number_response,
    my_structs::tracking_data_formats::register_tracking_number_response::RegisterResponse as register_tracking_number_response,
    my_structs::tracking_data_formats::retrack_stopped_number_response::RetrackStoppedNumberResponse as retrack_stopped_number_response,
    my_structs::tracking_data_formats::stop_tracking_response::StopTrackingResponse as stop_tracking_response,
    my_structs::tracking_data_formats::tracking_data_database_form::TrackingData_DBF as tracking_data_database_form,
    my_structs::tracking_data_formats::tracking_data_get_info::TrackingResponse as tracking_data_get_info,
    my_structs::tracking_data_formats::tracking_data_html_form::tracking_data_HTML,
    my_structs::tracking_data_formats::tracking_number_meta_data::NumberStatusCheck as number_status_check,
};
use actix_cors::Cors;
use actix_web::{
    middleware::Logger,
    options,
    web::{self, Json},
    App, HttpRequest, HttpResponse, HttpServer, Responder,
};
use dotenv::dotenv;
use futures::{stream::StreamExt, TryStreamExt};
use hex::encode;
use mongodb::{
    bson::doc,
    options::{ClientOptions, FindOptions},
    Client,
};
use notifications::{notification_service, notification_service_error};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{env, string, sync::Arc};
use trackingapi::{just_the_tracking_number, tracking_client, tracking_error};

/*
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

    Constants

-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
*/

// default tracking quota for users
const DEFAULT_TRACKING_QUOTA: i32 = 4;

/*
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
    Structs
    TODO: put in another file
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
*/

/// struct for parameter in the main server thread
struct AppState {
    notification_service: Arc<Result<notification_service, notification_service_error>>,
    tracking_client: Arc<tracking_client>,
    webhook_secret: String,
}

/// User structure for database
#[derive(Debug, Deserialize, Serialize)]
struct UserDatabaseForm {
    user_id: i64,
    user_id_hash: String,
    user_name: String,
    remaining_tracking_quota: i32,
}

/// ERORRS
#[derive(Debug, thiserror::Error)]
pub enum UserCheckError {
    #[error("head invalid")]
    InvalidHeader,
    #[error("no head?")]
    MissingHeader,
    #[error("user not found in database")]
    UserNotFound,
    #[error("database error: {0}")]
    DatabaseError(#[from] mongodb::error::Error),
    #[error("the user you are trying to create already exists")]
    UserAlreadyExists,
}

/// struct for testing connections
#[derive(Serialize, Deserialize, Debug)]
struct TestingDataFormat {
    key: String,
    value: String,
}

// struct for getting user details from the client
#[derive(Serialize, Deserialize, Debug)]
struct UserDetailsFromClient {
    user_id: i64,
    user_name: String,
}

// struct for saving tracking number + carrier (optional) + user id hash as a relation record in the database
// this also holds a bool that decides if the user is getting updates for the number or not
// TODO: redundant with webhook
#[derive(Serialize, Deserialize, Debug)]
pub struct TrackingNumberUserRelation {
    tracking_number: String,
    carrier: Option<i32>,
    user_id_hash: String,
    is_subscribed: bool,
}

/*
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
    API CALLS

-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
*/

/// Function for calling the API to push for info on a single tracking number
async fn pull_tracking_info(
    data: web::Data<AppState>,
    tracking_number: String,
) -> Result<tracking_data_get_info, trackingapi::tracking_error> {
    let tracking_client = data.tracking_client.clone();
    match tracking_client.gettrackinfo_pull(&tracking_number).await {
        Ok(data) => Ok(data),
        Err(e) => Err(e),
    }
}

/// Function for calling the API to register a single tracking number
async fn register_single(
    data: web::Data<AppState>,
    tracking_details: trackingapi::tracking_number_carrier,
) -> Result<register_tracking_number_response, trackingapi::tracking_error> {
    let tracking_client = data.tracking_client.clone();
    match tracking_client.register_tracking(tracking_details).await {
        Ok(data) => Ok(data),
        Err(tracking_error::TrackingNumberNotFoundByAPI) => {
            Err(tracking_error::TrackingNumberNotFoundByAPI)
        }
        Err(e) => Err(e),
    }
}

/// Function for calling the API to stop tracking a number
async fn stop_tracking_single(
    data: web::Data<AppState>,
    tracking_number: String,
) -> Result<stop_tracking_response, trackingapi::tracking_error> {
    let tracking_client = data.tracking_client.clone();
    match tracking_client.stop_tracking(&tracking_number).await {
        Ok(data) => Ok(data),
        Err(e) => Err(e),
    }
}

/// Function for calling the API to retrack a single number
async fn retrack_stopped_number_single(
    data: web::Data<AppState>,
    tracking_number: String,
) -> Result<retrack_stopped_number_response, trackingapi::tracking_error> {
    let tracking_client = data.tracking_client.clone();
    match tracking_client
        .retrack_stopped_number(&tracking_number)
        .await
    {
        Ok(data) => Ok(data),
        Err(e) => Err(e),
    }
}

/// Function for calling the API to delete a single number from the saved numbers (destructive)
async fn delete_number_single(
    data: web::Data<AppState>,
    tracking_number: String,
) -> Result<delete_tracking_number_response, trackingapi::tracking_error> {
    let tracking_client = data.tracking_client.clone();
    match tracking_client.delete_number(&tracking_number).await {
        Ok(data) => Ok(data),
        Err(e) => Err(e),
    }
}

/// Function for calling the API to check the status and other information about a number that is registered
async fn check_number_status_single(
    data: web::Data<AppState>,
    tracking_number: String,
) -> Result<number_status_check, trackingapi::tracking_error> {
    let tracking_client = data.tracking_client.clone();
    match tracking_client.get_number_metadata(&tracking_number).await {
        Ok(data) => Ok(data),
        Err(e) => Err(e),
    }
}

/*
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
    DATABASE FUNCTIONS
    TODO: function
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
*/

/// GET tracking data from tracking number
async fn database_tracking_data_from_number(
    client: web::Data<Client>,
    tracking_number: &str,
) -> tracking_data_database_form {
    // set database, relation and filter for getting tracking data
    let db = client.database("teletrack");
    let collection_tracking_data: mongodb::Collection<tracking_data_database_form> =
        db.collection("tracking_data");
    let filter = doc! {"data.number": &tracking_number};

    // get the tracking data from the database
    let query_result = match collection_tracking_data.find_one(filter, None).await {
        Ok(query_result) => Ok(query_result),
        Err(e) => {
            println!(
                "@get_tracking_data_from_database error, getting tracking data from db: {}",
                e
            );
            Err(HttpResponse::InternalServerError().body(e.to_string()))
        }
    }
    .unwrap();
    //

    // open the result
    match query_result {
        Some(tracking_data) => Ok(tracking_data),
        None => {
            println!("tracking data not found for the client query");
            Err(HttpResponse::InternalServerError().body("no tracking data found for that number"))
        }
    }
    .unwrap()
    //
}

/// GET user ID form user ID hash
async fn database_user_id_from_hash(client: web::Data<Client>, user_id_hash: &str) -> i64 {
    // get user id
    let db = client.database("teletrack");
    let collection_users: mongodb::Collection<UserDatabaseForm> = db.collection("users");
    let filter = doc! {"user_id_hash": &user_id_hash};
    match collection_users.find_one(filter, None).await {
        Ok(Some(user)) => Ok(user.user_id),
        Ok(None) => Err(HttpResponse::InternalServerError().body("user not found")),
        Err(e) => Err(HttpResponse::InternalServerError().body(e.to_string())),
    }
    .unwrap()
    //
}

/// GET delivered bool from tracking number
async fn database_delivered_status(client: web::Data<Client>, tracking_number: &str) -> bool {
    // set database, relation and filter for getting tracking data
    let db = client.database("teletrack");
    let collection_tracking_data: mongodb::Collection<tracking_data_database_form> =
        db.collection("tracking_data");
    let filter = doc! {"data.number": &tracking_number};

    // get the tracking data from the database
    let query_result = match collection_tracking_data.find_one(filter, None).await {
        Ok(query_result) => Ok(query_result),
        Err(e) => {
            println!(
                "@DATABASE_DELIVERED_STATUS: error getting tracking data from db: {}",
                e
            );
            Err(HttpResponse::InternalServerError().body(e.to_string()))
        }
    }
    .unwrap();
    //

    // open the result
    let latest_status = match query_result {
        Some(tracking_data) => &tracking_data.data.track_info.latest_status.status.unwrap(),
        None => {
            println!("tracking data not found for the client query");
            return false;
        }
    };
    //

    if latest_status == "Delivered" {
        return true;
    } else {
        return false;
    }
}

/// GET delivered bool from tracking_data_database_form
fn database_delivered_status_from_DBF(tracking_data_dbf: tracking_data_database_form) -> bool {
    // open the result
    match tracking_data_dbf.data.track_info.latest_status.status {
        Some(latest_status) => {
            if latest_status == "Delivered" {
                return true;
            } else {
                return false;
            }
        }
        None => {
            println!("value not set");
            return false;
        }
    };
    //
}

/// GET remaining tracking quota from user ID hash
async fn database_quota_from_hash(client: web::Data<Client>, user_id_hash: &str) -> i32 {
    // get user id
    let db = client.database("teletrack");
    let collection_users: mongodb::Collection<UserDatabaseForm> = db.collection("users");
    let filter = doc! {"user_id_hash": &user_id_hash};
    match collection_users.find_one(filter, None).await {
        Ok(Some(user)) => Ok(user.remaining_tracking_quota),
        Ok(None) => Err(HttpResponse::InternalServerError().body("user not found")),
        Err(e) => Err(HttpResponse::InternalServerError().body(e.to_string())),
    }
    .unwrap()
    //
}

/// DECREMENT the remaining tracking quota for a user by 1 from user id hash
async fn database_decrement_user_quota(client: web::Data<Client>, user_id_hash: &str) -> bool {
    // set database, relation and filter for getting tracking data
    let db = client.database("teletrack");
    let collection_user_data: mongodb::Collection<UserDatabaseForm> = db.collection("users");
    let filter = doc! {"user_id_hash": &user_id_hash};
    let update = doc! {"$inc": {"remaining_tracking_quota": -1}};

    // get the user data from the database
    let query_result = match collection_user_data.update_one(filter, update, None).await {
        Ok(query_result) => Ok(query_result),
        Err(e) => {
            println!(
                "@DATABASE_DECREMENT_USER_QUOTA: error getting tracking data from db: {}",
                e
            );
            Err(HttpResponse::InternalServerError().body(e.to_string()))
        }
    }
    .unwrap();
    //

    // open the result
    if query_result.modified_count > 0 {
        println!("user quota decremented");
        return true;
    } else {
        println!("user quota not decremented");
        return false;
    }
}

/*
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
    UTILITY FUNCTIONS
    TODO: function
    TODO: get the values for database and collection from one constant instead of writing them in each function
    TODO: move some logic to database functions
    TODO: add function for converting to html format which sets the is_user_tracked value based on latest status then relation record

-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
*/

/// Check if the userID hash exists on the data base, if it doesn't it means the request came from a new user and the server can't send notifications right now
/// respond with a status code 520:'User not found' which the client app should resolve by sending a UserID and Name of the user
/// INCASE the userID hash exists on the database anything to do with saving tracking history or sending addressed notifications should proceed with the raw userID
async fn check_user_exists(
    client: web::Data<Client>,
    request: HttpRequest,
) -> Result<String, HttpResponse> {
    // open the head and try to get the relevant value that should be there
    let user_id_hash = match request.headers().get("X-User-ID-Hash") {
        Some(header) => match header.to_str() {
            Ok(s) => {
                // println!("@CHECK_USER_EXISTS: {}", s);
                Ok(s.to_string())
            }
            Err(_) => {
                println!("invalid header");
                Err(HttpResponse::BadRequest().json(
                    serde_json::json!({"request header error": "missing header X-USER-ID-HASH"}),
                ))
            }
        },
        None => {
            println!("no hhead?");
            Err(HttpResponse::BadRequest().json(serde_json::json!({"header error": "no head?"})))
        }
    }
    .unwrap();

    // chose the right database and collection
    // search for the user id hash and return the actual ID if found, send errors otherwise
    // println!("@CHECK_USER_EXISTS: verifying user now...");
    let db = client.database("teletrack");
    let collection: mongodb::Collection<UserDatabaseForm> = db.collection("users");
    let filter = doc! {"user_id_hash": &user_id_hash};
    match collection.find_one(filter, None).await {
        Ok(Some(user)) => {
            println!("@CHECK_USER_EXISTS: user found: {:?}", user);
            Ok(user.user_id_hash) // Return the user ID hash as hex string
        }
        Ok(None) => {
            println!("@CHECK_USER_EXISTS: user not found");
            Err(HttpResponse::build(
                StatusCode::from_u16(520).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            )
            .json(serde_json::json!({"expected error": "user doesn't exist yet"})))
        }
        Err(e) => {
            eprintln!("@CHECK_USER_EXISTS: mongodb not goated: {}", e);
            Err(HttpResponse::InternalServerError().body(format!("database error: {}", e)))
        }
    }
}

/// Create the user but before check again if the user already exists on the database, double check act as a guard in case this function is ever used in a context
/// where it is not triggered by the predicted interaction
// TODO: add lock so this can't be accessed while another thread is running this function
async fn create_user(
    client: web::Data<Client>,
    user_details: UserDetailsFromClient,
) -> Result<bool, UserCheckError> {
    println!("@CREATE_USER: creating user now...");

    let db = client.database("teletrack");
    let collection: mongodb::Collection<UserDatabaseForm> = db.collection("users");

    // create the user document
    let user = UserDatabaseForm {
        user_id: user_details.user_id.clone(),
        user_id_hash: encode(Sha256::digest(
            user_details.user_id.abs().to_string().as_bytes(),
        )),
        user_name: user_details.user_name,
        remaining_tracking_quota: DEFAULT_TRACKING_QUOTA,
    };

    // check if the user exists already
    let filter = doc! {"user_id_hash": &user.user_id_hash};
    match collection.find_one(filter, None).await {
        Ok(Some(_)) => {
            println!("@CREATE_USER: user already exists");
            Err(UserCheckError::UserAlreadyExists)
        }
        Ok(None) => {
            println!("@CREATE_USER: user doesn't exist yet");
            Ok(())
        }
        Err(e) => {
            eprintln!("@CREATE_USER: database error in @CREATE_USER: {}", e);
            Err(UserCheckError::DatabaseError(e))
        }
    }
    .unwrap();

    // insert the user
    match collection.insert_one(user, None).await {
        Ok(_) => Ok(true),
        Err(e) => Err(UserCheckError::DatabaseError(e)),
    }
}

/// Function to check if the user has a relation to the tracking number in the database
// TODO: TEST and merge ->
async fn check_relation(
    client: web::Data<Client>,
    tracking_number: &str,
    user_id_hash: &str,
) -> Result<(), HttpResponse> {
    // set database
    let db = client.database("teletrack");
    // set collection
    let collection_relations: mongodb::Collection<TrackingNumberUserRelation> =
        db.collection("tracking_number_user_relation");
    // set search filter
    let filter = doc! {"tracking_number": &tracking_number, "user_id_hash": &user_id_hash};
    // find the relation record in the database
    match collection_relations.find_one(filter.clone(), None).await {
        Ok(Some(_)) => Ok(()),
        Ok(None) => {
            println!("@NO_PERMISSION: relation record not found");
            Err(HttpResponse::build(
                StatusCode::from_u16(525).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            ).json(serde_json::json!({"expected error": "user doesn't have access to that tracking number"})))
        }
        Err(e) => {
            println!("database error {}", e);
            Err(HttpResponse::InternalServerError().body(e.to_string()))
        }
    }
    //
}

/// Function to check if the user has a relation to the tracking number in the database and return subscribed status
// TODO: TEST and merge <-
async fn check_relation_and_subscribed_status(
    client: web::Data<Client>,
    tracking_number: &str,
    user_id_hash: &str,
) -> Result<bool, HttpResponse> {
    // set database
    let db = client.database("teletrack");
    // set collection
    let collection_relations: mongodb::Collection<TrackingNumberUserRelation> =
        db.collection("tracking_number_user_relation");
    // set search filter
    let filter = doc! {"tracking_number": &tracking_number, "user_id_hash": &user_id_hash};
    // find the relation record in the database
    match collection_relations.find_one(filter.clone(), None).await {
        Ok(Some(relation_record)) => Ok(relation_record.is_subscribed),
        Ok(None) => {
            println!("@NO_PERMISSION: relation record not found");
            Err(HttpResponse::build(
                StatusCode::from_u16(525).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            ).json(serde_json::json!({"expected error": "user doesn't have access to that tracking number"})))
        }
        Err(e) => {
            println!("database error {}", e);
            Err(HttpResponse::InternalServerError().body(e.to_string()))
        }
    }
    //
}

/// Function to insert a relation record between a user and a tracking number
async fn insert_relation(
    client: web::Data<Client>,
    tracking_number: String,
    user_id_hash: String,
) -> Result<(), HttpResponse> {
    // create the relation record and put it in the database
    let tracking_user_relation: TrackingNumberUserRelation = TrackingNumberUserRelation {
        tracking_number: tracking_number,
        carrier: None,
        user_id_hash: user_id_hash,
        is_subscribed: true,
    };
    // set database
    let db = client.database("teletrack");
    // set collection
    let collection_relations: mongodb::Collection<TrackingNumberUserRelation> =
        db.collection("tracking_number_user_relation");
    // insert the relation
    match collection_relations
        .insert_one(tracking_user_relation, None)
        .await
    {
        Ok(_) => {
            println!("@CREATING_RELATION_RECORD: relation record inserted");
            Ok(())
        }
        Err(e) => {
            println!(
                "@CREATING_RELATION_RECORD: error inserting relation record: {}",
                e
            );
            return Err(HttpResponse::InternalServerError().body(e.to_string()));
        }
    }
    //
}

/// Function to insert the tracking data in database format to the database
async fn refresh_and_return_tracking_data(
    client: web::Data<Client>,
    data: web::Data<AppState>,
    tracking_number: String,
) -> Result<tracking_data_database_form, HttpResponse> {
    // pull the tracking information from the API
    let gettrackinfo_result = match pull_tracking_info(data.clone(), tracking_number.clone()).await
    {
        Ok(tracking_data) => {
            println!("tracking data received");
            Ok(tracking_data)
        }
        Err(tracking_error::InfoNotReady) => {
            println!("@REFRESH_TRACKING_DATA: info not ready, abort");
            return Err(HttpResponse::InternalServerError().body("info not ready"));
        }
        Err(e) => {
            println!(
                "@REFRESH_TRACKING_DATA: error getting the tracking data: {}",
                e
            );
            Err(HttpResponse::InternalServerError().body(e.to_string()))
        }
    }
    .unwrap();
    //

    // convert the tracking_data_get_info to tracking_data_database_form
    let tracking_data_database_form = gettrackinfo_result.convert_to_tracking_data_dbf();
    // set database
    let db = client.database("teletrack");
    // set collection
    let collection_tracking_data: mongodb::Collection<tracking_data_database_form> =
        db.collection("tracking_data");
    let filter = doc! {"data.number": &tracking_number};

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
    }
    .unwrap();
    //

    // insert the fresh tracking info into the database
    match collection_tracking_data
        .insert_one(tracking_data_database_form.clone(), None)
        .await
    {
        Ok(_) => {
            // returning the fresh tracking info to the user
            println!("tracking data inserted");
            Ok(tracking_data_database_form)
        }
        Err(e) => {
            println!(
                "@REGISTER_TRACKING_NUMBER: error inserting tracking data: {}",
                e
            );
            Err(HttpResponse::InternalServerError().body(e.to_string()))
        }
    }
}

// Function for checking if a number is registered and getting some other useful information
async fn check_number_registered(
    data: web::Data<AppState>,
    tracking_number: String,
) -> Result<(), HttpResponse> {
    match check_number_status_single(data.clone(), tracking_number).await {
        Ok(_number_status) => Ok(()),
        Err(e) => {
            println!("error checking if number exists: {}", e);
            Err(HttpResponse::InternalServerError().body(e.to_string()))
        }
    }
}

// Simulate how the webhook does notifications, for single user only
async fn simulate_webhook_notification_one_user(
    client: web::Data<Client>,
    data: web::Data<AppState>,
    user_id: i64,
    tracking_number: &str,
) {
    // get tracking info from database
    let tracking_data = database_tracking_data_from_number(client.clone(), tracking_number).await;
    //

    // convert the tracking data to html format
    let tracking_data_html = tracking_data.convert_to_HTML_form();

    // build the message that will be displayed in the chat window and notification banner
    // dump the description
    let message = "Update on your order tracking: ".to_string()
        + tracking_data_html.tracking_number.as_str()
        + "\n"
        + tracking_data_html
            .latest_event
            .description
            .unwrap()
            .as_str();

    // me ne frega
    let _ =
        webhook::notify_of_tracking_event_update(data.clone(), user_id, &message, tracking_number)
            .await;
}

/*
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
    ROUTING HANDLERS

    TODO: figure out good 5XX error code responses for different types of errors so they can be handled client side with no body

    TODO: make an enum of the custom error codes

    list of custom 5XX codes:
            520 - user doesn't exist yet, client should send request to create user
    TODO:   521 - user already exists, handle error
    TODO:   525 - user doesn't have access to that number, no relation record found
            530 - carrier not found, client should send a register number request that includes a carrier
            533 - package has been marked delivered so it can't be re-tracked
            534 - already set to subscribed
            535 - already set to unsubscribed
            536 - no relation record found to delete
            540 - tracking quota reached limit, sorry
            541 - relation record already exists


-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
*/

/// Function for responding to a client request to create a new user
/// the header has to have the hashed user ID like the other function, and the raw user ID + name in the body of the function as a json
async fn create_user_handler(
    client: web::Data<Client>,
    request: HttpRequest,
    data: Json<UserDetailsFromClient>,
) -> impl Responder {
    // if it aint broke dont fix it
    // again check if user exists already
    match check_user_exists(client.clone(), request).await {
        // user already exists
        Ok(user_id) => {
            println!("user already exists");
            return HttpResponse::build(
                StatusCode::from_u16(521).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            )
            .json(serde_json::json!({"unexpected error": "user already exists"}));
        }

        // user doesn't exist yet
        Err(response) => match create_user(client.clone(), data.into_inner()).await {
            Ok(_) => return HttpResponse::Ok().body("user created"),
            Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
        },
    }
}

/// Function for handling client call to register a tracking number on the API, the number will be tested and if necessary the carrier will have to be provided
/// by the user, the number will be saved with the users hashed ID in a structure like {code, user_id_hashed, package_data}
// TODO: buy something that will be shipped long time (for testing :-)
async fn register_tracking_number(
    client: web::Data<Client>,
    data: web::Data<AppState>,
    tracking_details: web::Json<trackingapi::tracking_number_carrier>,
    request: HttpRequest,
) -> impl Responder {
    // check if user exists
    let user_id_hash = match check_user_exists(client.clone(), request).await {
        // continue
        Ok(user_id) => user_id,
        // user doesn't exist, respond with 520
        Err(response) => return response,
    };
    //

    // check if the user has reached the tracking quota limit
    let user_quota = database_quota_from_hash(client.clone(), &user_id_hash).await;
    if user_quota <= 0 {
        println!("@REGISTER_TRACKING_NUMBER: user has reached the tracking quota limit");
        return HttpResponse::build(
            StatusCode::from_u16(540).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        )
        .json(serde_json::json!({"expected error": "user has reached the tracking quota limit"}));
    }
    //

    // register the tracking number with the API, throws error
    // the bool value is for knowing whether to pull the tracking info to simulate a webhook update for the user
    let was_registered = match register_single(data.clone(), tracking_details.clone()).await {
        // continue
        Ok(_) => false,
        // tracking number was already registered, continue
        Err(tracking_error::TrackingAlreadyRegistered) => {
            println!("@REGISTER_TRACKING_NUMBER: tracking number already registered");
            true // it's okay if it's not registered+stopped on the API
        }
        // tracking number not found by the API
        Err(tracking_error::TrackingNumberNotFoundByAPI) => {
            println!("@REGISTER_TRACKING_NUMBER: tracking number not found");
            return HttpResponse::InternalServerError()
                .body(serde_json::json!({"error":tracking_error::TrackingNumberNotFoundByAPI.to_string()}).to_string());
        }
        // unable to find carrier, try again with specific carrier
        Err(tracking_error::RetryTrackRegisterWithCarrier) => {
            println!("@REGISTER_TRACKING_NUMBER: carrier not found, retry with specific carrier");
            return HttpResponse::build(
                StatusCode::from_u16(530).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            )
            .json(serde_json::json!({"expected error": "retry with carrier"}));
        }
        // unexpected error
        Err(e) => {
            println!("@REGISTER_TRACKING_NUMBER:{}", e);
            return HttpResponse::InternalServerError().body(e.to_string());
        }
    };
    //

    // check if a duplicate of the relation record exists
    let db = client.database("teletrack");
    let collection_relations: mongodb::Collection<TrackingNumberUserRelation> =
        db.collection("tracking_number_user_relation");
    let filter = doc! {"tracking_number": &tracking_details.number, "user_id_hash": &user_id_hash};
    let duplicate_relation_search = collection_relations.find_one(filter.clone(), None).await;
    if let Ok(Some(_)) = duplicate_relation_search {
        println!("relation already exists");
        return HttpResponse::build(
            StatusCode::from_u16(541).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        )
        .json(serde_json::json!({"expected error": "relation record already exists"}));
    } else if let Err(e) = duplicate_relation_search {
        println!("database error: {}", e);
        return HttpResponse::InternalServerError().body(e.to_string());
    }
    //

    // create the relation record and put it in the database
    match insert_relation(
        client.clone(),
        tracking_details.number.clone(),
        user_id_hash.clone(),
    )
    .await
    {
        Ok(_) => {
            println!("relation record inserted");
            ()
        }
        Err(response) => {
            println!("error inserting relation record");
            return response;
        }
    };

    if !was_registered {
        // decrement the user quota
        let _ = database_decrement_user_quota(client.clone(), &user_id_hash).await;
        return HttpResponse::Ok().body("registered tracking number");
    }
    // simulate the webhook update if the tracking number was already registered

    // get user id
    let user_id = database_user_id_from_hash(client.clone(), &user_id_hash).await;

    // forge and send notification
    simulate_webhook_notification_one_user(
        client.clone(),
        data.clone(),
        user_id,
        &tracking_details.number,
    )
    .await;

    return HttpResponse::Ok().body("registered tracking number");
}

/// Function for stopping the tracking of a single number, this will pause the updates sent to the webhook, check if any other user is subscribed to that
/// number on the database before proceeding, update in two stages, turn off notifications then if no one else is linked to that number, untrack it
async fn stop_tracking_number(
    client: web::Data<Client>,
    data: web::Data<AppState>,
    tracking_data: Json<just_the_tracking_number>,
    request: HttpRequest, // user in here
) -> impl Responder {
    // check if user exists
    let user_id_hash = match check_user_exists(client.clone(), request).await {
        // continue
        Ok(user_id) => user_id,
        // user doesn't exist, respond with 520
        Err(response) => return response,
    };
    //

    let tracking_number = tracking_data.into_inner().number.clone();
    // set database, relation and filter
    let db = client.database("teletrack");
    let collection_relations: mongodb::Collection<TrackingNumberUserRelation> =
        db.collection("tracking_number_user_relation");
    let filter = doc! {"tracking_number": &tracking_number, "user_id_hash": &user_id_hash};

    // check if the user has permission for that number
    check_relation(client.clone(), &tracking_number, &user_id_hash)
        .await
        .unwrap();

    // send request to the DB to change the is_subscribed value to false
    let database_update = doc! {"$set":{"is_subscribed": false}};
    let update_result = match collection_relations
        .update_one(filter, database_update, None)
        .await
    {
        Ok(result) => result,
        // database error
        Err(e) => {
            println!("{}", e);
            return HttpResponse::InternalServerError().body(e.to_string());
        }
    };
    //

    // resolve the response from the database, it's a bit weird here
    if update_result.modified_count.clone() > 0 {
        println!("successfully unsubscribed from a number by the user");
        // TODO: check if there are any other subscribed users that are linked to that file before and stop tracking it on the API if not
        // let _ = match stop_tracking_single(data.clone(), tracking_number.clone()).await {
        //     Ok(_) => {
        //         println!("number has been stopped on the API");
        //         Ok(())
        //     }
        //     Err(e) => {
        //         println!("@STOP_TRACKING_NUMBER: error stopping_number: {},", e);
        //         Err(e)
        //     }
        // };
        //

        return HttpResponse::Ok()
            .body("action successful, user won't be notified of updates to this tracking number ");
    } else {
        return HttpResponse::build(
            StatusCode::from_u16(535).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        )
        .json(serde_json::json!({"expected error": "already unsubscribed"}));
    }
}

/// Function for re-tracking a stopped tracking number, will be triggered when a users switches the tracking ON for the number on the client, there
/// will be no check internally if the number is tracked already i made all those errors in the api file for a reason :-)
async fn retrack_stopped_number(
    client: web::Data<Client>,
    data: web::Data<AppState>,
    tracking_data: Json<just_the_tracking_number>,
    request: HttpRequest, // user in here
) -> impl Responder {
    // check if user exists
    let user_id_hash = match check_user_exists(client.clone(), request).await {
        // continue
        Ok(user_id) => user_id,
        // user doesn't exist, respond with 520
        Err(response) => return response,
    };
    //

    let tracking_number = tracking_data.into_inner().number.clone();

    // check if the user has permission for that number
    check_relation(client.clone(), &tracking_number, &user_id_hash)
        .await
        .unwrap();

    // get the data about this number from the API
    let number_status =
        match check_number_status_single(data.clone(), tracking_number.clone()).await {
            Ok(number_status) => number_status,
            Err(e) => {
                println!(
                    "@RETRACK_STOPPED_NUMBER: error getting the number status data: {}",
                    e
                );
                return HttpResponse::InternalServerError().body(e.to_string());
            }
        };
    //

    // get the important tracking and status information to check
    let tracking_status = &number_status.data.accepted[0].tracking_status;
    let package_status = &number_status.data.accepted[0].package_status;

    // if the package has been delivered do not update the subscribe value in the database
    if package_status == "Delivered" {
        println!("the package has been marked delivered and there won't be ant new updates");
        return HttpResponse::build(
            StatusCode::from_u16(533).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        )
        .json(serde_json::json!({"expected error": "delivered packages can't be re-tracked"}));
    }
    //

    // send request to the DB to change the is_subscribed value to true
    let database_update = doc! {"$set":{"is_subscribed": true}};
    let db = client.database("teletrack");
    let collection_relations: mongodb::Collection<TrackingNumberUserRelation> =
        db.collection("tracking_number_user_relation");
    let filter = doc! {"tracking_number": &tracking_number, "user_id_hash": &user_id_hash};
    let update_result = match collection_relations
        .update_one(filter, database_update, None)
        .await
    {
        Ok(result) => result,
        // database error
        Err(e) => {
            println!("{}", e);
            return HttpResponse::InternalServerError().body(e.to_string());
        }
    };
    //

    // activate it if it's stopped and not yet delivered
    if tracking_status == "Stopped" && package_status != "Delivered" {
        match retrack_stopped_number_single(data.clone(), tracking_number).await {
            Ok(_) => {
                println!("number has been re-tracked on the API");
                ()
            }
            Err(e) => {
                println!("@RETRACK_STOPPED_NUMBER: error re-tracking_number: {},", e);
                return HttpResponse::InternalServerError().body("the number you are trying to retrack has been retracked before and cant be retracked again.");
            }
        };
    }
    //

    // resolve the response from the database, return response if user was already subscribed
    if update_result.modified_count.clone() == 0 {
        return HttpResponse::build(
            StatusCode::from_u16(534).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        )
        .json(serde_json::json!({"expected error": "already subscribed"}));
    }
    println!("successfully subscribed to a number by the user");
    //

    HttpResponse::Ok()
        .body("action successful, user will be notified of updates to this tracking number ")
}

/// Function for deleting tracking numbers from the database and from the API, this function's primary function is deleting the user-number record deletion
/// and the secondary function is checking if there are any other users recorded for that number, if not, delete it on the API
async fn delete_tracking_number(
    client: web::Data<Client>,
    data: web::Data<AppState>,
    tracking_data: Json<just_the_tracking_number>,
    request: HttpRequest, // user in here
) -> impl Responder {
    // check if user exists
    let user_id_hash = match check_user_exists(client.clone(), request).await {
        // continue
        Ok(user_id) => user_id,
        // user doesn't exist, respond with 520
        Err(response) => return response,
    };
    //

    let tracking_number = tracking_data.into_inner().number.clone();
    // set database, relation and filter
    let db = client.database("teletrack");
    let collection_relations: mongodb::Collection<TrackingNumberUserRelation> =
        db.collection("tracking_number_user_relation");

    // check if the user has permission for that number
    check_relation(client.clone(), &tracking_number, &user_id_hash)
        .await
        .unwrap();
    //

    // send request to the DB to remove the relation record
    let filter = doc! {"tracking_number": &tracking_number, "user_id_hash": &user_id_hash};
    let update_result = match collection_relations.delete_one(filter, None).await {
        Ok(update_result) => update_result,
        // database error
        Err(e) => {
            println!("{}", e);
            return HttpResponse::InternalServerError().body(e.to_string());
        }
    };
    //

    // resolve the delete response from the database
    if update_result.deleted_count > 0 {
        println!("successfully deleted the relation record from the database");

        // check if there are any other relation docs with that number
        let filter = doc! {"tracking_number": &tracking_number};
        let other_relations_count = match collection_relations.count_documents(filter, None).await {
            Ok(res) => res,
            Err(e) => {
                println!("{}", e);
                return HttpResponse::InternalServerError().body(e.to_string());
            }
        };
        //

        // check the response from the database and delete the number from the API register if there are none
        if other_relations_count == 0 {
            println!("delete from API would happen here but its been disabled for now");
            //TODO: put this back in later
            // let _ = match delete_number_single(data.clone(), tracking_number).await {
            //     Ok(_) => {
            //         println!("number has been deleted on the API");
            //         Ok(())
            //     }
            //     Err(e) => {
            //         println!("@DELETE_TRACKING_NUMBER: error deleting_number: {},", e);
            //         Err(e)
            //     }
            // };
        }

        return HttpResponse::Ok().finish(); // professionalism
    } else {
        // didn't delete
        println!("didn't delete the relation record because nothing was found");
        return HttpResponse::build(
            StatusCode::from_u16(536).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        )
        .json(serde_json::json!({"expected error": "no relation record found to delete"}));
    }
}

/// Function for the client to request the tracking data from the database, this will not call the API, it's going to be called when the user
/// opens the tracking page on the client, be that from the starting screen or from a notification, this is the only method that returns the tracking
/// data to the client because telegram miniapp is ass and doesn't have actual notifications
async fn get_tracking_data_from_database(
    client: web::Data<Client>,                     // for db
    tracking_data: Json<just_the_tracking_number>, // for knowing which number to query
    request: HttpRequest,                          // user in here
) -> impl Responder {
    // check if user exists
    let user_id_hash = match check_user_exists(client.clone(), request).await {
        // continue
        Ok(user_id) => user_id,
        // user doesn't exist, respond with 520
        Err(response) => return response,
    };
    //

    let tracking_number = tracking_data.into_inner().number.clone();

    // check if the user has permission for that number, also checks if the number is registered and gets the subscribed value
    let is_user_tracked =
        check_relation_and_subscribed_status(client.clone(), &tracking_number, &user_id_hash)
            .await
            .unwrap();
    //

    // get the tracking data from database
    let tracking_data = database_tracking_data_from_number(client.clone(), &tracking_number).await;
    //

    // convert the tracking data to the HTML form
    let mut tracking_data_html = tracking_data.convert_to_HTML_form();

    // set the is_user_tracked value
    match database_delivered_status(client, &tracking_number).await {
        true => {
            tracking_data_html.is_user_tracked = Some(false);
            println!("package has been marked delivered");
        }
        false => {
            tracking_data_html.is_user_tracked = Some(is_user_tracked);
            println!("package has not been marked delivered");
        }
    }

    HttpResponse::Ok().body(serde_json::to_string(&tracking_data_html).unwrap())
}

/// Function for responding to a user request for all their tracked numbers' tracking details and events
// TODO: faf around and find out @$lookup doc joint search actual SQL
async fn get_user_tracked_numbers_details(
    client: web::Data<Client>, // for db
    request: HttpRequest,      // user in here
) -> impl Responder {
    // check if user exists
    let user_id_hash = match check_user_exists(client.clone(), request).await {
        // continue
        Ok(user_id) => user_id,
        // user doesn't exist, respond with 520
        Err(response) => return response,
    };
    //

    // set database, relation and filter
    let db = client.database("teletrack");
    let collection_relations: mongodb::Collection<TrackingNumberUserRelation> =
        db.collection("tracking_number_user_relation");
    let filter = doc! {"user_id_hash": &user_id_hash};

    // TODO: faf around and find out @$lookup doc joint search actual SQL

    // get a result of a search for all the user's tracked numbers
    let tracking_numbers_cursor = match collection_relations.find(filter.clone(), None).await {
        Ok(query_result) => query_result,
        Err(e) => {
            println!("@GET_USER_TRACKED_NUMBERS_DETAILS: {}", e);
            return HttpResponse::InternalServerError().body(e.to_string());
        }
    };
    //

    // convert the cursor to a list of tracking numbers, and user subscribed status that the user is tracking
    let user_tracked_numbers_and_status: Vec<(String, bool)> = match tracking_numbers_cursor
        .try_collect::<Vec<TrackingNumberUserRelation>>()
        .await
    {
        Ok(relations) => relations
            .into_iter()
            .map(|r| (r.tracking_number, r.is_subscribed))
            .collect(),
        Err(e) => {
            println!("@GET_USER_TRACKED_NUMBERS_DETAILS: {}", e);
            return HttpResponse::InternalServerError().body(e.to_string());
        }
    };
    //

    // set database, relation and filter for tracking data
    let collection_tracking_data: mongodb::Collection<tracking_data_database_form> =
        db.collection("tracking_data");
    let filter = doc! {"data.number": { "$in":
    &user_tracked_numbers_and_status.iter().map(|(number, _)| number).collect::<Vec<_>>() }};

    // get every tracking numbers' details from the database
    let tracking_data_cursor = match collection_tracking_data.find(filter, None).await {
        Ok(query_result) => query_result,
        Err(e) => {
            println!("@GET_USER_TRACKED_NUMBERS_DETAILS: {}", e);
            return HttpResponse::InternalServerError().body(e.to_string());
        }
    };
    //

    // convert the cursor to a vector of tracking data HTML form
    let user_tracked_numbers_details: Vec<tracking_data_HTML> = match tracking_data_cursor
        .try_collect::<Vec<tracking_data_database_form>>()
        .await
    {
        // convert the tracking data to HTML form and set the is_user_tracked value from the relation record
        Ok(tracking_data_dbf) => tracking_data_dbf
            .into_iter()
            .map(|pkg| {
                let mut html_package_data_form = pkg.convert_to_HTML_form();
                // Check if the user is tracking this number and get subscription status
                let is_user_tracked = match database_delivered_status_from_DBF(pkg.clone()) {
                    // If package is delivered, not tracked regardless of subscription
                    true => Some(false),
                    // If not delivered, check if user is tracking it
                    false => user_tracked_numbers_and_status
                        .iter()
                        .find(|(tracking_num, _)| {
                            *tracking_num == html_package_data_form.tracking_number
                        })
                        .map(|(_, is_subscribed)| *is_subscribed),
                };
                html_package_data_form.is_user_tracked = is_user_tracked;
                html_package_data_form
            })
            .collect(),
        Err(e) => {
            println!("@GET_USER_TRACKED_NUMBERS_DETAILS: {}", e);
            return HttpResponse::InternalServerError().body(e.to_string());
        }
    };
    //

    HttpResponse::Ok().json(user_tracked_numbers_details)
}

/*
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
    PREFLIGHT OPTIONS HANDLERS FOR ROUTING HANDLERS

-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
*/

#[options("/write")]
async fn write_options() -> impl Responder {
    HttpResponse::NoContent()
        .insert_header((
            "Access-Control-Allow-Origin",
            "https://teletrack-twa-1b3480c228a6.herokuapp.com",
        ))
        .insert_header(("Access-Control-Allow-Methods", "POST, OPTIONS"))
        .insert_header((
            "Access-Control-Allow-Headers",
            "Content-Type, X-User-ID-Hash",
        ))
        .finish()
}
#[options("/create_user")]
async fn create_user_options() -> impl Responder {
    HttpResponse::NoContent()
        .insert_header((
            "Access-Control-Allow-Origin",
            "https://teletrack-twa-1b3480c228a6.herokuapp.com",
        ))
        .insert_header(("Access-Control-Allow-Methods", "POST, OPTIONS"))
        .insert_header((
            "Access-Control-Allow-Headers",
            "Content-Type, X-User-ID-Hash",
        ))
        .finish()
}
#[options("/register_tracking_number")]
async fn register_tracking_number_options() -> impl Responder {
    HttpResponse::NoContent()
        .insert_header((
            "Access-Control-Allow-Origin",
            "https://teletrack-twa-1b3480c228a6.herokuapp.com",
        ))
        .insert_header(("Access-Control-Allow-Methods", "POST, OPTIONS"))
        .insert_header((
            "Access-Control-Allow-Headers",
            "Content-Type, X-User-ID-Hash",
        ))
        .finish()
}
#[options("/stop_tracking_number")]
async fn stop_tracking_number_options() -> impl Responder {
    HttpResponse::NoContent()
        .insert_header((
            "Access-Control-Allow-Origin",
            "https://teletrack-twa-1b3480c228a6.herokuapp.com",
        ))
        .insert_header(("Access-Control-Allow-Methods", "POST, OPTIONS"))
        .insert_header((
            "Access-Control-Allow-Headers",
            "Content-Type, X-User-ID-Hash",
        ))
        .finish()
}
#[options("/retrack_stopped_number")]
async fn retrack_stopped_number_options() -> impl Responder {
    HttpResponse::NoContent()
        .insert_header((
            "Access-Control-Allow-Origin",
            "https://teletrack-twa-1b3480c228a6.herokuapp.com",
        ))
        .insert_header(("Access-Control-Allow-Methods", "POST, OPTIONS"))
        .insert_header((
            "Access-Control-Allow-Headers",
            "Content-Type, X-User-ID-Hash",
        ))
        .finish()
}
#[options("/delete_tracking_number")]
async fn delete_tracking_number_options() -> impl Responder {
    HttpResponse::NoContent()
        .insert_header((
            "Access-Control-Allow-Origin",
            "https://teletrack-twa-1b3480c228a6.herokuapp.com",
        ))
        .insert_header(("Access-Control-Allow-Methods", "POST, OPTIONS"))
        .insert_header((
            "Access-Control-Allow-Headers",
            "Content-Type, X-User-ID-Hash",
        ))
        .finish()
}
#[options("/get_tracking_data")]
async fn get_tracking_data_options() -> impl Responder {
    HttpResponse::NoContent()
        .insert_header((
            "Access-Control-Allow-Origin",
            "https://teletrack-twa-1b3480c228a6.herokuapp.com",
        ))
        .insert_header(("Access-Control-Allow-Methods", "POST, OPTIONS"))
        .insert_header((
            "Access-Control-Allow-Headers",
            "Content-Type, X-User-ID-Hash",
        ))
        .finish()
}
#[options("/get_user_tracked_numbers_details")]
async fn get_user_tracked_numbers_details_options() -> impl Responder {
    HttpResponse::NoContent()
        .insert_header((
            "Access-Control-Allow-Origin",
            "https://teletrack-twa-1b3480c228a6.herokuapp.com",
        ))
        .insert_header(("Access-Control-Allow-Methods", "POST, OPTIONS"))
        .insert_header((
            "Access-Control-Allow-Headers",
            "Content-Type, X-User-ID-Hash",
        ))
        .finish()
}

/*
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
    MAiN
    TODO: test only in production
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
*/

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();

    // MONGODB ATLAS SERVICE
    let mongo_uri = env::var("MONGODB_URI").expect("MONGODB_URI not set");
    let mongo_client_options = ClientOptions::parse(&mongo_uri).await.unwrap();
    let mongo_client = Client::with_options(mongo_client_options).unwrap();
    // NOTIFICATION SERVICE
    let notification_service = Arc::new(notification_service::new(
        std::env::var("TELEGRAM_BOT_TOKEN").expect("BOT_TOKEN must be set"),
        "teletrack",
    ));
    // TRACKING SERVICE
    let tracking_client = Arc::new(tracking_client::new());
    // SERVER
    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .expect("PORT must be a number");

    println!("active");

    HttpServer::new(move || {
        App::new()
            /*
                THREAD PARAMETERS
            */
            .app_data(web::Data::new(mongo_client.clone()))
            .app_data(web::Data::new(AppState {
                notification_service: notification_service.clone(),
                tracking_client: tracking_client.clone(),
                webhook_secret: env::var("WEBHOOK_SECRET").expect("WEBHOOK_SECRET must be set"),
            }))
            /*
                CORS
            */
            .wrap(Logger::default())
            .wrap(
                Cors::default()
                    .allowed_origin("https://telegram.org") // Telegram web app origin
                    .allowed_origin("https://webhook.lemoncardboard.uk")
                    .allowed_origin("https://teletrack-twa-1b3480c228a6.herokuapp.com") // Heroku origin
                    .allowed_methods(vec!["GET", "POST", "OPTIONS"])
                    .allowed_headers(vec!["X-User-ID-Hash", "Content-Type", "Authorization"])
                    .expose_headers(vec!["X-User-ID-Hash"])
                    .supports_credentials()
                    .max_age(3600),
            )
            /*
                ROUTING
            */
            // HTTPS webhook for recieving updates //TODO: add the hash verification when it's time to do security
            .service(webhook::handle_webhook)
            // HTTPS receive
            // prod
            .route("/create_user", web::post().to(create_user_handler))
            .route(
                "/register_tracking_number",
                web::post().to(register_tracking_number),
            )
            .route(
                "/stop_tracking_number",
                web::post().to(stop_tracking_number),
            )
            .route(
                "/retrack_stopped_number",
                web::post().to(retrack_stopped_number),
            )
            .route(
                "/delete_tracking_number",
                web::post().to(delete_tracking_number),
            )
            .route(
                "/get_tracking_data",
                web::post().to(get_tracking_data_from_database),
            )
            .route(
                "get_user_tracked_numbers_details",
                web::post().to(get_user_tracked_numbers_details),
            )
            // HTTPS preflight OPTIONS for test_write
            .service(write_options)
            .service(create_user_options)
            .service(register_tracking_number_options)
            .service(stop_tracking_number_options)
            .service(retrack_stopped_number_options)
            .service(delete_tracking_number_options)
            .service(get_tracking_data_options)
            .service(get_user_tracked_numbers_details_options)
    })
    // .bind(("127.0.0.1", 8080))?
    .bind(("0.0.0.0", port))? // bxind to all interfaces and the dynamic port
    .run()
    .await
}
