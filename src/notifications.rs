/*
    Cargo
*/

use base64::Engine as _;
use chrono::Utc;
use teloxide::prelude::*;
use teloxide::types::*;
use thiserror::Error;

/*
    Structs
*/

pub struct notification_service {
    bot: Bot,
    mini_app_name: String,
}

#[derive(Error, Debug)]
pub enum notification_service_error {
    #[error("telegram API error")]
    TelegramError(#[from] teloxide::RequestError),
    #[error("invalid bot configuration")]
    BotConfigurationError,
    #[error("bad url")]
    UrlFormatError,
}

/*
    Functions
*/

impl notification_service {
    /// initializer
    pub fn new(bot_token: String, mini_app_name: &str) -> Result<Self, notification_service_error> {
        // set the parameters and handle validation checks
        if bot_token.is_empty() || mini_app_name.is_empty() {
            return Err(notification_service_error::BotConfigurationError);
        }

        Ok(Self {
            // set the bot to the owned bot from telegram environment
            bot: Bot::new(bot_token),
            // set the name of the mini app from the app existing on the telegram environment
            mini_app_name: mini_app_name.to_string(),
        })
    }

    /// add features to the notification banner and catch url validation errors
    fn create_inline_keyboard(
        &self,
        url: &str,
    ) -> Result<InlineKeyboardMarkup, notification_service_error> {
        let parsed_url =
            reqwest::Url::parse(url).map_err(|_| notification_service_error::UrlFormatError)?;
        Ok(InlineKeyboardMarkup::new(vec![vec![
            InlineKeyboardButton::url("Open Mini App", parsed_url),
        ]]))
    }

    /// notification that opens the mini app
    pub async fn send_ma_notification(
        &self,
        user_id: i64,
        message: &str,
        tracking_number_that_was_updated: &str,
    ) -> Result<(), notification_service_error> {
        // build the deep link with the struct parameters
        // telegram rejects all parameters other than the start parameter so in order to send the other parameters they
        // need to be all put in a json and encoded and put as a string as the start parameter

        // prepare the startparam
        let mut parameter_map = serde_json::Map::new();
        parameter_map.insert(
            "notification_id".to_string(),
            serde_json::json!(Utc::now().timestamp()),
        );
        parameter_map.insert(
            "package_update".to_string(),
            serde_json::json!(tracking_number_that_was_updated),
        );
        let startapp_value = base64::engine::general_purpose::URL_SAFE
            .encode(serde_json::Value::Object(parameter_map).to_string());

        // deep link to open the app from the notification message button, includes the startparam
        let deep_link = format!(
            "https://t.me/{}/{}?startapp={}",
            self.bot.get_me().await?.username(),
            self.mini_app_name,
            startapp_value
        );

        // println!("{}", deep_link);

        let keyboard = self.create_inline_keyboard(&deep_link)?;
        match self
            .bot
            .send_message(ChatId(user_id), message)
            .reply_markup(keyboard)
            .parse_mode(teloxide::types::ParseMode::Html)
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(notification_service_error::TelegramError(e)),
        }
    }
}
