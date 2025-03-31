/*
    Cargo Stuff
*/

use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use anyhow::{Context, Result};
use bytes::Bytes;
use reqwest::{Client, Error};
use serde::{Deserialize, Serialize};
use std::env;

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
}

// client for executing requests to the api
pub struct tracking_client {
    client: Client,
    api_key: String,
    base_url: String,
}

/*




*/

#[derive(Debug, Serialize, Deserialize)]
pub struct TrackingResponse {
    pub code: i32,
    pub data: TrackingData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TrackingData {
    pub accepted: Vec<AcceptedPackage>,
    pub rejected: Vec<RejectedPackage>,
}

// Accepted package structure
#[derive(Debug, Serialize, Deserialize)]
pub struct AcceptedPackage {
    pub number: String,
    pub carrier: i32,
    pub param: Option<String>,
    pub tag: Option<String>,
    pub track_info: TrackInfo,
}

// Track info structure
#[derive(Debug, Serialize, Deserialize)]
pub struct TrackInfo {
    pub shipping_info: ShippingInfo,
    pub latest_status: StatusInfo,
    pub latest_event: Event,
    pub time_metrics: TimeMetrics,
    pub milestone: Vec<Milestone>,
    pub misc_info: MiscInfo,
    pub tracking: TrackingDetails,
}

// Shipping information
#[derive(Debug, Serialize, Deserialize)]
pub struct ShippingInfo {
    pub shipper_address: Address,
    pub recipient_address: Address,
}

// Address structure
#[derive(Debug, Serialize, Deserialize)]
pub struct Address {
    pub country: Option<String>,
    pub state: Option<String>,
    pub city: Option<String>,
    pub street: Option<String>,
    pub postal_code: Option<String>,
    pub coordinates: Coordinates,
}

// Coordinates
#[derive(Debug, Serialize, Deserialize)]
pub struct Coordinates {
    pub longitude: Option<String>,
    pub latitude: Option<String>,
}

// Status information
#[derive(Debug, Serialize, Deserialize)]
pub struct StatusInfo {
    pub status: String,
    pub sub_status: String,
    pub sub_status_descr: Option<String>,
}

// Event information
#[derive(Debug, Serialize, Deserialize)]
pub struct Event {
    pub time_iso: Option<String>,
    pub time_utc: Option<String>,
    pub time_raw: TimeRaw,
    pub description: String,
    pub description_translation: DescriptionTranslation,
    pub location: Option<String>,
    pub stage: String,
    pub sub_status: String,
    pub address: Address,
}

// Time raw format
#[derive(Debug, Serialize, Deserialize)]
pub struct TimeRaw {
    pub date: Option<String>,
    pub time: Option<String>,
    pub timezone: Option<String>,
}

// Description translation
#[derive(Debug, Serialize, Deserialize)]
pub struct DescriptionTranslation {
    pub lang: String,
    pub description: String,
}

// Time metrics
#[derive(Debug, Serialize, Deserialize)]
pub struct TimeMetrics {
    pub days_after_order: i32,
    pub days_of_transit: i32,
    pub days_of_transit_done: i32,
    pub days_after_last_update: i32,
    pub estimated_delivery_date: EstimatedDelivery,
}

// Estimated delivery
#[derive(Debug, Serialize, Deserialize)]
pub struct EstimatedDelivery {
    pub source: String,
    pub from: String,
    pub to: String,
}

// Milestone
#[derive(Debug, Serialize, Deserialize)]
pub struct Milestone {
    pub key_stage: String,
    pub time_iso: Option<String>,
    pub time_utc: Option<String>,
    pub time_raw: TimeRaw,
}

// Miscellaneous info
#[derive(Debug, Serialize, Deserialize)]
pub struct MiscInfo {
    pub risk_factor: i32,
    pub service_type: String,
    pub weight_raw: String,
    pub weight_kg: String,
    pub pieces: String,
    pub dimensions: String,
    pub customer_number: String,
    pub reference_number: Option<String>,
    pub local_number: String,
    pub local_provider: String,
    pub local_key: i32,
}

// Tracking details
#[derive(Debug, Serialize, Deserialize)]
pub struct TrackingDetails {
    pub providers_hash: i32,
    pub providers: Vec<ProviderInfo>,
}

// Provider information
#[derive(Debug, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub provider: Provider,
    pub service_type: String,
    pub latest_sync_status: String,
    pub latest_sync_time: String,
    pub events_hash: i32,
    pub events: Vec<Event>,
}

// Provider
#[derive(Debug, Serialize, Deserialize)]
pub struct Provider {
    pub key: i32,
    pub name: String,
    pub alias: String,
    pub tel: Option<String>,
    pub homepage: Option<String>,
    pub country: Option<String>,
}

// Rejected package
#[derive(Debug, Serialize, Deserialize)]
pub struct RejectedPackage {
    pub number: String,
    pub error: PackageError,
}

// Package error
#[derive(Debug, Serialize, Deserialize)]
pub struct PackageError {
    pub code: i32,
    pub message: String,
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
            .header("17token", &self.api_key)
            .json(&serde_json::json!({
                "number": tracking_number
            }))
            .send()
            .await?;

        // Clone the response body into a Bytes object
        let body_bytes: Bytes = response.bytes().await?;

        // Print the entire body to the terminal
        println!("Response body:\n{}", String::from_utf8_lossy(&body_bytes));

        // Parse the body as JSON
        let response_data: TrackingResponse =
            serde_json::from_slice(&body_bytes).context("Failed to parse response")?;

        Ok(response_data)
        // literally hallucinated how the api response structure looks like
        // TODO: consult api docs on response format

        // let response_data = response
        //     .json::<TrackingResponse>()
        //     .await
        //     .context("Failed to parse response")?;

        // Ok(response_data)
    }
}
