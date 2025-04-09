///  Base form structs that repeat
pub mod tracking_data_base {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct track_info {
        // here was the problem that took a whole day
        pub lastGatherTime: String, //TODO: this can go tits up
        pub shipping_info: shipping_info,
        pub latest_status: status,
        pub latest_event: event,
        pub time_metrics: time_metrics,
        pub milestone: Vec<milestone>,
        pub misc_info: misc_info,
        pub tracking: tracking_details,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct shipping_info {
        pub shipper_address: address,
        pub recipient_address: address,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct address {
        pub country: Option<String>,
        pub state: Option<String>,
        pub city: Option<String>,
        pub street: Option<String>,
        pub postal_code: Option<String>,
        pub coordinates: coordinates,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct coordinates {
        pub longitude: Option<f64>,
        pub latitude: Option<f64>,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct status {
        pub status: String,
        pub sub_status: String,
        pub sub_status_descr: Option<String>,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct event {
        pub time_iso: String,
        pub time_utc: String,
        pub time_raw: time_raw,
        pub description: String,
        pub location: String,
        pub stage: Option<String>,
        pub sub_status: String,
        pub address: address,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct time_raw {
        pub date: Option<String>,
        pub time: Option<String>,
        pub timezone: Option<String>,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct time_metrics {
        pub days_after_order: i32,
        pub days_of_transit: i32,
        pub days_of_transit_done: i32,
        pub days_after_last_update: i32,
        pub estimated_delivery_date: delivery_estimate,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct delivery_estimate {
        pub source: Option<String>,
        pub from: Option<String>,
        pub to: Option<String>,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct milestone {
        pub key_stage: String,
        pub time_iso: Option<String>,
        pub time_utc: Option<String>,
        pub time_raw: time_raw,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct misc_info {
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

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct tracking_details {
        pub providers_hash: i32,
        pub providers: Vec<provider>,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct provider {
        pub provider: carrier_info,
        pub provider_lang: Option<String>,
        pub service_type: Option<String>,
        pub latest_sync_status: String,
        pub latest_sync_time: String,
        pub events_hash: i32,
        pub events: Vec<event>,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct carrier_info {
        pub key: i32,
        pub name: String,
        pub alias: String,
        pub tel: String,
        pub homepage: String,
        pub country: String,
    }
}

/// Register Tracking
/*
    {
        "code": 0,
        "data": {
            "accepted": [
            {
                "origin": 1,
                "number": "RR123456789CN",
                "carrier": 3011,
                "email": null,
                "tag": "MyOrderID",
                "lang": null,
            }
            ],
            "rejected": [
            {
                "number": "1234",
                "tag": "My-Order-Id",
                "error": {
                "code": -18010012,
                "message": "The format of '1234' is invalid."
                }
            }
            ]
        }
    }
*/
pub mod register_tracking_number_response {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct RegisterResponse {
        pub code: i32,
        pub data: Data,
    }
    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct Data {
        pub accepted: Vec<Accepted>,
        pub rejected: Vec<Rejected>,
    }
    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct Accepted {
        pub origin: i32,
        pub number: String,
        pub carrier: i32,
        pub email: Option<String>,
        pub tag: String,
        pub lang: Option<String>,
    }
    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct Rejected {
        pub number: String,
        pub tag: String,
        pub error: RejectedError,
    }
    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct RejectedError {
        pub code: i32,
        pub message: String,
    }
}

/// Stop Tracking
/*
    {
        "code": 0,
        "data": {
            "accepted": [
            {
                "number": "RR123456789CN",
                "carrier": 3011
            }
            ],
            "rejected": [
                {
                "number": "21213123123230",
                "error": {
                "code": -18019902,
                "message": "The tracking number '21213123123230' does not register, please register first."
                }
            }
            ]
        }
    }
*/
pub mod stop_tracking_response {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct StopTrackingResponse {
        pub code: i32,
        pub data: Data,
    }
    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct Data {
        pub accepted: Vec<Accepted>,
        pub rejected: Vec<Rejected>,
    }
    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct Accepted {
        pub number: String,
        pub carrier: i32,
    }
    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct Rejected {
        pub number: String,
        pub error: RejectedError,
    }
    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct RejectedError {
        pub code: i32,
        pub message: String,
    }
}

/// Delete Tracking Number
/*
    {
        "code": 0,
        "data": {
            "accepted": [
            {
                "number": "RR123456789CN",
                "carrier": 3011
            }
            ],
            "rejected": [
            {
                "number": "21213123123230",
                "error": {
                "code": -18019902,
                "message": "The tracking number '21213123123230' does not register, please register first."
                }
            }
            ]
        }
    }
*/
pub mod delete_tracking_number_response {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct DeleteTrackingResponseNumber {
        pub code: i32,
        pub data: Data,
    }
    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct Data {
        pub accepted: Vec<Accepted>,
        pub rejected: Vec<Rejected>,
    }
    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct Accepted {
        pub number: String,
        pub carrier: i32,
    }
    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct Rejected {
        pub number: String,
        pub error: RejectedError,
    }
    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct RejectedError {
        pub code: i32,
        pub message: String,
    }
}

/// Gettrackinfo
/*
    refer to:
    https://api.17track.net/en/doc?version=v2.2&anchor=get-tracking-details---post-httpsapi17tracknettrackv22gettrackinfo
*/
pub mod tracking_data_get_info {
    use crate::{
        my_structs::tracking_data_formats::tracking_data_base::track_info,
        my_structs::tracking_data_formats::tracking_data_database_form::PackageData,
        my_structs::tracking_data_formats::tracking_data_database_form::TrackingData_DBF,
    };
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    pub struct TrackingResponse {
        pub code: i32,
        pub data: ResponseData,
    }

    impl TrackingResponse {
        pub fn convert_to_TrackingData_DBF(&self) -> TrackingData_DBF {
            let accepted_package = self.data.accepted.as_ref().unwrap().first().unwrap();
            TrackingData_DBF {
                code: self.code.clone(),
                data: PackageData {
                    number: accepted_package.number.clone(),
                    carrier: accepted_package.carrier.clone(),
                    param: accepted_package.param.clone(),
                    tag: accepted_package.tag.clone(),
                    track_info: accepted_package.track_info.clone(),
                },
            }
        }
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
        pub track_info: track_info,
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

/// Webhook update
/*
    refer to:
    https://api.17track.net/en/doc?version=v2.2&anchor=notification-status--content
*/
pub mod tracking_data_webhook_update {
    use crate::my_structs::tracking_data_formats::tracking_data_base::track_info;
    use serde::{Deserialize, Serialize};
    #[derive(Debug, Serialize, Deserialize)]
    pub struct TrackingResponse {
        pub event: String,
        pub data: TrackingData,
    }

    /// either one of the events try to deserialize
    #[derive(Debug, Serialize, Deserialize)]
    #[serde(untagged)]
    pub enum TrackingData {
        PackageData(PackageData),
        TrackingStopped(TrackingStopped),
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct TrackingStopped {
        number: String,
        carrier: i32,
        param: Option<()>,
        tag: String,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct PackageData {
        pub number: String,
        pub carrier: i32,
        pub param: Option<()>,
        pub tag: String,
        pub track_info: track_info,
    }
}

/// Custom format for storing in the database as a single tracking info
pub mod tracking_data_database_form {
    use crate::my_structs::tracking_data_formats::tracking_data_base::track_info;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct TrackingData_DBF {
        pub code: i32,
        pub data: PackageData,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct PackageData {
        pub number: String,
        pub carrier: i32,
        pub param: Option<()>,
        pub tag: String,
        pub track_info: track_info,
    }
}
