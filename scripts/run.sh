#!/usr/bin/env bash
# Локальный запуск на Linux: сервер в фоне, клиент на переднем плане.
# Сервер гасится, когда закрывается игра.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

MODE="${1:-play}"

case "$MODE" in
    server)
        exec cargo run --release -p cellborn-server
        ;;
    client)
        shift || true
        exec cargo run --release -p cellborn-client -- "$@"
        ;;
    play)
        # Собираем, только если бинарники устарели или их нет. Пересборка Bevy
        # занимает десятки минут, и запускать её ради «просто поиграть» не нужно.
        if [ ! -x target/release/cellborn-client ] || [ ! -x target/release/cellborn-server ]; then
            echo "Бинарников нет, собираю (первый раз это долго)..."
            cargo build --release -p cellborn-server -p cellborn-client
        fi
        ./target/release/cellborn-server &
        SERVER=$!
        # Что бы ни случилось с клиентом, сервер за собой убираем.
        trap 'kill $SERVER 2>/dev/null || true' EXIT
        sleep 1
        ./target/release/cellborn-client
        ;;
    build)
        cargo build --release -p cellborn-server -p cellborn-client
        echo "Готово:"
        ls -la target/release/cellborn-server target/release/cellborn-client
        ;;
    *)
        echo "Использование: $0 [play|build|server|client [адрес:порт]]"
        exit 1
        ;;
esac
