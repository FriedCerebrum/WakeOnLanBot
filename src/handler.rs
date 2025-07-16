use std::sync::Arc;
use teloxide::{prelude::*, dptree};
use teloxide::dispatching::{HandlerExt, UpdateFilterExt};

use crate::{Config, Command};

// Тип результата, подходящий для dptree::Handler
type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

// Бридж-функции больше не нужны.

pub async fn run(bot: Bot, cfg: Arc<Config>) {
    // Клонируем cfg для использования в замыканиях
    let cfg_allowed = cfg.clone();

    let handler = dptree::entry()
        // Фильтрация по разрешённым пользователям (без DI)
        .filter(move |upd: Update| crate::is_allowed(&cfg_allowed, &upd))
        // --- Сообщения ---
        .branch(
            Update::filter_message().branch({
                let cfg_cmd = cfg.clone();
                teloxide::filter_command::<Command, HandlerResult>()
                    .endpoint(move |bot: Bot, msg: Message, cmd: Command| {
                        let cfg = cfg_cmd.clone();
                        async move {
                            // Пробуем обработать команду, логируем возможную ошибку –
                            // dptree ожидает HandlerResult, а не ResponseResult.
                            if let Err(e) = crate::command_handler(cfg, bot, msg, cmd).await {
                                log::error!("command_handler error: {e}");
                            }
                            Ok(()) as HandlerResult
                        }
                    })
            })
        )
        // --- CallbackQuery ---
        .branch({
            let cfg_cb = cfg.clone();
            Update::filter_callback_query()
                .endpoint(move |bot: Bot, q: CallbackQuery| {
                    let cfg = cfg_cb.clone();
                    async move {
                        if let Err(e) = crate::callback_handler(cfg, bot, q).await {
                            log::error!("callback_handler error: {e}");
                        }
                        Ok(()) as HandlerResult
                    }
                })
        });

    Dispatcher::builder(bot.clone(), handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
} 