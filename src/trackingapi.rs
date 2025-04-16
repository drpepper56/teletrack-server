/*
    Cargo Stuff
*/

// TODO: mind which format is imported
use crate::{
    my_structs::tracking_data_formats::register_tracking_number_response::RegisterResponse,
    my_structs::tracking_data_formats::tracking_data_get_info::TrackingResponse as tracking_data_get_info,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;

/*
    Structs
*/

// error messages
#[derive(Debug, thiserror::Error)]
pub enum tracking_error {
    #[error("No tracking data found for your tracking number.")]
    NoDataFound,
    #[error("Problem with api, or invalid data form sent")]
    UnexpectedAPIerror,
    #[error("tracking rejected")]
    TrackingRejected,
    #[error("the number you are trying to register is already registered")]
    TrackingAlreadyRegistered,
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

// struct for getting a tracking number + carrier (optional) from client
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct tracking_number_carrier {
    pub number: String,
    pub carrier: Option<i32>,
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

    /// Register one tracking number
    pub async fn register_tracking(
        &self,
        tracking_details: tracking_number_carrier,
    ) -> Result<RegisterResponse, tracking_error> {
        // Create the body for the HTTP request since the api doesn't use a web endpoint
        // load the url, @ROUTE, api key and parameters into the URL and send it
        let url = format!("{}/register", self.base_url);

        println!("{}", serde_json::json!([&tracking_details]));

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("17token", &self.api_key)
            .json(&serde_json::json!([tracking_details]))
            .send()
            .await?;

        if !response.status().is_success() {
            println!("Error: {}", response.status());
            return Err(tracking_error::ReqwestError(Err(()).unwrap()));
        }

        let body_bytes = &response.bytes().await?;
        // match String::from_utf8(body_bytes.to_vec()) {
        //     Ok(text) => println!("Response text: {}", text),
        //     Err(e) => println!("Response is not valid UTF-8: {:?}", e),
        // }

        // Parse the json of the response into the structures created with the 17track api docs
        // and return the @TrackingResponse instance
        let response_data = serde_json::from_slice::<RegisterResponse>(&body_bytes)?;
        match response_data.code {
            // success
            // TODO: deal with the consequences of your actions
            0 => {
                // Even though it's an array treat it always like only one tracking number has been passed,
                // the array is just an API thing, it takes up to 40 numbers at once but here only one is always passed (in parameters)
                if Some(response_data.data.accepted.len()) == Some(1) {
                    // tracking was accepted, added successfully
                    println!("Success: {:?}", response_data.data.accepted[0]);
                    Ok(response_data)
                } else if Some(response_data.data.rejected.len()) == Some(1) {
                    // tracking rejected, limit reached or already registered
                    match response_data.data.rejected[0].error.code {
                        -18019901 => {
                            // already registered
                            // Here it's up to the program to decide if you it wants to allow multiple users to be tracking the same number
                            // the server will allow more than one users tracking one number (eg. sender and receiver)
                            println!("already tracked: {:?}", response_data.data.rejected[0]);
                            return Err(tracking_error::TrackingAlreadyRegistered);
                            // resolve the error in references
                        }
                        _ => {
                            println!("Rejected: {:?}", response_data.data.rejected[0]);
                            return Err(tracking_error::TrackingRejected);
                        }
                    }
                } else {
                    Err(tracking_error::UnexpectedAPIerror)
                }
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

    /// Pull tracking information for one tracking number
    pub async fn gettrackinfo_pull(
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

        // Parse the json of the response into the structures created with the 17track api docs
        // and return the @TrackingResponse instance
        let response_data = serde_json::from_slice::<tracking_data_get_info>(&body_bytes)?;
        match response_data.code {
            // success
            0 => {
                println!("Success: {:?}", response_data);
                Ok(response_data)
                // TODO: deal with rejected info in general and CHANGE RETURN TYPE to the database format
                // TODO: deal with code -18019909[delivered ages ago]
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

    //TODO: implement delete tracking
    //TODO: implement stop tracking
}
