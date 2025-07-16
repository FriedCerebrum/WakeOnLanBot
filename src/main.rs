use std::{env, net::TcpStream, path::Path, time::Duration, io::Read};

use anyhow::{Result, Context};
use ssh2::Session;
use teloxide::{
    dispatching::{HandlerExt, UpdateFilterExt},
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup, ParseMode},
    utils::command::BotCommands,
};
use teloxide::dptree::{endpoint, deps};
use teloxide::prelude::ResponseResult;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("Ошибка запуска: {:#}", e);
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    // Инициализируем логирование
    pretty_env_logger::init();
    log::info!("Запуск WakeOnLanBot...");

    // Читаем конфигурацию из переменных окружения
    let config = Config::from_env()?;

    // Создаем экземпляр бота
    let bot = Bot::new(config.bot_token.clone());

    let cfg = Arc::new(config);

    let handler = dptree::entry()
        .filter(|upd: Update, cfg: Arc<Config>| is_allowed(&cfg, &upd))
        .branch(
            Update::filter_message()
                .filter_command::<Command>()
                .endpoint(endpoint(command_handler)),
        )
        .branch(
            Update::filter_callback_query()
                .endpoint(endpoint(callback_handler)),
        );

    Dispatcher::builder(bot.clone(), handler)
        .dependencies(deps![cfg])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

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
        let bot_token = env::var("BOT_TOKEN").context("BOT_TOKEN пуст")?;
        let allowed_users = env::var("ALLOWED_USERS")
            .unwrap_or_default()
            .split(',')
            .filter_map(|s| s.trim().parse::<i64>().ok())
            .collect::<Vec<_>>();
        let server_mac = env::var("SERVER_MAC").context("SERVER_MAC пуст")?;

        if allowed_users.is_empty() {
            anyhow::bail!("ALLOWED_USERS пуст");
        }

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
                    .unwrap_or(10),
            ),
            nc_timeout: Duration::from_secs(
                env::var("NC_TIMEOUT")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(3),
            ),
        })
    }
}

// --------------------------------------------------
#[derive(BotCommands)]
#[command(rename_rule = "lowercase", description = "Список команд")]
enum Command {
    #[command(description = "Показать главное меню")]
    Start,
}

fn is_allowed(config: &Config, upd: &Update) -> bool {
    match upd.clone().user() {
        Some(user) => {
            let uid = user.id.0 as i64;
            config.allowed_users.contains(&uid)
        },
        None => false,
    }
}

async fn send_main_menu(bot: &Bot, msg: &Message, _config: &Config) -> Result<()> {
    bot.send_message(msg.chat.id, "🚀 Серверный менеджер\n\nВыберите действие:")
        .parse_mode(ParseMode::MarkdownV2)
        .reply_markup(main_keyboard())
        .await?;
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

async fn handle_wol(bot: &Bot, q: &CallbackQuery, config: &Config) -> Result<()> {
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
                .await?;
            }
        }
        Err(e) => {
            if let Some(msg) = &q.message {
                bot.edit_message_text(msg.chat.id, msg.id, format!("❌ Ошибка: {}", e)).await?;
            }
        }
    }

    Ok(())
}

fn send_wol(config: &Config) -> Result<()> {
    let tcp = TcpStream::connect_timeout(
        &format!("{}:{}", config.router_ssh_host, config.router_ssh_port).parse()?,
        config.ssh_timeout,
    )?;

    let mut sess = Session::new()?;
    sess.set_tcp_stream(tcp);
    sess.handshake()?;
    sess.userauth_pubkey_file(
        &config.router_ssh_user,
        None,
        Path::new(&config.router_ssh_key),
        None,
    )?;

    if !sess.authenticated() {
        anyhow::bail!("SSH authentication failed");
    }

    let mut ch = sess.channel_session()?;
    ch.exec(&format!("etherwake -i br-lan {}", config.server_mac))?;
    ch.close()?;
    Ok(())
}

async fn ask_shutdown_confirm(bot: &Bot, q: &CallbackQuery) -> Result<()> {
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
                bot.edit_message_text(msg.chat.id, msg.id, "🔴 Команда выключения отправлена!").await?;
            }
        }
        Err(e) => {
            if let Some(msg) = &q.message {
                bot.edit_message_text(msg.chat.id, msg.id, format!("❌ Ошибка: {}", e)).await?;
            }
        }
    }
    Ok(())
}

fn send_shutdown(config: &Config) -> Result<()> {
    let tcp = TcpStream::connect_timeout(
        &format!("{}:{}", config.server_ssh_host, config.server_ssh_port).parse()?,
        config.ssh_timeout,
    )?;

    let mut sess = Session::new()?;
    sess.set_tcp_stream(tcp);
    sess.handshake()?;
    sess.userauth_pubkey_file(
        &config.server_ssh_user,
        None,
        Path::new(&config.server_ssh_key),
        None,
    )?;

    if !sess.authenticated() {
        anyhow::bail!("SSH authentication failed");
    }

    let mut ch = sess.channel_session()?;
    ch.exec("sudo /sbin/shutdown -h now")?;
    ch.close()?;
    Ok(())
}

async fn handle_status(bot: &Bot, q: &CallbackQuery, config: &Config) -> Result<()> {
    if let Some(msg) = &q.message {
        bot.edit_message_text(msg.chat.id, msg.id, "⏳ Проверяю статус сервера...")
            .await?;
    }

    match tokio::time::timeout(config.nc_timeout, check_status(config.clone())).await {
        Ok(Ok(info)) => {
            if let Some(msg) = &q.message {
                bot.edit_message_text(msg.chat.id, msg.id, info).await?;
            }
        }
        Ok(Err(e)) => {
            if let Some(msg) = &q.message {
                bot.edit_message_text(msg.chat.id, msg.id, format!("❌ Ошибка: {}", e)).await?;
            }
        }
        Err(_) => {
            if let Some(msg) = &q.message {
                bot.edit_message_text(msg.chat.id, msg.id, "⏱️ Таймаут!").await?;
            }
        }
    }

    Ok(())
}

async fn check_status(config: Config) -> Result<String> {
    let addr = format!("{}:{}", config.server_ssh_host, config.server_ssh_port);
    match tokio::net::TcpStream::connect(addr.clone()).await {
        Ok(_) => {
            // Пробуем более детально получить uptime
            match tokio::task::spawn_blocking(move || {
                let tcp = TcpStream::connect(addr)?;
                let mut sess = Session::new()?;
                sess.set_tcp_stream(tcp);
                sess.handshake()?;
                sess.userauth_pubkey_file(
                    &config.server_ssh_user,
                    None,
                    Path::new(&config.server_ssh_key),
                    None,
                )?;
                if !sess.authenticated() {
                    anyhow::bail!("SSH auth failed");
                }
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
                _ => Ok("🟢 Сервер онлайн\n\nSSH-туннель активен.".into()),
            }
        }
        Err(_) => Ok("🔴 Сервер оффлайн\n\nSSH-туннель не отвечает.".into()),
    }
}

async fn cancel(bot: &Bot, q: &CallbackQuery) -> Result<()> {
    if let Some(msg) = &q.message {
        bot.edit_message_text(msg.chat.id, msg.id, "❌ Операция отменена")
            .reply_markup(main_keyboard())
            .await?;
    }
    Ok(())
}
// ----------- Endpoint handlers ------------

async fn command_handler(cfg: Arc<Config>, bot: Bot, msg: Message, cmd: Command) -> ResponseResult<()> {
    match cmd {
        Command::Start => {
            if let Err(e) = send_main_menu(&bot, &msg, &cfg).await {
                log::error!("Ошибка send_main_menu: {e}");
            }
        }
    }
    Ok(())
}

async fn callback_handler(cfg: Arc<Config>, bot: Bot, q: CallbackQuery) -> ResponseResult<()> {
    if let Some(data) = q.data.as_deref() {
        let res = match data {
            "wol" => handle_wol(&bot, &q, &cfg).await,
            "shutdown_confirm" => ask_shutdown_confirm(&bot, &q).await,
            "shutdown_yes" => handle_shutdown(&bot, &q, &cfg).await,
            "status" => handle_status(&bot, &q, &cfg).await,
            "cancel" => cancel(&bot, &q).await,
            _ => Ok(()),
        };
        if let Err(e) = res {
            log::error!("Ошибка callback {data}: {e}");
        }
    }
    Ok(())
} 