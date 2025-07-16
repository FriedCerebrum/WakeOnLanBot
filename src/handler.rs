use std::sync::Arc;
use teloxide::{prelude::*, dptree};
use teloxide::dispatching::UpdateFilterExt;

use crate::{Config, Command};

pub async fn run(bot: Bot, cfg: Arc<Config>) {
    let handler = dptree::entry()
        // Фильтрация по разрешённым пользователям
        .filter(|upd: Update, cfg: Arc<Config>| crate::is_allowed(&cfg, &upd))
        // --- Команды ---
        .branch(
            Update::filter_message()
                .filter_command::<Command>()
                .endpoint(crate::command_handler),
        )
        // --- CallbackQuery ---
        .branch(
            Update::filter_callback_query()
                .endpoint(crate::callback_handler),
        );

    Dispatcher::builder(bot.clone(), handler)
        .dependencies(dptree::deps![cfg])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
} 