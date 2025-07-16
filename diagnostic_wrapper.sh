#!/bin/bash

echo "==============================================="
echo "=== ДИАГНОСТИКА ЗАПУСКА ПРИЛОЖЕНИЯ ==="
echo "==============================================="

echo ""
echo "=== ПЕРЕМЕННЫЕ ОКРУЖЕНИЯ ==="
# Показываем все переменные которые нас интересуют
env | grep -E "(BOT_TOKEN|ALLOWED_USERS|SERVER_MAC|ROUTER_|SERVER_|RUST_LOG)" | sort
echo "=== КОНЕЦ ПЕРЕМЕННЫХ ==="

echo ""
echo "=== ПРОВЕРКА ФАЙЛОВ ==="
echo "Рабочая директория: $(pwd)"
echo "Содержимое /app:"
ls -la /app/ 2>/dev/null || echo "Директория /app не найдена"
echo "Содержимое текущей директории:"
ls -la .
echo "Проверка ключей SSH:"
ls -la /app/keys/ 2>/dev/null || echo "Директория /app/keys не найдена"
echo "=== КОНЕЦ ПРОВЕРКИ ФАЙЛОВ ==="

echo ""
echo "=== ПРОВЕРКА ДОСТУПНОСТИ БИНАРНИКА ==="
which wakeonlan_bot || echo "wakeonlan_bot не найден в PATH"
ls -la /usr/local/bin/wakeonlan_bot 2>/dev/null || echo "/usr/local/bin/wakeonlan_bot не найден"
echo "=== КОНЕЦ ПРОВЕРКИ БИНАРНИКА ==="

echo ""
echo "=== ЗАПУСК ПРИЛОЖЕНИЯ ==="
echo "Команда: wakeonlan_bot"
echo "Запускаем..."

# Запускаем приложение
exec wakeonlan_bot "$@"