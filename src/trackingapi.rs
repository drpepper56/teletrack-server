/*
    Cargo Stuff
*/

use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use anyhow::{Context, Result};
use bytes::Bytes;
use reqwest::{Client, Error};
use serde::{Deserialize, Serialize};
use std::env;
use tracking_response_structs::TrackingResponse;

/*
    Structs
*/

// source: made up
/*
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
*/

// error messages
#[derive(Debug, thiserror::Error)]
pub enum tracking_error {
    #[error("No tracking data found for your tracking number.")]
    NoDataFound,
    #[error("Request error: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("Serde error: {0}")]
    SerdeError(#[from] serde_json::Error),
}

// client for executing requests to the api
pub struct tracking_client {
    client: Client,
    api_key: String,
    base_url: String,
}

/*




*/

pub mod tracking_response_structs {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    pub struct TrackingResponse {
        pub code: i32,
        pub data: ResponseData,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct ResponseData {
        pub accepted: Vec<AcceptedPackage>,
        pub rejected: Vec<()>, // Empty array in the example
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct AcceptedPackage {
        pub number: String,
        pub carrier: i32,
        pub param: Option<()>,
        pub tag: String,
        pub track_info: TrackInfo,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct TrackInfo {
        pub last_gather_time: String, // Consider using chrono::DateTime if you need to work with dates
        pub shipping_info: ShippingInfo,
        pub latest_status: Status,
        pub latest_event: Event,
        pub time_metrics: TimeMetrics,
        pub milestone: Vec<Milestone>,
        pub misc_info: MiscInfo,
        pub tracking: TrackingDetails,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct ShippingInfo {
        pub shipper_address: Address,
        pub recipient_address: Address,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Address {
        pub country: String,
        pub state: Option<String>,
        pub city: Option<String>,
        pub street: Option<String>,
        pub postal_code: Option<String>,
        pub coordinates: Coordinates,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Coordinates {
        pub longitude: Option<f64>,
        pub latitude: Option<f64>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Status {
        pub status: String,
        pub sub_status: String,
        pub sub_status_descr: Option<String>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Event {
        pub time_iso: String,
        pub time_utc: String,
        pub time_raw: TimeRaw,
        pub description: String,
        pub location: String,
        pub stage: Option<String>,
        pub sub_status: String,
        pub address: Address,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct TimeRaw {
        pub date: Option<String>,
        pub time: Option<String>,
        pub timezone: Option<String>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct TimeMetrics {
        pub days_after_order: i32,
        pub days_of_transit: i32,
        pub days_of_transit_done: i32,
        pub days_after_last_update: i32,
        pub estimated_delivery_date: DeliveryEstimate,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct DeliveryEstimate {
        pub source: Option<String>,
        pub from: Option<String>,
        pub to: Option<String>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Milestone {
        pub key_stage: String,
        pub time_iso: Option<String>,
        pub time_utc: Option<String>,
        pub time_raw: TimeRaw,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct MiscInfo {
        pub risk_factor: i32,
        pub service_type: Option<String>,
        pub weight_raw: Option<String>,
        pub weight_kg: Option<f64>,
        pub pieces: Option<i32>,
        pub dimensions: Option<String>,
        pub customer_number: Option<String>,
        pub reference_number: Option<String>,
        pub local_number: Option<String>,
        pub local_provider: Option<String>,
        pub local_key: i32,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct TrackingDetails {
        pub providers_hash: i32,
        pub providers: Vec<Provider>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Provider {
        pub provider: CarrierInfo,
        pub provider_lang: Option<String>,
        pub service_type: Option<String>,
        pub latest_sync_status: String,
        pub latest_sync_time: String,
        pub events_hash: i32,
        pub events: Vec<Event>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct CarrierInfo {
        pub key: i32,
        pub name: String,
        pub alias: String,
        pub tel: String,
        pub homepage: String,
        pub country: String,
    }
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
            base_url: "https://api.17track.net/track/v2.2".to_string(),
        }
    }

    /// track 1 package
    pub async fn track_single_package(&self, tracking_number: &str) -> Result<TrackingResponse> {
        // Create the body for the HTTP request since the api doesn't use a web endpoint
        // load the url, route, api key and parameters into the URL and send it
        // unpack and return the response or throw errors
        //TODO: implement other routes such as register and delete
        let url = format!("{}/gettrackinfo", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("17token", &self.api_key)
            .json(&serde_json::json!([{
                "number": tracking_number
            }]))
            .send()
            .await?;

        // literally hallucinated how the api response structure looks like
        // TODO: consult api docs on response format

        let response_data = response
            .json::<TrackingResponse>()
            .await
            .context("Failed to parse response")?;

        Ok(response_data)
    }
}
