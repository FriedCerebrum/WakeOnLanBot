use std::{env, net::TcpStream, path::Path, time::Duration, io::Read, collections::HashMap, sync::Mutex};

use anyhow::{Result};
use ssh2::Session;
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup, ParseMode, CallbackQuery},
    utils::command::BotCommands,
};
use std::sync::Arc;

mod handler;

// Добавляем глобальное состояние для предотвращения спама кнопок
lazy_static::lazy_static! {
    static ref BUTTON_LOCKS: Arc<Mutex<HashMap<u64, std::time::Instant>>> = Arc::new(Mutex::new(HashMap::new()));
}

#[cfg(test)]
mod tests;

#[tokio::main]
async fn main() {
    // МАКСИМАЛЬНО РАННЯЯ ДИАГНОСТИКА
    println!("=== СТАРТ ПРИЛОЖЕНИЯ ===");
    println!("Rust приложение запущено успешно!");
    println!("Версия Rust: {}", env!("CARGO_PKG_RUST_VERSION", "неизвестна"));
    println!("Пакет: {} v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    println!("Teloxide версия: {}", option_env!("CARGO_PKG_VERSION_teloxide").unwrap_or("неизвестна"));
    
    // Проверяем критические переменные окружения ДО инициализации логгера
    println!("=== ПРОВЕРКА ПЕРЕМЕННЫХ ОКРУЖЕНИЯ ===");
    match env::var("BOT_TOKEN") {
        Ok(token) => println!("BOT_TOKEN: найден (длина: {})", token.len()),
        Err(_) => println!("BOT_TOKEN: НЕ НАЙДЕН!"),
    }
    
    match env::var("ALLOWED_USERS") {
        Ok(users) => println!("ALLOWED_USERS: '{}'", users),
        Err(_) => println!("ALLOWED_USERS: НЕ НАЙДЕН!"),
    }
    
    match env::var("SERVER_MAC") {
        Ok(mac) => println!("SERVER_MAC: '{}'", mac),
        Err(_) => println!("SERVER_MAC: НЕ НАЙДЕН!"),
    }
    println!("=== КОНЕЦ ПРОВЕРКИ ПЕРЕМЕННЫХ ===");
    
    if let Err(e) = run().await {
        eprintln!("Ошибка запуска: {:#}", e);
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    println!("=== ИНИЦИАЛИЗАЦИЯ ЛОГГЕРА ===");
    // Инициализируем логирование
    pretty_env_logger::init();
    println!("Логгер инициализирован успешно");
    log::info!("Запуск WakeOnLanBot...");

    println!("=== ЧТЕНИЕ КОНФИГУРАЦИИ ===");
    // Читаем конфигурацию из переменных окружения
    let config = Config::from_env()?;
    println!("Конфигурация загружена успешно!");
    log::info!("Конфигурация успешно загружена!");

    println!("=== СОЗДАНИЕ БОТА ===");
    // Создаем экземпляр бота
    let bot = Bot::new(config.bot_token.clone());
    println!("Экземпляр бота создан успешно");
    log::info!("Экземпляр бота создан");
    
    // Проверяем связь с Telegram API
    println!("=== ПРОВЕРКА СВЯЗИ С TELEGRAM API ===");
    match bot.get_me().await {
        Ok(me) => {
            println!("✅ Связь с Telegram API работает!");
            println!("   Имя бота: {}", me.first_name);
            println!("   Username: @{}", me.username.as_ref().map(|s| s.as_str()).unwrap_or("НЕТ"));
            log::info!("Telegram API отвечает, бот: {}", me.first_name);
        },
        Err(e) => {
            println!("❌ ОШИБКА связи с Telegram API: {}", e);
            log::error!("Ошибка связи с Telegram API: {}", e);
            return Err(anyhow::anyhow!("Не могу подключиться к Telegram API: {}", e));
        }
    }

    let cfg = Arc::new(config);

    println!("=== ЗАПУСК ОБРАБОТЧИКА ===");
    log::info!("Запускаем обработчик событий...");
    
    // Важно: обработчик должен работать бесконечно
    println!("⚠️ ВНИМАНИЕ: Запускаем обработчик событий (это должно работать бесконечно)");
    handler::run(bot.clone(), cfg.clone()).await;
    
    // Если мы здесь - это означает, что обработчик завершился (что не должно происходить)
    println!("🚨 КРИТИЧЕСКАЯ ОШИБКА: Обработчик событий завершился!");
    log::error!("Обработчик событий завершился неожиданно!");

    Ok(())
}

#[derive(Clone)]
struct Config {
    bot_token: String,
    allowed_users: Vec<i64>,
    server_mac: String,

    router_ssh_host: String,
    router_ssh_port: u16,
    router_ssh_user: String,
    router_ssh_key: String,

    server_ssh_host: String,
    server_ssh_port: u16,
    server_ssh_user: String,
    server_ssh_key: String,

    ssh_timeout: Duration,
    nc_timeout: Duration,
}

impl Config {
    fn from_env() -> Result<Self> {
        println!("=== ДЕТАЛЬНОЕ ЧТЕНИЕ КОНФИГУРАЦИИ ===");
        log::info!("Начинаю чтение конфигурации из переменных окружения...");
        
        println!("Читаю BOT_TOKEN...");
        let bot_token = match env::var("BOT_TOKEN") {
            Ok(token) => {
                println!("BOT_TOKEN прочитан успешно (длина: {})", token.len());
                token
            },
            Err(e) => {
                println!("ОШИБКА: BOT_TOKEN не найден: {}", e);
                return Err(anyhow::anyhow!("BOT_TOKEN пуст"));
            }
        };
        log::info!("BOT_TOKEN прочитан успешно");
        
        println!("Читаю ALLOWED_USERS...");
        let allowed_users_str = env::var("ALLOWED_USERS").unwrap_or_default();
        println!("ALLOWED_USERS строка: '{}'", allowed_users_str);
        log::info!("ALLOWED_USERS строка: '{}'", allowed_users_str);
        
        let allowed_users = allowed_users_str
            .split(',')
            .filter_map(|s| s.trim().parse::<i64>().ok())
            .collect::<Vec<_>>();
        println!("ALLOWED_USERS распарсены: {:?}", allowed_users);
        log::info!("ALLOWED_USERS распарсены: {:?}", allowed_users);
        
        println!("Читаю SERVER_MAC...");
        let server_mac = match env::var("SERVER_MAC") {
            Ok(mac) => {
                // Валидируем MAC адрес для безопасности
                if !is_valid_mac(&mac) {
                    println!("ОШИБКА: SERVER_MAC имеет некорректный формат: '{}'", mac);
                    return Err(anyhow::anyhow!("SERVER_MAC имеет некорректный формат"));
                }
                println!("SERVER_MAC прочитан и валиден: '{}'", mac);
                mac
            },
            Err(e) => {
                println!("ОШИБКА: SERVER_MAC не найден: {}", e);
                return Err(anyhow::anyhow!("SERVER_MAC пуст"));
            }
        };
        log::info!("SERVER_MAC прочитан: '{}'", server_mac);

        if allowed_users.is_empty() {
            println!("ОШИБКА: ALLOWED_USERS список пуст после парсинга!");
            anyhow::bail!("ALLOWED_USERS пуст");
        }

        println!("Все обязательные переменные прочитаны успешно");
        log::info!("Все обязательные переменные прочитаны успешно");

        Ok(Self {
            bot_token,
            allowed_users,
            server_mac,

            router_ssh_host: env::var("ROUTER_SSH_HOST").unwrap_or_else(|_| "localhost".into()),
            router_ssh_port: env::var("ROUTER_SSH_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(2223),
            router_ssh_user: env::var("ROUTER_SSH_USER").unwrap_or_else(|_| "root".into()),
            router_ssh_key: env::var("ROUTER_SSH_KEY_PATH").unwrap_or_else(|_| "/app/keys/id_router_vps_rsa_legacy".into()),

            server_ssh_host: env::var("SERVER_SSH_HOST").unwrap_or_else(|_| "localhost".into()),
            server_ssh_port: env::var("SERVER_SSH_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(2222),
            server_ssh_user: env::var("SERVER_SSH_USER").unwrap_or_else(|_| "friedcerebrum".into()),
            server_ssh_key: env::var("SERVER_SSH_KEY_PATH").unwrap_or_else(|_| "/app/keys/id_rsa".into()),

            ssh_timeout: Duration::from_secs(
                env::var("SSH_TIMEOUT")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(15), // Увеличиваем таймаут
            ),
            nc_timeout: Duration::from_secs(
                env::var("NC_TIMEOUT")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(5), // Увеличиваем таймаут
            ),
        })
    }
}

// Функция валидации MAC адреса для безопасности
fn is_valid_mac(mac: &str) -> bool {
    let mac_regex = regex::Regex::new(r"^([0-9A-Fa-f]{2}[:-]){5}([0-9A-Fa-f]{2})$").unwrap();
    mac_regex.is_match(mac)
}

// Функция проверки дебаунсинга кнопок
fn check_button_debounce(user_id: u64) -> bool {
    let mut locks = BUTTON_LOCKS.lock().unwrap();
    let now = std::time::Instant::now();
    
    if let Some(last_press) = locks.get(&user_id) {
        if now.duration_since(*last_press) < Duration::from_secs(2) {
            return false; // Слишком быстро нажимает
        }
    }
    
    locks.insert(user_id, now);
    true
}

// --------------------------------------------------
#[derive(BotCommands)]
#[command(rename_rule = "lowercase", description = "Список команд")]
enum Command {
    #[command(description = "Показать главное меню")]
    Start,
}

fn is_allowed(config: &Config, user_id: Option<u64>) -> bool {
    match user_id {
        Some(uid) => {
            let uid = uid as i64;
            let allowed = config.allowed_users.contains(&uid);
            println!("🔐 Проверка авторизации: пользователь {} -> {}", uid, allowed);
            log::info!("Проверка авторизации: пользователь {} -> {}", uid, allowed);
            allowed
        },
        None => {
            println!("🔐 Проверка авторизации: пользователь None -> false");
            log::warn!("Попытка авторизации без user_id");
            false
        },
    }
}

async fn send_main_menu(bot: &Bot, msg: &Message, _config: &Config) -> Result<()> {
    println!("📤 Отправляем главное меню");
    let keyboard = main_keyboard();
    println!("⌨️ Клавиатура создана: {:?}", keyboard);
    log::info!("Отправляем главное меню с клавиатурой");
    
    bot.send_message(msg.chat.id, "🚀 Серверный менеджер\n\nВыберите действие:")
        .parse_mode(ParseMode::MarkdownV2)
        .reply_markup(keyboard)
        .await?;
    println!("✅ Главное меню отправлено");
    Ok(())
}

fn main_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("🔌 Включить", "wol"),
            InlineKeyboardButton::callback("🔴 Выключить", "shutdown_confirm"),
        ],
        vec![InlineKeyboardButton::callback("🟢 Статус", "status")],
    ])
}

// Централизованная функция установления SSH соединения
fn establish_ssh_connection(
    host: &str,
    port: u16,
    user: &str,
    key_path: &str,
    timeout: Duration,
) -> Result<Session> {
    log::info!("Устанавливаем SSH соединение с {}:{}", host, port);
    
    // Преобразуем localhost в 127.0.0.1 для корректного парсинга
    let resolved_host = if host == "localhost" {
        "127.0.0.1"
    } else {
        host
    };
    
    let addr = format!("{}:{}", resolved_host, port);
    log::debug!("Подключаемся к адресу: {}", addr);
    
    let tcp = TcpStream::connect_timeout(
        &addr.parse()?,
        timeout,
    )?;

    let mut sess = Session::new()?;
    sess.set_tcp_stream(tcp);
    sess.handshake()?;
    
    sess.userauth_pubkey_file(user, None, Path::new(key_path), None)?;

    if !sess.authenticated() {
        log::error!("SSH аутентификация не удалась для {}@{}:{}", user, host, port);
        anyhow::bail!("SSH аутентификация не удалась");
    }
    
    log::info!("SSH соединение установлено успешно");
    Ok(sess)
}

// Улучшенная обработка callback query с защитой от ошибок
async fn safe_answer_callback_query(bot: &Bot, callback_id: &str) -> Result<()> {
    match bot.answer_callback_query(callback_id).await {
        Ok(_) => {
            log::debug!("Callback query отвечен успешно");
            Ok(())
        },
        Err(e) => {
            log::warn!("Не удалось ответить на callback query: {}", e);
            // Не возвращаем ошибку, т.к. это не критично
            Ok(())
        }
    }
}

async fn handle_wol(bot: &Bot, q: &CallbackQuery, config: &Config) -> Result<()> {
    let user_id = q.from.id.0;
    println!("🔌 WOL Handler: Начало обработки для пользователя {}", user_id);
    log::info!("Обрабатываем WOL запрос от пользователя {}", user_id);
    
    // Проверяем дебаунсинг
    if !check_button_debounce(user_id) {
        log::warn!("Пользователь {} нажимает кнопки слишком быстро", user_id);
        safe_answer_callback_query(bot, &q.id).await?;
        if let Some(msg) = &q.message {
            bot.edit_message_text(msg.chat.id, msg.id, "⏳ Пожалуйста, подождите перед повторным нажатием")
                .await?;
        }
        return Ok(());
    }
    
    safe_answer_callback_query(bot, &q.id).await?;
    
    if let Some(msg) = &q.message {
        bot.edit_message_text(msg.chat.id, msg.id, "⏳ Отправляю команду на включение...")
            .await?;
    }

    match tokio::task::spawn_blocking({
        let cfg = config.clone();
        move || send_wol(&cfg)
    })
    .await?
    {
        Ok(_) => {
            if let Some(msg) = &q.message {
                bot.edit_message_text(
                    msg.chat.id,
                    msg.id,
                    "🔌 Magic packet отправлен!\n\nСервер должен запуститься в течение 30 секунд.",
                )
                .reply_markup(main_keyboard())
                .await?;
            }
        }
        Err(e) => {
            log::error!("Ошибка WOL: {}", e);
            if let Some(msg) = &q.message {
                bot.edit_message_text(
                    msg.chat.id, 
                    msg.id, 
                    "❌ Не удалось отправить команду включения.\nПроверьте настройки сети."
                )
                .reply_markup(main_keyboard())
                .await?;
            }
        }
    }

    Ok(())
}

fn send_wol(config: &Config) -> Result<()> {
    let sess = establish_ssh_connection(
        &config.router_ssh_host,
        config.router_ssh_port,
        &config.router_ssh_user,
        &config.router_ssh_key,
        config.ssh_timeout,
    )?;

    let mut ch = sess.channel_session()?;
    
    // Используем безопасное форматирование команды
    let safe_mac = config.server_mac.replace(|c: char| !c.is_ascii_hexdigit() && c != ':' && c != '-', "");
    let command = format!("etherwake -i br-lan {}", safe_mac);
    
    log::info!("Выполняем WOL команду: {}", command);
    ch.exec(&command)?;
    ch.close()?;
    
    Ok(())
}

async fn ask_shutdown_confirm(bot: &Bot, q: &CallbackQuery) -> Result<()> {
    let user_id = q.from.id.0;
    println!("🔴 Shutdown Confirm Handler: Начало обработки для пользователя {}", user_id);
    log::info!("Запрос подтверждения выключения от пользователя {}", user_id);
    
    // Проверяем дебаунсинг
    if !check_button_debounce(user_id) {
        log::warn!("Пользователь {} нажимает кнопки слишком быстро", user_id);
        safe_answer_callback_query(bot, &q.id).await?;
        return Ok(());
    }
    
    safe_answer_callback_query(bot, &q.id).await?;
    
    if let Some(msg) = &q.message {
        let kb = InlineKeyboardMarkup::new(vec![vec![
            InlineKeyboardButton::callback("✅ Да, выключить", "shutdown_yes"),
            InlineKeyboardButton::callback("❌ Отмена", "cancel"),
        ]]);
        bot.edit_message_text(msg.chat.id, msg.id, "⚠️ Подтверждение\n\nВы уверены, что хотите выключить сервер?")
            .parse_mode(ParseMode::MarkdownV2)
            .reply_markup(kb)
            .await?;
    }
    Ok(())
}

async fn handle_shutdown(bot: &Bot, q: &CallbackQuery, config: &Config) -> Result<()> {
    let user_id = q.from.id.0;
    log::info!("Обрабатываем запрос выключения от пользователя {}", user_id);
    
    // Проверяем дебаунсинг
    if !check_button_debounce(user_id) {
        log::warn!("Пользователь {} нажимает кнопки слишком быстро", user_id);
        safe_answer_callback_query(bot, &q.id).await?;
        return Ok(());
    }
    
    safe_answer_callback_query(bot, &q.id).await?;
    
    if let Some(msg) = &q.message {
        bot.edit_message_text(msg.chat.id, msg.id, "⏳ Отправляю команду на выключение...")
            .await?;
    }

    match tokio::task::spawn_blocking({
        let cfg = config.clone();
        move || send_shutdown(&cfg)
    })
    .await?
    {
        Ok(_) => {
            if let Some(msg) = &q.message {
                bot.edit_message_text(
                    msg.chat.id, 
                    msg.id, 
                    "🔴 Команда выключения отправлена!"
                )
                .reply_markup(main_keyboard())
                .await?;
            }
        }
        Err(e) => {
            log::error!("Ошибка выключения: {}", e);
            if let Some(msg) = &q.message {
                bot.edit_message_text(
                    msg.chat.id, 
                    msg.id, 
                    "❌ Не удалось отправить команду выключения.\nПроверьте настройки SSH."
                )
                .reply_markup(main_keyboard())
                .await?;
            }
        }
    }
    Ok(())
}

fn send_shutdown(config: &Config) -> Result<()> {
    let sess = establish_ssh_connection(
        &config.server_ssh_host,
        config.server_ssh_port,
        &config.server_ssh_user,
        &config.server_ssh_key,
        config.ssh_timeout,
    )?;

    let mut ch = sess.channel_session()?;
    
    log::info!("Выполняем команду выключения");
    ch.exec("sudo /sbin/shutdown -h now")?;
    ch.close()?;
    
    Ok(())
}

async fn handle_status(bot: &Bot, q: &CallbackQuery, config: &Config) -> Result<()> {
    let user_id = q.from.id.0;
    println!("🟢 Status Handler: Начало обработки для пользователя {}", user_id);
    log::info!("Проверяем статус сервера по запросу пользователя {}", user_id);
    
    // Проверяем дебаунсинг
    if !check_button_debounce(user_id) {
        log::warn!("Пользователь {} нажимает кнопки слишком быстро", user_id);
        safe_answer_callback_query(bot, &q.id).await?;
        return Ok(());
    }
    
    safe_answer_callback_query(bot, &q.id).await?;
    
    if let Some(msg) = &q.message {
        bot.edit_message_text(msg.chat.id, msg.id, "⏳ Проверяю статус сервера...")
            .await?;
    }

    match tokio::time::timeout(config.nc_timeout, check_status(config.clone())).await {
        Ok(Ok(info)) => {
            if let Some(msg) = &q.message {
                bot.edit_message_text(msg.chat.id, msg.id, info)
                    .reply_markup(main_keyboard())
                    .await?;
            }
        }
        Ok(Err(e)) => {
            log::error!("Ошибка проверки статуса: {}", e);
            if let Some(msg) = &q.message {
                bot.edit_message_text(
                    msg.chat.id, 
                    msg.id, 
                    "❌ Не удалось проверить статус сервера.\nПроверьте настройки SSH."
                )
                .reply_markup(main_keyboard())
                .await?;
            }
        }
        Err(_) => {
            if let Some(msg) = &q.message {
                bot.edit_message_text(
                    msg.chat.id, 
                    msg.id, 
                    "⏱️ Таймаут проверки статуса!"
                )
                .reply_markup(main_keyboard())
                .await?;
            }
        }
    }

    Ok(())
}

async fn check_status(config: Config) -> Result<String> {
    // Преобразуем localhost в 127.0.0.1 для корректного соединения
    let resolved_host = if config.server_ssh_host == "localhost" {
        "127.0.0.1"
    } else {
        &config.server_ssh_host
    };
    
    let addr = format!("{}:{}", resolved_host, config.server_ssh_port);
    log::debug!("Проверяем статус по адресу: {}", addr);
    
    match tokio::net::TcpStream::connect(addr.clone()).await {
        Ok(_) => {
            // Пробуем более детально получить uptime
            match tokio::task::spawn_blocking(move || {
                let sess = establish_ssh_connection(
                    &config.server_ssh_host,
                    config.server_ssh_port,
                    &config.server_ssh_user,
                    &config.server_ssh_key,
                    config.ssh_timeout,
                )?;
                
                let mut ch = sess.channel_session()?;
                ch.exec("uptime")?;
                let mut s = String::new();
                ch.read_to_string(&mut s)?;
                ch.close()?;
                Ok::<_, anyhow::Error>(format!("🟢 Сервер онлайн\n\n{}", s.trim()))
            })
            .await
            {
                Ok(Ok(s)) => Ok(s),
                Ok(Err(e)) => {
                    log::warn!("Не удалось получить uptime: {}", e);
                    Ok("🟢 Сервер онлайн\n\nSSH-туннель активен.".into())
                },
                Err(_) => Ok("🟢 Сервер онлайн\n\nSSH-туннель активен.".into()),
            }
        }
        Err(_) => Ok("🔴 Сервер оффлайн\n\nSSH-туннель не отвечает.".into()),
    }
}

async fn cancel(bot: &Bot, q: &CallbackQuery) -> Result<()> {
    log::info!("Отмена операции пользователем {}", q.from.id.0);
    
    safe_answer_callback_query(bot, &q.id).await?;
    
    if let Some(msg) = &q.message {
        bot.edit_message_text(msg.chat.id, msg.id, "❌ Операция отменена")
            .reply_markup(main_keyboard())
            .await?;
    }
    Ok(())
}
// Note: Handler functions are now directly integrated into the dispatcher
// These were the original wrapper functions that are no longer needed 