pub mod join_request;
pub mod start;

use async_trait::async_trait;
use teloxide::prelude::Requester;
use teloxide::types::{CallbackQuery, Message};
use teloxide::{Bot, RequestError};

use crate::error::AppError;

#[async_trait]
pub trait TelegramApi: Send + Sync {
    async fn send_message(&self, chat_id: i64, text: String) -> Result<(), RequestError>;
    async fn decline_chat_join_request(
        &self,
        chat_id: i64,
        user_id: i64,
    ) -> Result<(), RequestError>;
}

pub struct TeloxideApi {
    bot: Bot,
}

impl TeloxideApi {
    pub fn new(bot: Bot) -> Self {
        Self { bot }
    }
}

#[async_trait]
impl TelegramApi for TeloxideApi {
    async fn send_message(&self, chat_id: i64, text: String) -> Result<(), RequestError> {
        self.bot
            .send_message(teloxide::types::ChatId(chat_id), text)
            .await
            .map(|_| ())
    }

    async fn decline_chat_join_request(
        &self,
        chat_id: i64,
        user_id: i64,
    ) -> Result<(), RequestError> {
        self.bot
            .decline_chat_join_request(
                teloxide::types::ChatId(chat_id),
                teloxide::types::UserId(user_id as u64),
            )
            .await
            .map(|_| ())
    }
}

pub async fn handle_private_message(_msg: Message) -> Result<(), AppError> {
    Ok(())
}

pub async fn handle_callback_query(_query: CallbackQuery) -> Result<(), AppError> {
    Ok(())
}

pub fn is_private_chat(msg: Message) -> bool {
    msg.chat.is_private()
}
