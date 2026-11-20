# prediction-fit

Fits the prediction of other players' inputs on recorded matches and scores
each model. Results and the choice: `docs/notes/prediction_fit.md`. No
recording, decoded file or sample is kept in the repo: they hold account
names and ids. Keep them under `private/` (ignored).

## 1. Copy recordings

The live server keeps them in the data volume's `recordings/` (path from
`docker inspect srp26-srp26-1`, read-only). The host has 1 vCPU and swaps:
copy at about 2 MB/s, in batches, at the lowest priority, skipping `.part`:

```
rsync -t --bwlimit=2000 --files-from=<batch> \
  -e 'ssh root@<jump host> ssh' --rsync-path='nice -n19 ionice -c3 rsync' \
  root@<live host>:<volume>/recordings/ private/prediction-data/rec/
```

## 2. Decode

`decode/` (workspace member `prediction-decode`) reads each `.srprec` with the
match server's own readers (record, transport, messages, uploads, world
players) and writes per recording `inputs.bin`, `worlds.bin` and `meta.json`
(formats at the top of `decode/src/main.rs`; places only, no names):

```
cd srp-026g && nix develop -c cargo build -p prediction-decode --release
target/release/prediction-decode private/prediction-data/dec private/prediction-data/rec/*.srprec
```

A recording already decoded is skipped.

## 3. Fit and score

Python 3 with numpy (lightgbm for the learned ceiling):

```
cd srp-026g/scripts/prediction-fit
nix-shell -p "python3.withPackages(ps: with ps; [numpy lightgbm])" --run \
  "python3 -m predfit.run --data <dec> --cache <samples> --out results.json --ceiling --flow"
nix-shell -p "python3.withPackages(ps: with ps; [numpy])" --run \
  "python3 -m predfit.stats --data <dec> --cache <samples> --out stats.json"
python3 -m predfit.report results.json            # the note's tables
python3 -m predfit.export results.json 'projected-look[turn]' > ../../docs/notes/prediction_fit_table.tsv
```

The full run (906 matches, the ceiling included) takes about 25 minutes on 24 cores; without
`--ceiling` about 3.

- `predfit/load.py`: decoded files to per-player tick timelines (keys held,
  turns, inputs, alive, combat, positions carried on from worlds).
- `predfit/kin.py`: the movement proxy (the bots' `kin.rs` model).
- `predfit/proxy.py`: the proxy against the uploaded worlds.
- `predfit/harness.py`: samples (moment, delay), the split by match, scores.
- `predfit/models.py`, `tables.py`, `projection.py`, `flow.py`, `ceiling.py`: the models
  (`projection.py` is the one a client can run: one input after the newest node).
- `predfit/stats.py`, `report.py`, `export.py`: the aggregates, the tables, the client's table.
- `predfit/samples.py`: builds and caches the samples (`--every` ticks apart
  per player and delay set).

## Tests

```
nix-shell -p "python3.withPackages(ps: with ps; [numpy])" --run "python3 -m unittest test_predfit"
```
