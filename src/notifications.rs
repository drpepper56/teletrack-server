/*
    Cargo
*/

use chrono::Utc;
use teloxide::prelude::*;
use teloxide::types::*;
use thiserror::Error;
use urlencoding::encode;

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
    // initalizer
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

    // add features to the notification banner and catch url validation errors
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

    // notification that opens the mini app
    pub async fn send_ma_notification(
        &self,
        user_id: i64,
        message: &str,
        // optional parameters
        params: Option<Vec<(&str, &str)>>,
    ) -> Result<(), notification_service_error> {
        // build the deep link with the struct parameters
        let mut deep_link = format!(
            "https://t.me/{}/{}?startapp=notification_{}",
            self.bot.get_me().await?.username(),
            self.mini_app_name,
            Utc::now().timestamp()
        );
        // add the custom parameters if they exist
        if let Some(params) = params {
            for (key, value) in params {
                deep_link.push_str(&format!("&{}={}", key, encode(value)));
            }
        }
        // create the keyboard that can (and will) throw errors
        let keyboard = self.create_inline_keyboard(&deep_link)?;
        // send itttt
        self.bot
            .send_message(ChatId(user_id), message)
            .reply_markup(keyboard)
            .parse_mode(teloxide::types::ParseMode::Html)
            .await?;

        Ok(())
    }

    // silent notification
    pub async fn send_silent_update(&self, user_id: i64) -> Result<(), notification_service_error> {
        // build the deep link with the struct parameters
        let deep_link = format!(
            "https://t.me/{}/{}?startapp=update_{}",
            self.bot.get_me().await?.username(),
            self.mini_app_name,
            Utc::now().timestamp()
        );
        // send the update
        self.bot
            .send_message(ChatId(user_id), "You have updates!")
            .disable_notification(true) // Silent notification
            .await?;

        Ok(())
    }
}
