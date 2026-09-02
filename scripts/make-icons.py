#!/usr/bin/env python3
"""Рисует иконки интерфейса и кладёт их в assets/ui.

Картинки генерируются один раз и коммитятся, а в игру попадают через
`include_bytes!` — ровно как шрифт. Дистрибутив от этого не толстеет отдельными
файлами и не ломается, если кто-то запустит .exe из другой папки.

Рисуем кодом, а не в редакторе, по той же причине, по какой всё остальное в
игре процедурное: иконку можно перекрасить или пересобрать под другой размер
одной правкой, и она не разъедется со стилем.

    python3 scripts/make-icons.py
"""

from PIL import Image, ImageDraw
import math
import os

SIZE = 64
OUT = os.path.join(os.path.dirname(__file__), "..", "assets", "ui")

# Цвета берём такие же, какими интерфейс уже пользуется, чтобы иконки не
# выглядели пришельцами.
ENERGY = (92, 220, 170)
HEALTH = (226, 96, 102)
MASS = (150, 190, 210)
POINTS = (245, 205, 110)
KIN = (140, 216, 140)
DANGER = (242, 118, 84)


def canvas():
    return Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))


def glow(draw, shape, color, width=0):
    """Заливка плюс более светлый контур: без контура иконка тонет в фоне."""
    light = tuple(min(255, int(c * 1.4) + 30) for c in color)
    if width:
        draw.line(shape, fill=light + (255,), width=width, joint="curve")
    else:
        draw.polygon(shape, fill=color + (235,), outline=light + (255,))


def drop():
    """Энергия: капля.

    Строится как окружность, радиус которой у вершины стягивается в ноль:
    низ остаётся круглым, верх сходится в остриё. Наивная формула давала
    кляксу — важно, чтобы сужение было резким только у самой макушки.
    """
    image = canvas()
    d = ImageDraw.Draw(image)
    points = []
    for i in range(96):
        # Угол от макушки: 0 — остриё, pi — донышко.
        a = i / 95 * math.tau
        # Ширина капли: у макушки ноль, к низу выходит на полный радиус.
        width = math.sin(a / 2) ** 1.6
        x = 32 + math.sin(a) * 19 * width
        y = 10 + (1 - math.cos(a)) * 21
        points.append((x, y))
    glow(d, points, ENERGY)
    # Блик: капля читается как жидкость только с ним.
    d.ellipse((25, 36, 33, 46), fill=(255, 255, 255, 120))
    return image


def heart():
    """Здоровье: сердце."""
    image = canvas()
    d = ImageDraw.Draw(image)
    points = []
    for i in range(96):
        t = i / 95 * math.tau
        x = 16 * math.sin(t) ** 3
        y = 13 * math.cos(t) - 5 * math.cos(2 * t) - 2 * math.cos(3 * t) - math.cos(4 * t)
        points.append((32 + x * 1.25, 32 - y * 1.25))
    glow(d, points, HEALTH)
    return image


def weight():
    """Масса: гиря."""
    image = canvas()
    d = ImageDraw.Draw(image)
    body = [(14, 52), (20, 24), (44, 24), (50, 52)]
    glow(d, body, MASS)
    d.arc((22, 8, 42, 30), 180, 360, fill=tuple(min(255, c + 40) for c in MASS) + (255,), width=5)
    return image


def star():
    """Очки мутаций: звезда."""
    image = canvas()
    d = ImageDraw.Draw(image)
    points = []
    for i in range(10):
        t = -math.pi / 2 + i * math.pi / 5
        r = 24 if i % 2 == 0 else 10
        points.append((32 + math.cos(t) * r, 32 + math.sin(t) * r))
    glow(d, points, POINTS)
    return image


def helix():
    """Мутации: двойная спираль.

    Рисуется с запасом по разрешению и потом уменьшается. При 64 пикселях в лоб
    тонкие линии спирали слипались в пятна — вместо ДНК получались три свёклы.
    Сглаживание решает это там, где толщину уже не уменьшить.
    """
    scale = 6
    big = Image.new("RGBA", (SIZE * scale, SIZE * scale), (0, 0, 0, 0))
    d = ImageDraw.Draw(big)
    s = scale

    left, right = [], []
    steps = 220
    for i in range(steps):
        t = i / (steps - 1)
        y = (7 + t * 50) * s
        # Полтора витка: меньше — не читается как спираль, больше — рябит.
        x = math.sin(t * math.tau * 1.5) * 15 * s
        left.append((32 * s + x, y))
        right.append((32 * s - x, y))

    # Перекладины идут первыми, чтобы нити ложились поверх них.
    for i in range(14, steps - 10, 34):
        d.line([left[i], right[i]], fill=(214, 240, 224, 190), width=int(2.2 * s))

    strand = int(3.4 * s)
    d.line(left, fill=KIN + (255,), width=strand, joint="curve")
    # Дальняя нить темнее: без разницы в тоне спираль выглядит плоской сеткой.
    dim = tuple(int(c * 0.62) for c in KIN)
    d.line(right, fill=dim + (255,), width=strand, joint="curve")

    return big.resize((SIZE, SIZE), Image.LANCZOS)


def skull():
    """Опасность: череп, предельно упрощённый."""
    image = canvas()
    d = ImageDraw.Draw(image)
    d.ellipse((14, 10, 50, 44), fill=DANGER + (235,), outline=(255, 200, 180, 255))
    d.rectangle((24, 40, 40, 52), fill=DANGER + (235,), outline=(255, 200, 180, 255))
    for x in (22, 34):
        d.ellipse((x, 22, x + 9, 32), fill=(20, 10, 10, 255))
    return image


ICONS = {
    "energy": drop,
    "health": heart,
    "mass": weight,
    "points": star,
    "mutation": helix,
    "danger": skull,
}

if __name__ == "__main__":
    os.makedirs(OUT, exist_ok=True)
    for name, make in ICONS.items():
        path = os.path.join(OUT, f"{name}.png")
        make().save(path)
        print("нарисовано:", os.path.relpath(path))
