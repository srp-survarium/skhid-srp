# Damage
section: mechanics
summary: Damage, armour, penetration, falloff, pain.

Cards show damage, armour and penetration ×100.

```
D, P  = weapon × ammo
hit   = D × (1 − clamp((armour − P) / 100, 0, 1))
pool −= hit × share      (the hit box's table below)
dead when body, head or pain reaches 0
```

- Armour at or below the penetration stops nothing; 100 more than it stops everything.
- Full damage and penetration up to the weapon's near distance; then both fall with the bullet's speed to the weapon's lowest share at its far distance, and stay there. Each weapon page lists them per ammo.
- Ammo scales the weapon: damage, penetration and both distances.
- Shotguns: each pellet is a hit of its own.
- One bullet through several hit boxes: each box counts with its own armour; each pool takes that bullet's largest share, not the sum.
- Limbs do not kill: an empty leg or hand is an injury. The pain they take can kill. See [Healing](healing).

## Where it goes

{{table:routes}}

## Damage types

{{table:hit_types}}

Armour is per type and per body part: each [equipment](equipment) page has its table. Anomalies: [Anomalies](anomalies).

[Calculator](/damage)
