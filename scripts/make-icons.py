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


def bolt():
    """Энергия: молния.

    Капля читалась как вода, а не как запас сил, — и рядом с полоской, которая
    убывает от голода, это сбивало. Молния говорит «заряд» без пояснений.

    Рисуется с запасом по разрешению и уменьшается: у молнии острые углы, и на
    64 пикселях в лоб они превращаются в лесенку.
    """
    scale = 6
    big = Image.new("RGBA", (SIZE * scale, SIZE * scale), (0, 0, 0, 0))
    d = ImageDraw.Draw(big)
    s = scale

    # Классический зигзаг: верхняя половина уходит вправо, нижняя влево.
    shape = [
        (36, 6), (17, 34), (28, 34), (24, 58), (46, 27), (34, 27), (40, 6),
    ]
    points = [(x * s, y * s) for x, y in shape]
    light = tuple(min(255, int(c * 1.4) + 30) for c in ENERGY)
    d.polygon(points, fill=ENERGY + (240,), outline=light + (255,))
    # Блик по верхней грани: без него молния выглядит плоской наклейкой.
    d.line([points[0], points[1]], fill=(255, 255, 255, 150), width=int(1.6 * s))

    return big.resize((SIZE, SIZE), Image.LANCZOS)


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


ATTACK = (240, 150, 120)
SPEED = (150, 205, 245)
DEFENSE = (185, 190, 205)
RESIST = (170, 215, 175)


def sharp(draw_on, points, color, scale):
    """Заливка с более светлым контуром — общий приём для угловатых иконок."""
    light = tuple(min(255, int(c * 1.4) + 30) for c in color)
    draw_on.polygon(points, fill=color + (240,), outline=light + (255,))
    del scale


def big_canvas(scale):
    """Холст с запасом по разрешению: острые углы иначе идут лесенкой."""
    return Image.new("RGBA", (SIZE * scale, SIZE * scale), (0, 0, 0, 0))


def claw():
    """Атака: коготь из трёх изогнутых зубцов.

    Прямой клык читался просто треугольником: у симметричной фигуры без изгиба
    нет ничего, что говорило бы «оно рвёт». Изгиб и повтор — есть.
    """
    scale = 6
    big = big_canvas(scale)
    d = ImageDraw.Draw(big)
    s = scale

    light = tuple(min(255, int(c * 1.4) + 30) for c in ATTACK)
    for index, (base_x, curve, length) in enumerate(
        ((17, -7.0, 40.0), (32, 0.0, 47.0), (47, 7.0, 40.0))
    ):
        # Зубец: сужающаяся к концу дуга. Строим две стороны и замыкаем.
        outer, inner = [], []
        for i in range(24):
            t = i / 23
            # Кончик уходит вбок тем сильнее, чем дальше от середины когтя.
            x = base_x + curve * t * t
            y = 9 + t * length
            half = (1.0 - t) ** 0.85 * 6.0
            outer.append(((x + half) * s, y * s))
            inner.append(((x - half) * s, y * s))
        d.polygon(outer + inner[::-1], fill=ATTACK + (240,), outline=light + (255,))
        if index == 1:
            d.line(
                [(32 * s, 14 * s), (32 * s, 48 * s)],
                fill=(255, 255, 255, 130),
                width=int(1.3 * s),
            )

    return big.resize((SIZE, SIZE), Image.LANCZOS)


def fin():
    """Скорость: плавник со следом.

    Не стрелка: стрелка означает «направление», а нужно «быстро». Плавник с
    полосами позади читается как движение и остаётся в теме моря.
    """
    scale = 6
    big = big_canvas(scale)
    d = ImageDraw.Draw(big)
    s = scale
    shape = [(46, 8), (52, 46), (20, 40), (34, 26)]
    sharp(d, [(x * s, y * s) for x, y in shape], SPEED, s)
    for i, y in enumerate((20, 30, 40)):
        length = 16 - i * 3
        d.line(
            [(8 * s, y * s), ((8 + length) * s, y * s)],
            fill=SPEED + (150,),
            width=int(1.8 * s),
        )
    return big.resize((SIZE, SIZE), Image.LANCZOS)


def shield():
    """Защита: щит."""
    scale = 6
    big = big_canvas(scale)
    d = ImageDraw.Draw(big)
    s = scale
    shape = [(32, 6), (52, 15), (49, 38), (32, 58), (15, 38), (12, 15)]
    sharp(d, [(x * s, y * s) for x, y in shape], DEFENSE, s)
    d.line(
        [(32 * s, 14 * s), (32 * s, 48 * s)],
        fill=(255, 255, 255, 110),
        width=int(1.4 * s),
    )
    return big.resize((SIZE, SIZE), Image.LANCZOS)


def leaf():
    """Стойкость к среде: лист.

    Приспособленность — это про то, чтобы жить в чужой воде, а не про броню.
    Лист говорит «выживает здесь» лучше, чем ещё один щит.
    """
    scale = 6
    big = big_canvas(scale)
    d = ImageDraw.Draw(big)
    s = scale
    # Лист строится по половинкам: правая сверху вниз, левая обратно. Попытка
    # задать его одним углом даёт две доли вместо одной — получается арахис.
    right, left = [], []
    for i in range(60):
        # От макушки к кончику.
        v = -1.0 + i / 59 * 2.0
        y = 32 + v * 26
        # Ширина: ноль на концах, максимум посередине, с наклоном к макушке.
        half = math.cos(v * math.pi / 2) ** 0.75 * 17
        right.append(((32 + half) * s, y * s))
        left.append(((32 - half) * s, y * s))
    sharp(d, right + left[::-1], RESIST, s)
    d.line(
        [(32 * s, 10 * s), (32 * s, 54 * s)],
        fill=(255, 255, 255, 120),
        width=int(1.3 * s),
    )
    return big.resize((SIZE, SIZE), Image.LANCZOS)


ICONS = {
    "energy": bolt,
    "health": heart,
    "mass": weight,
    "points": star,
    "mutation": helix,
    "danger": skull,
    "attack": claw,
    "speed": fin,
    "defense": shield,
    "resist": leaf,
}

if __name__ == "__main__":
    os.makedirs(OUT, exist_ok=True)
    for name, make in ICONS.items():
        path = os.path.join(OUT, f"{name}.png")
        make().save(path)
        print("нарисовано:", os.path.relpath(path))
