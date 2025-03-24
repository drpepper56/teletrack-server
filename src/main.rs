use actix_web::{
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
use reqwest::Client as ReqwestClient;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{collections, env, result};

#[derive(Serialize, Deserialize, Debug)]
struct TrackingData {
    tracking_number: String,
    status: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Track17Response {
    code: String,
    dat: Option<Vec<Track17Data>>,
    msg: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Track17Data {
    e: String,
    n: String,
    c: String,
    no: String,
    status: String,
    track: Vec<Track17Track>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Track17Track {
    a: String,
    c: String,
    d: String,
    z: String,
}

// test
#[derive(Serialize, Deserialize, Debug)]
struct testing_data_format {
    key: String,
    value: String,
}

// async fn track_package(tracking_number: web::Path<String>) -> impl Responder {
//     let api_key = env::var("TRACK17_API_KEY").expect("TRACK17_API_KEY not set");
//     let url = "https://api.17track.net/track/v1/track";

//     let client = ReqwestClient::new();
//     let response = client
//         .post(url)
//         .header("17token", api_key)
//         .header("Content-Type", "application/json")
//         .json(&json!({
//             "number": tracking_number.into_inner()
//         }))
//         .send()
//         .await;

//     match response {
//         Ok(res) => {
//             if res.status().is_success() {
//                 let body = res.text().await.unwrap_or_else(|_| String::from(""));
//                 println!("Response Body: {}", body);

//                 match serde_json::from_str::<Track17Response>(&body) {
//                     Ok(track17_response) => {
//                         if track17_response.code == "200" {
//                             if let Some(data) = track17_response.dat {
//                                 if let Some(first_data) = data.first() {
//                                     let tracking_data = TrackingData {
//                                         tracking_number: first_data.no.clone(),
//                                         status: first_data.status.clone(),
//                                     };
//                                     return HttpResponse::Ok().json(tracking_data);
//                                 } else {
//                                     return HttpResponse::NotFound().body("No tracking data found");
//                                 }
//                             } else {
//                                 return HttpResponse::NotFound().body("No tracking data found");
//                             }
//                         } else {
//                             return HttpResponse::InternalServerError()
//                                 .body(format!("Track17 API error: {}", track17_response.msg));
//                         }
//                     }
//                     Err(e) => {
//                         eprintln!("Error parsing Track17 response: {:?}", e);
//                         return HttpResponse::InternalServerError()
//                             .body("Failed to parse tracking data");
//                     }
//                 }
//             } else {
//                 HttpResponse::InternalServerError().body(format!(
//                     "Failed to fetch tracking data. Status: {}",
//                     res.status()
//                 ))
//             }
//         }
//         Err(e) => {
//             eprintln!("Error sending request: {:?}", e);
//             HttpResponse::InternalServerError().body("Failed to fetch tracking data")
//         }
//     }
// }

async fn store_tracking_data(
    client: web::Data<Client>,
    data: web::Json<TrackingData>,
) -> impl Responder {
    let db = client.database("tracking_db");
    let collection = db.collection("tracking_data");
    let result = collection.insert_one(data.into_inner(), None).await;

    match result {
        Ok(_) => HttpResponse::Ok().body("Data stored successfully"),
        Err(e) => {
            eprintln!("Error storing data: {:?}", e);
            HttpResponse::InternalServerError().body("Failed to store data")
        }
    }
}

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

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();

    let mongo_uri = env::var("MONGODB_URI").expect("MONGODB_URI not set");
    let client_options = ClientOptions::parse(&mongo_uri).await.unwrap();
    let client = Client::with_options(client_options).unwrap();

    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .expect("PORT must be a number");

    println!("active");

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(client.clone()))
            .service(web::resource("/").to(|| async { HttpResponse::Ok().body("Hello, World!") }))
            .route("/write", web::post().to(write_to_db_test))
            .route("/store_tracking_data", web::post().to(store_tracking_data))
            .route("/test_read", web::get().to(test_read))
    })
    // .bind(("127.0.0.1", 8080))?
    .bind(("0.0.0.0", port))? // Bind to all interfaces and the dynamic port
    .run()
    .await
}
