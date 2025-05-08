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
    my_structs::tracking_data_formats::tracking_number_meta_data::NumberStatusCheck as number_status_check,
};
use actix_cors::Cors;
use actix_web::{
    middleware::Logger,
    options,
    web::{self, Json},
    App, HttpRequest, HttpResponse, HttpServer, Responder,
};
use core::error;
use dotenv::dotenv;
use futures::{stream::StreamExt, stream::TryStreamExt};
use hex::encode;
use mongodb::{
    bson::{de, doc},
    options::{ClientOptions, FindOneAndUpdateOptions, FindOptions},
    Client,
};
use notifications::{notification_service, notification_service_error};
use reqwest::{Client as ReqwestClient, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{collections, env, f32::consts::E, result, sync::Arc};
use trackingapi::{tracking_client, tracking_error};

/*
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
    Structs
    TODO: put in another file
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
*/

/// struct for parameter in the main server thread
struct app_state {
    notification_service: Arc<Result<notification_service, notification_service_error>>,
    tracking_client: Arc<tracking_client>,
    webhook_secret: String,
}

/// User structure
#[derive(Debug, Deserialize, Serialize)]
struct user {
    user_id: i64,
    user_id_hash: String,
    user_name: String,
}

/// ERORRS
#[derive(Debug, thiserror::Error)]
pub enum user_check_error {
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
struct testing_data_format {
    key: String,
    value: String,
}

// struct for getting user details from the client
#[derive(Serialize, Deserialize, Debug)]
struct user_details {
    user_id: i64,
    user_name: String,
}

// just the tracking number
#[derive(Serialize, Deserialize, Debug)]
struct just_the_tracking_number {
    number: String,
}

// struct for saving tracking number + carrier (optional) + user id hash as a relation record in the database
// this also holds a bool that decides if the user is getting updates for the number or not
// TODO: redundant with webhook
#[derive(Serialize, Deserialize, Debug)]
pub struct tracking_number_user_relation {
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
    data: web::Data<app_state>,
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
    data: web::Data<app_state>,
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
    data: web::Data<app_state>,
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
    data: web::Data<app_state>,
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
    data: web::Data<app_state>,
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
    data: web::Data<app_state>,
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
    UTILITY FUNCTIONS
    TODO: function
    TODO: get the values for database and collection from one constant instead of writing them in each function
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
                s.to_string()
            }
            Err(_) => {
                println!("invalid header");
                return Err(HttpResponse::BadRequest().json(
                    serde_json::json!({"request header error": "missing header X-USER-ID-HASH"}),
                ));
            }
        },
        None => {
            println!("no hhead?");
            return Err(
                HttpResponse::BadRequest().json(serde_json::json!({"header error": "no head?"}))
            );
        }
    };

    // chose the right database and collection
    // search for the user id hash and return the actual ID if found, send errors otherwise
    // println!("@CHECK_USER_EXISTS: verifying user now...");
    let db = client.database("teletrack");
    let collection: mongodb::Collection<user> = db.collection("users");
    let filter = doc! {"user_id_hash": user_id_hash};
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
    user_details: user_details,
) -> Result<bool, user_check_error> {
    println!("@CREATE_USER: creating user now...");

    let db = client.database("teletrack");
    let collection: mongodb::Collection<user> = db.collection("users");

    // create the user document
    let user = user {
        user_id: user_details.user_id.clone(),
        user_id_hash: encode(Sha256::digest(
            user_details.user_id.abs().to_string().as_bytes(),
        )),
        user_name: user_details.user_name,
    };

    // check if the user exists already
    let filter = doc! {"user_id_hash": &user.user_id_hash};
    let duplicate_present = collection.find_one(filter, None).await;
    match duplicate_present {
        Ok(Some(_)) => {
            println!("@CREATE_USER: user already exists");
            return Err(user_check_error::UserAlreadyExists);
        }
        Err(e) => {
            eprintln!("@CREATE_USER: database error in @CREATE_USER: {}", e);
            return Err(user_check_error::DatabaseError(e));
        }
        Ok(None) => {
            println!("@CREATE_USER: user doesn't exist yet");
        }
    }

    // insert the user
    match collection.insert_one(user, None).await {
        Ok(_) => Ok(true),
        Err(e) => Err(user_check_error::DatabaseError(e)),
    }
}

/// Function to check if the user has a relation to the tracking number in the database
async fn check_relation(client: web::Data<Client>, tracking_number: &str, user_id_hash: &str) {
    // set database
    let db = client.database("teletrack");
    // set collection
    let collection_relations: mongodb::Collection<tracking_number_user_relation> =
        db.collection("tracking_number_user_relation");
    // set search filter
    let filter = doc! {"tracking_number": &tracking_number, "user_id_hash": &user_id_hash};
    // find the relation record in the database
    let permission_check = collection_relations.find_one(filter.clone(), None).await;
    if let Ok(Some(relation_record)) = permission_check {
        println!(
            "@PERMISSION_GRANTED: relation record found, subscribed: {}",
            relation_record.is_subscribed
        );
    } else if let Ok(None) = permission_check {
        println!("@NO_PERMISSION: relation record not found");
        HttpResponse::InternalServerError()
            .body("user doesn't have access to that tracking number.");
    } else if let Err(e) = permission_check {
        println!("database error {}", e);
        HttpResponse::InternalServerError().body(e.to_string());
    } else {
        HttpResponse::InternalServerError().body("error when checking permissions");
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
    let tracking_user_relation: tracking_number_user_relation = tracking_number_user_relation {
        tracking_number: tracking_number,
        carrier: None,
        user_id_hash: user_id_hash,
        is_subscribed: true,
    };
    // set database
    let db = client.database("teletrack");
    // set collection
    let collection_relations: mongodb::Collection<tracking_number_user_relation> =
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
    data: web::Data<app_state>,
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
    };
    //

    // convert the tracking_data_get_info to tracking_data_database_form
    let tracking_data_database_form = gettrackinfo_result.unwrap().convert_to_TrackingData_DBF();
    // set database
    let db = client.database("teletrack");
    // set collection
    let collection_tracking_data: mongodb::Collection<tracking_data_database_form> =
        db.collection("tracking_data");
    let filter = doc! {"data.number": &tracking_number};

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
    let insert_tracking_data_result = match collection_tracking_data
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
    };
    insert_tracking_data_result
}

/*
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
    ROUTING HANDLERS

    every function should check the hashed user ID and check if the user has permissions to do things on that number
    TODO: figure out good 5XX error code responses for different types of errors so they can be handled client side with no body

-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
*/

/// testing: write to db
async fn write_to_db_test(
    client: web::Data<Client>,
    data: web::Json<testing_data_format>,
    request: HttpRequest,
) -> impl Responder {
    // check if user exists
    match check_user_exists(client.clone(), request).await {
        // user exists, continue
        Ok(user_id) => {
            // start connection to DB
            let db = client.database("testbase");
            let collection: mongodb::Collection<testing_data_format> = db.collection("test");
            let result = collection.insert_one(data.into_inner(), None).await;

            match result {
                Ok(_) => HttpResponse::Ok().body("write good"),
                Err(e) => {
                    eprintln!("write bad: {:?}", e);
                    HttpResponse::InternalServerError().body("Failed to store data")
                }
            }
        }
        // user doesn't exist, respond with 520
        Err(response) => response,
    }
}

/// testing: reading from the database
async fn test_read(client: web::Data<Client>, data: Json<testing_data_format>) -> impl Responder {
    println!("reading from DB");

    let db = client.database("testbase");
    let collection: mongodb::Collection<testing_data_format> = db.collection("test");

    // let filter: mongodb::bson::Document = doc! {};
    let filter: mongodb::bson::Document = doc! {"key": data.into_inner().key}; // Corrected filter
    let find_options = FindOptions::builder().limit(2).build();

    let cursor = collection.find(filter, find_options).await;

    match cursor {
        Ok(mut cursor) => {
            let mut results = Vec::new();
            println!("mayb ey ou got it?");
            while let Some(doc) = cursor.next().await {
                match doc {
                    Ok(data) => {
                        println!("{:?}", data);
                        results.push(data);
                    }
                    Err(e) => {
                        println!("Error: {:?}", e);
                        return HttpResponse::InternalServerError()
                            .body("Failed to parse document");
                    }
                }
            }
            return HttpResponse::Ok().json(results);
        }
        Err(e) => {
            println!("{}", e);
            HttpResponse::InternalServerError().body("Failed to read data")
        }
    }
}

/// Function for responding to a client request to create a new user
/// the header has to have the hashed user ID like the other function, and the raw user ID + name in the body of the function as a json
async fn create_user_handler(
    client: web::Data<Client>,
    request: HttpRequest,
    data: Json<user_details>,
) -> impl Responder {
    // again check if user exists already
    match check_user_exists(client.clone(), request).await {
        // user already exists
        Ok(user_id) => {
            println!("user already exists");
            HttpResponse::build(
                StatusCode::from_u16(521).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            )
            .json(serde_json::json!({"unexpected error": "user already exists"}))
        }

        // user doesn't exist yet
        Err(response) => match create_user(client.clone(), data.into_inner()).await {
            Ok(_) => HttpResponse::Ok().body("user created"),
            Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
        },
    }
}

/// Function for handling client call to register a tracking number on the API, the number will be tested and if necessary the carrier will have to be provided
/// by the user, the number will be saved with the users hashed ID in a structure like {code, user_id_hashed, package_data}
// TODO: buy something that will be shipped long time (for testing :-)
// TODO: add carrier search in a catch clause
async fn register_tracking_number(
    client: web::Data<Client>,
    data: web::Data<app_state>,
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

    // register the tracking number with the API
    let _ = match register_single(data.clone(), tracking_details.clone()).await {
        // continue
        Ok(_) => Ok(()),
        // tracking number was already registered, continue
        Err(tracking_error::TrackingAlreadyRegistered) => {
            println!("@REGISTER_TRACKING_NUMBER: tracking number already registered");
            Ok(()) // it's okay if it's not registered+stopped on the API
        }
        // tracking number not found by the API
        Err(tracking_error::TrackingNumberNotFoundByAPI) => {
            // TODO: add carrier search in a catch clause
            println!("@REGISTER_TRACKING_NUMBER: tracking number not found");
            Err(HttpResponse::InternalServerError()
                .body(serde_json::json!({"error":"tracking number not found"}).to_string()))
        }
        // unexpected error
        Err(e) => {
            println!("@REGISTER_TRACKING_NUMBER:{}", e);
            Err(HttpResponse::InternalServerError().body(e.to_string()))
        }
    };
    //

    // check if a duplicate of the relation record exists
    let db = client.database("teletrack");
    let collection_relations: mongodb::Collection<tracking_number_user_relation> =
        db.collection("tracking_number_user_relation");
    let filter = doc! {"tracking_number": &tracking_details.number, "user_id_hash": &user_id_hash};
    let duplicate_relation_search = collection_relations.find_one(filter.clone(), None).await;
    if let Ok(Some(_)) = duplicate_relation_search {
        println!("relation already exists");
        return HttpResponse::Ok().body("OK");
    } else if let Err(e) = duplicate_relation_search {
        println!("database error: {}", e);
        return HttpResponse::InternalServerError().body(e.to_string());
    }
    //

    // create the relation record and put it in the database
    match insert_relation(
        client.clone(),
        tracking_details.number.clone(),
        user_id_hash,
    )
    .await
    {
        Ok(_) => {
            println!("relation record inserted");
            Ok(HttpResponse::Ok().body("relation record inserted"))
        }
        Err(response) => {
            println!("error inserting relation record");
            Err(response)
        }
    }
    .unwrap()
}

/// Function for stopping the tracking of a single number, this will pause the updates sent to the webhook, check if any other user is subscribed to that
/// number on the database before proceeding, update in two stages, turn off notifications then if no one else is linked to that number, untrack it
async fn stop_tracking_number(
    client: web::Data<Client>,
    data: web::Data<app_state>,
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
    let collection_relations: mongodb::Collection<tracking_number_user_relation> =
        db.collection("tracking_number_user_relation");
    let filter = doc! {"tracking_number": &tracking_number, "user_id_hash": &user_id_hash};

    // check if the user has permission for that number
    check_relation(client.clone(), &tracking_number, &user_id_hash).await;

    // send request to the DB to change the is_subscribed value to false
    let database_update = doc! {"$set":{"is_subscribed": false}};
    let update_result = match collection_relations
        .update_one(filter, database_update, None)
        .await
    {
        Ok(result) => Ok(result),
        // database error
        Err(e) => {
            println!("{}", e);
            Err(HttpResponse::InternalServerError().body(e.to_string()))
        }
    };
    //

    // resolve the response from the database, it's a bit weird here
    if update_result.unwrap().modified_count.clone() > 0 {
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

        HttpResponse::Ok()
            .body("action successful, user won't be notified of updates to this tracking number ")
    } else {
        // not found (impossible)
        println!("found but not changed, was already set to false");
        HttpResponse::InternalServerError().body("already not subscribed to that number")
    }
}

/// Function for re-tracking a stopped tracking number, will be triggered when a users switches the tracking ON for the number on the client, there
/// will be no check internally if the number is tracked already i made all those errors in the api file for a reason :-)
async fn retrack_stopped_number(
    client: web::Data<Client>,
    data: web::Data<app_state>,
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
    check_relation(client.clone(), &tracking_number, &user_id_hash).await;

    // get the data about this number from the API
    let number_status =
        match check_number_status_single(data.clone(), tracking_number.clone()).await {
            Ok(number_status) => Ok(number_status),
            Err(e) => {
                println!(
                    "@RETRACK_STOPPED_NUMBER: error getting the number status data: {}",
                    e
                );
                Err(e)
            }
        };
    //

    // get the important tracking and status information to check
    let tracking_status = &number_status.as_ref().unwrap().data.accepted[0].tracking_status;
    let package_status = &number_status.as_ref().unwrap().data.accepted[0].package_status;

    // if the package has been delivered do not update the subscribe value in the database
    if package_status == "Delivered" {
        println!("the package has been marked delivered and there won't be ant new updates");
        return HttpResponse::Ok()
            .body("number cannot be subscribed to because the package has been marked as delivered and there won't be new updates");
    }
    //

    // send request to the DB to change the is_subscribed value to true
    let database_update = doc! {"$set":{"is_subscribed": true}};
    let db = client.database("teletrack");
    let collection_relations: mongodb::Collection<tracking_number_user_relation> =
        db.collection("tracking_number_user_relation");
    let filter = doc! {"tracking_number": &tracking_number, "user_id_hash": &user_id_hash};
    let update_result = match collection_relations
        .update_one(filter, database_update, None)
        .await
    {
        Ok(result) => Ok(result),
        // database error
        Err(e) => {
            println!("{}", e);
            Err(HttpResponse::InternalServerError().body(e.to_string()))
        }
    };
    //

    // resolve the response from the database, return response if user was already subscribed
    if update_result.unwrap().modified_count.clone() == 0 {
        println!("found but not changed, was already set to true");
        return HttpResponse::InternalServerError().body("already subscribed to that number");
    }
    println!("successfully subscribed to a number by the user");
    //

    // activate it if it's stopped and not yet delivered
    if tracking_status == "Stopped" && package_status != "Delivered" {
        let _ = match retrack_stopped_number_single(data.clone(), tracking_number).await {
            Ok(_) => {
                println!("number has been re-tracked on the API");
                Ok(())
            }
            Err(e) => {
                println!("@RETRACK_STOPPED_NUMBER: error re-tracking_number: {},", e);
                Err(HttpResponse::InternalServerError().body("the number you are trying to retrack has been retracked before and cant be retracked again."))
            }
        };
    }
    //

    HttpResponse::Ok()
        .body("action successful, user will be notified of updates to this tracking number ")
}

/// Function for deleting tracking numbers from the database and from the API, this function's primary function is deleting the user-number record deletion
/// and the secondary function is checking if there are any other users recorded for that number, if not, delete it on the API
async fn delete_tracking_number(
    client: web::Data<Client>,
    data: web::Data<app_state>,
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
    let collection_relations: mongodb::Collection<tracking_number_user_relation> =
        db.collection("tracking_number_user_relation");

    // check if the user has permission for that number
    check_relation(client.clone(), &tracking_number, &user_id_hash).await;
    //

    // send request to the DB to remove the relation record
    let filter = doc! {"tracking_number": &tracking_number, "user_id_hash": &user_id_hash};
    let update_result = match collection_relations.delete_one(filter, None).await {
        Ok(update_result) => Ok(update_result),
        // database error
        Err(e) => {
            println!("{}", e);
            Err(HttpResponse::InternalServerError().body(e.to_string()))
        }
    };
    //

    // resolve the delete response from the database
    if update_result.unwrap().deleted_count.clone() > 0 {
        println!("successfully deleted the relation record from the database");

        // check if there are any other relation docs with that number
        let filter = doc! {"tracking_number": &tracking_number};
        let other_relations_count = match collection_relations.count_documents(filter, None).await {
            Ok(res) => Ok(res),
            Err(e) => {
                println!("{}", e);
                Err(HttpResponse::InternalServerError().body(e.to_string()))
            }
        };
        //

        // check the response from the database and delete the number from the API register if there are none
        if other_relations_count.unwrap() == 0 {
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

        HttpResponse::Ok().finish() // professionalism
    } else {
        // didn't delete
        println!("didn't delete the relation record because nothing was found");
        HttpResponse::InternalServerError().body("didn't delete")
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

    // check if the user has permission for that number
    check_relation(client.clone(), &tracking_number, &user_id_hash).await;
    //

    // set database, relation and filter
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
    let tracking_data = match query_result {
        Some(tracking_data) => Ok(tracking_data),
        None => {
            println!("tracking data not found for the client query");
            Err(HttpResponse::InternalServerError().body("no tracking data found for that number"))
        }
    }
    .unwrap();
    //

    // convert the tracking data to the HTML form
    let tracking_data_html = tracking_data.convert_to_HTML_form();

    HttpResponse::Ok().body(serde_json::to_string(&tracking_data_html).unwrap())
    //
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
            .app_data(web::Data::new(app_state {
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
            // test the service
            .service(web::resource("/").to(|| async { HttpResponse::Ok().body("Hello, World!") }))
            // HTTPS webhook for recieving updates //TODO: add the hash verification when it's time to do security
            .service(webhook::handle_webhook)
            // HTTPS receive
            // testing
            .route("/write", web::post().to(write_to_db_test))
            .route("/test_read", web::get().to(test_read))
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
            // HTTPS preflight OPTIONS for test_write
            .service(write_options)
            .service(create_user_options)
            .service(register_tracking_number_options)
            .service(stop_tracking_number_options)
            .service(retrack_stopped_number_options)
            .service(delete_tracking_number_options)
            .service(get_tracking_data_options)
    })
    // .bind(("127.0.0.1", 8080))?
    .bind(("0.0.0.0", port))? // bxind to all interfaces and the dynamic port
    .run()
    .await
}
