use std::sync::Arc;
use teloxide::{prelude::*, dptree};
use teloxide::dispatching::UpdateFilterExt;

use crate::{Config, Command};

pub async fn run(bot: Bot, cfg: Arc<Config>) {
    let handler = dptree::entry()
        .filter(|upd: Update, cfg: Arc<Config>| crate::is_allowed(&cfg, &upd))
        // --- Команды ---
        .branch(
            Update::filter_message()
                .filter_command::<Command>()
                .branch(
                    dptree::case![Command::Start].endpoint(|cfg: Arc<Config>, bot: Bot, msg: Message| async move {
                        crate::send_main_menu(&bot, &msg, &cfg).await.ok();
                        Ok(())
                    }),
                ),
        )
        // --- CallbackQuery ---
        .branch(
            Update::filter_callback_query()
                .branch(dptree::filter(|q: CallbackQuery| q.data.as_deref() == Some("wol"))
                    .endpoint(|cfg: Arc<Config>, bot: Bot, q: CallbackQuery| async move {
                        crate::handle_wol(&bot, &q, &cfg).await.ok();
                        Ok(())
                    }))
                .branch(dptree::filter(|q: CallbackQuery| q.data.as_deref() == Some("shutdown_confirm"))
                    .endpoint(|bot: Bot, q: CallbackQuery| async move {
                        crate::ask_shutdown_confirm(&bot, &q).await.ok();
                        Ok(())
                    }))
                .branch(dptree::filter(|q: CallbackQuery| q.data.as_deref() == Some("shutdown_yes"))
                    .endpoint(|cfg: Arc<Config>, bot: Bot, q: CallbackQuery| async move {
                        crate::handle_shutdown(&bot, &q, &cfg).await.ok();
                        Ok(())
                    }))
                .branch(dptree::filter(|q: CallbackQuery| q.data.as_deref() == Some("status"))
                    .endpoint(|cfg: Arc<Config>, bot: Bot, q: CallbackQuery| async move {
                        crate::handle_status(&bot, &q, &cfg).await.ok();
                        Ok(())
                    }))
                .branch(dptree::filter(|q: CallbackQuery| q.data.as_deref() == Some("cancel"))
                    .endpoint(|bot: Bot, q: CallbackQuery| async move {
                        crate::cancel(&bot, &q).await.ok();
                        Ok(())
                    })),
        );

    Dispatcher::builder(bot.clone(), handler)
        .dependencies(dptree::deps![cfg])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
} 