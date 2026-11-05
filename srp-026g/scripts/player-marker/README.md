# Battery marker regression

Run against a source-built `player_icons.swf`:

```sh
srp-026g/scripts/player-marker/check.sh /path/to/player_icons.swf /tmp/player-marker-check
```

The runner uses the existing Flex tools in `private/actionscript/tools.json`
(`SRP_AS_TOOLS` overrides it), Ruffle, Xvfb and Mesa/lavapipe. It builds the
harness and checks the real movie through its public API. The optional work
directory retains compiler and runtime logs. `RUFFLE_BIN`, `XVFB_BIN` and
`LVP_ICD` can select installed tools; otherwise the runner obtains them via Nix.

The 17 assertions cover carry-state changes on visible and hidden markers,
partial and repeated updates, enemy filtering, whole-HUD hiding, world-object
visibility and unknown IDs. The old movie fails hidden drop and hidden pickup;
the fixed movie passes both without revealing hidden markers.

Combined hide/carry updates reproduce a Flash state-handling bug. They are not
a confirmed native gameplay reproduction of Kube's reported stale icon.
