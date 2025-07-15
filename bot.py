import logging
import os
from telegram import Update
from telegram.ext import Updater, CommandHandler, CallbackContext
import paramiko
import subprocess

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
ROUTER_SSH_KEY_PATH = "/app/keys/id_router_vps"  # Ключ для роутера (добавьте в keys/)

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
@restricted
def start(update: Update, context: CallbackContext):
    update.message.reply_text(
        "🚀 **Серверный менеджер**\n\n"
        "Доступные команды:\n"
        "/wol - Включить домашний сервер\n"
        "/shutdown - Выключить домашний сервер\n"
        "/status - Проверить состояние сервера"
    , parse_mode='Markdown')

@restricted
def wake_on_lan(update: Update, context: CallbackContext):
    """Отправка Wake-on-LAN через SSH-туннель к роутеру"""
    update.message.reply_text("⏳ Отправляю команду на включение...")
    try:
        client = paramiko.SSHClient()
        client.set_missing_host_key_policy(paramiko.AutoAddPolicy())
        
        private_key = paramiko.Ed25519Key.from_private_key_file(ROUTER_SSH_KEY_PATH)
        
        client.connect(
            ROUTER_SSH_HOST,
            port=ROUTER_SSH_PORT,
            username=ROUTER_SSH_USER,
            pkey=private_key
        )
        
        # Команда etherwake для OpenWrt
        stdin, stdout, stderr = client.exec_command(f"ether-wake -i br-lan {SERVER_MAC}")
        error = stderr.read().decode()
        client.close()
        
        if error:
            update.message.reply_text(f"❌ Ошибка при выполнении команды на роутере: {error}")
        else:
            update.message.reply_text("🔌 **Magic packet отправлен!** Сервер должен скоро запуститься.")
            
    except Exception as e:
        logger.error(f"WOL Error: {e}")
        update.message.reply_text(f"⚠️ **Ошибка подключения к роутеру:**\n`{str(e)}`\n\nПроверьте, активен ли SSH-туннель от роутера.", parse_mode='Markdown')

@restricted
def shutdown_server(update: Update, context: CallbackContext):
    """Выключение сервера через SSH-туннель к домашнему серверу"""
    update.message.reply_text("⏳ Отправляю команду на выключение...")
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
        
        # Важно: для выполнения sudo без пароля настройте /etc/sudoers на домашнем сервере
        stdin, stdout, stderr = client.exec_command("sudo /sbin/shutdown -h now")
        error = stderr.read().decode()
        client.close()

        if error:
            update.message.reply_text(f"❌ Ошибка при выполнении команды на сервере: {error}")
        else:
            update.message.reply_text("🛑 **Команда выключения отправлена!** Сервер завершает работу.")

    except Exception as e:
        logger.error(f"Shutdown Error: {e}")
        update.message.reply_text(f"⚠️ **Ошибка подключения к домашнему серверу:**\n`{str(e)}`\n\nСервер уже выключен или туннель неактивен.", parse_mode='Markdown')


@restricted
def server_status(update: Update, context: CallbackContext):
    """Проверка доступности порта сервера через туннель"""
    try:
        result = subprocess.run(
            ["nc", "-z", "-w", "3", SERVER_SSH_HOST, str(SERVER_SSH_PORT)],
            capture_output=True, text=True
        )
        if result.returncode == 0:
            update.message.reply_text("🟢 **Сервер онлайн** (SSH-туннель активен).")
        else:
            update.message.reply_text("🔴 **Сервер оффлайн** (SSH-туннель не отвечает).")
    except Exception as e:
        logger.error(f"Status Check Error: {e}")
        update.message.reply_text(f"⚠️ Ошибка при проверке статуса: {str(e)}")


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
    dispatcher.add_handler(CommandHandler("wol", wake_on_lan))
    dispatcher.add_handler(CommandHandler("shutdown", shutdown_server))
    dispatcher.add_handler(CommandHandler("status", server_status))
    updater.start_polling()
    logger.info("Бот запущен...")
    updater.idle()

if __name__ == '__main__':
    main()