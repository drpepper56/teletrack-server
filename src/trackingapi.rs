/*
    Cargo Stuff
*/

use reqwest::Client;
use std::env;
use tracking_response_structs::TrackingResponse;

/*
    Structs
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

// structs for parsing the API response
pub mod tracking_response_structs {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    pub struct TrackingResponse {
        pub code: i32,
        pub data: ResponseData,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct ResponseData {
        pub accepted: Option<Vec<AcceptedPackage>>,
        pub rejected: Option<Vec<RejectedPackage>>, // Empty array in the example
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
        // here was the problem that took a whole day
        pub lastGatherTime: String,
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
        pub country: Option<String>,
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

    // Rejected
    #[derive(Debug, Serialize, Deserialize)]
    pub struct RejectedPackage {
        pub number: String,
        pub error: RejectedError,
    }
    #[derive(Debug, Serialize, Deserialize)]
    pub struct RejectedError {
        code: i32,
        message: String,
    }
}

/*

    FUNctions

*/

impl tracking_client {
    /// initializer
    pub fn new() -> Self {
        tracking_client {
            client: Client::new(),
            api_key: env::var("TRACK17_API_KEY")
                .expect("TRACK17_API_KEY must be set in environment"),
            base_url: "https://api.17track.net/track/v2.2".to_string(),
        }
    }

    //TODO: implement other routes such as register and delete
    /// track 1 package
    pub async fn track_single_package(
        &self,
        tracking_number: &str,
    ) -> Result<TrackingResponse, tracking_error> {
        // Create the body for the HTTP request since the api doesn't use a web endpoint
        // load the url, route, api key and parameters into the URL and send it
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

        if !response.status().is_success() {
            println!("Error: {}", response.status());
            return Err(tracking_error::ReqwestError(Err(()).unwrap()));
        }

        // for debugging, print the whole body of the response in the terminal
        let body_bytes = response.bytes().await?;
        /*
            // Convert to string for debugging/printing
            if let Ok(body_str) = String::from_utf8(body_bytes.to_vec()) {
                println!("Response body ({} bytes):\n{}", body_str.len(), body_str);
            }
        */

        // Parse the json of the response into the structures created with the 17track api docs
        // and return the @TrackingResponse instance
        let response_data = serde_json::from_slice::<TrackingResponse>(&body_bytes)?;
        match response_data.code {
            // success
            0 => {
                println!("Success: {:?}", response_data);
                Ok(response_data)
            }
            // error
            1 => {
                println!("{}: {:?}", response_data.code, response_data);
                return Err(tracking_error::SerdeError(Err(()).unwrap()));
            }
            // unexpected error
            _ => Err(tracking_error::NoDataFound),
        }
    }
}
