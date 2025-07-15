import logging
import os
from telegram import Update, InlineKeyboardButton, InlineKeyboardMarkup
from telegram.ext import Updater, CommandHandler, CallbackContext, CallbackQueryHandler
import paramiko
import subprocess
import traceback

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
ROUTER_SSH_KEY_PATH = "/app/keys/id_router_vps_rsa_legacy"  # Ключ для роутера (RSA legacy)

# Для домашнего сервера (Shutdown, Status)
SERVER_SSH_HOST = "localhost"
SERVER_SSH_PORT = int(os.getenv("SERVER_SSH_PORT", "2222"))
SERVER_SSH_USER = os.getenv("SERVER_SSH_USER", "friedcerebrum")
SERVER_SSH_KEY_PATH = "/app/keys/id_rsa"  # Ключ для сервера (добавьте в keys/)


# --- Проверка обязательных переменных ---
if not all([BOT_TOKEN, ALLOWED_USERS, SERVER_MAC]):
    raise ValueError("Не все обязательные переменные окружения заданы (BOT_TOKEN, ALLOWED_USERS, SERVER_MAC)!")

# --- Декоратор для проверки доступа ---
def restricted(func):
    """Декоратор для ограничения доступа к боту"""
    def wrapped(update: Update, context: CallbackContext, *args, **kwargs):
        user = update.effective_user
        user_id = user.id if user is not None else None
        if user_id not in ALLOWED_USERS:
            logger.warning(f"Unauthorized access denied for {user_id}.")
            update.message.reply_text("🚫 Доступ запрещен!")
            return
        return func(update, context, *args, **kwargs)
    return wrapped

# --- Команды бота ---

def main_menu_keyboard():
    keyboard = [
        [
            InlineKeyboardButton("\U0001F50C Включить", callback_data='wol'),
            InlineKeyboardButton("\U0001F534 Выключить", callback_data='shutdown_confirm'),
        ],
        [
            InlineKeyboardButton("\U0001F7E2 Статус", callback_data='status')
        ]
    ]
    return InlineKeyboardMarkup(keyboard)

@restricted
def start(update: Update, context: CallbackContext):
    update.message.reply_text(
        "\U0001F680 **Серверный менеджер**\n\nВыберите действие:",
        parse_mode='Markdown',
        reply_markup=main_menu_keyboard()
    )

@restricted
def button_handler(update: Update, context: CallbackContext):
    query = update.callback_query
    user_id = query.from_user.id
    if user_id not in ALLOWED_USERS:
        query.answer()
        query.edit_message_text("\u274C Доступ запрещен!")
        return
    data = query.data
    if data == 'wol':
        query.answer()
        wake_on_lan_callback(query, context)
    elif data == 'shutdown_confirm':
        query.answer()
        confirm_keyboard = InlineKeyboardMarkup([
            [
                InlineKeyboardButton("\u2705 Да, выключить", callback_data='shutdown_yes'),
                InlineKeyboardButton("\u274C Нет", callback_data='cancel')
            ]
        ])
        query.edit_message_text(
            "\u26A0\ufe0f Вы уверены, что хотите выключить сервер?",
            reply_markup=confirm_keyboard
        )
    elif data == 'shutdown_yes':
        query.answer()
        shutdown_server_callback(query, context)
    elif data == 'cancel':
        query.answer("Отменено")
        query.edit_message_text(
            "\u274C Операция отменена.",
            reply_markup=main_menu_keyboard()
        )
    elif data == 'status':
        query.answer()
        server_status_callback(query, context)
    else:
        query.answer("Неизвестная команда")

# --- Callback versions of commands ---
def wake_on_lan_callback(query, context):
    query.edit_message_text("\u23F3 Отправляю команду на включение...")
    try:
        client = paramiko.SSHClient()
        client.set_missing_host_key_policy(paramiko.AutoAddPolicy())
        private_key = paramiko.RSAKey.from_private_key_file(ROUTER_SSH_KEY_PATH)
        client.connect(
            ROUTER_SSH_HOST,
            port=ROUTER_SSH_PORT,
            username=ROUTER_SSH_USER,
            pkey=private_key
        )
        stdin, stdout, stderr = client.exec_command(f"etherwake -i br-lan {SERVER_MAC}")
        raw_stdout = stdout.read()
        raw_stderr = stderr.read()
        try:
            output = raw_stdout.decode("utf-8")
        except Exception as e:
            output = f"[decode error: {e}] raw: {raw_stdout}"
        try:
            error = raw_stderr.decode("utf-8")
        except Exception as e:
            error = f"[decode error: {e}] raw: {raw_stderr}"
        logger.info(f"WOL output: {output}")
        logger.info(f"WOL error: {error}")
        client.close()
        if error.strip():
            query.edit_message_text(f"\u274C Ошибка при выполнении команды на роутере: {error}", reply_markup=main_menu_keyboard())
        else:
            query.edit_message_text("\U0001F50C **Magic packet отправлен!** Сервер должен скоро запуститься.", parse_mode='Markdown', reply_markup=main_menu_keyboard())
    except Exception as e:
        tb = traceback.format_exc()
        logger.error(f"WOL Error: {e}\n{tb}")
        tb_md = tb.replace('`', "'")
        query.edit_message_text(
            f"\u26A0\ufe0f **Ошибка подключения к роутеру:**\n`{str(e)}`\n\nТрейсбек:\n```\n{tb_md}\n```\n\nПроверьте, активен ли SSH-туннель от роутера.",
            parse_mode='Markdown',
            reply_markup=main_menu_keyboard()
        )

def shutdown_server_callback(query, context):
    query.edit_message_text("\u23F3 Отправляю команду на выключение...")
    try:
        client = paramiko.SSHClient()
        client.set_missing_host_key_policy(paramiko.AutoAddPolicy())
        private_key = paramiko.RSAKey.from_private_key_file(SERVER_SSH_KEY_PATH)
        client.connect(
            SERVER_SSH_HOST,
            port=SERVER_SSH_PORT,
            username=SERVER_SSH_USER,
            pkey=private_key
        )
        stdin, stdout, stderr = client.exec_command("sudo /sbin/shutdown -h now")
        error = stderr.read().decode()
        client.close()
        if error:
            query.edit_message_text(f"\u274C Ошибка при выполнении команды на сервере: {error}", reply_markup=main_menu_keyboard())
        else:
            query.edit_message_text("\U0001F534 **Команда выключения отправлена!** Сервер завершает работу.", parse_mode='Markdown', reply_markup=main_menu_keyboard())
    except Exception as e:
        logger.error(f"Shutdown Error: {e}")
        query.edit_message_text(f"\u26A0\ufe0f **Ошибка подключения к домашнему серверу:**\n`{str(e)}`\n\nСервер уже выключен или туннель неактивен.", parse_mode='Markdown', reply_markup=main_menu_keyboard())

def server_status_callback(query, context):
    try:
        result = subprocess.run(
            ["nc", "-z", "-w", "3", SERVER_SSH_HOST, str(SERVER_SSH_PORT)],
            capture_output=True, text=True
        )
        if result.returncode == 0:
            query.edit_message_text("\U0001F7E2 **Сервер онлайн** (SSH-туннель активен).", parse_mode='Markdown', reply_markup=main_menu_keyboard())
        else:
            query.edit_message_text("\U0001F534 **Сервер оффлайн** (SSH-туннель не отвечает).", parse_mode='Markdown', reply_markup=main_menu_keyboard())
    except Exception as e:
        logger.error(f"Status Check Error: {e}")
        query.edit_message_text(f"\u26A0\ufe0f Ошибка при проверке статуса: {str(e)}", reply_markup=main_menu_keyboard())

# Отключаем старые команды, чтобы не было дублирования
#@restricted
#def wake_on_lan(update: Update, context: CallbackContext):
#    ...
#@restricted
#def shutdown_server(update: Update, context: CallbackContext):
#    ...
#@restricted
#def server_status(update: Update, context: CallbackContext):
#    ...

def main():
    """Запуск бота"""
    if not os.path.exists(ROUTER_SSH_KEY_PATH) or not os.path.exists(SERVER_SSH_KEY_PATH):
        logger.error("SSH ключи не найдены по указанным путям! Проверьте Docker-том.")
        return
    if not BOT_TOKEN:
        logger.error("BOT_TOKEN не задан!")
        return
    updater = Updater(BOT_TOKEN)
    if updater is None:
        logger.error("Updater не инициализирован!")
        return
    dispatcher = updater.dispatcher
    if dispatcher is None:
        logger.error("Dispatcher не инициализирован!")
        return
    dispatcher.add_handler(CommandHandler("start", start))
    dispatcher.add_handler(CallbackQueryHandler(button_handler))
    # dispatcher.add_handler(CommandHandler("wol", wake_on_lan))
    # dispatcher.add_handler(CommandHandler("shutdown", shutdown_server))
    # dispatcher.add_handler(CommandHandler("status", server_status))
    updater.start_polling()
    logger.info("Бот запущен...")
    updater.idle()

if __name__ == '__main__':
    main()