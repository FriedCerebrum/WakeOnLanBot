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
    
    println!("Создаем обработчик событий...");
    
    // Создаем дерево обработчиков
    let handler = ::dptree::entry()
        .branch(Update::filter_message().endpoint(message_handler))
        .branch(Update::filter_callback_query().endpoint(callback_handler));
    
    // Создаем диспетчер
    let mut dispatcher = Dispatcher::builder(bot.clone(), handler)
        .dependencies(::dptree::deps![cfg.clone()])
        .default_handler(|upd| async move {
            log::warn!("Необработанное обновление: {:?}", upd);
        })
        .error_handler(LoggingErrorHandler::with_custom_text(
            "Произошла ошибка в диспетчере",
        ))
        .enable_ctrlc_handler()
        .build();
    
    println!("Диспетчер создан успешно");
    log::info!("Запускаем диспетчер...");
    
    dispatcher.dispatch().await;
}

async fn message_handler(bot: Bot, msg: Message, cfg: Arc<Config>) -> ResponseResult<()> {
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
    
    Ok(())
}

async fn callback_handler(bot: Bot, q: CallbackQuery, cfg: Arc<Config>) -> ResponseResult<()> {
    println!("🔔 Получен CALLBACK QUERY!");
    println!("👤 От пользователя: {}", q.from.id.0);
    println!("📊 Callback data: {:?}", q.data);
    println!("🔍 Полный callback query: {:?}", q);
    log::info!("Получен callback query: '{:?}' от пользователя {}", q.data, q.from.id.0);
    
    let user_id = Some(q.from.id.0);
    if !crate::is_allowed(&cfg, user_id) {
        println!("❌ Пользователь {} не авторизован для callback", q.from.id.0);
        log::warn!("Неавторизованный callback от пользователя: {}", q.from.id.0);
        
        // Все равно отвечаем на callback query, чтобы убрать индикатор загрузки
        if let Err(e) = crate::safe_answer_callback_query(&bot, &q.id).await {
            log::error!("Не удалось ответить на неавторизованный callback: {}", e);
        }
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
                
                // Отвечаем на неизвестный callback query
                if let Err(e) = crate::safe_answer_callback_query(&bot, &q.id).await {
                    log::error!("Не удалось ответить на неизвестный callback: {}", e);
                }
                
                // Показываем пользователю сообщение об ошибке
                if let Some(msg) = &q.message {
                    if let Err(e) = bot.edit_message_text(
                        msg.chat.id,
                        msg.id,
                        "❌ Неизвестная команда. Используйте /start для возврата в главное меню."
                    )
                    .reply_markup(crate::main_keyboard())
                    .await {
                        log::error!("Не удалось отправить сообщение об ошибке неизвестной команды: {}", e);
                    }
                }
                
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
                
                // Пытаемся уведомить пользователя о проблеме
                if let Some(msg) = &q.message {
                    if let Err(edit_err) = bot.edit_message_text(
                        msg.chat.id,
                        msg.id,
                        "❌ Произошла ошибка при выполнении команды.\nПопробуйте позже или обратитесь к администратору."
                    )
                    .reply_markup(crate::main_keyboard())
                    .await {
                        log::error!("Не удалось отправить сообщение об общей ошибке: {}", edit_err);
                    }
                }
            }
        }
    } else {
        println!("⚠️ Callback query без data");
        log::warn!("Получен callback query без data");
        
        // Отвечаем на callback query без data
        if let Err(e) = crate::safe_answer_callback_query(&bot, &q.id).await {
            log::error!("Не удалось ответить на callback без data: {}", e);
        }
    }
    
    Ok(())
} 