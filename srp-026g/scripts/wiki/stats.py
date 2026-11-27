"""Weapon display schema shared by catalogue details and comparisons.

Only labels, units and comparison semantics are defined here. Values come from
the released weapon and ammo configs; wear rates and reliability are
deliberately omitted.
"""
import math

# What a round scales, by its own `data` keys (ammo_data::load 0x332440): the
# bullet's damage and piercing (bullet::bullet 0x32BA60), its least damage
# (weapon::min_damage 0x2E1C80), both damage distances, stopping power
# (base_player::hit_impl 0x2D9B90) and stamina damage.
ROUND_FACTORS = (
    ("bullet_damage", "k_damage"), ("bullet_pierce", "k_arp"),
    ("min_damage_amount", "k_min_damage_amount"),
    ("max_damage_distance", "k_max_damage_distance"), ("min_damage_distance", "k_min_damage_distance"),
    ("stopping_power", "k_power"), ("stamina_damage", "k_stamina_damage"),
)


def number(value):
    return f"{value:.3f}".rstrip("0").rstrip(".")


def with_round(weapon, ammo):
    """The weapon as it fires `ammo`: its scaled values, the spread's base
    times the round's `dispersion` (get_static_dispersion 0x331220) and the
    round's pellets (`buck_shot`)."""
    fired = dict(weapon)
    for key, factor in ROUND_FACTORS:
        if key in fired:
            fired[key] = fired[key] * ammo[factor]
    spread = weapon.get("dispersion", {})
    if "base_dispersion" in spread:
        fired["dispersion"] = {**spread, "base_dispersion": spread["base_dispersion"] * ammo["dispersion"]}
    fired["pellets"] = max(1, round(ammo["buck_shot"]))
    return fired


def sections(weapon, ammo, texts):
    """The card's rows for `weapon` firing `ammo`, their words `texts`' (site-texts/texts.py)."""
    t = texts.t

    weapon = with_round(weapon, ammo)

    def read(path):
        value = weapon
        for key in path.split("."):
            if not isinstance(value, dict) or key not in value:
                return None
            value = value[key]
        return value

    def stat(key, label, direction="neutral", unit="", scale=1, values=None):
        if values is None:
            value = read(key)
            values = [] if value is None else [value * scale]
        # Compare at displayed precision, so equal visible values stay neutral.
        values = [float(number(v)) for v in values]
        text = " – ".join(number(v) for v in values) if values else "—"
        return {"id": key, "label": t(label), "values": values,
                "text": text + (f" {unit}" if values and unit else ""), "direction": direction}

    def damage():
        # A shot's damage: each pellet's, compared as all of them together.
        row = stat("bullet_damage", "Damage", "higher", scale=100)
        pellets = weapon["pellets"]
        if pellets > 1 and row["values"]:
            row["text"] = f'{row["text"]} × {pellets} {t("pellets")}'
            row["values"] = [float(number(row["values"][0] * pellets))]
        return row

    seconds, metres, kg = t("s", "unit"), t("m", "unit"), t("kg", "unit")
    recoil = weapon.get("recoil", {})
    def interval(axis):
        low, high = recoil.get(f"min_{axis}_recoil"), recoil.get(f"max_{axis}_recoil")
        return [] if low is None or high is None else [low, high]
    left, right = interval("left"), interval("right")
    horizontal = [right[0] - left[1], right[1] - left[0]] if left and right else []
    bias = [(sum(right) - sum(left)) / 2] if left and right else []
    variability = [math.sqrt(((right[1]-right[0])**2 + (left[1]-left[0])**2) / 12)] if left and right else []
    modes = [t("Automatic") if m == -1 else t("Single shot") if m == 1
             else texts.tf("{m}-round burst", m=m) for m in weapon.get("fire_queue_types") or []]
    hip = read("dispersion.from_the_hip_multiplier")
    # Native dispersion loader: 0x330FE3, missing hip multiplier defaults to 1.
    hip = 1 if hip is None else hip
    return [
        {"id": "damage", "title": t("Damage & range"), "rows": [
            damage(),
            stat("bullet_pierce", "Armor penetration", "higher", scale=100),
            stat("max_damage_distance", "Full damage up to", "higher", metres),
            stat("min_damage_distance", "Minimum damage from", "higher", metres),
            stat("min_damage_amount", "Minimum damage retained", "higher", "%", 100),
            stat("stopping_power", "Stopping power", "higher", "%", 100),
            stat("stamina_damage", "Stamina damage", "higher"),
        ]},
        {"id": "handling", "title": t("Firing & handling"), "rows": [
            stat("rounds_per_minute", "Fire rate", "higher", t("rpm", "unit")),
            stat("magazine_capacity", "Magazine capacity", "higher"),
            stat("reload_time", "Reload time", "lower", seconds),
            stat("aim_time", "Aim time", "lower", seconds),
            stat("show_time", "Draw time", "lower", seconds),
            stat("hide_time", "Holster time", "lower", seconds),
            stat("weight", "Weight", "lower", kg),
            stat("movement_speed_modifier", "Movement speed modifier", "higher", "%", 100),
            stat("aim_zoom_factor", "Base aim zoom", unit="×"),
            {"id": "fire_queue_types", "label": t("Fire modes"), "values": [],
             "text": ", ".join(modes) or "—", "direction": "neutral"},
        ]},
        {"id": "spread", "title": t("Accuracy & spread"),
         "note": t("Spread values use the game's own units. Growth and recovery parameters are shown without a better/worse rating."), "rows": [
            stat("dispersion.base_dispersion", "Base dispersion", "lower"),
            stat("hip_multiplier", "Hip-fire spread multiplier", "lower", "×", values=[hip]),
            stat("dispersion.one_shoot_dispersion_amount", "Spread added per shot", "lower"),
            stat("dispersion.max_dispersion", "Spread limit", "lower"),
            stat("dispersion.growth_speed", "Spread growth parameter"),
            stat("dispersion.speed_of_aiming", "Spread recovery parameter"),
        ]},
        {"id": "recoil", "title": t("Recoil"),
         "note": t("Base kick in game units, not degrees. Horizontal kick is right minus left; variability is its standard deviation. These do not describe an entire burst."), "rows": [
            stat("vertical_kick", "Base vertical kick range", "lower", values=interval("top")),
            stat("horizontal_kick", "Horizontal kick range", values=horizontal),
            stat("horizontal_variability", "Horizontal kick variability", "lower", values=variability),
            stat("horizontal_bias", "Horizontal bias (− left / + right)", values=bias),
            stat("recoil.recoil_power", "Recoil accumulation exponent"),
            stat("recoil.compensation_proportional_gain", "Recovery response (P)"),
            stat("recoil.compensation_integral_gain", "Recovery accumulation (I)"),
            stat("recoil.compensation_derivational_gain", "Recovery damping (D)"),
        ]},
        {"id": "sway", "title": t("Aim sway"), "rows": [
            stat("breath_vibration.base_horizontal_amplitude", "Horizontal sway", "lower"),
            stat("breath_vibration.base_vertical_amplitude", "Vertical sway", "lower"),
            stat("breath_vibration.base_period", "Aim sway period", unit=seconds),
        ]},
    ]
