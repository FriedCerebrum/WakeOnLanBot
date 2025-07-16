use std::sync::Arc;
use teloxide::prelude::*;

use crate::Config;

pub async fn run(bot: Bot, cfg: Arc<Config>) {
    teloxide::repl_with_listener(
        bot.clone(),
        {
            let cfg = cfg.clone();
            move |bot: Bot, upd: Update| {
                let cfg = cfg.clone();
                async move {
                    handle_update(bot, upd, cfg).await
                }
            }
        },
        teloxide::update_listeners::polling_default(bot).await,
    )
    .await;
}

async fn handle_update(bot: Bot, upd: Update, cfg: Arc<Config>) -> ResponseResult<()> {
    match upd.kind {
        teloxide::types::UpdateKind::Message(msg) => {
            let user_id = msg.from.as_ref().map(|u| u.id.0);
            if !crate::is_allowed(&cfg, user_id) {
                return Ok(());
            }
            
            if let Some(text) = msg.text() {
                if text.starts_with("/start") {
                    match crate::send_main_menu(&bot, &msg, &cfg).await {
                        Ok(_) => {},
                        Err(e) => log::error!("Error sending main menu: {}", e),
                    }
                }
            }
        }
        teloxide::types::UpdateKind::CallbackQuery(q) => {
            let user_id = Some(q.from.id.0);
            if !crate::is_allowed(&cfg, user_id) {
                return Ok(());
            }
            
            if let Some(data) = q.data.as_deref() {
                let result = match data {
                    "wol" => crate::handle_wol(&bot, &q, &cfg).await,
                    "shutdown_confirm" => crate::ask_shutdown_confirm(&bot, &q).await,
                    "shutdown_yes" => crate::handle_shutdown(&bot, &q, &cfg).await,
                    "status" => crate::handle_status(&bot, &q, &cfg).await,
                    "cancel" => crate::cancel(&bot, &q).await,
                    _ => Ok(()),
                };
                if let Err(e) = result {
                    log::error!("Callback handler error for {}: {}", data, e);
                }
            }
        }
        _ => {} // Ignore other update types
    }
    
    Ok(())
} 