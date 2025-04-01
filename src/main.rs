/*
    Cargo stuff
*/

mod my_structs;
mod notifications;
mod trackingapi;
//TODO: remove later
mod webhook;

use actix_cors::Cors;
use actix_web::{
    middleware::Logger,
    web::{self, Json},
    App, HttpResponse, HttpServer, Responder,
};
use dotenv::dotenv;
use futures::{stream::StreamExt, TryStreamExt};
use mongodb::{
    bson::doc,
    options::{ClientOptions, FindOptions},
    Client,
};
use notifications::{notification_service, notification_service_error};
use reqwest::Client as ReqwestClient;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{collections, env, result, sync::Arc};
use trackingapi::{tracking_client, tracking_error};

/*
    Structs
    TODO:
*/

/// struct for parameter in the main server thread
struct app_state {
    notification_service: Arc<Result<notification_service, notification_service_error>>,
    tracking_client: Arc<tracking_client>,
    webhook_secret: String,
}

/// struct for testing connections
#[derive(Serialize, Deserialize, Debug)]
struct testing_data_format {
    key: String,
    value: String,
}

/*
    Functions
    TODO: function
*/

async fn write_to_db_test(
    client: web::Data<Client>,
    data: web::Json<testing_data_format>,
) -> impl Responder {
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
                    .allowed_origin_fn(|origin, _req_head| {
                        // Allow heroku front end
                        origin
                            .as_bytes()
                            .starts_with(b"https://teletrack-twa-1b3480c228a6.herokuapp.com")
                    })
                    .allowed_methods(vec!["GET", "POST"])
                    .allowed_headers(vec![
                        actix_web::http::header::CONTENT_TYPE,
                        actix_web::http::header::AUTHORIZATION,
                    ])
                    .supports_credentials()
                    .max_age(3600),
            )
            /*
                ROUTING
            */
            // test the service
            .service(web::resource("/").to(|| async { HttpResponse::Ok().body("Hello, World!") }))
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
            // HTTPS webhook for recieving updates //TODO: add the hash verification when it's time to do security
            .service(webhook::handle_webhook)
    })
    .bind(("127.0.0.1", 8080))?
    // .bind(("0.0.0.0", port))? // Bind to all interfaces and the dynamic port
    .run()
    .await
}
