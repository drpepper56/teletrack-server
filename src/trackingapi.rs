/*
    Cargo Stuff
*/

// TODO: mind which format is imported
use crate::my_structs::tracking_data_formats::tracking_data_get_info::TrackingResponse as tracking_data_get_info;
use reqwest::Client;
use std::env;

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
    ) -> Result<tracking_data_get_info, tracking_error> {
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
        let response_data = serde_json::from_slice::<tracking_data_get_info>(&body_bytes)?;
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
