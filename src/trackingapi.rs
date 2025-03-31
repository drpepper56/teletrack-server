/*
    Cargo Stuff
*/

use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use reqwest::{Client, Error};
use serde::{Deserialize, Serialize};
use std::env;

/*
    Structs
*/

#[derive(Debug, Serialize, Deserialize)]
pub struct tracking_event {
    pub date: String,
    pub location: String,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct api_response {
    data: Vec<tracking_data>,
}

// API Response Structures
#[derive(Debug, Serialize, Deserialize)]
pub struct tracking_data {
    pub tracking_number: String,
    pub status: String,
    pub events: Vec<tracking_event>,
}

// error messages
#[derive(Debug, thiserror::Error)]
pub enum tracking_error {
    #[error("No tracking data found for your tracking number.")]
    NoDataFound,
    #[error("Request error: {0}")]
    ReqwestError(#[from] reqwest::Error),
}

// client for executing requests to the api
pub struct tracking_client {
    client: Client,
    api_key: String,
    base_url: String,
}

/*
    Functions
*/

impl tracking_client {
    pub fn new() -> Self {
        tracking_client {
            client: Client::new(),
            api_key: env::var("TRACK17_API_KEY")
                .expect("TRACK17_API_KEY must be set in environment"),
            //TODO: Maybe wrong link
            base_url: "https://api.17track.net/track/v2".to_string(),
        }
    }

    /// track 1 package
    pub async fn track_single_package(
        &self,
        tracking_number: &str,
    ) -> Result<tracking_data, tracking_error> {
        // Create the body for the HTTP request since the api doesn't use a web endpoint
        // load the url, route, api key and parameters into the URL and send it
        // unpack and return the response or throw errors
        //TODO: change route
        let url = format!("{}/gettrack", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("17token", &self.api_key)
            .json(&serde_json::json!({
                "tracking_number": tracking_number
            }))
            .send()
            .await?;

        let api_response: api_response = response.json().await?;
        api_response
            .data
            .into_iter()
            .next()
            .ok_or(tracking_error::NoDataFound)
    }

    /*

    track many packages -> // TODO -> deserialize and error handle

        pub async fn track_multiple(
            &self,
            tracking_numbers: Vec<String>,
        ) -> Result<Vec<tracking_data>, Error> {
            let url = format!("{}/gettrack", self.base_url);

            let response = self
                .client
                .post(&url)
                .header("17token", &self.api_key)
                .json(&serde_json::json!({
                    "numbers": tracking_numbers
                }))
                .send()
                .await?;

            let api_response: api_response = response.json().await?;
            Ok(api_response.data)
        }
    */
}
