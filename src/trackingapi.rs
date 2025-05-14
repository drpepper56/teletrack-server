/*
    Cargo Stuff
*/

// TODO: mind which format is imported
use crate::{
    my_structs::tracking_data_formats::delete_tracking_number_response::DeleteTrackingResponseNumber as delete_tracking_number_response,
    my_structs::tracking_data_formats::register_tracking_number_response::RegisterResponse as register_tracking_number_response,
    my_structs::tracking_data_formats::retrack_stopped_number_response::RetrackStoppedNumberResponse as retrack_stopped_number_response,
    my_structs::tracking_data_formats::stop_tracking_response::StopTrackingResponse as stop_tracking_response,
    my_structs::tracking_data_formats::tracking_data_get_info::TrackingResponse as tracking_data_get_info,
    my_structs::tracking_data_formats::tracking_number_meta_data::NumberStatusCheck as number_status_check,
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
    #[error("unexpected error happened, check logs or run more verbose.")]
    UnexpectedError,
    #[error("API couldn't find the API number, retry with carrier number.")]
    TrackingNumberNotFoundByAPI,
    #[error("invalid register tracking data format sent to the API")]
    InvalidRegisterDataFormat,
    #[error("the number is for tracking a package that is marked as delivered, no new information will come and the number cannot be retracked")]
    AlreadyDelivered,
    #[error("the number you are trying to query may not ready yet, try again later or wait of the info to come trough the WEBHOOK")]
    InfoNotReady,
    #[error("tracking rejected")]
    GetTrackInfoError,
    #[error("failed to fetch the tracking info from the api for your number")]
    TrackingRejected,
    #[error("number can't be re-tracked since it's being actively tracked")]
    ReTrackRejectedAlreadyTracked,
    #[error("the number you are trying to stop tracking is already stopped or it doesn't exist")]
    ReTrackRejectedAlreadyRetrackedBefore,
    #[error("error trying to re-track a number")]
    RetrackError,
    #[error("error trying to stop tracking a number")]
    TrackingStopError,
    #[error("the number you are trying to delete is not registered")]
    NumberNotFound,
    #[error("the number you are trying to register is already registered")]
    TrackingAlreadyRegistered,
    #[error("Request error: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("Serde error: {0}")]
    SerdeError(#[from] serde_json::Error),
    #[error(
        "database error, maybe wrong format used for registering user-tracking number record: {0}"
    )]
    DatabaseError(#[from] mongodb::error::Error),
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
// just the tracking number
#[derive(Serialize, Deserialize, Debug)]
pub struct just_the_tracking_number {
    pub number: String,
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
    ) -> Result<register_tracking_number_response, tracking_error> {
        // Create the body for the HTTP request since the api doesn't use a web endpoint
        // load the url, @ROUTE, api key and parameters into the URL and send it
        let url = format!("{}/register", self.base_url);

        // println!("{}", serde_json::json!([&tracking_details]));
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
        // print
        match String::from_utf8(body_bytes.to_vec()) {
            Ok(text) => println!("Response text: {}", text),
            Err(e) => println!("Response is not valid UTF-8: {:?}", e),
        }
        println!("{:?}", tracking_details);

        // Parse the json of the response into the structures created with the 17track api docs
        // and return the @register_tracking_number_response instance
        let response_data =
            serde_json::from_slice::<register_tracking_number_response>(&body_bytes)?;
        match response_data.code {
            // success
            0 => {
                // Even though it's an array treat it always like only one tracking number has been passed,
                // the array is just an API thing, it takes up to 40 numbers at once but here only one is always passed (in parameters)
                if Some(response_data.data.accepted.len()) == Some(1) {
                    // tracking was accepted, added successfully, array has something
                    println!(
                        "number register success: {:?}",
                        response_data.data.accepted[0]
                    );
                    Ok(response_data)
                } else if Some(response_data.data.rejected.len()) == Some(1) {
                    // tracking rejected, limit reached or already registered
                    match response_data.data.rejected[0].error.code {
                        -18019901 => {
                            // already registered
                            // Here it's up to the program to decide if you it wants to allow multiple users to be tracking the same number
                            // the server will allow more than one users tracking one number (eg. sender and receiver)
                            println!("number is already tracked on the API");
                            return Err(tracking_error::TrackingAlreadyRegistered);
                            // resolve the error in references
                        }
                        -18010013 => {
                            // invalid data format sent to the API
                            println!("invalid data format sent to the API");
                            return Err(tracking_error::InvalidRegisterDataFormat);
                        }
                        _ => {
                            println!(
                                "number register error: {:?}",
                                response_data.data.rejected[0]
                            );
                            return Err(tracking_error::TrackingRejected);
                        }
                    }
                } else {
                    Err(tracking_error::UnexpectedError)
                }
            }
            // error
            1 => {
                println!("{}: {:?}", response_data.code, response_data);
                return Err(tracking_error::SerdeError(Err(()).unwrap()));
            }
            // unexpected error
            _ => Err(tracking_error::UnexpectedError),
        }
    }

    /// Pull tracking information for one tracking number, works only after a number has been registered
    // TODO: force retrack when getting a response that the number has been stopped, then either re send the request or make it come via the webhook with a tag to know what it's for
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
        let body_bytes = &response.bytes().await?;
        // match String::from_utf8(body_bytes.to_vec()) {
        //     Ok(text) => println!("Response text: {}", text),
        //     Err(e) => println!("Response is not valid UTF-8: {:?}", e),
        // }

        // Parse the json of the response into the structures created with the 17track api docs
        // and return the @tracking_data_get_info instance
        let response_data = serde_json::from_slice::<tracking_data_get_info>(&body_bytes)?;

        // throw error if the API request wasn't processed
        if response_data.code == 1 {
            println!("CODE 1 FROM API: {:?}", response_data);
            return Err(tracking_error::UnexpectedError);
        }

        if response_data.data.accepted.len() == 1 {
            // get track accepted, return the info
            Ok(response_data)
        } else {
            // get track info rejected, process the error
            match response_data.data.rejected[0].error.code {
                -18019909 => {
                    println!("@GET_TRACKING_INFO: no tracking info at this time...");
                    //
                    // try to set the number to tracked
                    match self.retrack_stopped_number(tracking_number).await {
                        Ok(_result) => {
                            // request for the track info again, recursive
                            Box::pin(self.gettrackinfo_pull(tracking_number))
                                .await
                                .map_err(|_| tracking_error::GetTrackInfoError)
                        }
                        Err(tracking_error::ReTrackRejectedAlreadyTracked) => {
                            println!(
                                "@GET_TRACKING_INFO: number is tracked but not ready on the API"
                            );
                            // TODO: add async function to retry in a few minutes
                            Err(tracking_error::InfoNotReady)
                        }
                        Err(tracking_error::ReTrackRejectedAlreadyRetrackedBefore) => {
                            // TODO: add tracking_error::AlreadyDelivered if it helps
                            println!("@GET_TRACKING_INFO: number is dead, stopped and retracked once before");
                            Err(tracking_error::ReTrackRejectedAlreadyRetrackedBefore)
                        }
                        Err(e) => Err(e),
                    }
                }
                _ => {
                    println!(
                        "get track info unexpected error: {:?}",
                        response_data.data.rejected[0]
                    );
                    return Err(tracking_error::GetTrackInfoError);
                }
            }
        }
    }

    /// Stop tracking one number, pure API, consider multi user number in the main function that calls this
    pub async fn stop_tracking(
        &self,
        tracking_number: &str,
    ) -> Result<stop_tracking_response, tracking_error> {
        // Create the body for the HTTP request since the api doesn't use a web endpoint
        // load the url, @ROUTE, api key and parameters into the URL and send it
        let url = format!("{}/stoptrack", self.base_url);

        // println!("{}", serde_json::json!([&tracking_details]));

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

        let body_bytes = &response.bytes().await?;
        // test print whole body
        // match String::from_utf8(body_bytes.to_vec()) {
        //     Ok(text) => println!("Response text: {}", text),
        //     Err(e) => println!("Response is not valid UTF-8: {:?}", e),
        // }

        // Parse the json of the response into the structures created with the 17track api docs
        // and return the @stop_tracking_response instance
        let response_data = serde_json::from_slice::<stop_tracking_response>(&body_bytes)?;
        match response_data.code {
            // success
            0 => {
                // Even though it's an array treat it always like only one tracking number has been passed,
                // the array is just an API thing, it takes up to 40 numbers at once but here only one is always passed (in parameters)
                if Some(response_data.data.accepted.len()) == Some(1) {
                    println!(
                        "tracking stop success: {:?}",
                        response_data.data.accepted[0]
                    );
                    Ok(response_data)
                } else if Some(response_data.data.rejected.len()) == Some(1) {
                    match response_data.data.rejected[0].error.code {
                        // see what errors happen here and add something to handle specifically if necessary
                        -18019906 => {
                            println!("tracking stop rejected: number is not being tracked so it can't be untracked");
                            return Err(tracking_error::TrackingStopError);
                        }
                        _ => {
                            println!(
                                "tracking stop rejected: {:?}",
                                response_data.data.rejected[0]
                            );
                            return Err(tracking_error::TrackingStopError);
                        }
                    }
                } else {
                    Err(tracking_error::UnexpectedError)
                }
            }
            // error
            1 => {
                println!("{}: {:?}", response_data.code, response_data);
                return Err(tracking_error::SerdeError(Err(()).unwrap()));
            }
            // unexpected error
            _ => Err(tracking_error::UnexpectedError),
        }
    }

    /// Start tracking again a number that is registered but inactive (30 day passed or stopped trough api call)
    pub async fn retrack_stopped_number(
        &self,
        tracking_number: &str,
    ) -> Result<retrack_stopped_number_response, tracking_error> {
        // Create the body for the HTTP request since the api doesn't use a web endpoint
        // load the url, @ROUTE, api key and parameters into the URL and send it
        let url = format!("{}/retrack", self.base_url);

        // println!("{}", serde_json::json!([&tracking_details]));

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

        let body_bytes = &response.bytes().await?;
        // test print whole body
        // match String::from_utf8(body_bytes.to_vec()) {
        //     Ok(text) => println!("Response text: {}", text),
        //     Err(e) => println!("Response is not valid UTF-8: {:?}", e),
        // }

        // Parse the json of the response into the structures created with the 17track api docs
        // and return the @stop_tracking_response instance
        let response_data = serde_json::from_slice::<retrack_stopped_number_response>(&body_bytes)?;
        match response_data.code {
            // success
            0 => {
                // Even though it's an array treat it always like only one tracking number has been passed,
                // the array is just an API thing, it takes up to 40 numbers at once but here only one is always passed (in parameters)
                if Some(response_data.data.accepted.len()) == Some(1) {
                    println!("re-tracking success: {:?}", response_data.data.accepted[0]);
                    Ok(response_data)
                } else if Some(response_data.data.rejected.len()) == Some(1) {
                    match response_data.data.rejected[0].error.code {
                        -18019904 => {
                            println!("re-tracking error: only allowed to retrack stopped numbers");
                            return Err(tracking_error::ReTrackRejectedAlreadyTracked);
                        }
                        -18019905 => {
                            println!("re-tracking error: number was re-tracked before, not allowed to repeat that");
                            return Err(tracking_error::ReTrackRejectedAlreadyRetrackedBefore);
                            // TODO: re-add the number :)
                        }
                        _ => {
                            println!(
                                "stop tracking rejected unknown reason: {:?}",
                                response_data.data.rejected[0]
                            );
                            return Err(tracking_error::RetrackError);
                        }
                    }
                } else {
                    Err(tracking_error::UnexpectedError)
                }
            }
            // error
            1 => {
                println!("{}: {:?}", response_data.code, response_data);
                return Err(tracking_error::SerdeError(Err(()).unwrap()));
            }
            // unexpected error
            _ => Err(tracking_error::UnexpectedError),
        }
    }

    /// Delete a tracking number from the api, destructive action so handle multi user numbers appropriately when calling this
    pub async fn delete_number(
        &self,
        tracking_number: &str,
    ) -> Result<delete_tracking_number_response, tracking_error> {
        // Create the body for the HTTP request since the api doesn't use a web endpoint
        // load the url, @ROUTE, api key and parameters into the URL and send it
        let url = format!("{}/deletetrack", self.base_url);

        // println!("{}", serde_json::json!([&tracking_details]));

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

        let body_bytes = &response.bytes().await?;
        // test print whole body
        // match String::from_utf8(body_bytes.to_vec()) {
        //     Ok(text) => println!("Response text: {}", text),
        //     Err(e) => println!("Response is not valid UTF-8: {:?}", e),
        // }

        // Parse the json of the response into the structures created with the 17track api docs
        // and return the @stop_tracking_response instance
        let response_data = serde_json::from_slice::<delete_tracking_number_response>(&body_bytes)?;
        match response_data.code {
            // success
            0 => {
                // Even though it's an array treat it always like only one tracking number has been passed,
                // the array is just an API thing, it takes up to 40 numbers at once but here only one is always passed (in parameters)
                if Some(response_data.data.accepted.len()) == Some(1) {
                    println!(
                        "delete number success: {:?}",
                        response_data.data.accepted[0]
                    );
                    Ok(response_data)
                } else if Some(response_data.data.rejected.len()) == Some(1) {
                    match response_data.data.rejected[0].error.code {
                        -18019902 => {
                            println!(
                                "delete number error: number not registered {:?}",
                                response_data.data.rejected[0]
                            );
                            return Err(tracking_error::NumberNotFound);
                        }
                        _ => {
                            println!(
                                "retrack stop rejected: {:?}",
                                response_data.data.rejected[0]
                            );
                            return Err(tracking_error::UnexpectedError);
                        }
                    }
                } else {
                    Err(tracking_error::UnexpectedError)
                }
            }
            // error
            1 => {
                println!("{}: {:?}", response_data.code, response_data);
                return Err(tracking_error::SerdeError(Err(()).unwrap()));
            }
            // unexpected error
            _ => Err(tracking_error::UnexpectedError),
        }
    }

    /// Get info about a tracking number (meta data)
    pub async fn get_number_metadata(
        &self,
        tracking_number: &str,
    ) -> Result<number_status_check, tracking_error> {
        // Create the body for the HTTP request since the api doesn't use a web endpoint
        // load the url, @ROUTE, api key and parameters into the URL and send it
        let url = format!("{}/gettracklist", self.base_url);

        // println!("{}", serde_json::json!([&tracking_details]));

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("17token", &self.api_key)
            .json(&serde_json::json!({
                "number": tracking_number
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            println!("Error: {}", response.status());
            return Err(tracking_error::ReqwestError(Err(()).unwrap()));
        }

        let body_bytes = &response.bytes().await?;
        // test print whole body
        // match String::from_utf8(body_bytes.to_vec()) {
        //     Ok(text) => println!("Response text: {}", text),
        //     Err(e) => println!("Response is not valid UTF-8: {:?}", e),
        // }

        // Parse the json of the response into the structures created with the 17track api docs
        // and return the @number_status_check instance
        let response_data = serde_json::from_slice::<number_status_check>(&body_bytes)?;
        match response_data.code {
            // success
            0 => {
                // Even though it's an array treat it always like only one tracking number has been passed,
                // the array is just an API thing, it takes up to 40 numbers at once but here only one is always passed (in parameters)
                if Some(response_data.data.accepted.len()) == Some(1) {
                    // println!(
                    //     "data about your number found: {:?}",
                    //     response_data.data.accepted[0]
                    // );
                    Ok(response_data)
                } else {
                    println!("{:?}", response_data);
                    Err(tracking_error::UnexpectedError)
                }
            }
            // error
            1 => {
                println!("{}: {:?}", response_data.code, response_data);
                return Err(tracking_error::SerdeError(Err(()).unwrap()));
            }
            // unexpected error
            _ => Err(tracking_error::UnexpectedError),
        }
    }
}
