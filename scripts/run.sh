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
        # Собираем ВСЕГДА. Решать, нужна ли работа, умеет сам cargo: если ничего
        # не менялось, он отвечает за долю секунды.
        #
        # Здесь была «оптимизация»: пропускать сборку, если бинарники просто
        # существуют. Она стоила вечера отладки. Клиент оказался на сутки старше
        # сервера, рукопожатие прошло (протокол тогда ещё совпадал), а
        # реплицированные данные не разобрались — и клиент падал в недрах
        # библиотеки с сообщением, по которому причину не угадать.
        #
        # Секунда проверки против возможности молча запустить чужой код — обмен
        # очевидно неудачный.
        echo "Проверяю сборку..."
        cargo build --release -p cellborn-server -p cellborn-client
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
