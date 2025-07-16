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
            teloxide::filter_command::<Command, _>()
                .endpoint(|cfg: Arc<Config>, bot: Bot, msg: Message, cmd: Command| async move {
                    crate::command_handler(cfg, bot, msg, cmd).await
                }),
        )
        // --- CallbackQuery ---
        .branch(
            Update::filter_callback_query()
                .endpoint(|cfg: Arc<Config>, bot: Bot, q: CallbackQuery| async move {
                    crate::callback_handler(cfg, bot, q).await
                }),
        );

    Dispatcher::builder(bot.clone(), handler)
        .dependencies(dptree::deps![cfg])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
} 