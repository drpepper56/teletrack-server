/*
    Cargo stuff
*/

mod my_structs;
mod notifications;
mod trackingapi;
//TODO: CHANGE THE WEBHOOK.LEMONCARDBOARD.UK ROOT TO SOMETHING BETTER THAN WEBHOOK (LIKE TELETRACK)
mod webhook;

use actix_cors::Cors;
use actix_web::{
    middleware::Logger,
    options,
    web::{self, Json},
    App, HttpRequest, HttpResponse, HttpServer, Responder,
};
use dotenv::dotenv;
use futures::{stream::StreamExt, stream::TryStreamExt};
use mongodb::{
    bson::doc,
    options::{ClientOptions, FindOptions},
    Client,
};
use notifications::{notification_service, notification_service_error};
use reqwest::{Client as ReqwestClient, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{collections, env, result, sync::Arc};
use trackingapi::{tracking_client, tracking_error};

/*
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
    Structs
    TODO:

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

/*
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
    Functions
    TODO: function
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
*/

/// Check if the userID hash exists on the data base, if it doesn't it means the request came from a new user and the server can't send notifications right now
/// respond with a status code 520:'User not found' which the client app should resolve by sending a UserID and Name of the user, the server will then send a silent
/// notification to confirm with the app that the user was created and then process the original request
/// INCASE the userID hash exists on the database anything to do with saving tracking history or sending addressed notifications should proceed with the raw userID
async fn check_user_exists(
    client: web::Data<Client>,
    request: HttpRequest,
) -> Result<String, user_check_error> {
    // open the head and try to get the relevant value that should be there
    let user_id_hash = match request.headers().get("X-User-ID-Hash") {
        Some(header) => match header.to_str() {
            Ok(s) => {
                println!("{}", s);
                s.to_string()
            }
            Err(_) => {
                println!("invalid header");
                return Err(user_check_error::InvalidHeader);
            }
        },
        None => {
            println!("no hhead?");
            return Err(user_check_error::MissingHeader);
        }
    };

    println!("verifying user now...");
    // chose the right database and collection
    let db = client.database("teletrack");
    let collection: mongodb::Collection<user> = db.collection("users");

    // search for the userid hash and return the actual ID if found, send errors otherwise
    let filter = doc! {"user_id_hash": user_id_hash};
    match collection.find_one(filter, None).await {
        Ok(Some(user)) => {
            println!("user found: {:?}", user);
            Ok(user.user_id) // Return the user ID as hex string
        }
        Ok(None) => {
            println!("user not found");
            Err(user_check_error::UserNotFound)
        }
        Err(e) => {
            eprintln!("mongodb not goated: {}", e);
            Err(user_check_error::DatabaseError(e))
        }
    }
}

// async fn create_user(
//     client: web::Data<Client>,
//     user_id: String,
//     user_name: String,
// ) -> impl Responder {
//      println!("creating user now...");
// }

async fn write_to_db_test(
    client: web::Data<Client>,
    data: web::Json<testing_data_format>,
    request: HttpRequest,
) -> impl Responder {
    // check if user exists
    match check_user_exists(client.clone(), request).await {
        // user exists, continue
        Ok(user_id) => {
            //TODO: we got him
            println!("{}", user_id);

            println!("writing to DB");

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
        Err(user_check_error::UserNotFound) => {
            // println!("user not found");
            HttpResponse::build(
                StatusCode::from_u16(520).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            )
            .body("user doesn't yet exist")
        }
        // some other error
        Err(e) => {
            println!("errur: {}", e);
            HttpResponse::InternalServerError().body(e.to_string())
        }
    }
}

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

async fn track_single(
    data: web::Data<app_state>,
    tracking_number: web::Path<String>,
) -> impl Responder {
    let tracking_client = data.tracking_client.clone();
    //TODO: implement logic for notifying the right user of the update on their package
    match tracking_client
        .track_single_package(&tracking_number.into_inner())
        .await
    {
        Ok(data) => HttpResponse::Ok().json(data),
        Err(e) => {
            eprintln!("Error tracking package: {}", e);
            if e.to_string().contains("No tracking data found") {
                HttpResponse::NotFound().body("No tracking data found")
            } else {
                HttpResponse::InternalServerError().body("Request error")
            }
        }
    }
}

/*
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
    PREFLIGHT OPTIONS HANDLERS

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

/*
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
    MAiN

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
            // HTTPS trigger notification TESTING //TODO: route to be removed and function called by api update event
            .route(
                "/notify/{user_id}",
                web::post().to(notify_of_tracking_event_update),
            )
            // HTTPS send request to tracking API //TODO: route to be removed and function called a user request
            .route("/track_one/{tracking_number}", web::get().to(track_single))
            // HTTPS preflight OPTIONS for test_write
            .service(write_options)
    })
    // .bind(("127.0.0.1", 8080))?
    .bind(("0.0.0.0", port))? // Bxind to all interfaces and the dynamic port
    .run()
    .await
}
