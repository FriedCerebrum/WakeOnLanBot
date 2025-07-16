use std::sync::Arc;
use teloxide::{prelude::*, dptree};
use teloxide::dispatching::{HandlerExt, UpdateFilterExt};

use crate::{Config, Command};

// Бридж-функции больше не нужны.

pub async fn run(bot: Bot, cfg: Arc<Config>) {
    // Клонируем cfg для использования в замыканиях
    let cfg_allowed = cfg.clone();

    let handler = dptree::entry()
        // Фильтрация по разрешённым пользователям (без DI)
        .filter(move |upd: Update| crate::is_allowed(&cfg_allowed, &upd))
        // --- Команды ---
        .branch({
            let cfg_cmd = cfg.clone();
            teloxide::filter_command::<Command, _>()
                .endpoint(move |bot: Bot, msg: Message, cmd: Command| {
                    let cfg = cfg_cmd.clone();
                    async move { crate::command_handler(cfg, bot, msg, cmd).await }
                })
        })
        // --- CallbackQuery ---
        .branch({
            let cfg_cb = cfg.clone();
            Update::filter_callback_query()
                .endpoint(move |bot: Bot, q: CallbackQuery| {
                    let cfg = cfg_cb.clone();
                    async move { crate::callback_handler(cfg, bot, q).await }
                })
        });

    Dispatcher::builder(bot.clone(), handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
} 