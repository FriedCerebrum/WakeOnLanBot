use std::sync::Arc;
use teloxide::{prelude::*, dptree};
use teloxide::dispatching::UpdateFilterExt;

use crate::{Config, Command};

// Проброс к функциям, определённым в crate:: (main.rs)
async fn command_bridge(cfg: Arc<Config>, bot: Bot, msg: Message, cmd: Command) -> teloxide::prelude::ResponseResult<()> {
    crate::command_handler(cfg, bot, msg, cmd).await
}

async fn callback_bridge(cfg: Arc<Config>, bot: Bot, q: CallbackQuery) -> teloxide::prelude::ResponseResult<()> {
    crate::callback_handler(cfg, bot, q).await
}

pub async fn run(bot: Bot, cfg: Arc<Config>) {
    let handler = dptree::entry()
        // Фильтрация по разрешённым пользователям
        .filter(|upd: Update, cfg: Arc<Config>| crate::is_allowed(&cfg, &upd))
        // --- Команды ---
        .branch({
            let cfg = cfg.clone();
            teloxide::filter_command::<Command, _>()
                .endpoint(move |bot: Bot, msg: Message, cmd: Command| {
                    let cfg = cfg.clone();
                    async move { crate::command_handler(cfg, bot, msg, cmd).await }
                })
        })
        // --- CallbackQuery ---
        .branch({
            let cfg = cfg.clone();
            Update::filter_callback_query()
                .endpoint(move |bot: Bot, q: CallbackQuery| {
                    let cfg = cfg.clone();
                    async move { crate::callback_handler(cfg, bot, q).await }
                })
        });

    Dispatcher::builder(bot.clone(), handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
} 