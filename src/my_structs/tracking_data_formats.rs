/*
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
    FORMATS FOR THE API RESPONSES

-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
*/

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
        // pub errors: Option<Vec<RejectedError>>,
    }
    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct Accepted {
        pub origin: i32,
        pub number: String,
        pub carrier: i32,
        pub email: Option<String>,
        pub tag: Option<String>,
        pub lang: Option<String>,
    }
    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct Rejected {
        pub number: String,
        pub tag: Option<String>,
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

/// Re-Track Number
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
                "code": -18019904,
                "message": "Retrack is not allowed. You can only retrack stopped number."
                }
            }
            ]
        }
    }
*/
pub mod retrack_stopped_number_response {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct RetrackStoppedNumberResponse {
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
        my_structs::tracking_data_formats::tracking_data_base::TrackInfo,
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
        pub fn convert_to_tracking_data_dbf(&self) -> TrackingData_DBF {
            let accepted_package = self.data.accepted.first().expect("bad format of GETTRACKINFO method, error happened in converting it to the database format");
            TrackingData_DBF {
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
        pub accepted: Vec<AcceptedPackage>,
        pub rejected: Vec<RejectedPackage>, // Empty array in the example
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct AcceptedPackage {
        pub number: String,
        pub carrier: i32,
        pub param: Option<()>,
        pub tag: Option<String>,
        pub track_info: TrackInfo,
    }

    // Rejected
    #[derive(Debug, Serialize, Deserialize)]
    pub struct RejectedPackage {
        pub number: String,
        pub error: RejectedError,
    }
    #[derive(Debug, Serialize, Deserialize)]
    pub struct RejectedError {
        pub code: i32,
        pub message: String,
    }
}

/// Search info about a registered tracking number, useful for checking if it's stopped on the API
/*
    Example from the API Docs:
    {
        "page": {
            "data_total": 43,
            "page_total": 2,
            "page_no": 1,
            "page_size": 40
        },
        "code": 0,
        "data": {
            "accepted": [
            {
                "number": "RR123456789CN",
                "param": null,
                "param_type": "None",
                "data_origin": "Api",
                "carrier": 3011,
                "shipping_country": "CN",
                "final_carrier": 0,
                "recipient_country": "RU",
                "register_time": "2022-03-14T07:45:38Z",
                "tracking_status": "Tracking",
                "package_status": "Delivered",
                "track_time": "2022-03-14T07:45:22Z",
                "push_time": "2022-03-14T07:47:42Z",
                "push_status": "Success",
                "push_status_code":200,
                "stop_track_time": null,
                "stop_track_reason": null,
                "is_retracked": false,
                "carrier_change_count": 0,
                "tag": null,
                "email":"",
                "order_no": "86574382938",
                "order_time": "2022-04-25T22:22:47+05:00",
                "lang":"",
                "remark": "test",
                "latest_event_time": "2023-08-05T10:00:21+05:00",
                "latest_event_info": "FAISALABAD,Shipment has been Delivered. Delivery Date & Time Aug 5 2023 9:48AM and Received By: Shahzad",
                "days_after_order ":2,
                "days_after_last_update ":null,
                "days_of_transit ":2,
                "days_of_transit_done ":2,
                "delievery_time": "2023-08-05T05:00:21Z",
                "pickup_time": ""
            }
        ]
    }

}

*/
pub mod tracking_number_meta_data {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct NumberStatusCheck {
        pub page: Page,
        pub code: i32,
        pub data: PageData,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct Page {
        pub data_total: i32,
        pub page_total: i32,
        pub page_no: i32,
        pub page_size: i32,
    }
    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct PageData {
        pub accepted: Vec<AcceptedPage>,
    }
    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct AcceptedPage {
        pub number: Option<String>,
        pub param: Option<()>,
        pub param_type: Option<String>,
        pub data_origin: Option<String>,
        pub carrier: Option<i32>,
        pub shipping_country: Option<String>,
        pub final_carrier: Option<i32>,
        pub recipient_country: Option<String>,
        pub register_time: Option<String>,
        pub tracking_status: String,
        pub package_status: String,
        pub track_time: Option<String>,
        pub push_time: Option<String>,
        pub push_status: Option<String>,
        pub push_status_code: Option<i32>,
        pub stop_track_time: Option<String>,
        pub stop_track_reason: Option<String>,
        pub is_retracked: Option<bool>,
        pub carrier_change_count: Option<i32>,
        pub tag: Option<String>,
        pub email: Option<String>,
        pub order_no: Option<String>,
        pub order_time: Option<String>,
        pub lang: Option<String>,
        pub remark: Option<String>,
        pub latest_event_time: Option<String>,
        pub latest_event_info: Option<String>,
        pub days_after_order: Option<String>,
        pub days_after_last_update: Option<String>,
        pub days_of_transit: Option<String>,
        pub days_of_transit_done: Option<String>,
        pub delievery_time: Option<String>,
        pub pickup_time: Option<String>,
    }
}

/// Webhook update
/*
    refer to:
    https://api.17track.net/en/doc?version=v2.2&anchor=notification-status--content
*/
pub mod tracking_data_webhook_update {
    use crate::my_structs::tracking_data_formats::tracking_data_database_form::TrackingData_DBF;
    use serde::{Deserialize, Serialize};

    use super::tracking_data_database_form::PackageData;

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct TrackingResponse {
        pub event: String,
        pub data: TrackingData,
    }

    impl TrackingResponse {
        pub fn convert_to_tracking_data_dbf(&self) -> Option<TrackingData_DBF> {
            if let TrackingData::PackageData(accepted_package) = &self.data {
                Some(TrackingData_DBF {
                    data: PackageData {
                        number: accepted_package.number.clone(),
                        carrier: accepted_package.carrier.clone(),
                        param: accepted_package.param.clone(),
                        tag: accepted_package.tag.clone(),
                        track_info: accepted_package.track_info.clone(),
                    },
                })
            } else {
                None
            }
        }
    }

    /// either one of the events try to deserialize
    #[derive(Debug, Serialize, Deserialize, Clone)]
    #[serde(untagged)]
    pub enum TrackingData {
        PackageData(PackageDataWebhook),
        TrackingStopped(TrackingStopped),
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct TrackingStopped {
        pub number: String,
        pub carrier: i32,
        pub param: Option<()>,
        pub tag: Option<String>,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct PackageDataWebhook {
        pub number: String,
        pub carrier: i32,
        pub param: Option<()>,
        pub tag: Option<String>,
        pub track_info: super::tracking_data_base::TrackInfo,
    }

    impl PackageDataWebhook {
        // to dbf
        pub fn convert_to_tracking_data_dbf(&self) -> Option<TrackingData_DBF> {
            Some(TrackingData_DBF {
                data: PackageData {
                    number: self.number.clone(),
                    carrier: self.carrier.clone(),
                    param: self.param.clone(),
                    tag: self.tag.clone(),
                    track_info: self.track_info.clone(),
                },
            })
        }
        // to HTMLf
        pub fn convert_to_tracking_data_html_form(
            &self,
        ) -> super::tracking_data_html_form::tracking_data_HTML {
            super::tracking_data_html_form::tracking_data_HTML {
                tracking_number: self.number.clone(),
                tag: self.tag.clone(),
                latest_event: self.track_info.latest_event.convert_to_HTML_event(),
                providers_data: self
                    .track_info
                    .tracking
                    .providers
                    .iter()
                    .map(|provider| provider.convert_to_HTML_provider())
                    .collect(),
                time_metrics: Some(self.track_info.time_metrics.clone()),
            }
        }
    }
}

/*
-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
    CUSTOM FORMATS

-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
*/

///  Base form structs that repeat
pub mod tracking_data_base {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct TrackInfo {
        // here was the problem that took a whole day, name has to stay like this
        pub lastGatherTime: Option<String>,
        pub shipping_info: ShippingInfo,
        pub latest_status: Status,
        pub latest_event: event,
        pub time_metrics: time_metrics,
        pub milestone: Vec<milestone>,
        pub misc_info: misc_info,
        pub tracking: tracking_details,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct ShippingInfo {
        pub shipper_address: Address,
        pub recipient_address: Address,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct Address {
        pub country: Option<String>,
        pub state: Option<String>,
        pub city: Option<String>,
        pub street: Option<String>,
        pub postal_code: Option<String>,
        pub coordinates: Coordinates,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct Coordinates {
        pub longitude: Option<f64>,
        pub latitude: Option<f64>,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct Status {
        pub status: Option<String>,
        pub sub_status: Option<String>,
        pub sub_status_descr: Option<String>,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct event {
        pub time_iso: Option<String>,
        pub time_utc: Option<String>,
        pub time_raw: time_raw,
        pub description: Option<String>,
        pub location: Option<String>,
        pub stage: Option<String>,
        pub sub_status: Option<String>,
        pub address: Address,
    }

    // convert event to HTML (smaller) event format
    impl event {
        pub fn convert_to_HTML_event(&self) -> super::tracking_data_html_form::event {
            super::tracking_data_html_form::event {
                description: self.description.clone(),
                location: self.location.clone(),
                stage: self.stage.clone(),
                sub_status: self.sub_status.clone(),
                address: Some(self.address.clone()),
                time: Some(self.time_raw.clone()),
            }
        }
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct time_raw {
        pub date: Option<String>,
        pub time: Option<String>,
        pub timezone: Option<String>,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct time_metrics {
        pub days_after_order: Option<i32>,
        pub days_of_transit: Option<i32>,
        pub days_of_transit_done: Option<i32>,
        pub days_after_last_update: Option<i32>,
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
        pub key_stage: Option<String>,
        pub time_iso: Option<String>,
        pub time_utc: Option<String>,
        pub time_raw: time_raw,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct misc_info {
        pub risk_factor: i32,
        pub service_type: Option<String>,
        pub weight_raw: Option<String>,
        pub weight_kg: Option<String>,
        pub pieces: Option<String>,
        pub dimensions: Option<String>,
        pub customer_number: Option<String>,
        pub reference_number: Option<String>,
        pub local_number: Option<String>,
        pub local_provider: Option<String>,
        pub local_key: Option<i32>,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct tracking_details {
        pub providers_hash: Option<i32>,
        pub providers: Vec<provider>,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct provider {
        pub provider: carrier_info,
        pub provider_lang: Option<String>,
        pub service_type: Option<String>,
        pub latest_sync_status: Option<String>,
        pub latest_sync_time: Option<String>,
        pub events_hash: Option<i32>,
        pub events: Vec<event>,
    }

    // convert the provider to the HTML provider
    impl provider {
        pub fn convert_to_HTML_provider(
            &self,
        ) -> super::tracking_data_html_form::tracking_provider_provided_events {
            super::tracking_data_html_form::tracking_provider_provided_events {
                provider_name: self.provider.name.clone(),
                provider_key: self.provider.key.clone(),
                provider_events: self
                    .events
                    .iter()
                    .map(|event| event.convert_to_HTML_event())
                    .collect(),
            }
        }
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct carrier_info {
        pub key: Option<i32>,
        pub name: Option<String>,
        pub alias: Option<String>,
        pub tel: Option<String>,
        pub homepage: Option<String>,
    }
}

/// form used to create html objects and send to the user
// TODO: add functions that convert to HTML elements
pub mod tracking_data_html_form {
    use crate::my_structs::tracking_data_formats::tracking_data_base;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct tracking_data_HTML {
        pub tracking_number: String,
        pub tag: Option<String>,
        pub latest_event: event,
        pub providers_data: Vec<tracking_provider_provided_events>,
        pub time_metrics: Option<tracking_data_base::time_metrics>,
    }

    // case where multiple providers kms
    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct tracking_provider_provided_events {
        pub provider_name: Option<String>,
        pub provider_key: Option<i32>,
        pub provider_events: Vec<event>,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct event {
        pub description: Option<String>,
        pub location: Option<String>,
        pub stage: Option<String>,
        pub sub_status: Option<String>,
        pub address: Option<tracking_data_base::Address>,
        pub time: Option<tracking_data_base::time_raw>,
    }
}
/// Custom format for storing in the database as a single tracking info
pub mod tracking_data_database_form {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct TrackingData_DBF {
        pub data: PackageData,
    }

    // convert to HTML format
    impl TrackingData_DBF {
        pub fn convert_to_HTML_form(&self) -> super::tracking_data_html_form::tracking_data_HTML {
            super::tracking_data_html_form::tracking_data_HTML {
                tracking_number: self.data.number.clone(),
                tag: self.data.tag.clone(),
                latest_event: self.data.track_info.latest_event.convert_to_HTML_event(),
                providers_data: self
                    .data
                    .track_info
                    .tracking
                    .providers
                    .iter()
                    .map(|provider| provider.convert_to_HTML_provider())
                    .collect(),
                time_metrics: Some(self.data.track_info.time_metrics.clone()),
            }
        }
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct PackageData {
        pub number: String,
        pub carrier: i32,
        pub param: Option<()>,
        pub tag: Option<String>,
        pub track_info: super::tracking_data_base::TrackInfo,
    }
}
