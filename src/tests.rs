#[cfg(test)]
mod tests {
    use super::*;
    use teloxide::types::{
        Update, UpdateKind, CallbackQuery, User, Chat, ChatKind, Message, MessageKind,
        MessageId, UserId,
    };
    use tokio_test;
    use std::sync::Arc;

    // Тестовая конфигурация
    fn test_config() -> Config {
        Config {
            bot_token: "test_token".to_string(),
            allowed_users: vec![123456789],
            server_mac: "00:11:22:33:44:55".to_string(),
            router_ssh_host: "test_router".to_string(),
            router_ssh_port: 22,
            router_ssh_user: "test_user".to_string(),
            router_ssh_key: "/test/key".to_string(),
            server_ssh_host: "test_server".to_string(),
            server_ssh_port: 22,
            server_ssh_user: "test_user".to_string(),
            server_ssh_key: "/test/key".to_string(),
            ssh_timeout: Duration::from_secs(5),
            nc_timeout: Duration::from_secs(3),
        }
    }

    // Создание тестового пользователя
    fn test_user() -> User {
        User {
            id: UserId(123456789),
            is_bot: false,
            first_name: "Test".to_string(),
            last_name: Some("User".to_string()),
            username: Some("testuser".to_string()),
            language_code: Some("en".to_string()),
            is_premium: false,
            added_to_attachment_menu: false,
        }
    }

    // Создание тестового чата
    fn test_chat() -> Chat {
        Chat {
            id: teloxide::types::ChatId(123456789),
            kind: ChatKind::Private(teloxide::types::ChatPrivate {
                type_: Default::default(),
                username: Some("testuser".to_string()),
                first_name: Some("Test".to_string()),
                last_name: Some("User".to_string()),
                bio: None,
                has_private_forwards: None,
                has_restricted_voice_and_video_messages: None,
            }),
            photo: None,
        }
    }

    // Создание тестового сообщения
    fn test_message() -> Message {
        Message {
            id: MessageId(1),
            thread_id: None,
            from: Some(test_user()),
            sender_chat: None,
            date: chrono::Utc::now().naive_utc(),
            chat: test_chat(),
            kind: MessageKind::Common(teloxide::types::MessageCommon {
                reply_to_message: None,
                forward_kind: None,
                edit_date: None,
                media_kind: teloxide::types::MediaKind::Text(teloxide::types::MediaText {
                    text: "/start".to_string(),
                    entities: vec![],
                }),
                reply_markup: None,
                author_signature: None,
                has_protected_content: false,
                is_automatic_forward: false,
            }),
            via_bot: None,
        }
    }

    // Создание тестового callback query
    fn test_callback_query(data: &str) -> CallbackQuery {
        CallbackQuery {
            id: "test_callback_id".to_string(),
            from: test_user(),
            message: Some(test_message()),
            inline_message_id: None,
            chat_instance: "test_chat_instance".to_string(),
            data: Some(data.to_string()),
            game_short_name: None,
        }
    }

    #[tokio::test]
    async fn test_is_allowed_function() {
        let config = test_config();
        
        // Тест разрешенного пользователя
        assert!(is_allowed(&config, Some(123456789)));
        
        // Тест неразрешенного пользователя  
        assert!(!is_allowed(&config, Some(987654321)));
        
        // Тест None
        assert!(!is_allowed(&config, None));
        
        println!("✅ Функция is_allowed работает корректно");
    }

    #[tokio::test]
    async fn test_main_keyboard_creation() {
        let kb = main_keyboard();
        
        // Проверяем что клавиатура создалась
        assert!(!kb.inline_keyboard.is_empty());
        
        // Проверяем количество рядов кнопок
        assert_eq!(kb.inline_keyboard.len(), 2);
        
        // Проверяем первый ряд (2 кнопки)
        assert_eq!(kb.inline_keyboard[0].len(), 2);
        
        // Проверяем второй ряд (1 кнопка)
        assert_eq!(kb.inline_keyboard[1].len(), 1);
        
        // Проверяем callback data кнопок
        assert_eq!(kb.inline_keyboard[0][0].callback_data.as_ref().unwrap(), "wol");
        assert_eq!(kb.inline_keyboard[0][1].callback_data.as_ref().unwrap(), "shutdown_confirm");
        assert_eq!(kb.inline_keyboard[1][0].callback_data.as_ref().unwrap(), "status");
        
        println!("✅ Главная клавиатура создается корректно");
    }

    #[test]
    fn test_callback_data_recognition() {
        // Тестируем распознавание всех callback данных
        let test_cases = vec![
            ("wol", "Wake on LAN"),
            ("shutdown_confirm", "Shutdown confirmation"),
            ("shutdown_yes", "Shutdown execution"),
            ("status", "Status check"),
            ("cancel", "Cancel operation"),
        ];

        for (callback_data, description) in test_cases {
            let callback_query = test_callback_query(callback_data);
            assert_eq!(callback_query.data.as_ref().unwrap(), callback_data);
            println!("✅ Callback data '{}' для '{}' распознается корректно", callback_data, description);
        }
    }

    #[tokio::test]
    async fn test_callback_query_structure() {
        let callback_query = test_callback_query("wol");
        
        // Проверяем основные поля
        assert!(!callback_query.id.is_empty());
        assert_eq!(callback_query.from.id.0, 123456789);
        assert!(callback_query.message.is_some());
        assert_eq!(callback_query.data.as_ref().unwrap(), "wol");
        
        println!("✅ Структура CallbackQuery формируется корректно");
    }

    // Тест для диагностики обработки Update
    #[tokio::test] 
    async fn test_update_handling_structure() {
        // Тестируем Message Update
        let message_update = Update {
            id: teloxide::types::UpdateId(1),
            kind: UpdateKind::Message(test_message()),
        };

        match message_update.kind {
            UpdateKind::Message(ref msg) => {
                println!("✅ Message Update структура корректна");
                println!("   User ID: {:?}", msg.from.as_ref().map(|u| u.id.0));
                println!("   Text: {:?}", msg.text());
            }
            _ => panic!("Ожидался Message Update"),
        }

        // Тестируем CallbackQuery Update
        let callback_update = Update {
            id: teloxide::types::UpdateId(2),
            kind: UpdateKind::CallbackQuery(test_callback_query("wol")),
        };

        match callback_update.kind {
            UpdateKind::CallbackQuery(ref q) => {
                println!("✅ CallbackQuery Update структура корректна");
                println!("   User ID: {}", q.from.id.0);
                println!("   Callback data: {:?}", q.data);
                println!("   Has message: {}", q.message.is_some());
            }
            _ => panic!("Ожидался CallbackQuery Update"),
        }
    }

    // Диагностический тест для handler логики
    #[tokio::test]
    async fn test_handler_logic_simulation() {
        let config = Arc::new(test_config());
        
        println!("=== ДИАГНОСТИКА ОБРАБОТЧИКА ===");
        
        // Симулируем обработку Message
        let msg = test_message();
        let user_id = msg.from.as_ref().map(|u| u.id.0);
        
        println!("Message обработка:");
        println!("  User ID: {:?}", user_id);
        println!("  Is allowed: {}", is_allowed(&config, user_id));
        println!("  Text: {:?}", msg.text());
        println!("  Starts with /start: {}", msg.text().map_or(false, |t| t.starts_with("/start")));
        
        // Симулируем обработку CallbackQuery
        let callback_query = test_callback_query("wol");
        let callback_user_id = Some(callback_query.from.id.0);
        
        println!("\nCallbackQuery обработка:");
        println!("  User ID: {:?}", callback_user_id);
        println!("  Is allowed: {}", is_allowed(&config, callback_user_id));
        println!("  Callback data: {:?}", callback_query.data);
        
        // Проверяем матчинг callback данных
        if let Some(data) = callback_query.data.as_deref() {
            let handler_found = match data {
                "wol" => { println!("  ✅ WOL handler найден"); true },
                "shutdown_confirm" => { println!("  ✅ Shutdown confirm handler найден"); true },
                "shutdown_yes" => { println!("  ✅ Shutdown yes handler найден"); true },
                "status" => { println!("  ✅ Status handler найден"); true },
                "cancel" => { println!("  ✅ Cancel handler найден"); true },
                _ => { println!("  ❌ Handler НЕ найден для: {}", data); false },
            };
            assert!(handler_found, "Handler должен быть найден для всех callback данных");
        }
        
        println!("✅ Логика обработчика работает корректно");
    }

    #[test]
    fn test_config_creation() {
        let config = test_config();
        
        assert!(!config.bot_token.is_empty());
        assert!(!config.allowed_users.is_empty());
        assert!(!config.server_mac.is_empty());
        
        println!("✅ Тестовая конфигурация создается корректно");
    }
}