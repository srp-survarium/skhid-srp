#!/usr/bin/env bash
# The stats page (scripts/stats/check_history.mjs) in a headless Firefox of its own, over the web
# server's preview (its made-up history), on ports of its own; no window. Firefox and Node 22+ are
# the machine's (on the PATH).
set -euo pipefail
command -v firefox >/dev/null && command -v node >/dev/null || { echo "stats-page needs firefox and node on the PATH" >&2; exit 1; }
work=${CHECK_WORK:?run by tools/check}
free_port() { python3 -c 'import socket; s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])'; }
preview=$(free_port) bidi=$(free_port) own=$(free_port)
groups=()
trap 'for g in "${groups[@]}"; do kill -- "-$g" 2>/dev/null || true; done' EXIT
nix develop ./srp-026g --command bash -c 'cd srp-026g && cargo test --quiet -p web-server --no-run' >/dev/null
(cd srp-026g && SRP_PREVIEW_PORT=$preview exec setsid nix develop --command \
    cargo test --quiet -p web-server serve_wiki_preview -- --ignored --nocapture) >"$work/preview.log" 2>&1 &
groups+=($!)
for _ in $(seq 600); do
    curl -sf -o /dev/null "http://127.0.0.1:$preview/stats" && break
    sleep 0.5
done
curl -sf -o /dev/null "http://127.0.0.1:$preview/stats" || { tail -n 20 "$work/preview.log"; exit 1; }
mkdir -p "$work/profile"
setsid env -u DISPLAY -u WAYLAND_DISPLAY MOZ_HEADLESS=1 firefox --headless --no-remote --profile "$work/profile" \
    --remote-debugging-port "$bidi" >"$work/firefox.log" 2>&1 &
groups+=($!)
for _ in $(seq 120); do
    grep -q "WebDriver BiDi listening" "$work/firefox.log" && break
    sleep 0.5
done
node srp-026g/scripts/stats/check_history.mjs "http://127.0.0.1:$preview" "$bidi" "$work/screenshots" "$own"
