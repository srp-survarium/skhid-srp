# Fall damage
section: mechanics
summary: How high is safe, and what a fall costs.

```
v = √(2 · {{fall.gravity}} · height)
nothing up to {{fall.harmless}}; the full {{fall.damage}} from {{fall.fatal}}; in between, along v
```

- The damage hits each foot as a blunt hit: [Damage](damage).

{{table:fall}}
