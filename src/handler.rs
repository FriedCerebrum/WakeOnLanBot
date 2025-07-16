use std::sync::Arc;
use teloxide::prelude::*;

use crate::Config;

pub async fn run(bot: Bot, cfg: Arc<Config>) {
    println!("=== НАЧАЛО ЗАПУСКА ОБРАБОТЧИКА ===");
    log::info!("Запуск обработчика событий...");
    
    // Сначала удаляем возможный webhook
    println!("Удаляем webhook (если есть)...");
    if let Err(e) = bot.delete_webhook().await {
        log::warn!("Не удалось удалить webhook: {}", e);
        println!("ВНИМАНИЕ: Не удалось удалить webhook: {}", e);
    } else {
        log::info!("Webhook удален успешно");
        println!("Webhook удален успешно");
    }
    
    println!("Создаем listener для polling...");
    
    // Настраиваем polling с явным указанием типов обновлений
    use teloxide::types::AllowedUpdate;
    use teloxide::update_listeners::Polling;
    let allowed_updates = vec![
        AllowedUpdate::Message,
        AllowedUpdate::CallbackQuery,
    ];
    
    let listener = Polling::builder(bot.clone())
        .timeout(std::time::Duration::from_secs(10))
        .limit(100)
        .allowed_updates(allowed_updates)
        .build();
    
    log::info!("Polling listener создан с поддержкой Message и CallbackQuery");
    println!("Polling listener создан успешно с поддержкой кнопок");
    println!("📋 Разрешенные типы обновлений: {:?}", allowed_updates);
    
    // Настройки для более стабильного polling
    println!("Настраиваем параметры polling...");
    // В teloxide 0.12 мы можем настроить timeout и лимиты
    
    println!("Запускаем REPL...");
    log::info!("Запускаем обработку событий...");

    teloxide::repl_with_listener(
        bot.clone(),
        {
            let cfg = cfg.clone();
            move |bot: Bot, upd: Update| {
                let cfg = cfg.clone();
                async move {
                    println!("=== ПОЛУЧЕНО ОБНОВЛЕНИЕ ===");
                    println!("🔍 Тип обновления: {:?}", upd.kind);
                    log::info!("Получено обновление: {:?}", upd.kind);
                    
                    match handle_update(bot, upd, cfg).await {
                        Ok(_) => {
                            println!("✅ Обновление обработано успешно");
                            log::info!("Обновление обработано успешно");
                        },
                        Err(e) => {
                            println!("❌ Ошибка обработки обновления: {}", e);
                            log::error!("Ошибка обработки обновления: {}", e);
                        }
                    }
                    Ok(())
                }
            }
        },
        listener,
    )
    .await;
}

async fn handle_update(bot: Bot, upd: Update, cfg: Arc<Config>) -> ResponseResult<()> {
    match upd.kind {
        teloxide::types::UpdateKind::Message(msg) => {
            println!("📨 Получено сообщение");
            log::info!("Получено сообщение от пользователя {:?}", msg.from().map(|u| u.id.0));
            
            let user_id = msg.from().as_ref().map(|u| u.id.0);
            if !crate::is_allowed(&cfg, user_id) {
                println!("❌ Пользователь {:?} не авторизован", user_id);
                log::warn!("Неавторизованный пользователь: {:?}", user_id);
                return Ok(());
            }
            
            if let Some(text) = msg.text() {
                println!("📝 Текст сообщения: '{}'", text);
                match text.to_lowercase().as_str() {
                    "/start" | "/wol" => {
                        println!("🚀 Обрабатываем команду {}", text);
                        match crate::send_main_menu(&bot, &msg, &cfg).await {
                            Ok(_) => {
                                println!("✅ Главное меню отправлено");
                                log::info!("Главное меню отправлено пользователю {:?}", user_id);
                            },
                            Err(e) => {
                                println!("❌ Ошибка отправки главного меню: {}", e);
                                log::error!("Error sending main menu: {}", e);
                            },
                        }
                    }
                    _ => {
                        println!("⚠️ Неизвестная команда: '{}'", text);
                        log::warn!("Неизвестная команда: '{}' от пользователя {:?}", text, user_id);
                    }
                }
            }
        }
        teloxide::types::UpdateKind::CallbackQuery(q) => {
            println!("🔔 Получен CALLBACK QUERY!");
            println!("👤 От пользователя: {}", q.from.id.0);
            println!("📊 Callback data: {:?}", q.data);
            println!("🔍 Полный callback query: {:?}", q);
            log::info!("Получен callback query: '{:?}' от пользователя {}", q.data, q.from.id.0);
            
            let user_id = Some(q.from.id.0);
            if !crate::is_allowed(&cfg, user_id) {
                println!("❌ Пользователь {} не авторизован для callback", q.from.id.0);
                log::warn!("Неавторизованный callback от пользователя: {}", q.from.id.0);
                return Ok(());
            }
            
            if let Some(data) = q.data.as_deref() {
                println!("🎯 Обрабатываем callback data: '{}'", data);
                log::info!("Обрабатываем callback query: '{}' от пользователя {}", data, q.from.id.0);
                
                let result = match data {
                    "wol" => {
                        println!("🔌 Запуск WOL handler");
                        crate::handle_wol(&bot, &q, &cfg).await
                    },
                    "shutdown_confirm" => {
                        println!("🔴 Запуск shutdown confirm handler");
                        crate::ask_shutdown_confirm(&bot, &q).await
                    },
                    "shutdown_yes" => {
                        println!("💀 Запуск shutdown handler");
                        crate::handle_shutdown(&bot, &q, &cfg).await
                    },
                    "status" => {
                        println!("🟢 Запуск status handler");
                        crate::handle_status(&bot, &q, &cfg).await
                    },
                    "cancel" => {
                        println!("❌ Запуск cancel handler");
                        crate::cancel(&bot, &q).await
                    },
                    _ => {
                        println!("⚠️ Неизвестный callback data: '{}'", data);
                        log::warn!("Неизвестный callback data: '{}'", data);
                        Ok(())
                    },
                };
                
                match result {
                    Ok(_) => {
                        println!("✅ Callback handler '{}' выполнен успешно", data);
                        log::info!("Callback handler '{}' выполнен успешно", data);
                    },
                    Err(e) => {
                        println!("❌ Ошибка в callback handler '{}': {}", data, e);
                        log::error!("Callback handler error for {}: {}", data, e);
                    }
                }
            } else {
                println!("⚠️ Callback query без data");
                log::warn!("Получен callback query без data");
            }
        }
        _ => {
            println!("🔍 Получен другой тип обновления: {:?}", upd.kind);
            log::debug!("Игнорируем обновление типа: {:?}", upd.kind);
        }
    }
    
    Ok(())
} 