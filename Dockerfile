# Используем легковесный образ Python
FROM python:3.9-slim

# Устанавливаем рабочую директорию
WORKDIR /app

# Устанавливаем зависимости системы
RUN apt-get update && apt-get install -y --no-install-recommends \
    netcat-openbsd \
    && rm -rf /var/lib/apt/lists/*

# Копируем файл с зависимостями Python и устанавливаем их
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt
RUN apt-get update && apt-get install -y openssh-client && rm -rf /var/lib/apt/lists/*

# Копируем скрипт бота и ключи (ключи должны быть добавлены в папку keys/ до сборки)
COPY bot.py .
COPY keys /app/keys/

# Задаем права на ключи
RUN chmod 600 /app/keys/*

# Запускаем бота
CMD ["python", "bot.py"]