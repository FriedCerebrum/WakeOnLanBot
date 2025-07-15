import logging
import os
from telegram import Update, InlineKeyboardButton, InlineKeyboardMarkup
from telegram.ext import Updater, CommandHandler, CallbackContext, CallbackQueryHandler
import paramiko
import subprocess
import traceback
import time
from functools import wraps

# --- Конфигурация ---
logging.basicConfig(
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s',
    level=logging.INFO
)
logger = logging.getLogger(__name__)

# --- Получение переменных окружения ---
BOT_TOKEN = os.getenv("BOT_TOKEN")
# ALLOWED_USERS должен быть списком числовых ID
ALLOWED_USERS = [int(uid) for uid in os.getenv("ALLOWED_USERS", "").replace(' ', '').split(',') if uid.isdigit()]
SERVER_MAC = os.getenv("SERVER_MAC")

# --- Настройки для подключения через туннели ---
# Для роутера (WOL)
ROUTER_SSH_HOST = "localhost"
ROUTER_SSH_PORT = int(os.getenv("ROUTER_SSH_PORT", "2223"))
ROUTER_SSH_USER = os.getenv("ROUTER_SSH_USER", "root")
ROUTER_SSH_KEY_PATH = "/app/keys/id_router_vps_rsa_legacy"

# Для домашнего сервера (Shutdown, Status)
SERVER_SSH_HOST = "localhost"
SERVER_SSH_PORT = int(os.getenv("SERVER_SSH_PORT", "2222"))
SERVER_SSH_USER = os.getenv("SERVER_SSH_USER", "friedcerebrum")
SERVER_SSH_KEY_PATH = "/app/keys/id_rsa"

# Таймауты
SSH_TIMEOUT = 10
NC_TIMEOUT = 3

# --- Проверка обязательных переменных ---
if not all([BOT_TOKEN, ALLOWED_USERS, SERVER_MAC]):
    raise ValueError("Не все обязательные переменные окружения заданы (BOT_TOKEN, ALLOWED_USERS, SERVER_MAC)!")

# --- Декоратор для проверки доступа ---
def restricted(func):
    """Декоратор для ограничения доступа к боту"""
    @wraps(func)
    def wrapped(update: Update, context: CallbackContext, *args, **kwargs):
        user = update.effective_user
        user_id = user.id if user is not None else None
        
        if user_id not in ALLOWED_USERS:
            logger.warning(f"Unauthorized access denied for {user_id}.")
            
            # Определяем тип обновления
            if update.message:
                update.message.reply_text("🚫 Доступ запрещен!")
            elif update.callback_query:
                update.callback_query.answer("🚫 Доступ запрещен!", show_alert=True)
                update.callback_query.edit_message_text("🚫 Доступ запрещен!")
            return
            
        return func(update, context, *args, **kwargs)
    return wrapped

# --- Вспомогательные функции ---

def escape_markdown(text):
    """Экранирует специальные символы для Markdown"""
    escape_chars = ['_', '*', '[', ']', '(', ')', '~', '`', '>', '#', '+', '-', '=', '|', '{', '}', '.', '!']
    for char in escape_chars:
        text = text.replace(char, f'\\{char}')
    return text

def main_menu_keyboard():
    """Создает клавиатуру главного меню"""
    keyboard = [
        [
            InlineKeyboardButton("🔌 Включить", callback_data='wol'),
            InlineKeyboardButton("🔴 Выключить", callback_data='shutdown_confirm'),
        ],
        [
            InlineKeyboardButton("🟢 Статус", callback_data='status')
        ]
    ]
    return InlineKeyboardMarkup(keyboard)

def check_ssh_key_exists(key_path):
    """Проверяет существование SSH ключа"""
    if not os.path.exists(key_path):
        logger.error(f"SSH ключ не найден: {key_path}")
        return False
    return True

# --- Команды бота ---

@restricted
def start(update: Update, context: CallbackContext):
    """Обработчик команды /start"""
    user = update.effective_user
    user_id = user.id if user is not None else 'unknown'
    logger.info(f"User {user_id} started the bot")
    
    if update.message:
        update.message.reply_text(
            "🚀 **Серверный менеджер**\n\nВыберите действие:",
            parse_mode='Markdown',
            reply_markup=main_menu_keyboard()
        )

@restricted
def button_handler(update: Update, context: CallbackContext):
    """Обработчик нажатий на inline кнопки"""
    query = update.callback_query
    
    # Всегда отвечаем на callback, чтобы убрать "часики"
    query.answer()
    
    data = query.data
    logger.info(f"User {query.from_user.id} pressed button: {data}")
    
    handlers = {
        'wol': wake_on_lan_callback,
        'shutdown_confirm': shutdown_confirm_callback,
        'shutdown_yes': shutdown_server_callback,
        'cancel': cancel_callback,
        'status': server_status_callback,
        'back_to_menu': back_to_menu_callback
    }
    
    handler = handlers.get(data)
    if handler:
        handler(query, context)
    else:
        logger.warning(f"Unknown callback data: {data}")
        query.edit_message_text(
            "⚠️ Неизвестная команда",
            reply_markup=main_menu_keyboard()
        )

# --- Callback функции ---

def wake_on_lan_callback(query, context):
    """Включение сервера через Wake-on-LAN"""
    query.edit_message_text("⏳ Отправляю команду на включение...")
    
    # Проверяем SSH ключ
    if not check_ssh_key_exists(ROUTER_SSH_KEY_PATH):
        query.edit_message_text(
            "⚠️ **Ошибка:** SSH ключ для роутера не найден",
            parse_mode='Markdown',
            reply_markup=main_menu_keyboard()
        )
        return
    
    client = None
    try:
        # Подключаемся к роутеру
        client = paramiko.SSHClient()
        client.set_missing_host_key_policy(paramiko.AutoAddPolicy())
        
        private_key = paramiko.RSAKey.from_private_key_file(ROUTER_SSH_KEY_PATH)
        
        client.connect(
            ROUTER_SSH_HOST,
            port=ROUTER_SSH_PORT,
            username=ROUTER_SSH_USER,
            pkey=private_key,
            timeout=SSH_TIMEOUT
        )
        
        # Выполняем команду WOL
        command = f"etherwake -i br-lan {SERVER_MAC}"
        stdin, stdout, stderr = client.exec_command(command, timeout=SSH_TIMEOUT)
        
        output = stdout.read().decode('utf-8', errors='replace').strip()
        error = stderr.read().decode('utf-8', errors='replace').strip()
        
        logger.info(f"WOL output: {output}")
        if error:
            logger.error(f"WOL error: {error}")
        
        if error:
            query.edit_message_text(
                f"❌ **Ошибка при выполнении команды:**\n`{escape_markdown(error)}`",
                parse_mode='Markdown',
                reply_markup=main_menu_keyboard()
            )
        else:
            query.edit_message_text(
                "🔌 **Magic packet отправлен!**\n\nСервер должен запуститься в течение 30 секунд.",
                parse_mode='Markdown',
                reply_markup=main_menu_keyboard()
            )
            
    except paramiko.AuthenticationException:
        logger.error("SSH Authentication failed")
        query.edit_message_text(
            "⚠️ **Ошибка аутентификации SSH**\n\nПроверьте ключ и настройки подключения.",
            parse_mode='Markdown',
            reply_markup=main_menu_keyboard()
        )
    except paramiko.SSHException as e:
        logger.error(f"SSH Error: {e}")
        query.edit_message_text(
            f"⚠️ **Ошибка SSH:** `{escape_markdown(str(e))}`",
            parse_mode='Markdown',
            reply_markup=main_menu_keyboard()
        )
    except Exception as e:
        logger.error(f"WOL Error: {e}", exc_info=True)
        query.edit_message_text(
            f"⚠️ **Ошибка подключения к роутеру:**\n`{escape_markdown(str(e))}`\n\nПроверьте SSH-туннель.",
            parse_mode='Markdown',
            reply_markup=main_menu_keyboard()
        )
    finally:
        if client:
            client.close()

def shutdown_confirm_callback(query, context):
    """Запрос подтверждения выключения"""
    confirm_keyboard = InlineKeyboardMarkup([
        [
            InlineKeyboardButton("✅ Да, выключить", callback_data='shutdown_yes'),
            InlineKeyboardButton("❌ Отмена", callback_data='cancel')
        ]
    ])
    
    query.edit_message_text(
        "⚠️ **Подтверждение**\n\nВы уверены, что хотите выключить сервер?",
        parse_mode='Markdown',
        reply_markup=confirm_keyboard
    )

def shutdown_server_callback(query, context):
    """Выключение сервера"""
    query.edit_message_text("⏳ Отправляю команду на выключение...")
    
    # Проверяем SSH ключ
    if not check_ssh_key_exists(SERVER_SSH_KEY_PATH):
        query.edit_message_text(
            "⚠️ **Ошибка:** SSH ключ для сервера не найден",
            parse_mode='Markdown',
            reply_markup=main_menu_keyboard()
        )
        return
    
    client = None
    try:
        # Подключаемся к серверу
        client = paramiko.SSHClient()
        client.set_missing_host_key_policy(paramiko.AutoAddPolicy())
        
        private_key = paramiko.RSAKey.from_private_key_file(SERVER_SSH_KEY_PATH)
        
        client.connect(
            SERVER_SSH_HOST,
            port=SERVER_SSH_PORT,
            username=SERVER_SSH_USER,
            pkey=private_key,
            timeout=SSH_TIMEOUT
        )
        
        # Выполняем команду выключения
        stdin, stdout, stderr = client.exec_command("sudo /sbin/shutdown -h now", timeout=SSH_TIMEOUT)
        
        error = stderr.read().decode('utf-8', errors='replace').strip()
        
        if error and "sudo" not in error.lower():  # Игнорируем предупреждения sudo
            logger.error(f"Shutdown error: {error}")
            query.edit_message_text(
                f"❌ **Ошибка при выключении:**\n`{escape_markdown(error)}`",
                parse_mode='Markdown',
                reply_markup=main_menu_keyboard()
            )
        else:
            query.edit_message_text(
                "🔴 **Команда выключения отправлена!**\n\nСервер завершает работу.",
                parse_mode='Markdown',
                reply_markup=main_menu_keyboard()
            )
            
    except paramiko.AuthenticationException:
        logger.error("SSH Authentication failed")
        query.edit_message_text(
            "⚠️ **Ошибка аутентификации SSH**\n\nПроверьте ключ и настройки подключения.",
            parse_mode='Markdown',
            reply_markup=main_menu_keyboard()
        )
    except Exception as e:
        logger.error(f"Shutdown Error: {e}", exc_info=True)
        query.edit_message_text(
            f"⚠️ **Ошибка подключения к серверу:**\n`{escape_markdown(str(e))}`\n\nВозможно, сервер уже выключен или туннель неактивен.",
            parse_mode='Markdown',
            reply_markup=main_menu_keyboard()
        )
    finally:
        if client:
            client.close()

def server_status_callback(query, context):
    """Проверка статуса сервера"""
    query.edit_message_text("⏳ Проверяю статус сервера...")
    
    try:
        # Проверяем доступность SSH порта
        result = subprocess.run(
            ["nc", "-z", "-w", str(NC_TIMEOUT), SERVER_SSH_HOST, str(SERVER_SSH_PORT)],
            capture_output=True,
            text=True,
            timeout=NC_TIMEOUT + 2
        )
        
        if result.returncode == 0:
            # Пробуем подключиться по SSH для более точной проверки
            client = None
            try:
                if check_ssh_key_exists(SERVER_SSH_KEY_PATH):
                    client = paramiko.SSHClient()
                    client.set_missing_host_key_policy(paramiko.AutoAddPolicy())
                    private_key = paramiko.RSAKey.from_private_key_file(SERVER_SSH_KEY_PATH)
                    
                    client.connect(
                        SERVER_SSH_HOST,
                        port=SERVER_SSH_PORT,
                        username=SERVER_SSH_USER,
                        pkey=private_key,
                        timeout=5
                    )
                    
                    # Получаем uptime
                    stdin, stdout, stderr = client.exec_command("uptime", timeout=5)
                    uptime_info = stdout.read().decode('utf-8', errors='replace').strip()
                    
                    query.edit_message_text(
                        f"🟢 **Сервер онлайн**\n\n`{escape_markdown(uptime_info)}`",
                        parse_mode='Markdown',
                        reply_markup=main_menu_keyboard()
                    )
                else:
                    query.edit_message_text(
                        "🟢 **Сервер онлайн**\n\nSSH-туннель активен.",
                        parse_mode='Markdown',
                        reply_markup=main_menu_keyboard()
                    )
                    
            except Exception as e:
                logger.warning(f"Could not get detailed status: {e}")
                query.edit_message_text(
                    "🟢 **Сервер онлайн**\n\nSSH-туннель активен.",
                    parse_mode='Markdown',
                    reply_markup=main_menu_keyboard()
                )
            finally:
                if client:
                    client.close()
        else:
            query.edit_message_text(
                "🔴 **Сервер оффлайн**\n\nSSH-туннель не отвечает.",
                parse_mode='Markdown',
                reply_markup=main_menu_keyboard()
            )
            
    except subprocess.TimeoutExpired:
        query.edit_message_text(
            "⚠️ **Таймаут при проверке статуса**\n\nСервер не отвечает.",
            parse_mode='Markdown',
            reply_markup=main_menu_keyboard()
        )
    except Exception as e:
        logger.error(f"Status Check Error: {e}", exc_info=True)
        query.edit_message_text(
            f"⚠️ **Ошибка при проверке статуса:**\n`{escape_markdown(str(e))}`",
            parse_mode='Markdown',
            reply_markup=main_menu_keyboard()
        )

def cancel_callback(query, context):
    """Отмена операции"""
    query.edit_message_text(
        "❌ Операция отменена",
        reply_markup=main_menu_keyboard()
    )

def back_to_menu_callback(query, context):
    """Возврат в главное меню"""
    query.edit_message_text(
        "🚀 **Серверный менеджер**\n\nВыберите действие:",
        parse_mode='Markdown',
        reply_markup=main_menu_keyboard()
    )

def error_handler(update: object, context: CallbackContext):
    """Обработчик ошибок"""
    logger.error(f"Update {update} caused error {context.error}", exc_info=True)
    
    # Пытаемся отправить сообщение об ошибке пользователю
    try:
        from telegram import Update as TgUpdate
        if isinstance(update, TgUpdate) and update.effective_message:
            update.effective_message.reply_text(
                "⚠️ Произошла ошибка при обработке команды. Попробуйте еще раз.",
                reply_markup=main_menu_keyboard()
            )
    except:
        pass

def main():
    """Запуск бота"""
    # Проверяем наличие SSH ключей при старте
    missing_keys = []
    if not os.path.exists(ROUTER_SSH_KEY_PATH):
        missing_keys.append(f"Router key: {ROUTER_SSH_KEY_PATH}")
    if not os.path.exists(SERVER_SSH_KEY_PATH):
        missing_keys.append(f"Server key: {SERVER_SSH_KEY_PATH}")
    
    if missing_keys:
        logger.error(f"SSH ключи не найдены:\n" + "\n".join(missing_keys))
        logger.error("Проверьте Docker-том и наличие файлов ключей.")
        return
    
    if not BOT_TOKEN:
        logger.error("BOT_TOKEN не задан!")
        return
    
    if not ALLOWED_USERS:
        logger.error("ALLOWED_USERS не заданы!")
        return
    
    logger.info(f"Starting bot with allowed users: {ALLOWED_USERS}")
    
    # Создаем updater
    updater = Updater(BOT_TOKEN, use_context=True)
    
    if not updater:
        logger.error("Не удалось создать Updater!")
        return
    
    dispatcher = updater.dispatcher
    
    if dispatcher is None:
        logger.error("Dispatcher is None!")
        return
    # Регистрируем обработчики
    dispatcher.add_handler(CommandHandler("start", start))
    dispatcher.add_handler(CallbackQueryHandler(button_handler))
    dispatcher.add_error_handler(error_handler)
    
    # Запускаем бота
    try:
        updater.start_polling(drop_pending_updates=True)
        logger.info("Бот успешно запущен!")
        updater.idle()
    except Exception as e:
        logger.error(f"Ошибка при запуске бота: {e}", exc_info=True)

if __name__ == '__main__':
    main()
