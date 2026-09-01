#!/usr/bin/env bash
# Кросс-сборка Windows-версии из Linux.
#
# Собирает оба бинарника через cross (Docker внутри) и складывает готовый
# дистрибутив в dist/windows: два .exe и .bat-скрипты запуска.
# Ассеты в дистрибутив не нужны — шрифт вшит в бинарник, остальное процедурное.
set -euo pipefail

TARGET=x86_64-pc-windows-gnu
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST="$ROOT/dist/windows"

cd "$ROOT"

if ! command -v cross > /dev/null; then
    echo "cross не установлен. Поставить:"
    echo "  cargo install cross --git https://github.com/cross-rs/cross"
    exit 1
fi

if ! docker info > /dev/null 2>&1; then
    echo "Docker недоступен: cross запускает сборку в контейнере."
    echo "Проверь, что демон запущен и пользователь в группе docker."
    exit 1
fi

echo "Собираю под $TARGET (первый раз долго: тянется образ и весь Bevy)..."
cross build --release --target "$TARGET"

# mingw кладёт в release полную отладочную информацию: без strip каждый .exe
# весит около 230 МБ вместо 125. Инструмент берём из того же образа cross,
# чтобы не требовать mingw на хосте.
echo "Убираю отладочные символы..."
docker run --rm -v "$ROOT:/w" "ghcr.io/cross-rs/$TARGET:main" bash -c \
    "x86_64-w64-mingw32-strip /w/target/$TARGET/release/cellborn-client.exe \
     /w/target/$TARGET/release/cellborn-server.exe"

rm -rf "$DIST"
mkdir -p "$DIST"
cp "target/$TARGET/release/cellborn-server.exe" "$DIST/"
cp "target/$TARGET/release/cellborn-client.exe" "$DIST/"
cp "$ROOT/scripts/windows/"*.bat "$DIST/"

# Тексты для Windows кладём с CRLF, иначе блокнот покажет их одной строкой.
to_crlf() { sed 's/$/\r/' "$1" > "$2"; }
to_crlf "$ROOT/scripts/windows/ЧИТАТЬ.txt" "$DIST/ЧИТАТЬ.txt"
to_crlf "$ROOT/docs/TESTING.md" "$DIST/ЧТО-ПРОВЕРИТЬ.txt"
to_crlf "$ROOT/docs/MECHANICS.md" "$DIST/МЕХАНИКА.txt"

# Конфиг сервера: сервер создаст его и сам, но пусть лежит сразу — так видно,
# что его вообще можно править.
to_crlf "$ROOT/scripts/windows/cellborn-server.cfg" "$DIST/cellborn-server.cfg"

echo
echo "Готово: $DIST"
ls -la "$DIST"
echo
echo "Перенеси папку на Windows и запусти ИГРАТЬ.bat"
