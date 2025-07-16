use std::sync::Arc;
use teloxide::{prelude::*, dptree};
use teloxide::dispatching::UpdateFilterExt;

use crate::{Config, Command};

// Проброс к функциям, определённым в crate:: (main.rs)
async fn command_bridge(bot: Bot, cfg: Arc<Config>, msg: Message, cmd: Command) -> teloxide::prelude::ResponseResult<()> {
    crate::command_handler(cfg, bot, msg, cmd).await
}

async fn callback_bridge(bot: Bot, cfg: Arc<Config>, q: CallbackQuery) -> teloxide::prelude::ResponseResult<()> {
    crate::callback_handler(cfg, bot, q).await
}

pub async fn run(bot: Bot, cfg: Arc<Config>) {
    let handler = dptree::entry()
        // Фильтрация по разрешённым пользователям
        .filter(|upd: Update, cfg: Arc<Config>| crate::is_allowed(&cfg, &upd))
        // --- Команды ---
        .branch(
            teloxide::filter_command::<Command, _>()
                .endpoint(command_bridge),
        )
        // --- CallbackQuery ---
        .branch(
            Update::filter_callback_query()
                .endpoint(callback_bridge),
        );

    Dispatcher::builder(bot.clone(), handler)
        .dependencies(dptree::deps![cfg])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
} 