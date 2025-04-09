/*
    Cargo stuff
*/

mod my_structs;
mod notifications;
mod trackingapi;
//TODO: CHANGE THE WEBHOOK.LEMONCARDBOARD.UK ROOT TO SOMETHING BETTER THAN WEBHOOK (LIKE TELETRACK)
mod webhook;

use crate::{
    my_structs::tracking_data_formats::register_tracking_number_response::RegisterResponse,
    my_structs::tracking_data_formats::tracking_data_database_form::TrackingData_DBF as tracking_data_database_form,
    my_structs::tracking_data_formats::tracking_data_get_info::TrackingResponse as tracking_data_get_info,
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
    bson::doc,
    options::{ClientOptions, FindOptions},
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
    user_id: String,
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
    user_id: String,
    user_name: String,
}

// struct for saving tracking number + carrier (optional) + user id hash as a relation record in the database
#[derive(Serialize, Deserialize, Debug)]
struct tracking_number_user_relation {
    tracking_number: String,
    carrier: Option<i32>,
    user_id_hash: String,
}

/*
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
    UTILITY FUNCTIONS
    TODO: function
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
                println!("@CHECK_USER_EXISTS:{}", s);
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
    println!("@CHECK_USER_EXISTS: verifying user now...");
    let db = client.database("teletrack");
    let collection: mongodb::Collection<user> = db.collection("users");
    let filter = doc! {"user_id_hash": user_id_hash};
    match collection.find_one(filter, None).await {
        Ok(Some(user)) => {
            println!("@CHECK_USER_EXISTS: user found: {:?}", user);
            Ok(user.user_id) // Return the user ID as hex string
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
        user_id_hash: encode(Sha256::digest(user_details.user_id.as_bytes())),
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
    let result = collection.insert_one(user, None).await;

    match result {
        Ok(_) => Ok(true),
        Err(e) => Err(user_check_error::DatabaseError(e)),
    }
}

/*
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
    API CALLS

-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
*/

/// Function for calling the API to push for info on a single tracking number
async fn track_single(
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
) -> Result<RegisterResponse, trackingapi::tracking_error> {
    let tracking_client = data.tracking_client.clone();
    match tracking_client.register_tracking(tracking_details).await {
        Ok(data) => Ok(data),
        Err(e) => Err(e),
    }
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
    ROUTING HANDLERS

-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
*/

/// Function for writing to the database, later convert this to a function that can resolve many different type of writes like '.../write/update' or '.../write/register_number'
/// Implement authenticating user with the utility functions
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

/// Function for reading from the database
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
///TODO: make the nesting possible to look at
async fn register_tracking_number(
    client: web::Data<Client>,
    data: web::Data<app_state>,
    tracking_details: web::Path<trackingapi::tracking_number_carrier>,
    request: HttpRequest,
) -> impl Responder {
    // check if user exists
    match check_user_exists(client.clone(), request).await {
        // user exists, continue
        Ok(user_id_hash) => {
            let tracking_details_copy = tracking_details.clone();

            // registered the tracking number with the API successfully
            match register_single(data.clone(), tracking_details_copy).await {
                Ok(register_response) => {
                    println!("tracking number registered");

                    // create the relation record
                    let tracking_user_relation = tracking_number_user_relation {
                        tracking_number: tracking_details.tracking_number.clone(),
                        carrier: Some(register_response.data.accepted.unwrap()[0].carrier.clone()),
                        user_id_hash: user_id_hash.clone(),
                    };
                    // put it in the database
                    let db = client.database("teletrack");
                    let collection_relations: mongodb::Collection<tracking_number_user_relation> =
                        db.collection("tracking_number_user_relation");
                    let insert_result = collection_relations
                        .insert_one(tracking_user_relation, None)
                        .await;

                    match insert_result {
                        // relation record inserted,
                        // proceed to pull the tracking information now and push it to the tracking_data collection
                        Ok(_) => {
                            println!("relation record inserted");
                            let gettrackinfo_result = track_single(
                                data.clone(),
                                tracking_details.clone().tracking_number,
                            )
                            .await;

                            match gettrackinfo_result {
                                // first tracking data retrieved successfully
                                // the relation collection holds this tracking info's tracking number in relation
                                // to the appropriate user code so save it loosely in the tracking_data collection
                                Ok(tracking_data) => {
                                    println!("tracking data received");

                                    // convert the tracking_data_get_info to tracking_data_database_form and save it
                                    let tracking_data_database_form =
                                        tracking_data.convert_to_TrackingData_DBF();
                                    let collection_tracking_data: mongodb::Collection<
                                        tracking_data_database_form,
                                    > = db.collection("tracking_data");
                                    let insert_tracking_data = collection_tracking_data
                                        .insert_one(tracking_data_database_form.clone(), None)
                                        .await;

                                    match insert_tracking_data {
                                        // first tracking data saved successfully
                                        // return something relevant to the client
                                        Ok(_) => {
                                            println!("tracking data inserted");
                                            // return relevant tracking information to the user
                                            // TODO: figure out what is relevant
                                            HttpResponse::Ok().json(tracking_data_database_form)
                                        }
                                        Err(e) => {
                                            println!("{}", e);
                                            HttpResponse::InternalServerError().body(e.to_string())
                                        }
                                    }
                                }
                                Err(e) => {
                                    println!("{}", e);
                                    HttpResponse::InternalServerError().body(e.to_string())
                                }
                            }
                        }
                        Err(e) => {
                            println!("error inserting relation record: {}", e);
                            HttpResponse::InternalServerError().body(e.to_string())
                        }
                    }
                }
                Err(e) => match e {
                    // tracking information for the number not found
                    tracking_error::NoDataFound => {
                        // TODO: add carrier search in a catch clause
                        println!(
                            "@REGISTER_TRACKING_NUMBER: tracking number not found => {}",
                            e
                        );
                        HttpResponse::InternalServerError().body(e.to_string())
                    }
                    e => {
                        println!("@REGISTER_TRACKING_NUMBER:{}", e);
                        HttpResponse::InternalServerError().body(e.to_string())
                    }
                },
            }
        }
        // user doesn't exist, respond with 520
        Err(response) => response,
    }
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
                    ////TODO: Add routing through one function that resolves the user
            */
            // test the service
            .service(web::resource("/").to(|| async { HttpResponse::Ok().body("Hello, World!") }))
            // HTTPS webhook for recieving updates //TODO: add the hash verification when it's time to do security
            .service(webhook::handle_webhook)
            // HTTPS receive
            .route("/write", web::post().to(write_to_db_test))
            .route("/test_read", web::get().to(test_read))
            .route("/create_user", web::post().to(create_user_handler))
            .route(
                "register_tracking_number",
                web::post().to(register_tracking_number),
            )
            // HTTPS trigger notification TESTING //TODO: route to be removed and function called by api update event
            .route(
                "/notify/{user_id}",
                web::post().to(notify_of_tracking_event_update),
            )
            // HTTPS preflight OPTIONS for test_write
            .service(write_options)
            .service(create_user_options)
            .service(register_tracking_number_options)
    })
    // .bind(("127.0.0.1", 8080))?
    .bind(("0.0.0.0", port))? // bxind to all interfaces and the dynamic port
    .run()
    .await
}
