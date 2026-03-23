pub mod join_request;
pub mod callbacks;
pub mod questionnaire;
pub mod start;
pub mod language_selection;
pub mod stats;

use async_trait::async_trait;
use teloxide::payloads::{
    AnswerCallbackQuerySetters, EditMessageTextSetters, SendMessageSetters,
};
use teloxide::prelude::Requester;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, Message, ParseMode};
use teloxide::{Bot, RequestError};

#[async_trait]
pub trait TelegramApi: Send + Sync {
    async fn send_message(&self, chat_id: i64, text: String) -> Result<(), RequestError>;
    async fn send_message_html(
        &self,
        chat_id: i64,
        text: String,
        reply_markup: Option<InlineKeyboardMarkup>,
    ) -> Result<i64, RequestError>;
    async fn send_message_with_inline_keyboard(
        &self,
        chat_id: i64,
        text: String,
        keyboard: Vec<Vec<(String, String)>>,
    ) -> Result<(), RequestError>;
    async fn edit_message_html(
        &self,
        chat_id: i64,
        message_id: i64,
        text: String,
    ) -> Result<(), RequestError>;
    async fn edit_message_html_with_markup(
        &self,
        chat_id: i64,
        message_id: i32,
        text: String,
        reply_markup: Option<Vec<Vec<(String, String)>>>,
    ) -> Result<(), RequestError>;
    async fn clear_message_reply_markup(
        &self,
        chat_id: i64,
        message_id: i64,
    ) -> Result<(), RequestError>;
    async fn answer_callback_query(
        &self,
        callback_query_id: String,
        text: String,
    ) -> Result<(), RequestError>;
    async fn approve_chat_join_request(
        &self,
        chat_id: i64,
        user_id: i64,
    ) -> Result<(), RequestError>;
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

    async fn send_message_html(
        &self,
        chat_id: i64,
        text: String,
        reply_markup: Option<InlineKeyboardMarkup>,
    ) -> Result<i64, RequestError> {
        let mut request = self
            .bot
            .send_message(teloxide::types::ChatId(chat_id), text)
            .parse_mode(ParseMode::Html);

        if let Some(reply_markup) = reply_markup {
            request = request.reply_markup(reply_markup);
        }

        request.await.map(|message| message.id.0 as i64)
    }

    async fn edit_message_html(
        &self,
        chat_id: i64,
        message_id: i64,
        text: String,
    ) -> Result<(), RequestError> {
        self.bot
            .edit_message_text(
                teloxide::types::ChatId(chat_id),
                teloxide::types::MessageId(message_id as i32),
                text,
            )
            .parse_mode(ParseMode::Html)
            .await
            .map(|_| ())
    }

    async fn edit_message_html_with_markup(
        &self,
        chat_id: i64,
        message_id: i32,
        text: String,
        reply_markup: Option<Vec<Vec<(String, String)>>>,
    ) -> Result<(), RequestError> {
        let mut request = self
            .bot
            .edit_message_text(
                teloxide::types::ChatId(chat_id),
                teloxide::types::MessageId(message_id),
                text,
            )
            .parse_mode(ParseMode::Html);

        if let Some(keyboard) = reply_markup {
            let buttons: Vec<Vec<InlineKeyboardButton>> = keyboard
                .into_iter()
                .map(|row| {
                    row.into_iter()
                        .map(|(text, data)| InlineKeyboardButton::callback(text, data))
                        .collect()
                })
                .collect();
            request = request.reply_markup(InlineKeyboardMarkup::new(buttons));
        }

        request.await.map(|_| ())
    }

    async fn clear_message_reply_markup(
        &self,
        chat_id: i64,
        message_id: i64,
    ) -> Result<(), RequestError> {
        self.bot
            .edit_message_reply_markup(
                teloxide::types::ChatId(chat_id),
                teloxide::types::MessageId(message_id as i32),
            )
            .await
            .map(|_| ())
    }

    async fn answer_callback_query(
        &self,
        callback_query_id: String,
        text: String,
    ) -> Result<(), RequestError> {
        self.bot
            .answer_callback_query(teloxide::types::CallbackQueryId(callback_query_id))
            .text(text)
            .await
            .map(|_| ())
    }

    async fn approve_chat_join_request(
        &self,
        chat_id: i64,
        user_id: i64,
    ) -> Result<(), RequestError> {
        self.bot
            .approve_chat_join_request(
                teloxide::types::ChatId(chat_id),
                teloxide::types::UserId(user_id as u64),
            )
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

    async fn send_message_with_inline_keyboard(
        &self,
        chat_id: i64,
        text: String,
        keyboard: Vec<Vec<(String, String)>>,
    ) -> Result<(), RequestError> {
        let buttons: Vec<Vec<InlineKeyboardButton>> = keyboard
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|(text, data)| InlineKeyboardButton::callback(text, data))
                    .collect()
            })
            .collect();

        let markup = InlineKeyboardMarkup::new(buttons);

        self.bot
            .send_message(teloxide::types::ChatId(chat_id), text)
            .reply_markup(markup)
            .await?;

        Ok(())
    }
}

pub use questionnaire::handle_private_message;

pub use callbacks::handle_callback_query;

pub fn is_private_chat(msg: Message) -> bool {
    msg.chat.is_private()
}
