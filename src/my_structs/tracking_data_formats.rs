/*

    Format for parsing the response of gettrackinfo from the api

    Tested for:
    1 accepted

    Not Testes with:
    >1 accepted
    0 accepted
    any rejected

*/

pub mod tracking_data_get_info {
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

    Format for parsing the response of the webhook update payload

    Tested for:

    Not Testes with:

    wish me luck

*/

/* //TODO: add handling for the second format
    {
        "event": "TRACKING_STOPPED",
        "data": {
            "number": "RR123456789CN",
            "carrier": 3011,
            "param": null,
            "tag": ""
        }
    }
*/
pub mod tracking_data_webhook_update {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    pub struct TrackingResponse {
        pub event: String,
        pub data: PackageData,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct PackageData {
        pub number: String,
        pub carrier: i32,
        pub param: Option<()>,
        pub tag: String,
        pub track_info: TrackInfo,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct TrackInfo {
        // here was the problem that took a whole day,
        // and there will be even more, the docs are inconsistent between the example response of gettrackinfo and the update message
        pub lastGatherTime: Option<String>, //TODO: check if throwing errors
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
        pub sub_status: Option<String>, //TODO: option here maybe unnecessary
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
