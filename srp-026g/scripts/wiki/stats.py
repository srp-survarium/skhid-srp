"""Weapon display schema shared by catalogue details and comparisons.

Only labels, units and comparison semantics are defined here. Values come from
the released weapon configs; wear rates and reliability are deliberately omitted.
"""
import math


def number(value):
    return f"{value:.3f}".rstrip("0").rstrip(".")


def sections(weapon, code):
    def tr(en, uk):
        return en if code == "en" else uk

    def read(path):
        value = weapon
        for key in path.split("."):
            if not isinstance(value, dict) or key not in value:
                return None
            value = value[key]
        return value

    def stat(key, en, uk, direction="neutral", unit="", scale=1, values=None):
        if values is None:
            value = read(key)
            values = [] if value is None else [value * scale]
        # Compare at displayed precision, so equal visible values stay neutral.
        values = [float(number(v)) for v in values]
        text = " – ".join(number(v) for v in values) if values else "—"
        return {"id": key, "label": tr(en, uk), "values": values,
                "text": text + (f" {unit}" if values and unit else ""), "direction": direction}

    seconds, metres, kg = tr("s", "с"), tr("m", "м"), tr("kg", "кг")
    recoil = weapon.get("recoil", {})
    def interval(axis):
        low, high = recoil.get(f"min_{axis}_recoil"), recoil.get(f"max_{axis}_recoil")
        return [] if low is None or high is None else [low, high]
    left, right = interval("left"), interval("right")
    horizontal = [right[0] - left[1], right[1] - left[0]] if left and right else []
    bias = [(sum(right) - sum(left)) / 2] if left and right else []
    variability = [math.sqrt(((right[1]-right[0])**2 + (left[1]-left[0])**2) / 12)] if left and right else []
    modes = [tr("Automatic", "Автоматичний") if m == -1 else tr("Single shot", "Одиночний") if m == 1
             else tr(f"{m}-round burst", f"Черга по {m}") for m in weapon.get("fire_queue_types") or []]
    hip = read("dispersion.from_the_hip_multiplier")
    # Native dispersion loader: 0x330FE3, missing hip multiplier defaults to 1.
    hip = 1 if hip is None else hip
    return [
        {"id": "damage", "title": tr("Damage & range", "Ушкодження та дальність"), "rows": [
            stat("bullet_damage", "Damage", "Ушкодження", "higher", scale=100),
            stat("bullet_pierce", "Armor penetration", "Бронебійність", "higher", scale=100),
            stat("max_damage_distance", "Full damage up to", "Повні ушкодження до", "higher", metres),
            stat("min_damage_distance", "Minimum damage from", "Мінімальні ушкодження від", "higher", metres),
            stat("min_damage_amount", "Minimum damage retained", "Залишок ушкоджень", "higher", "%", 100),
            stat("stopping_power", "Stopping power", "Зупиняюча дія", "higher", "%", 100),
            stat("stamina_damage", "Stamina damage", "Ушкодження витривалості", "higher"),
        ]},
        {"id": "handling", "title": tr("Firing & handling", "Стрільба та поводження зі зброєю"), "rows": [
            stat("rounds_per_minute", "Fire rate", "Темп стрільби", "higher", tr("rpm", "постр./хв")),
            stat("magazine_capacity", "Magazine capacity", "Місткість магазину", "higher"),
            stat("reload_time", "Reload time", "Час перезаряджання", "lower", seconds),
            stat("aim_time", "Aim time", "Час прицілювання", "lower", seconds),
            stat("show_time", "Draw time", "Час діставання", "lower", seconds),
            stat("hide_time", "Holster time", "Час прибирання", "lower", seconds),
            stat("weight", "Weight", "Вага", "lower", kg),
            stat("movement_speed_modifier", "Movement speed modifier", "Модифікатор швидкості руху", "higher", "%", 100),
            stat("aim_zoom_factor", "Base aim zoom", "Базове збільшення прицілу", unit="×"),
            {"id": "fire_queue_types", "label": tr("Fire modes", "Режими вогню"), "values": [],
             "text": ", ".join(modes) or "—", "direction": "neutral"},
        ]},
        {"id": "spread", "title": tr("Accuracy & spread", "Точність і розкид"),
         "note": tr("Spread values use the game's own units. Growth and recovery parameters are shown without a better/worse rating.",
                    "Значення розкиду наведено в одиницях гри. Параметри зростання й відновлення показано без оцінки «краще/гірше»."), "rows": [
            stat("dispersion.base_dispersion", "Base dispersion", "Базовий розкид", "lower"),
            stat("hip_multiplier", "Hip-fire spread multiplier", "Множник розкиду від стегна", "lower", "×", values=[hip]),
            stat("dispersion.one_shoot_dispersion_amount", "Spread added per shot", "Розкид від пострілу", "lower"),
            stat("dispersion.max_dispersion", "Spread limit", "Межа розкиду", "lower"),
            stat("dispersion.growth_speed", "Spread growth parameter", "Параметр зростання розкиду"),
            stat("dispersion.speed_of_aiming", "Spread recovery parameter", "Параметр відновлення точності"),
        ]},
        {"id": "recoil", "title": tr("Recoil", "Відбій"),
         "note": tr("Base kick in game units, not degrees. Horizontal kick is right minus left; variability is its standard deviation. These do not describe an entire burst.",
                    "Базовий поштовх в одиницях гри, не в градусах. Бічний поштовх — правий мінус лівий; мінливість — його стандартне відхилення. Це не описує всю чергу."), "rows": [
            stat("vertical_kick", "Base vertical kick range", "Діапазон базового відбою вгору", "lower", values=interval("top")),
            stat("horizontal_kick", "Horizontal kick range", "Діапазон бічного відбою", values=horizontal),
            stat("horizontal_variability", "Horizontal kick variability", "Мінливість бічного відбою", "lower", values=variability),
            stat("horizontal_bias", "Horizontal bias (− left / + right)", "Бічне зміщення (− ліворуч / + праворуч)", values=bias),
            stat("recoil.recoil_power", "Recoil accumulation exponent", "Показник накопичення відбою"),
            stat("recoil.compensation_proportional_gain", "Recovery response (P)", "Реакція відновлення (P)"),
            stat("recoil.compensation_integral_gain", "Recovery accumulation (I)", "Накопичення відновлення (I)"),
            stat("recoil.compensation_derivational_gain", "Recovery damping (D)", "Згасання відновлення (D)"),
        ]},
        {"id": "sway", "title": tr("Aim sway", "Хитання прицілу"), "rows": [
            stat("breath_vibration.base_horizontal_amplitude", "Horizontal sway", "Горизонтальне хитання", "lower"),
            stat("breath_vibration.base_vertical_amplitude", "Vertical sway", "Вертикальне хитання", "lower"),
            stat("breath_vibration.base_period", "Aim sway period", "Період хитання прицілу", unit=seconds),
        ]},
    ]
