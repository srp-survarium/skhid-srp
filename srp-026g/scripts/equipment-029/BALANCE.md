# Balance against existing equipment

The owner requested that the additions be worthwhile alongside existing
equipment, specifically Zubr. Version 2 benchmarks the additions against the
published 0.26g equipment, using protection, covered body regions, occupied
slots, weight, built-in modifiers, level and price. `items.tsv` records each
regular piece's benchmark, final weight and modifier IDs. No existing item
is retuned.

These are design choices supported by stat comparisons. They still need
match playtesting: this audit does not model weapon damage breakpoints,
player skill, map routes or the engine's movement response to carried weight.

## Endgame comparison

Five pieces here means vest, pants, boots, gloves and helmet; no backpack,
separate mask, weapons, ammunition, skills, artefacts or rolled upgrades.
Protection is the item rating, not a claim about final damage reduction.

| Set | Level | Body armour | Helmet armour | Isolation | Five-piece weight |
| --- | ---: | ---: | ---: | ---: | ---: |
| Zubr UM-4, existing | 15 | 90 | 75 | 50 | 34.6 kg |
| Pilgrim, existing | 15 | 85 | 70 | 90 | 28.3 kg |
| Nomad, new | 15 | 80 | 60 | 30 | 17.3 kg |

Zubr retains the strongest ballistic protection, three built-in ammunition
bags, one pocket and one pocket-size bonus. Pilgrim retains much higher
isolation and its artefact slot. Both helmets protect face and brain and
occupy the mask slot. Nomad's helmet protects the brain only: a separate mask
adds its own weight and face protection.

Nomad's niche is mobility and handling. Its built-in modifiers total +10%
movement speed, -10% aim time and +10% health-regeneration speed, with one
ammunition bag and one pocket. Those are modifier totals, before the engine
combines equipment, weight, skills and other effects. Its protection was
raised from 75/50 to 80/60 (body/helmet), while its weight rose from 10.7 kg
to 17.3 kg. It has a useful protection floor and pays for mobility with lower
armour, isolation, face coverage and ammunition capacity than Zubr.

## Other tiers

| New group | Native benchmark | Intended trade-off |
| --- | --- | --- |
| Animal skin | Quilted jacket / Linen pants / Leather boots | Starter isolation and pocket option; pants and boots pay for isolation with weight. |
| Private | Leather sleeveless jacket / Jeans / Cossacks | Starter ammunition option; extra arm coverage or ammunition adds weight. Boots trade more armour for weight and no isolation. |
| Hunter | Bandit's jacket and pants | Better isolation, lower jacket armour; prices now match the native level-3 counterparts. |
| Saboteur | Reinforced padded jacket and pants | 45/20 protection versus 50/10; lighter, with a different pocket utility. Vest now 5.5 kg rather than 3 kg. |
| Paratrooper | Military vest and Army pants | 55 armour versus 60, with moderate weight savings; vest now 7 kg rather than 4 kg. |
| Viper | Sokol / Scout equipment | More ballistic protection, much less isolation; boot and pant weights raised to avoid free protection upgrades. |
| Warrior | Stalker equipment | Similar protection, different ammunition/pocket bonuses; pants reduced from 3.2 to 2 kg so their utility remains worthwhile. |
| Watchman | Stalker equipment / Assault gloves | More isolation with different pocket and recoil bonuses; added weight where the first conversion was too favourable. |
| Pathfinder | Shturm equipment | 65 rather than 70 body armour, lighter; vest is 9 kg so it also trades weight against the 60-armour Stalker jacket. |
| Omicron | Molot and OZK-18KB | Hybrid protection: 70/60 body, lighter than heavy armour but weaker ballistically than Molot and less isolated than OZK. Weight raised across the set. |
| Combat | Molot UM-2 | 70 body armour against Molot's 75 for 2.5 kg less (12.5 kg); vest has one ammunition bag plus a pocket versus Molot's two bags; gloves and boots have distinct handling/stamina bonuses. |
| Firefighter | OZK-18KB | Similar environmental role; slightly less vest isolation, different ammunition/pocket bonuses. Boots now 2.8 kg with a modest stamina bonus; helmet matches the existing 35/70, 2 kg option. |
| Mercenary | Keeper | Same 70 body armour, less isolation and a weaker helmet; lighter with +9% total movement modifiers and a handling bonus. Movement bonuses reduced from the first conversion's +12%. |

The Firefighter double cylinder retains 140 seconds versus UKV-1's 120, but
weighs 9 kg versus 8 kg and costs the same 47,250 silver. Its first-conversion
4 kg weight and 22,550 price were too favourable. The Hunter jacket/pants
prices likewise rise from 4,750/2,380 to 9,450/4,730 silver.

Premium pieces inherit every combat change from their regular counterparts.
Their extra benefits remain +5% experience and +5% faction reputation; buying
the reskin does not buy greater protection or better handling. Keeper of the
Sun remains a premium counterpart of the existing Keeper set.

## Comparison coverage

`balance_audit.py` finds 110 comparable native/new pairs at the same level,
covering 56 of the 62 new regular pieces. For those pairs it compares actual
hit-region protection, weight, oxygen capacity and every built-in modifier,
and rejects an item that is no worse in every dimension and strictly better
in any one. Equal stats are allowed for appearance choices. Prices are
reported separately; this comparison is about equipment strength.

Six pieces have no native piece at the same level with identical coverage:

- Watchman backpack: a level-9 improvement on the level-3 Marodeur container,
  4 kg versus 5 kg for the same pocket and pocket-size bonuses, at higher cost.
- M-1 life support: level 12, 5 kg, +8% regeneration and +6% stamina-regeneration
  modifiers; Pilgrim's level-15 container has +15% regeneration and an artefact
  slot at 7 kg. M-1 trades capacity and regeneration for stamina and weight.
- Omicron mask: 35/60, 0.8 kg at level 12; PMK-7S gives 35/70 at 1.2 kg and
  level 14. The later environmental specialist keeps stronger isolation.
- Field radio: level 9, 4 kg, -25% respawn-time modifier; gives up the pockets,
  ammunition or artefact capacity another backpack could supply.
- Big backpack: level 12, 6 kg, one pocket and two pocket-size bonuses;
  the level-14 Assault fighter's backpack gives two pockets and two size
  bonuses at 8 kg.
- Nomad helmet: the face-coverage distinction described above; its weaker
  protection and separate-mask requirement remain part of the trade-off.

Regular equipment keeps access to native upgrade pools through its template.
The audit evaluates built-in equipment values; fully upgraded mixed sets and
the value of the radio in different modes remain playtest questions.
