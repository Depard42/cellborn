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
        cargo build --release -p cellborn-server -p cellborn-client
        ./target/release/cellborn-server &
        SERVER=$!
        # Что бы ни случилось с клиентом, сервер за собой убираем.
        trap 'kill $SERVER 2>/dev/null || true' EXIT
        sleep 1
        ./target/release/cellborn-client
        ;;
    *)
        echo "Использование: $0 [play|server|client [адрес:порт]]"
        exit 1
        ;;
esac
