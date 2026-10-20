#!/usr/bin/env bash
# The release scripts against stand-ins, on this machine only: ssh runs the
# command here (so the jump host, the live host and the patches servers are
# folders under a temporary one), caddy and systemctl only log, docker plays
# srp26 (a container that stops, starts and is made anew from an image ID,
# its volume a folder holding srp.db; a start writes to srp.db, makes the
# site's /patches that of the server data in place and ends the maintenance,
# or refuses, as srp does when the data does not suit it), and small HTTP
# servers stand for the patches servers and the site with its admin page.
# Nothing is sent anywhere. Every confirmation is answered by a pipe (setsid:
# no terminal to ask). ssh, curl and docker log their arguments, to check
# that no secret is in a command line.
#
#   nix shell nixpkgs#python3 nixpkgs#shellcheck -c release/test.sh
#
# Needs python3 (the HTTP servers, a ZIP) and the release srp-game-data
# (built if missing); shellcheck runs if there is one.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
root=$(dirname "$here")
command -v python3 >/dev/null || { echo "test.sh: python3 is needed" >&2; exit 1; }

if command -v shellcheck >/dev/null; then
    (cd "$here" && shellcheck -x -s bash lib.sh build-patch upload-patch add-mirror build-server-db build-site build-zip deploy rollback setup-patches-server release.sh test.sh &&
        shellcheck -s sh host-rollback.sh)
    echo "ok   shellcheck"
else
    echo "skip shellcheck (none here)"
fi

bin=${SRP_GAME_DATA_BIN:-$root/srp-026g/target/release/srp-game-data}
if [[ ! -x $bin ]]; then
    (cd "$root" && nix develop ./srp-026g -c bash -c \
        'cd srp-026g && cargo build --quiet --release -p resources-db --bin srp-game-data')
fi
export SRP_GAME_DATA_BIN=$bin

T=$(mktemp -d)
pids=()
cleanup() {
    local pid
    for pid in "${pids[@]}"; do kill "$pid" 2>/dev/null || true; done
    rm -rf "$T"
}
trap cleanup EXIT
mkdir -p "$T/bin" "$T/patches" "$T/patches-test" "$T/site/api" "$T/live/download" "$T/work" "$T/volume" "$T/docker/tags"
# The scripts' cache (the URLs checked, the stock db's sha256) is the test's.
export XDG_CACHE_HOME=$T/cache
# The owner's go (right after the preflight, before the maintenance) is
# given beforehand; the tests of the go below wait for it.
export SKHID_RELEASE_GO=1

# The stand-ins.
cat >"$T/bin/ssh" <<'EOF'
#!/usr/bin/env bash
# ssh [-o <option>]... <host> <command>: runs the command here.
printf 'ssh %s\n' "$*" >>"$TEST_ARGS"
while [[ $1 == -o ]]; do shift 2; done
printf '%s\n' "$1" >>"$TEST_LOG"
if [[ -e $TEST_DIR/ssh-fails-$1 ]]; then exit 255; fi
exec bash -c "$2"
EOF
cat >"$T/bin/curl" <<EOF
#!/usr/bin/env bash
printf 'curl %s\n' "\$*" >>"\$TEST_ARGS"
exec $(command -v curl) "\$@"
EOF
for tool in caddy systemctl; do
    cat >"$T/bin/$tool" <<EOF
#!/usr/bin/env bash
printf '$tool %s\n' "\$*" >>"\$TEST_LOG"
if [[ -e \$TEST_DIR/$tool-fails ]]; then exit 1; fi
EOF
done
# docker: srp26. $TEST_DIR/docker: `latest` (the ID srp-026g:latest names),
# `running` (the container's image), `state` (running, stopped,
# restarting), `starts` (a count), tags/ (the other tags). A start of srp
# (`up`, `start`) first copies srp.db to the volume's backups/srp-<unix>.db
# (not with $TEST_DIR/no-backup), then appends `start <n> <image>` to it and
# makes the site's /patches that of the data in place (not with
# $TEST_DIR/patches-stale); with $TEST_DIR/refuse-start (once) it keeps
# restarting and logs that file's line (why), backing srp.db up first only
# when the accounts are the reason. `exec` runs in the running container
# (the volume is $TEST_DIR/volume); `run` is refused (no other container is
# ever run on a release's hosts). A build fails with $TEST_DIR/build-fails.
cat >"$T/bin/docker" <<'EOF'
#!/usr/bin/env bash
printf 'docker %s\n' "$*" >>"$TEST_LOG"
printf 'docker %s\n' "$*" >>"$TEST_ARGS"
live=$TEST_DIR/live/download
state=$TEST_DIR/docker
offered() {
    if [[ -f $live/server-game-data.db ]]; then
        "$SRP_GAME_DATA_BIN" info "$live/server-game-data.db" |
            sed -n 's/^  resources\.db\.\(patch_[0-9]*\) [0-9]* bytes sha256 \([0-9a-f]*\) urls \(.*\)$/\1 \2 \3/p'
    fi
}
backup() { # what srp does when it opens srp.db: VACUUM INTO backups/srp-<unix>.db
    local n max
    [[ -e $TEST_DIR/no-backup ]] && return 0
    mkdir -p "$TEST_DIR/volume/backups"
    n=$(date +%s)
    max=$(ls "$TEST_DIR/volume/backups" | sed -n 's/^srp-\([0-9]*\)\.db$/\1/p' | sort -n | tail -n 1)
    if ((n <= ${max:-0})); then n=$((max + 1)); fi
    cp "$TEST_DIR/volume/srp.db" "$TEST_DIR/volume/backups/srp-$n.db"
}
started() { # the container runs: what srp does at a start
    local n
    backup
    n=$(($(cat "$state/starts") + 1))
    echo "$n" >"$state/starts"
    echo running >"$state/state"
    echo "start $n $(cat "$state/running")" >>"$TEST_DIR/volume/srp.db"
    if [[ ! -e $TEST_DIR/patches-stale ]]; then
        offered | jq -Rn '[inputs | split(" ") | {key: .[0], value: {sha256: .[1], urls: .[2:]}}] | from_entries' >"$TEST_DIR/site/patches"
    fi
    rm -f "$TEST_DIR/state/maint"
}
case $1 in
compose)
    case " $* " in
    *" build "*)
        if [[ -e $TEST_DIR/build-fails ]]; then echo "build: cargo failed" >&2; exit 1; fi
        echo "sha256:$(date +%s%N | sha256sum | cut -c1-64)" >"$state/latest" ;;
    *" ps "*) echo c0ffee ;;
    *" stop "*) echo stopped >"$state/state" ;;
    *" start "*) started ;;
    *" logs "*)
        if [[ -s $state/refused ]]; then
            cat "$state/refused"
        else
            echo "INFO game data: /srv/srp/download/server-game-data.db (2 gameplay files, 1 projects; made of resources.db; $(offered | grep -c .) patch(es) offered at /patches)"
        fi ;;
    *" up "*)
        cp "$state/latest" "$state/running"
        if [[ -e $TEST_DIR/refuse-start ]]; then
            # srp exits 1 and says why; it never becomes healthy. It opened
            # srp.db (and so backed it up) only when the accounts are the
            # reason.
            if grep -q "cannot load the 0.26g accounts" "$TEST_DIR/refuse-start"; then backup; fi
            { echo "INFO srp starting"; cat "$TEST_DIR/refuse-start"; } >"$state/refused"
            rm "$TEST_DIR/refuse-start"
            echo restarting >"$state/state"
            rm -f "$TEST_DIR/state/maint"
        else
            rm -f "$state/refused"
            started
        fi ;;
    esac ;;
inspect)
    case "$*" in
    *"{{.Image}}"*)
        if [[ "$*" == *"image {{.Image}}"* ]]; then echo "image $(cat "$state/running")"; else cat "$state/running"; fi ;;
    *".Mounts"*) echo "srp26_srp26-data" ;;
    *".State.Status"*)
        case $(cat "$state/state") in
        running) line="running healthy 0" ;;
        restarting) line="restarting starting 3" ;;
        *) line="exited none 0" ;;
        esac
        if [[ "$*" == *".State.StartedAt"* ]]; then line+=" 2026-10-16T12:00:00Z"; fi
        echo "$line" ;;
    esac ;;
volume) echo "mount $TEST_DIR/volume" ;;
image)
    if [[ "$*" == *"latest {{.Id}}"* ]]; then echo "latest $(cat "$state/latest")"; fi ;;
tag)
    if [[ $3 == srp-026g:latest ]]; then echo "$2" >"$state/tags/latest"; cp "$state/tags/latest" "$state/latest"; else echo "$2" >"$state/tags/${3//[:\/]/_}"; fi ;;
rmi) rm -f "$state/tags/${2//[:\/]/_}" ;;
exec) # exec <id> <command> <path in /srv/srp/data>...: a running container only
    [[ $(cat "$state/state") == running ]] || { echo "Error response from daemon: container is not running" >&2; exit 1; }
    shift 2
    exec "${@//\/srv\/srp\/data/$TEST_DIR/volume}" ;;
run) echo "docker run: not on a release's hosts" >&2; exit 1 ;;
esac
EOF
chmod +x "$T/bin/"*
export PATH="$T/bin:$PATH" TEST_LOG=$T/log TEST_ARGS=$T/args TEST_DIR=$T
# release.sh and the site: a minute is a second.
export SKHID_TEST_SECONDS_PER_MINUTE=1
: >"$T/log"
: >"$T/args"
mkdir -p "$T/state"
echo "sha256:$(echo one | sha256sum | cut -c1-64)" >"$T/docker/latest"
cp "$T/docker/latest" "$T/docker/running"
echo running >"$T/docker/state"
echo 0 >"$T/docker/starts"
printf 'srp.db schema 1\n' >"$T/volume/srp.db"

serve() { # <folder> <port file>: a port answering from it, with "Server: Caddy"
    local port_file=$2
    python3 - "$1" "$port_file" >/dev/null 2>&1 <<'EOF' &
import functools, http.server, sys
class Handler(http.server.SimpleHTTPRequestHandler):
    server_version, sys_version = "Caddy", ""
    def log_message(self, *args): pass
server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), functools.partial(Handler, directory=sys.argv[1]))
open(sys.argv[2], "w").write(str(server.server_address[1]))
server.serve_forever()
EOF
    pids+=($!)
    local _
    for _ in $(seq 50); do [[ -s $port_file ]] && break; sleep 0.1; done
}
serve "$T/patches" "$T/patches.port"
serve "$T/patches-test" "$T/patches-test.port"
# A mirror: /files/<name> of $T/mirror; /hop/<k>/<name> redirects k times
# (on this host) before it is /files/<name>; /page/<name> is a web page
# (Google Drive's virus-scan page). Each request is a line of
# $T/mirror.log: its path and its Range header (- for none).
mkdir -p "$T/mirror"
python3 - "$T/mirror" "$T/mirror.port" "$T/mirror.log" >/dev/null 2>&1 <<'EOF' &
import http.server, os, sys
root, port_file, log = sys.argv[1:4]
class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args): pass
    def answer(self, status, body, kind="application/octet-stream", location=None):
        self.send_response(status)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(body)))
        if location:
            self.send_header("Location", location)
        self.end_headers()
        try:
            self.wfile.write(body)
        except OSError:
            pass
    def do_GET(self):
        open(log, "a").write(f"{self.path} {self.headers.get('Range', '-')}\n")
        parts = self.path.split("/")
        if len(parts) == 4 and parts[1] == "hop":
            hops = int(parts[2])
            target = f"/hop/{hops - 1}/{parts[3]}" if hops > 1 else f"/files/{parts[3]}"
            # As Google's: a redirect says it is a web page; only the last
            # answer is the file's.
            return self.answer(302, b"<html>Moved</html>", "text/html; charset=UTF-8", location=target)
        if len(parts) == 3 and parts[1] == "page":
            return self.answer(200, b"<!DOCTYPE html><html><title>Google Drive - Virus scan warning</title></html>",
                               "text/html; charset=utf-8")
        if len(parts) == 3 and parts[1] == "files" and os.path.isfile(os.path.join(root, parts[2])):
            body = open(os.path.join(root, parts[2]), "rb").read()
            asked = self.headers.get("Range", "")
            # Ranges only while root/../mirror.ranges is there (as Google
            # Drive answers them): 206 and Content-Range.
            if asked.startswith("bytes=") and os.path.exists(root + ".ranges"):
                first, last = (int(n) for n in asked[6:].split("-"))
                part = body[first:last + 1]
                self.send_response(206)
                self.send_header("Content-Type", "application/octet-stream")
                self.send_header("Content-Range", f"bytes {first}-{first + len(part) - 1}/{len(body)}")
                self.send_header("Content-Length", str(len(part)))
                self.end_headers()
                return self.wfile.write(part)
            return self.answer(200, body)
        return self.answer(404, b"not found\n", "text/plain")
server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
open(port_file, "w").write(str(server.server_address[1]))
server.serve_forever()
EOF
pids+=($!)
for _ in $(seq 50); do [[ -s $T/mirror.port ]] && break; sleep 0.1; done
M=http://127.0.0.1:$(cat "$T/mirror.port")
# The site: its files, and as srp does, the launcher's places (the
# maintenance countdown of $T/state/maint), the admin page at
# /$(cat $T/state/admin-path) (its maintenance tab; the token of
# $T/state/admin-token; the answer holds the token, as the real page's
# does; every request of it, GET or POST, is a line of
# $T/state/admin-seen.log; a plan is a line of $T/state/admin-staged.log,
# saying whether a release folder was staged then), /api/stats saying a
# match runs for the first $T/state/busy asks, or (with $T/state/matches)
# the next line of that file for each ask (its last line once it is the only
# one), with "last_match_ends_in" of $T/state/ends-in when there is one (an
# srp from before it without), the launcher's matches with the "Time left"
# of each line of $T/state/page-left (minutes, or - for none yet),
# /api/client-files of the ZIP in place when asked.
python3 - "$T/site" "$T/site.port" "$T/state" "$T/live/download/Survarium-0.26g-srp-files.zip" >/dev/null 2>&1 <<'EOF' &
import functools, glob, hashlib, http.server, math, os, sys, time, urllib.parse
root, port_file, state, zip_path = sys.argv[1:5]
live = os.path.dirname(os.path.dirname(zip_path))
minute = float(os.environ.get("SKHID_TEST_SECONDS_PER_MINUTE", "60"))
def read(name, default=None):
    try:
        return open(os.path.join(state, name)).read().strip()
    except FileNotFoundError:
        return default
class Handler(http.server.SimpleHTTPRequestHandler):
    server_version, sys_version = "Caddy", ""
    def log_message(self, *args): pass
    def stats(self, online, matches):
        body = '{"online":%d,"registered":3,"matches":%d,"queued":0' % (online, matches)
        if read("ends-in") is not None:
            body += ',"last_match_ends_in":%d' % int(read("ends-in"))
        return self.answer(200, body + "}")
    def answer(self, status, body):
        data = body.encode()
        self.send_response(status)
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)
    def do_GET(self):
        if self.path == "/launcher/places/en":
            at = read("maint")
            if at is None:
                text = '<div class="cap">On the server now</div>'
            else:
                left = math.ceil(max(0.0, float(at) - time.time()) / minute)
                text = '<div class="cap warn">' + (f"Maintenance in {left} min" if left else "Maintenance: the server is down") + "</div>"
            rows = "".join("<tr><td class=m>Rudnya</td><td class=e>"
                           + ("&mdash;" if each == "-" else f"&le; {each} min") + "</td></tr>"
                           for each in (read("page-left") or "").split())
            return self.answer(200, "<table><tr><th class=m>Match</th><th class=e>Time left</th></tr>" + rows + "</table>" + text)
        if self.path == "/api/stats" and read("matches"):
            lines = read("matches").split("\n")
            if len(lines) > 1:
                open(os.path.join(state, "matches"), "w").write("\n".join(lines[1:]) + "\n")
            n = int(lines[0])
            return self.stats(3, n)
        if self.path == "/api/stats" and read("busy"):
            busy = int(read("busy"))
            open(os.path.join(state, "busy"), "w").write(str(busy - 1))
            if busy > 0:
                return self.stats(3, 1)
            return self.stats(0, 0)
        if self.path == "/api/stats" and read("ends-in") is not None:
            return self.stats(0, 0)
        if self.path == "/api/client-files":
            try:
                data = open(zip_path, "rb").read()
            except FileNotFoundError:
                return self.answer(404, "no client files are offered here\n")
            return self.answer(200, f"skhid-client-files 1\nsha256 {hashlib.sha256(data).hexdigest()}\nsize {len(data)}\n")
        if self.path == "/" + read("admin-path", "-"):
            open(os.path.join(state, "admin-seen.log"), "a").write("GET\n")
            return self.answer(200, "<form>the admin page</form>")
        return super().do_GET()
    def do_POST(self):
        open(os.path.join(state, "admin-seen.log"), "a").write(f"POST {self.path}\n")
        if self.path != "/" + read("admin-path", "-"):
            return self.answer(404, "Not found.")
        length = int(self.headers.get("Content-Length", 0))
        form = urllib.parse.parse_qs(self.rfile.read(length).decode())
        field = lambda name: form.get(name, [""])[0]
        log = open(os.path.join(state, "admin.log"), "a")
        token = read("admin-token")
        if field("token") != token:
            log.write("wrong token\n")
            return self.answer(200, "Wrong token.")
        page = f'<input type=hidden name=token value="{token}">'
        if field("tab") != "maint":
            return self.answer(200, page)
        if field("cancel"):
            was = read("maint") is not None
            if was:
                os.remove(os.path.join(state, "maint"))
            log.write("cancel\n")
            return self.answer(200, page + ("Maintenance cancelled." if was else "No maintenance was planned."))
        minutes = int(field("minutes"))
        open(os.path.join(state, "maint"), "w").write(str(time.time() + minutes * minute))
        log.write(f"plan {minutes}\n")
        staged = any(open(os.path.join(folder, "state")).read().strip() == "staging"
                     and os.path.isfile(os.path.join(folder, "server-game-data.db"))
                     and os.path.isfile(os.path.join(folder, "Survarium-0.26g-srp-files.zip"))
                     for folder in glob.glob(os.path.join(live, "release-rev*")) if os.path.isfile(os.path.join(folder, "state")))
        open(os.path.join(state, "admin-staged.log"), "a").write(f"plan {minutes} {'staged' if staged else 'NOT staged'}\n")
        return self.answer(200, page + f"Maintenance starts in {minutes} min, at 12:00 UTC.")
server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), functools.partial(Handler, directory=root))
open(port_file, "w").write(str(server.server_address[1]))
server.serve_forever()
EOF
pids+=($!)
for _ in $(seq 50); do [[ -s $T/site.port ]] && break; sleep 0.1; done
patches_port=$(cat "$T/patches.port")
test_patches_port=$(cat "$T/patches-test.port")
site_port=$(cat "$T/site.port")
P=http://127.0.0.1:$patches_port

# The lists start empty (their comment lines only), whatever the real ones hold.
grep "^#" "$here/patches.tsv" >"$T/patches.tsv"
grep "^#" "$here/patches-test.tsv" >"$T/patches-test.tsv"
cat >"$T/release.env" <<EOF
SKHID_JUMP=jump
SKHID_LIVE=live
SKHID_LIVE_DIR=$T/live
SKHID_SITE=http://127.0.0.1:$site_port
SKHID_PATCHES_SSH=patches
SKHID_PATCHES_DIR=$T/patches
SKHID_PATCHES_URL=$P
SKHID_PATCHES_CADDYFILE=$T/Caddyfile
SKHID_PATCH_LIST=$T/patches.tsv
SKHID_TEST_PATCH_LIST=$T/patches-test.tsv
SKHID_STOCK_DB=$T/resources.db
SKHID_RELEASE_LOCK=$T/lock
SKHID_BACKUP_DIR=$T/backups
EOF
export SKHID_RELEASE_ENV=$T/release.env

failures=0
ok() { echo "ok   $*"; }
fail() { echo "FAIL $*"; failures=$((failures + 1)); }
# expect <what> <pattern> <command...>: the command fails, saying <pattern>.
expect_refused() {
    local what=$1 pattern=$2 out
    shift 2
    if out=$(setsid -w "$@" </dev/null 2>&1); then
        fail "$what: ran ($out)"
    elif grep -q -- "$pattern" <<<"$out"; then
        ok "$what"
    else
        fail "$what: $out"
    fi
}
yes_to() { { yes || true; } | setsid -w "$@" 2>&1; }
check() { # <what> <test...>
    local what=$1
    shift
    if "$@"; then ok "$what"; else fail "$what"; fi
}
has() { grep -q -- "$2" <<<"$1"; }
quiet() { "$@" >/dev/null; }
rows() { grep -v '^#' "$1" | cut -f1 | tr '\n' ' ' | sed 's/ $//'; }
changed=0
changed_env() { # <sed expressions>: a copy of release.env changed so; prints its path
    changed=$((changed + 1))
    sed "$1" "$T/release.env" >"$T/changed-$changed.env"
    printf '%s\n' "$T/changed-$changed.env"
}

# The stock db: an archive of a dictionary and a project.
mkdir -p "$T/stock/gameplay/items" "$T/stock/projects/p/day"
echo "dictionary" >"$T/stock/gameplay/db_static_dictionaries"
echo "old item" >"$T/stock/gameplay/items/a.options"
# A project of nothing: a binary config of an empty table (srp-game-data
# keeps a project cut to its server objects, and refuses one that is not a
# config).
printf '\x18\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x03\0\0\0' >"$T/stock/projects/p/day/client_project"
"$bin" pack --dir "$T/stock" --out "$T/resources.db" >/dev/null

# build-patch
mkdir -p "$T/p3/gameplay/items"
echo "new item" >"$T/p3/gameplay/items/a.options"
echo "added" >"$T/p3/gameplay/items/b.options"
out=$(cd "$T/work" && "$here/build-patch" "$T/p3" 3 2>&1)
check "build-patch packs a folder" test -f "$T/work/resources.db.patch_3"
check "build-patch prints its sha256" has "$out" "sha256 $(sha256sum <"$T/work/resources.db.patch_3" | cut -d' ' -f1)"
expect_refused "build-patch never overwrites" "never overwritten" bash -c "cd $T/work && $here/build-patch $T/p3 3"
for bad in 0 256 03 x; do
    expect_refused "build-patch refuses revision $bad" "1 to 255" "$here/build-patch" "$T/p3" "$bad" --out "$T/x"
done
expect_refused "build-patch refuses another name" "must be called resources.db.patch_3" \
    "$here/build-patch" "$T/p3" 3 --out "$T/work/patch3.db"
check "--help is the usage" has "$("$here/build-patch" --help)" "usage: release/build-patch"

# upload-patch
patch=$T/work/resources.db.patch_3
sha3=$(sha256sum <"$patch" | cut -d' ' -f1)
out=$(echo n | setsid -w "$here/upload-patch" "$patch" 2>&1 || true)
check "upload-patch: a no uploads nothing" test ! -e "$T/patches/resources.db.patch_3.tmp" -a ! -e "$T/patches/resources.db.patch_3"
check "upload-patch shows the command" has "$out" "cat > $T/patches/resources.db.patch_3.tmp"
check "upload-patch: a no adds no row" test -z "$(rows "$T/patches.tsv")"
out=$("$here/upload-patch" --dry-run "$patch" 2>&1)
check "upload-patch --dry-run writes nothing" test -z "$(ls -A "$T/patches")"
check "upload-patch --dry-run adds no row" test -z "$(rows "$T/patches.tsv")"
out=$(yes_to "$here/upload-patch" "$patch")
check "upload-patch publishes" cmp -s "$patch" "$T/patches/resources.db.patch_3"
check "upload-patch: mode 644" test "$(stat -c %a "$T/patches/resources.db.patch_3")" = 644
check "upload-patch: no .tmp left" test ! -e "$T/patches/resources.db.patch_3.tmp"
check "upload-patch checks the URL" has "$out" "published: $P/resources.db.patch_3 (200"
check "upload-patch adds the row to the list" grep -qxF "$(printf 'resources.db.patch_3\t%s\t%s\t%s' "$sha3" "$(stat -c %s "$patch")" "$P/resources.db.patch_3")" "$T/patches.tsv"
expect_refused "upload-patch: the same file again" "with these same bytes" "$here/upload-patch" "$patch"
mkdir -p "$T/other" && echo other >"$T/p3/gameplay/items/c.options"
"$bin" pack --dir "$T/p3" --out "$T/other/resources.db.patch_3" >/dev/null
expect_refused "upload-patch never replaces a patch" "OTHER bytes" "$here/upload-patch" "$T/other/resources.db.patch_3"
check "the published patch is the first one" cmp -s "$patch" "$T/patches/resources.db.patch_3"
mkdir -p "$T/p2/gameplay" && echo two >"$T/p2/gameplay/two" && "$bin" pack --dir "$T/p2" --out "$T/other/resources.db.patch_2" >/dev/null
expect_refused "upload-patch refuses a patch below the list's last" "not above every patch" "$here/upload-patch" "$T/other/resources.db.patch_2"
cp "$patch" "$T/work/resources.db.patch_03"
expect_refused "upload-patch refuses a name the game does not mount" "must be resources.db.patch_<n>" \
    "$here/upload-patch" "$T/work/resources.db.patch_03"
head -c 100 /dev/zero >"$T/work/resources.db.patch_4"
expect_refused "upload-patch refuses what is not an archive" "not an archive" "$here/upload-patch" "$T/work/resources.db.patch_4"
# patch_12 (the menus): on the patches server already, not in the list (a
# run that stopped): upload-patch adds only the row.
mkdir -p "$T/p12/gameplay/menus" "$T/r12"
echo "menus" >"$T/p12/gameplay/menus/main"
"$bin" pack --dir "$T/p12" --out "$T/r12/resources.db.patch_12" >/dev/null
cp "$T/r12/resources.db.patch_12" "$T/patches/"
out=$(yes_to "$here/upload-patch" "$T/r12/resources.db.patch_12")
check "upload-patch: published, not listed: adds the row only" bash -c "grep -q 'published already, with these same bytes' <<<\"\$1\" && ! grep -q 'cat >' <<<\"\$1\"" _ "$out"
check "upload-patch: the list is 3, 12" test "$(rows "$T/patches.tsv")" = "resources.db.patch_3 resources.db.patch_12"
# --test: its own settings, never the production patches.
expect_refused "upload-patch --test needs the test patches server" "SKHID_TEST_PATCHES_SSH is not set" \
    "$here/upload-patch" --test "$T/r12/resources.db.patch_12"
cat >>"$T/release.env" <<EOF
SKHID_TEST_PATCHES_SSH=patches-test
SKHID_TEST_PATCHES_DIR=$T/patches-test
SKHID_TEST_PATCHES_URL=http://127.0.0.1:$test_patches_port
EOF
expect_refused "upload-patch --test refuses the production URL" "is the production patches URL" \
    env SKHID_RELEASE_ENV="$(changed_env "s|^SKHID_TEST_PATCHES_URL=.*|SKHID_TEST_PATCHES_URL=$P/|")" "$here/upload-patch" --test "$T/r12/resources.db.patch_12"
expect_refused "upload-patch --test refuses the production folder" "is the production patches folder" \
    env SKHID_RELEASE_ENV="$(changed_env "s|^SKHID_TEST_PATCHES_SSH=.*|SKHID_TEST_PATCHES_SSH=patches|; s|^SKHID_TEST_PATCHES_DIR=.*|SKHID_TEST_PATCHES_DIR=$T/patches/|")" \
    "$here/upload-patch" --test "$T/r12/resources.db.patch_12"
published_live=$(ls "$T/patches")
out=$(yes_to "$here/upload-patch" --test "$T/r12/resources.db.patch_12")
check "upload-patch --test publishes on the test patches server" cmp -s "$T/r12/resources.db.patch_12" "$T/patches-test/resources.db.patch_12"
check "upload-patch --test: its row in the test list" test "$(rows "$T/patches-test.tsv")" = "resources.db.patch_12"
check "upload-patch --test: the production list and folder as they were" \
    test "$(rows "$T/patches.tsv")|$(ls "$T/patches")" = "resources.db.patch_3 resources.db.patch_12|$published_live"
check "upload-patch --test says so" has "$out" "On the TEST patches server"

# build-server-db
sha12=$(sha256sum <"$T/r12/resources.db.patch_12" | cut -d' ' -f1)
"$bin" pack --dir "$T/p2" --out "$T/patches/resources.db.patch_9" >/dev/null
out=$(cd "$T/work" && "$here/build-server-db" --out "$T/work/sgd.db" 2>&1) || fail "build-server-db: $out"
check "build-server-db builds" test -f "$T/work/sgd.db"
check "build-server-db prints the offered list" \
    has "$out" "  patch_3 sha256 $sha3 urls $P/resources.db.patch_3"
check "build-server-db: a published file not in the list is not offered" bash -c "! grep -q patch_9 <<<\"\$1\"" _ "$out"
check "the data is made of the stock db and the listed patches" quiet "$bin" check "$T/work/sgd.db" --db "$T/resources.db" \
    --patch "$patch=$P/resources.db.patch_3" --patch "$T/r12/resources.db.patch_12=$P/resources.db.patch_12"
rm "$T/patches/resources.db.patch_9"
expect_refused "build-server-db never overwrites" "never overwritten" "$here/build-server-db" --out "$T/work/sgd.db"
expect_refused "build-server-db refuses another server's list" "is not http://127.0.0.1:$site_port/resources.db.patch_3" \
    env SKHID_RELEASE_ENV="$(changed_env "s|^SKHID_PATCHES_URL=.*|SKHID_PATCHES_URL=http://127.0.0.1:$site_port|")" "$here/build-server-db" --out "$T/work/sgd2.db"
printf 'resources.db.patch_7\t%s\t5\t%s\n' "$sha3" "$P/resources.db.patch_7" >"$T/bad.tsv"
expect_refused "build-server-db needs each listed patch at 200" "answered 404, not 200" \
    env SKHID_RELEASE_ENV="$(changed_env "s|^SKHID_PATCH_LIST=.*|SKHID_PATCH_LIST=$T/bad.tsv|")" "$here/build-server-db" --out "$T/work/sgd2.db"
printf 'resources.db.patch_3\t%s\t%s\t%s\n' "$sha12" "$(stat -c %s "$patch")" "$P/resources.db.patch_3" >"$T/bad.tsv"
expect_refused "build-server-db checks the served sha256 against the list" "the list says\|says $sha12" \
    env SKHID_RELEASE_ENV="$(changed_env "s|^SKHID_PATCH_LIST=.*|SKHID_PATCH_LIST=$T/bad.tsv|")" "$here/build-server-db" --out "$T/work/sgd2.db"
printf 'resources.db.patch_12\t%s\t1\t%s\nresources.db.patch_3\t%s\t1\t%s\n' "$sha12" "$P/resources.db.patch_12" "$sha3" "$P/resources.db.patch_3" >"$T/bad.tsv"
expect_refused "build-server-db refuses a list out of order" "not above the row before it" \
    env SKHID_RELEASE_ENV="$(changed_env "s|^SKHID_PATCH_LIST=.*|SKHID_PATCH_LIST=$T/bad.tsv|")" "$here/build-server-db" --out "$T/work/sgd2.db"
check "build-server-db: no file left by a refusal" test ! -e "$T/work/sgd2.db"
out=$("$here/build-server-db" --test --out "$T/work/sgd-test.db" 2>&1) || fail "build-server-db --test: $out"
check "build-server-db --test: the test list, the test URLs" has "$out" "  patch_12 sha256 $sha12 urls http://127.0.0.1:$test_patches_port/resources.db.patch_12"

# Mirrors: add-mirror, a row of several URLs, build-server-db of it. On a
# copy of the list (the live one stays that of sgd.db, which deploy takes).
size3=$(stat -c %s "$patch")
cp "$patch" "$T/mirror/resources.db.patch_3"
# The patch's size, other bytes: an older file under the mirror's name.
{ head -c $((size3 - 1)) "$patch"; printf '\377'; } >"$T/mirror/wrong_3"
wrong3=$(sha256sum <"$T/mirror/wrong_3" | cut -d' ' -f1)
cp "$T/patches.tsv" "$T/mirrored.tsv"
menv=$(changed_env "s|^SKHID_PATCH_LIST=.*|SKHID_PATCH_LIST=$T/mirrored.tsv|")
am=(env SKHID_RELEASE_ENV="$menv" "$here/add-mirror")
check "--help is the usage" has "$("$here/add-mirror" --help)" "usage: release/add-mirror"
expect_refused "add-mirror: a patch not in the list" "is not in $T/mirrored.tsv" "${am[@]}" resources.db.patch_5 "$M/files/x"
expect_refused "add-mirror: not a patch's name" "a patch's name is resources.db.patch_<n>" "${am[@]}" patch_3 "$M/files/x"
for bad in "ftp://127.0.0.1/x" "http://user:pw@127.0.0.1/x" "http://127.0.0.1/a b" "http://127.0.0.1/a#b" "http://127.0.0.1/" "127.0.0.1/x"; do
    expect_refused "add-mirror: a URL the updater does not take ($bad)" "is not a URL the players' updater takes" \
        "${am[@]}" resources.db.patch_3 "$bad"
done
expect_refused "add-mirror: a URL the row has" "is in the row of resources.db.patch_3 already" \
    "${am[@]}" resources.db.patch_3 "$P/resources.db.patch_3"
expect_refused "add-mirror: a web page in the place of the file" "answers with a web page (text/html; charset=utf-8)" \
    "${am[@]}" resources.db.patch_3 "$M/page/resources.db.patch_3"
expect_refused "add-mirror: other bytes" "gave sha256 $wrong3, not $sha3" "${am[@]}" resources.db.patch_3 "$M/files/wrong_3"
expect_refused "add-mirror: no such file" "answers 404, not 200" "${am[@]}" resources.db.patch_3 "$M/files/none"
expect_refused "add-mirror: six redirects, as the updater refuses them" "hop/6/resources.db.patch_3 answers" \
    "${am[@]}" resources.db.patch_3 "$M/hop/6/resources.db.patch_3"
check "add-mirror: a refusal changes no row" cmp -s "$T/patches.tsv" "$T/mirrored.tsv"
check "add-mirror: a refusal is not in the cache" bash -c "! grep -q 127.0.0.1:${M##*:}/ $T/cache/skhid-release/patch-urls.checked 2>/dev/null"
out=$("${am[@]}" resources.db.patch_3 "$M/hop/5/resources.db.patch_3" 2>&1) || fail "add-mirror: $out"
check "add-mirror: five redirects are followed, the URL added to the row" grep -qxF \
    "$(printf 'resources.db.patch_3\t%s\t%s\t%s %s' "$sha3" "$size3" "$P/resources.db.patch_3" "$M/hop/5/resources.db.patch_3")" "$T/mirrored.tsv"
check "add-mirror: every other line as it was" test "$(grep -v '^resources.db.patch_3' "$T/mirrored.tsv")" = "$(grep -v '^resources.db.patch_3' "$T/patches.tsv")"
check "add-mirror: the URL is in the cache" grep -qxF "$sha3 $size3 $M/hop/5/resources.db.patch_3" "$T/cache/skhid-release/patch-urls.checked"
out=$("${am[@]}" resources.db.patch_3 "$M/files/resources.db.patch_3" 2>&1) || fail "add-mirror: $out"
check "add-mirror: says the order" has "$out" "now has 3 URLs, tried in this order"
check "add-mirror: says to commit, and the next release" has "$out" "commit it; players get it with the next release"
: >"$T/mirror.log"
out=$(env SKHID_RELEASE_ENV="$menv" "$here/build-server-db" --out "$T/work/sgd-mirrors.db" 2>&1) || fail "build-server-db, mirrors: $out"
check "build-server-db: the data offers the mirrors in their order" has "$out" \
    "  patch_3 sha256 $sha3 urls $P/resources.db.patch_3 $M/hop/5/resources.db.patch_3 $M/files/resources.db.patch_3"
check "build-server-db: a mirror checked before is asked for its first bytes only" \
    bash -c "test -s $T/mirror.log && ! grep -v ' bytes=0-12\$' $T/mirror.log"
check "build-server-db: says so" has "$out" "mirror $M/files/resources.db.patch_3: as checked before"
check "the data is made of the list with its mirrors" quiet "$bin" check "$T/work/sgd-mirrors.db" --db "$T/resources.db" \
    --patch "$patch=$P/resources.db.patch_3 $M/hop/5/resources.db.patch_3 $M/files/resources.db.patch_3" \
    --patch "$T/r12/resources.db.patch_12=$P/resources.db.patch_12"
mv "$T/mirror/resources.db.patch_3" "$T/mirror/kept_3"
expect_refused "build-server-db: a mirror that no longer gives the patch" \
    "resources.db.patch_3: $M/hop/5/resources.db.patch_3 answers 404 now (it gave the file before)" \
    env SKHID_RELEASE_ENV="$menv" "$here/build-server-db" --out "$T/work/sgd-m2.db"
mv "$T/mirror/kept_3" "$T/mirror/resources.db.patch_3"
# The list's own rules for a row's URLs.
nine=$(for n in 1 2 3 4 5 6 7 8; do printf ' %s/files/m%s' "$M" "$n"; done)
for case in "$P/resources.db.patch_3  $M/x|one space apart" "$P/resources.db.patch_3 $M/x $M/x|is there twice" \
    "$P/resources.db.patch_3$nine|9 URLs; the players' updater takes at most 8" \
    "$P/resources.db.patch_3 ftp://127.0.0.1/x|ftp://127.0.0.1/x is not a URL the players' updater takes" \
    "$M/x $P/resources.db.patch_3|$M/x is not $P/resources.db.patch_3"; do
    printf 'resources.db.patch_3\t%s\t%s\t%s\n' "$sha3" "$size3" "${case%%|*}" >"$T/bad.tsv"
    expect_refused "the list refuses a row: ${case#*|}" "${case#*|}" \
        env SKHID_RELEASE_ENV="$(changed_env "s|^SKHID_PATCH_LIST=.*|SKHID_PATCH_LIST=$T/bad.tsv|")" "$here/build-server-db" --out "$T/work/sgd2.db"
done

# setup-patches-server
printf 'other.example {\n\timport stalker\n}\n' >"$T/Caddyfile"
cp "$T/Caddyfile" "$T/Caddyfile.before"
mv "$T/patches" "$T/patches.kept"
expect_refused "setup --check: something to do" "would do the above" "$here/setup-patches-server" --check
check "setup --check changes nothing" cmp -s "$T/Caddyfile" "$T/Caddyfile.before"
check "setup --check makes no folder" test ! -e "$T/patches"
mv "$T/patches.kept" "$T/patches"
touch "$T/caddy-fails"
out=$(yes_to "$here/setup-patches-server" || true)
check "setup: a failed caddy validate puts the Caddyfile back" cmp -s "$T/Caddyfile" "$T/Caddyfile.before"
rm "$T/caddy-fails"
touch "$T/systemctl-fails"
out=$(yes_to "$here/setup-patches-server" || true)
check "setup: a failed reload puts the Caddyfile back" cmp -s "$T/Caddyfile" "$T/Caddyfile.before"
check "setup: ... and says so" has "$out" "validate or reload failed: the backup is back"
rm "$T/systemctl-fails"
: >"$T/log"
out=$(yes_to "$here/setup-patches-server")
check "setup appends the block" test "$(grep -cxF "$P {" "$T/Caddyfile")" = 1
check "setup keeps the other site as it was" cmp -s "$T/Caddyfile.before" <(head -n 3 "$T/Caddyfile")
check "setup keeps a dated backup" bash -c "ls $T/Caddyfile.bak-*-before-skhid-patches >/dev/null"
check "setup reloads caddy" grep -q "systemctl reload caddy" "$T/log"
check "setup probes the patches" has "$out" "patches there: 2, each at 200"
out=$("$here/setup-patches-server" --check 2>&1)
check "setup --check after: nothing to do" has "$out" "nothing to do"
out=$(echo n | setsid -w "$here/setup-patches-server" 2>&1)
check "setup again: nothing to do, nothing asked" bash -c "! grep -q 'Go ahead' <<<'$out' && grep -q 'nothing to do' <<<'$out'"
expect_refused "setup --test --check: the test site is another block" "would do the above" \
    env SKHID_RELEASE_ENV="$(changed_env "s|^SKHID_PATCHES_CADDYFILE=.*||; \$aSKHID_TEST_PATCHES_CADDYFILE=$T/Caddyfile")" "$here/setup-patches-server" --test --check
cp "$T/Caddyfile" "$T/Caddyfile.good"
printf 'http://127.0.0.1:%s {\n\tfile_server\n}\n' "$patches_port" >>"$T/Caddyfile"
expect_refused "setup refuses the block twice" "2 times" "$here/setup-patches-server" --check
printf 'other.example {\n}\n:%s {\n\tfile_server\n}\n' "$patches_port" >"$T/Caddyfile"
expect_refused "setup refuses another site on the port" "another site on port" "$here/setup-patches-server" --check

# deploy
python3 - "$T/work" <<'EOF'
import sys, zipfile
at = 0x380862 - 0x11000 + 0x400
for revision in range(2, 21):
    exe = bytearray(at + 16)
    exe[at:at + 6] = bytes([0xC6, 0x44, 0x24, 0x27, revision, 0x90])
    with zipfile.ZipFile(f"{sys.argv[1]}/r{revision}.zip", "w") as z:
        z.writestr("game/binaries/x86/survarium_skhid.exe", bytes(exe))
        z.writestr("game/binaries/x86/updater_skhid.exe", b"updater")
EOF
cp "$T/work/r11.zip" "$T/live/download/Survarium-0.26g-srp-files.zip"
echo "old data" >"$T/live/download/server-game-data.db"
# The original menus' blobs the updater's restore to stock fetches.
mkdir -p "$T/live/download/resources"
for blob in 0ff118a8 ab6f22d8 805ea5f8; do echo "$blob" >"$T/live/download/resources/${blob}0000"; done
printf 'SRP_ADMIN_TOKEN=very-secret\nSRP_MIN_PATCH_REVISION=11\nOTHER=1' >"$T/live/.env"
zip12=$T/work/r12.zip
printf '{"patch_3": {"url": "%s/resources.db.patch_3", "sha256": "%s"}}' "$P" "$sha3" >"$T/site/patches"
echo '{"online":0,"registered":3,"matches":0,"queued":0}' >"$T/site/api/stats"
"$bin" build --db "$T/resources.db" --patch "$patch=$P/resources.db.patch_3" --out "$T/work/sgd-no12.db" >/dev/null
tree() { (cd "$T/live" && find . -path "./release-rev*" -prune -o -type f -exec sha256sum {} + | sort); }
folder() { find "$T/live" -maxdepth 1 -name "release-rev$1-*-$2" | head -n 1; } # <revision> <label>
before=$(tree)
db_before=$(cat "$T/volume/srp.db")
: >"$T/log"
out=$("$here/deploy" --dry-run --zip "$zip12" --server-data "$T/work/sgd.db" --revision 12 2>&1) || fail "deploy --dry-run: $out"
check "deploy --dry-run changes nothing" test "$(tree)" = "$before"
check "deploy --dry-run runs nothing there" test ! -s "$T/log"
check "deploy --dry-run shows the restart" has "$out" "docker compose up -d --no-deps --force-recreate srp26"
check "deploy --dry-run shows the release folder" has "$out" "mkdir release-rev12-[0-9]\{8\}-[0-9]\{4\} "
expect_refused "deploy refuses a ZIP of another revision" "says patch revision 12, not 13" \
    "$here/deploy" --zip "$zip12" --server-data "$T/work/sgd.db" --revision 13
expect_refused "deploy refuses data offering a later patch" "offers patch_3, above revision 2" \
    "$here/deploy" --zip "$T/work/r2.zip" --server-data "$T/work/sgd.db" --revision 2
expect_refused "deploy refuses revision 12 without patch_12 (the menus)" "does not offer patch_12 (the menus)" \
    "$here/deploy" --zip "$zip12" --server-data "$T/work/sgd-no12.db" --revision 12
expect_refused "deploy refuses data that is not the list's" "does not offer exactly the patches of" \
    "$here/deploy" --zip "$T/work/r11.zip" --server-data "$T/work/sgd-no12.db" --revision 11
mv "$T/live/download/resources/ab6f22d80000" "$T/ab6f.kept"
expect_refused "deploy refuses when an original blob is gone from download/resources" "lacks the original blobs .*lobby_menu ab6f22d8" \
    "$here/deploy" --zip "$zip12" --server-data "$T/work/sgd.db" --revision 12
mv "$T/ab6f.kept" "$T/live/download/resources/ab6f22d80000"
check "deploy: the refusals changed nothing" test "$(tree)" = "$before"
: >"$T/args"
out=$(yes_to "$here/deploy" --zip "$zip12" --server-data "$T/work/sgd.db" --revision 12 --label test) || fail "deploy: $out"
F=$(folder 12 test)
check "deploy makes the release folder release-rev12-<UTC YYYYMMDD-HHMM>-test" bash -c "[[ \${1##*/} =~ ^release-rev12-[0-9]{8}-[0-9]{4}-test$ ]]" _ "$F"
check "deploy puts the ZIP" cmp -s "$zip12" "$T/live/download/Survarium-0.26g-srp-files.zip"
check "deploy puts the server data" cmp -s "$T/work/sgd.db" "$T/live/download/server-game-data.db"
check "deploy: the new files stay in the folder (hard links)" test "$F/Survarium-0.26g-srp-files.zip" -ef "$T/live/download/Survarium-0.26g-srp-files.zip" -a "$F/server-game-data.db" -ef "$T/live/download/server-game-data.db"
check "deploy keeps the old ZIP in the folder" cmp -s "$T/work/r11.zip" "$F/old-Survarium-0.26g-srp-files.zip"
check "deploy keeps the old data in the folder" grep -q 'old data' "$F/old-server-game-data.db"
check "deploy: old.sha and new.sha" bash -c "grep -q ' download/server-game-data.db$' $F/old.sha && grep -q \"^$(sha256sum <"$zip12" | cut -d' ' -f1)  Survarium-0.26g-srp-files.zip$\" $F/new.sha"
check "deploy keeps the image IDs" test "$(cat "$F/image.old")|$(cat "$F/image.new")" = "$(cat "$T/docker/running")|$(cat "$T/docker/running")"
check "deploy keeps srp's backup from the new start: srp.db before the release" bash -c "test -f $F/\$(cat $F/srp-db.name) && cmp -s $F/\$(cat $F/srp-db.name) <(printf '%s\n' \"\$1\")" _ "$db_before"
name=$(cat "$F/srp-db.name" 2>/dev/null || echo none)
check "deploy copies it to the jump host (folder 700, file 600)" bash -c "cmp -s $F/$name $T/backups/$name && test \$(stat -c %a $T/backups) = 700 -a \$(stat -c %a $T/backups/$name) = 600"
check "deploy: ... with its line in SHA256SUMS there" grep -q "  $name\$" "$T/backups/SHA256SUMS"
check "deploy: ... the sha256 compared on both ends" has "$out" "srp.db off the live host: jump:$T/backups/$name, sha256 [0-9a-f]* on both ends"
check "deploy runs no other container" bash -c "! grep -q '^docker run' $T/log"
check "deploy writes the rollback in the folder" test -f "$F/rollback.sh" -a -f "$F/rollback.env"
check "deploy: the folder's state is released" grep -qx released "$F/state"
check "deploy: the folder's release.log, without .env" bash -c "grep -q 'deployed revision 12' $F/release.log && ! grep -q very-secret $F/release.log"
check "deploy tags the running image by its ID" test "$(cat "$T"/docker/tags/srp-026g_before-rev12-*-test)" = "$(cat "$T/docker/running")"
check "deploy sets the revision" grep -qx "SRP_MIN_PATCH_REVISION=12" "$T/live/.env"
check "deploy keeps the rest of .env" test "$(grep -v '^SRP_MIN_PATCH_REVISION=' "$T/live/.env")" = $'SRP_ADMIN_TOKEN=very-secret\nOTHER=1'
check "deploy never prints .env" bash -c "! grep -q very-secret <<<\"\$1\"" _ "$out"
check "deploy makes srp26 anew" grep -q "docker compose up -d --no-deps --force-recreate srp26" "$T/log"
check "deploy: the ZIP goes in after the restart" bash -c "a=\$(grep -n 'up -d --no-deps' $T/args | head -n1 | cut -d: -f1); z=\$(grep -n 'mv -f download/.Survarium-0.26g-srp-files.zip.new' $T/args | head -n1 | cut -d: -f1); test -n \"\$a\" -a -n \"\$z\" && ((a < z))"
check "deploy checks /patches and /api/client-files" has "$out" "/api/client-files: the new ZIP"
check "deploy goes through the jump host" bash -c "grep -qx jump $T/log && grep -qx live $T/log"
check "deploy leaves nothing half-done" bash -c "! ls $T/live/download/.*.new $F/*.part 2>/dev/null"
check "deploy frees the lock" test ! -e "$T/lock"
sed -i '/^SRP_MIN_PATCH_REVISION=/d' "$T/live/.env"
out=$(yes_to "$here/deploy" --zip "$zip12" --server-data "$T/work/sgd.db" --revision 12 --label again) || fail "deploy again: $out"
check "deploy adds the setting when missing, on a line of its own" grep -qx "SRP_MIN_PATCH_REVISION=12" "$T/live/.env"
mkdir "$T/lock" && echo "someone@else, since then" >"$T/lock/owner"
expect_refused "deploy waits for no other release (the lock)" "holds the lock on the jump host: someone@else" \
    "$here/deploy" --zip "$T/work/r13.zip" --server-data "$T/work/sgd.db" --revision 13
check "deploy: ... and leaves the other's lock" test -f "$T/lock/owner"
rm -r "$T/lock"
# /patches that is not the data's: srp's log, then the rollback.
touch "$T/patches-stale"
echo '{}' >"$T/site/patches"
starts=$(cat "$T/docker/starts")
out=$(yes_to "$here/deploy" --zip "$T/work/r13.zip" --server-data "$T/work/sgd.db" --revision 13 --label wrong || true)
rm "$T/patches-stale"
F=$(folder 13 wrong)
check "deploy says when /patches is not the data's" has "$out" "/patches is {}, not"
check "deploy: ... shows srp's log" has "$out" "srp26's last log lines:"
check "deploy: ... rolls back" has "$out" "ROLLING BACK"
check "deploy: ... the ZIP never moved" cmp -s "$zip12" "$T/live/download/Survarium-0.26g-srp-files.zip"
check "deploy: ... the setting back" grep -qx "SRP_MIN_PATCH_REVISION=12" "$T/live/.env"
check "deploy: ... srp.db back from the new start's backup (its writes gone)" bash -c "! grep -q '^start $((starts + 1)) ' $T/volume/srp.db && grep -q '^start $((starts + 2)) ' $T/volume/srp.db"
check "deploy: ... srp26 running again" test "$(cat "$T/docker/state")" = running
check "deploy: ... the folder says rolled-back" grep -qx rolled-back "$F/state"
check "deploy: ... nothing half-done" bash -c "! ls $T/live/download/.*.new 2>/dev/null"
out=$(echo n | setsid -w "$here/deploy" --zip "$zip12" --server-data "$T/work/sgd.db" --revision 12 2>&1 || true)
check "deploy: a no stops before any write" has "$out" "stopped: not confirmed"

# release.sh
# main: a checkout of its own (its origin a bare repository here, which the
# release's record goes to: its tag, its branch, the merge of its PR) with
# the REVISION, the srp-026g files the image is built of, the revision
# table and the changelog; the live tree is main's. build-zip is a stand-in
# (the ZIP of the revision it is given); gh is one too (a PR is a file, its
# merge a merge commit pushed to the bare repository's main); a minute is a
# second.
git_t() { git -c user.name=test -c user.email=test@example.org -c init.defaultBranch=main "$@"; }
git_t init -q --bare "$T/origin.git"
git_t clone -q "$T/origin.git" "$T/repo" 2>/dev/null
git -C "$T/repo" config user.name "the owner"
git -C "$T/repo" config user.email owner@example.org
src=$T/repo/srp-026g
mkdir -p "$src/client-patches" "$src/certs" "$src/crates/a/src" "$src/resources/binary-config-examples" \
    "$src/resources/game-026g" "$src/scripts" "$src/servers/srp/src" "$src/docs" "$src/crates/session/src/v026g"
# The changes of what a match computes (the replays' list), none to come.
sim=$src/crates/session/src/v026g/simulation_changes.tsv
printf '# revision\twhat changed\n11\tthe TDM end\n' >"$sim"
for f in Cargo.toml Cargo.lock Dockerfile .dockerignore docker-compose.yml certs/login.crt crates/a/src/lib.rs \
    resources/binary-config-examples/a.cfg resources/game-026g/README scripts/run.sh servers/srp/src/main.rs docs/notes.md; do
    echo "$f 1" >"$src/$f"
done
cat >"$src/Dockerfile" <<'DOCKERFILE'
FROM rust:1-slim-bookworm AS build
COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY servers servers
COPY scripts scripts
COPY resources/binary-config-examples resources/binary-config-examples
COPY resources/game-026g resources/game-026g
COPY resources/not-in-main resources/not-in-main
FROM debian:bookworm-slim
COPY --from=build /srp /usr/local/bin/srp
COPY certs ./certs
DOCKERFILE
commit() { # <message>: a commit on main, after the records merged there
    git_t -C "$T/repo" pull -q --ff-only origin main 2>/dev/null || true
    git_t -C "$T/repo" add -A && git_t -C "$T/repo" commit -q --allow-empty -m "$1" && git_t -C "$T/repo" push -q origin HEAD:main 2>/dev/null
}
merge_pr() { # <number> <title>: a feature PR merged into main as GitHub does
    git_t -C "$T/repo" pull -q --ff-only origin main 2>/dev/null || true
    git_t -C "$T/repo" checkout -q -b "feat-$1"
    echo "feature $1" >"$src/docs/feature-$1.md"
    git_t -C "$T/repo" add -A && git_t -C "$T/repo" commit -qm "$2, the work"
    git_t -C "$T/repo" checkout -q main
    git_t -C "$T/repo" merge -q --no-ff "feat-$1" -m "Merge pull request #$1 from srp/feat-$1" -m "$2"
    git_t -C "$T/repo" branch -q -D "feat-$1"
    git_t -C "$T/repo" push -q origin HEAD:main 2>/dev/null
}
# REVISION is the last release's: the live exe's, 11.
echo "REVISION = 11" >"$src/client-patches/patch_026g.py"
printf '# Client patches\n\n| | |\n|---|---|\n| Revision 10 | ten |\n| Revision 11 | eleven |\n| Revision 4 | four |\n| The players'"'"' download | the zip |\n' \
    >"$src/client-patches/README.md"
printf '# Skhid 0.26g — what changed\n\nNewest first.\n\n---\n\n## 15 Oct, 20:02 — an older entry\n\n- something\n' >"$T/repo/CHANGELOG-SKHID-026G.md"
commit "revision 11, as released"
git -C "$T/repo" archive HEAD:srp-026g Cargo.toml Cargo.lock Dockerfile .dockerignore docker-compose.yml certs crates \
    resources/binary-config-examples resources/game-026g scripts servers | tar -x -C "$T/live"
cat >"$T/build-zip" <<'STANDIN'
#!/usr/bin/env bash
# build-zip --ref <commit> --revision <n> --work <dir> [--test]: the ZIP of
# r<n>.zip and the exe's two sha256 (exe.sha256); its arguments are the
# last line of $TEST_DIR/build-zip.args.
printf '%s\n' "$*" >>"$TEST_DIR/build-zip.args"
while (($#)); do
    case $1 in
    --work) work=$2; shift 2 ;;
    --ref) ref=$2; shift 2 ;;
    --revision) revision=$2; shift 2 ;;
    *) shift ;;
    esac
done
if [[ -e $TEST_DIR/build-zip-fails ]]; then echo "build-zip: the build failed" >&2; exit 1; fi
# A slow build: the seconds of $TEST_DIR/build-zip-sleep.
if [[ -s $TEST_DIR/build-zip-sleep ]]; then sleep "$(cat "$TEST_DIR/build-zip-sleep")"; fi
git -C "$TEST_DIR/repo" cat-file -e "$ref^{commit}" || exit 1
mkdir -p "$work"
cp "$TEST_DIR/work/r$revision.zip" "$work/Survarium-0.26g-srp-files.zip"
printf '%s with-server\n%s without-server\n' "$(printf 'with %s' "$revision" | sha256sum | cut -c1-64)" \
    "$(printf 'without %s' "$revision" | sha256sum | cut -c1-64)" >"$work/exe.sha256"
echo "  CHANGED  game/binaries/x86/survarium_skhid.exe"
echo "updater_skhid.exe: unchanged (the same file as live)"
STANDIN
chmod +x "$T/build-zip"
# build-site --tree <dir> [--with <patch>]: the site's pages in <dir> built
# from the list (SKHID_PATCH_LIST of the release.env) and --with's patch:
# each built-from.tsv, and a page.js naming them; its arguments are a line of
# $TEST_DIR/build-site.args. $TEST_DIR/build-site-fails: it fails;
# $TEST_DIR/build-site-wrong: it leaves --with's patch out.
cat >"$T/build-site" <<'STANDIN'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >>"$TEST_DIR/build-site.args"
tree='' with=''
while (($#)); do
    case $1 in
    --tree) tree=$2; shift 2 ;;
    --with) with=$2; shift 2 ;;
    *) shift ;;
    esac
done
if [[ -e $TEST_DIR/build-site-fails ]]; then echo "build.py: the Maps tab's build failed" >&2; exit 1; fi
. "$SKHID_RELEASE_ENV"
rows=$(grep -v '^#' "$SKHID_PATCH_LIST" | cut -f1,2)
if [[ -n $with && ! -e $TEST_DIR/build-site-wrong ]]; then rows+=$'\n'"${with##*/}"$'\t'"$(sha256sum <"$with" | cut -c1-64)"; fi
for dir in maps damage; do
    mkdir -p "$tree/srp-026g/servers/web-server/$dir"
    { echo "# The patches the $dir page was built from: name, sha256."; grep . <<<"$rows"; } >"$tree/srp-026g/servers/web-server/$dir/built-from.tsv"
    printf 'built from %s\n' "$(cut -f1 <<<"$rows" | tr '\n' ' ')" >"$tree/srp-026g/servers/web-server/$dir/page.js"
done
echo "the Maps tab and the calculator: built"
STANDIN
chmod +x "$T/build-site"
: >"$T/build-site.args"
# gh: `auth status` (not with $TEST_DIR/gh-no-auth), `pr create` (a PR is
# $TEST_DIR/gh/<n>: its head branch, title and body; it prints its URL),
# `pr merge <url> --merge` (a merge commit of the head branch pushed to the
# bare repository's main, as GitHub makes it; refused with
# $TEST_DIR/gh-merge-fails). Its calls are lines of $TEST_DIR/gh.log.
cat >"$T/bin/gh" <<'STANDIN'
#!/usr/bin/env bash
printf 'gh %s\n' "$*" >>"$TEST_DIR/gh.log"
mkdir -p "$TEST_DIR/gh"
case "$1 $2" in
"auth status")
    if [[ -e $TEST_DIR/gh-no-auth ]]; then echo "You are not logged into any GitHub hosts. To log in, run: gh auth login" >&2; exit 1; fi
    echo "Logged in to github.com account the-owner" ;;
"pr create")
    shift 2
    while (($#)); do
        case $1 in
        --head) head=$2; shift 2 ;;
        --base) base=$2; shift 2 ;;
        --title) title=$2; shift 2 ;;
        --body-file) body=$(cat "$2"); shift 2 ;;
        *) shift ;;
        esac
    done
    [[ $base == main && -n $head && -n $title ]] || { echo "gh pr create: not a PR to main" >&2; exit 1; }
    git -C "$TEST_DIR/origin.git" rev-parse -q --verify "refs/heads/$head" >/dev/null || { echo "no branch $head on origin" >&2; exit 1; }
    n=$(($(ls "$TEST_DIR/gh" | grep -c '^[0-9]*$') + 150))
    mkdir "$TEST_DIR/gh/$n"
    printf '%s\n' "$head" >"$TEST_DIR/gh/$n/head"
    printf '%s\n' "$title" >"$TEST_DIR/gh/$n/title"
    printf '%s\n' "$body" >"$TEST_DIR/gh/$n/body"
    echo open >"$TEST_DIR/gh/$n/state"
    echo "https://github.com/srp/skhid-srp/pull/$n" ;;
"pr merge")
    n=${3##*/}
    [[ -d $TEST_DIR/gh/$n && " $* " == *" --merge "* ]] || { echo "gh pr merge: no PR $3, or not --merge" >&2; exit 1; }
    if [[ -e $TEST_DIR/gh-merge-fails ]]; then
        echo "GraphQL: Repository rule violations found: Changes must be made through a pull request with an approving review (mergePullRequest)" >&2
        exit 1
    fi
    clone=$(mktemp -d "$TEST_DIR/tmp/gh-merge.XXXXXX")
    git clone -q "$TEST_DIR/origin.git" "$clone/r" 2>/dev/null &&
        git -C "$clone/r" -c user.name=GitHub -c user.email=noreply@github.com merge -q --no-ff "origin/$(cat "$TEST_DIR/gh/$n/head")" \
            -m "Merge pull request #$n from srp/$(cat "$TEST_DIR/gh/$n/head")" -m "$(cat "$TEST_DIR/gh/$n/title")" &&
        git -C "$clone/r" push -q origin HEAD:main 2>/dev/null || exit 1
    rm -rf "$clone"
    echo merged >"$TEST_DIR/gh/$n/state" ;;
*) echo "gh $*: not in the stand-in" >&2; exit 1 ;;
esac
STANDIN
chmod +x "$T/bin/gh"
# The record on the bare repository: a tag's object, main's file at <path>.
origin_tag() { git -C "$T/origin.git" rev-parse -q --verify "refs/tags/$1" 2>/dev/null; }
origin_branch() { git -C "$T/origin.git" rev-parse -q --verify "refs/heads/$1" 2>/dev/null; }
on_main() { git -C "$T/origin.git" show "main:$1"; }
main_revision() { on_main srp-026g/client-patches/patch_026g.py | sed -n 's/^REVISION = //p'; }
no_record() { # <revision>: neither the tag, nor the branch, nor REVISION <revision> on main
    ! origin_tag "026g-rev$1" >/dev/null && ! origin_branch "release/record-rev$1" >/dev/null &&
        test "$(main_revision)" != "$1" && ! git -C "$T/repo" rev-parse -q --verify "refs/tags/026g-rev$1" >/dev/null
}
mkdir -p "$T/game" "$T/tmp"
cat >>"$T/release.env" <<ENV
SKHID_GAME=$T/game
SKHID_GAME_SERVER=127.0.0.1:1234
SKHID_LOCALIZATION=english
SKHID_MIN_FREE_MB=1
SKHID_LIVE_MIN_FREE_MB=1
ENV
export SKHID_TEST_REPO=$T/repo SKHID_TEST_BUILD_ZIP=$T/build-zip SKHID_TEST_BUILD_SITE=$T/build-site XDG_CACHE_HOME=$T/cache TMPDIR=$T/tmp
SKHID_TEST_STOCK_SHA256=$(sha256sum <"$T/resources.db" | cut -d' ' -f1)
export SKHID_TEST_STOCK_SHA256
token=tok3n-$RANDOM$RANDOM$RANDOM
admin_path=hidden-$RANDOM$RANDOM
printf '%s\n' "$token" >"$T/state/admin-token"
printf '%s\n' "$admin_path" >"$T/state/admin-path"
cp "$T/work/r11.zip" "$T/live/download/Survarium-0.26g-srp-files.zip"
# The first release with server data: there is none on the live host yet,
# nor /patches.
rm "$T/live/download/server-game-data.db" "$T/site/patches"
printf 'SRP_ADMIN_PATH=%s\nSRP_ADMIN_TOKEN="%s"\nSRP_WEB_TRUST_PROXY=on\nSRP_MIN_PATCH_REVISION=11\nSRP_DAILY_BONUS=5\nOTHER=1\n' \
    "$admin_path" "$token" >"$T/live/.env"
ln -sf "$T/live/download/Survarium-0.26g-srp-files.zip" "$T/site/download"
echo '{"online":0,"registered":3,"matches":0,"queued":0}' >"$T/site/api/stats"
touch "$T/state/admin.log"
: >"$T/outputs"
run() { # <how: yes | no | wait | quiet> release.sh <args...>: its output into $out (and $T/outputs)
    local how=$1
    shift
    : >"$T/log"
    : >"$T/args.run"
    case $how in
    yes) out=$({ yes || true; } | setsid -w "$here/release.sh" "$@" 2>&1) ;;
    no) out=$(echo n | setsid -w "$here/release.sh" "$@" 2>&1) ;;
    wait) out=$({ sleep 6; } | setsid -w "$here/release.sh" "$@" 2>&1) ;;
    quiet) out=$(setsid -w "$here/release.sh" "$@" </dev/null 2>&1) ;;
    esac
    local status=$?
    printf '%s\n' "$out" >>"$T/outputs"
    return "$status"
}
admin_log() { tr '\n' ' ' <"$T/state/admin.log" | sed 's/ $//'; }
offers() { "$bin" info "$T/live/download/server-game-data.db" 2>/dev/null | grep -q "resources.db.$1 "; }
nothing_staged() { ! ls "$T/live/download/".*.new "$T"/live/release-rev*/*.part "$T"/live/release-rev*/src >/dev/null 2>&1 && ! grep -qxE 'staging|swapping' "$T"/live/release-rev*/state; }
first_line() { grep -n -- "$1" "$T/args" | tail -n +"${2:-1}" | head -n 1 | cut -d: -f1; }

# A whole release, no new content (patch_3 and patch_12 listed), the server
# code as live; a feature PR merged since the last release (11, no tag yet).
merge_pr 7 "A feature | with a bar"
released=$(git -C "$T/repo" rev-parse HEAD)
: >"$T/args"
run yes --minutes 2 --label a || fail "release.sh: $out"
check "release: its revision is the live exe's + 1" has "$out" "the live exe is revision 11: this release is revision 12"
check "release: ... built so, main's patch_026g.py not edited (REVISION 11 there)" bash -c \
    "tail -n 1 $T/build-zip.args | grep -q -- '--ref $released --revision 12 ' && git -C $T/repo show $released:srp-026g/client-patches/patch_026g.py | grep -qx 'REVISION = 11'"
check "release: plans the maintenance from the jump host" test "$(admin_log)" = "plan 2"
check "release: sees the countdown on the launcher page" has "$out" 'the launcher says "Maintenance in [12] min"'
check "release: finds the server code as live" has "$out" "the live tree is main's"
check "release: ... and builds no image" bash -c "! grep -q ' build ' $T/log"
check "release: no site pages on main: nothing to build, none built" bash -c \
    "grep -q 'no srp-026g/servers/web-server/maps/built-from.tsv on main: nothing to build' <<<\"\$1\" && test ! -s $T/build-site.args" _ "$out"
check "release: checks the listed patches on the patches server" has "$out" "the offered-patch list: 2 patch(es), each on the patches server"
check "release: the new ZIP is in place" cmp -s "$T/work/r12.zip" "$T/live/download/Survarium-0.26g-srp-files.zip"
check "release: the new server data is in place (patch_3)" offers patch_3
check "release: ... and patch_12" offers patch_12
check "release: the old ZIP is kept in its folder" bash -c "cmp -s $T/work/r11.zip $(folder 12 a)/old-Survarium-0.26g-srp-files.zip"
check "release: srp.db as it was before the release, in the folder" bash -c "F=$(folder 12 a); test -s \$F/\$(cat \$F/srp-db.name)"
check "release: ... and on the jump host" bash -c "F=$(folder 12 a); cmp -s \$F/\$(cat \$F/srp-db.name) $T/backups/\$(cat \$F/srp-db.name)"
check "release: the folder's changed.txt" grep -q "the players' ZIP" "$(folder 12 a)/changed.txt"
check "release: the folder's release.log" grep -q "released revision 12" "$(folder 12 a)/release.log"
check "release: no old data to keep (there was none)" test ! -e "$(folder 12 a)/old-server-game-data.db"
check "release: the rollback removes the data (there was none)" grep -qx "HAVE_DATA=0" "$(folder 12 a)/rollback.env"
check "release: SRP_MIN_PATCH_REVISION=12" grep -qx "SRP_MIN_PATCH_REVISION=12" "$T/live/.env"
check "release: the rest of .env as it was" test "$(grep -vc '^SRP_MIN_PATCH_REVISION=' "$T/live/.env")" = 5
check "release: restarts srp26" grep -q "docker compose up -d --no-deps --force-recreate srp26" "$T/log"
check "release: stops srp26 before it moves anything" bash -c "s=\$(grep -n 'compose stop srp26' $T/args | head -n1 | cut -d: -f1); b=\$(grep -n 'mv -f download/.server-game-data.db.new' $T/args | head -n1 | cut -d: -f1); test -n \"\$s\" -a -n \"\$b\" && ((s < b))"
check "release: the ZIP goes in after the restart" bash -c "a=\$(grep -n 'up -d --no-deps' $T/args | head -n1 | cut -d: -f1); z=\$(grep -n 'mv -f download/.Survarium-0.26g-srp-files.zip.new' $T/args | head -n1 | cut -d: -f1); test -n \"\$a\" -a -n \"\$z\" && ((a < z))"
check "release: waits for no match" has "$out" "5. no match is running"
check "release: checks /patches and /api/client-files" has "$out" "/api/client-files: the new ZIP"
check "release: checks the log's game data line" has "$out" "log: INFO game data: .* 2 patch(es) offered"
check "release: prints the rollback" has "$out" "release/rollback release-rev12-[0-9-]*-a"
check "release: the rollback puts the running image back, by its ID" grep -qx "OLD_IMAGE=sha256:[0-9a-f]*" "$(folder 12 a)/rollback.env"
check "release: leaves nothing staged" nothing_staged
check "release: the maintenance ended with the restart" test ! -e "$T/state/maint"
check "release: took the lock on the jump host" grep -q "^ssh jump mkdir $T/lock" "$T/args"
check "release: ... and freed it" test ! -e "$T/lock"
# The record: the tag on the commit built, the commit "Release revision 12"
# merged into main as a merge commit.
zip_sha=$(sha256sum <"$T/work/r12.zip" | cut -d' ' -f1)
data_sha=$(sha256sum <"$T/live/download/server-game-data.db" | cut -d' ' -f1)
exe_with=$(printf 'with 12' | sha256sum | cut -c1-64) exe_without=$(printf 'without 12' | sha256sum | cut -c1-64)
check "release: the record: an annotated tag 026g-rev12 on origin" test "$(git -C "$T/origin.git" cat-file -t 026g-rev12 2>/dev/null)" = tag
check "release: ... on the commit that was built" test "$(git -C "$T/origin.git" rev-parse '026g-rev12^{commit}' 2>/dev/null)" = "$released"
tag_text=$(git -C "$T/origin.git" cat-file tag 026g-rev12 2>/dev/null || true)
check "release: ... its message: the UTC time, the ZIP's and the server data's sha256" bash -c \
    "grep -qE '^Revision 12, released 20[0-9]{2}-[0-9]{2}-[0-9]{2} [0-9]{2}:[0-9]{2}:[0-9]{2} UTC$' <<<\"\$1\" && grep -q 'sha256 $zip_sha$' <<<\"\$1\" && grep -q 'server-game-data.db: sha256 $data_sha$' <<<\"\$1\" && grep -q 'sha256 $exe_with (--server=127.0.0.1:1234), $exe_without without a server' <<<\"\$1\"" _ "$tag_text"
check "release: ... its tagger is the owner (no one else)" grep -q '^tagger the owner <owner@example.org>' <<<"$tag_text"
check "release: main is a merge commit: the released commit, then the record" \
    test "$(git -C "$T/origin.git" log -1 --format='%P' main)" = "$released $(git -C "$T/origin.git" rev-parse 'main^2')"
check "release: ... the record PR's merge, as GitHub's" has "$(git -C "$T/origin.git" log -1 --format=%B main)" "^Merge pull request #[0-9]* from srp/release/record-rev12"
record=$(git -C "$T/origin.git" log -1 --format=%B 'main^2')
check "release: ... the record's commit: \"Release revision 12\", by the owner" bash -c \
    "test \"\$(git -C $T/origin.git log -1 --format='%s|%an' 'main^2')\" = 'Release revision 12|the owner'"
check "release: ... it says what shipped, no AI line" bash -c "grep -qx -- '- #7 A feature | with a bar' <<<\"\$1\" && ! grep -qi 'co-authored\|claude' <<<\"\$1\"" _ "$record"
check "release: ... it changes the record's files and nothing else" test \
    "$(git -C "$T/origin.git" diff --name-only "$released" 'main^2' | tr '\n' ' ')" = "CHANGELOG-SKHID-026G.md release/patches.tsv srp-026g/client-patches/README.md srp-026g/client-patches/patch_026g.py "
check "release: ... REVISION = 12 on main" test "$(main_revision)" = 12
row=$(on_main srp-026g/client-patches/README.md | grep '^| Revision 12 |' || true)
check "release: ... the revision table's row, after 11's" bash -c "on_main() { git -C $T/origin.git show main:\$1; }; on_main srp-026g/client-patches/README.md | grep -A1 '^| Revision 11 |' | tail -n 1 | grep -q '^| Revision 12 |'"
check "release: ... the row: the UTC time, the tag, the commit, what shipped" has "$row" \
    "^| Revision 12 | Released 20[0-9-]* [0-9:]* UTC (tag \`026g-rev12\`, \`main\` ${released:0:7}). Shipped: #7 A feature \\\\| with a bar. "
check "release: ... the row: the exe with and without the server, the ZIP, the server data" has "$row" \
    "Without a server sha256 \`$exe_without\`, with \`--server=127.0.0.1:1234\` \`$exe_with\`. The ZIP: [0-9]* bytes, sha256 \`$zip_sha\`; the server data: sha256 \`$data_sha\`. No content patch of its own |$"
changelog=$(on_main CHANGELOG-SKHID-026G.md)
check "release: ... the changelog's entry to word, first under the rule" bash -c \
    "sed -n '/^---\$/,\$p' <<<\"\$1\" | grep -m1 '^## ' | grep -qE '^## [0-9]+ [A-Z][a-z]{2}, [0-9]{2}:[0-9]{2} — TODO: the owner.s words \\(update required\\)\$' && grep -qx -- '- TODO: #7 A feature | with a bar' <<<\"\$1\" && grep -q '^## 15 Oct, 20:02 — an older entry' <<<\"\$1\"" _ "$changelog"
check "release: ... release/patches.tsv as the server offers it" cmp -s <(on_main release/patches.tsv) "$T/patches.tsv"
check "release: ... the record's branch pushed, its PR opened and merged" bash -c \
    "grep -q '^gh pr create --base main --head release/record-rev12 --title Release revision 12 ' $T/gh.log && grep -q '^gh pr merge https://github.com/srp/skhid-srp/pull/[0-9]* --merge$' $T/gh.log && test -n \"\$(git -C $T/origin.git rev-parse -q --verify refs/heads/release/record-rev12)\""
check "release: ... no worktree of the record left" bash -c "! git -C $T/repo worktree list | grep -q /record"
check "release: ... says it all, and ends 0" bash -c "grep -q 'done     the PR merged (a merge commit): main has REVISION 12' <<<\"\$1\" && ! grep -q 'NOT COMPLETE' <<<\"\$1\"" _ "$out"
check "release: no change of what a match computes: said, the replays' list as it was" bash -c \
    "grep -q 'replays: no change of what a match computes is marked (.*): recordings from revision 11 on stay watchable' <<<\"\$1\" && git -C $T/origin.git show main:srp-026g/crates/session/src/v026g/simulation_changes.tsv | grep -qx \$'11\tthe TDM end'" _ "$out"

# New content and new server code.
echo "crates 2" >"$src/crates/a/src/lib.rs"
printf 'services:\n  srp26:\n    environment:\n      - SRP_DAILY_BONUS\n' >"$src/docker-compose.yml"
commit "server code"
merge_pr 8 "Another feature"
# A PR changed what a match computes: it marks it `next`.
printf 'next\ta new kit before the start (#8)\n' >>"$sim"
commit "a change of what a match computes"
expect_refused "release: a change of what a match computes needs the server built" \
    "simulation_changes.tsv has a \`next\` row, but the server code is not built (--server-code no)" "$here/release.sh" --server-code no
mkdir -p "$T/p13/gameplay/items" "$T/r13"
echo "item 13" >"$T/p13/gameplay/items/d.options"
"$bin" pack --dir "$T/p13" --out "$T/r13/resources.db.patch_13" >/dev/null
# The site's pages on main, built from the list (patch_3, patch_12): the
# release of new content rebuilds them (build-site's stand-in).
for dir in maps damage; do
    mkdir -p "$src/servers/web-server/$dir"
    grep -v '^#' "$T/patches.tsv" | cut -f1,2 >"$src/servers/web-server/$dir/built-from.tsv"
    echo "built by hand" >"$src/servers/web-server/$dir/page.js"
done
commit "the site's pages, built"
pages_commit=$(git -C "$T/repo" rev-parse HEAD)
: >"$T/state/admin.log"
running_before=$(cat "$T/docker/running")
# --resources relative (of the folder it is run in): build-site, in a
# worktree, has it absolute.
cd "$T"
run yes --resources r13/resources.db.patch_13 --minutes 2 --label b || fail "release.sh --resources: $out"
cd - >/dev/null
check "release --resources (a relative path): rebuilds the site's pages with the new patch, in a worktree of the commit" bash -c \
    "test \"\$(grep -c . $T/build-site.args)\" = 1 && grep -q -- '^--tree $T/tmp/skhid-release\.[^ ]*/site --with $T/r13/resources.db.patch_13\$' $T/build-site.args"
check "release --resources: ... in the preflight, before the maintenance" bash -c \
    "r=\$(grep -n 'the Maps tab and the calculator: rebuilt from the patches this release offers' <<<\"\$1\" | head -n 1 | cut -d: -f1); p=\$(grep -n '3. planning the maintenance' <<<\"\$1\" | head -n 1 | cut -d: -f1); test -n \"\$r\" -a -n \"\$p\" && ((r < p))" _ "$out"
check "release --resources: ... the server's source carries them" bash -c \
    "grep -qx \$'resources.db.patch_13\t'\$(sha256sum <$T/r13/resources.db.patch_13 | cut -c1-64) $T/live/servers/web-server/maps/built-from.tsv && grep -qx 'built from resources.db.patch_3 resources.db.patch_12 resources.db.patch_13 ' $T/live/servers/web-server/damage/page.js"
check "release --resources: ... the record commits them: main's pages built from main's list" bash -c \
    "for d in maps damage; do cmp -s <(git -C $T/origin.git show main:srp-026g/servers/web-server/\$d/built-from.tsv | grep -v '^#') <(git -C $T/origin.git show main:release/patches.tsv | grep -v '^#' | cut -f1,2) || exit 1; done"
check "release --resources: ... the record's commit: the pages, the list and its own files, and says so" bash -c \
    "test \"\$(git -C $T/origin.git diff --name-only $pages_commit 'main^2' | tr '\n' ' ')\" = 'CHANGELOG-SKHID-026G.md release/patches.tsv srp-026g/client-patches/README.md srp-026g/client-patches/patch_026g.py srp-026g/crates/session/src/v026g/simulation_changes.tsv srp-026g/servers/web-server/damage/built-from.tsv srp-026g/servers/web-server/damage/page.js srp-026g/servers/web-server/maps/built-from.tsv srp-026g/servers/web-server/maps/page.js ' && git -C $T/origin.git log -1 --format=%B 'main^2' | grep -q '^The Maps tab and the calculator (srp-026g/servers/web-server/maps/ and'"
check "release --resources: ... no worktree of the rebuild left" bash -c "! git -C $T/repo worktree list | grep -q '/site\$'"
check "release --resources: publishes the patch" cmp -s "$T/r13/resources.db.patch_13" "$T/patches/resources.db.patch_13"
check "release --resources: adds its row to the list" test "$(rows "$T/patches.tsv")" = "resources.db.patch_3 resources.db.patch_12 resources.db.patch_13"
check "release --resources: the server data offers it" offers patch_13
check "release --resources: /patches offers it" grep -q patch_13 "$T/site/patches"
check "release --resources: its record: the tag 026g-rev13, REVISION 13 on main" bash -c "test -n \"\$(git -C $T/origin.git rev-parse -q --verify refs/tags/026g-rev13)\" && test \"\$1\" = 13" _ "$(main_revision)"
check "release --resources: ... main's release/patches.tsv has the new row" cmp -s <(on_main release/patches.tsv) "$T/patches.tsv"
row=$(on_main srp-026g/client-patches/README.md | grep '^| Revision 13 |' || true)
check "release --resources: ... the row names the content patch" has "$row" \
    "Its content: \`resources.db.patch_13\` ($(stat -c %s "$T/r13/resources.db.patch_13") bytes, sha256 \`$(sha256sum <"$T/r13/resources.db.patch_13" | cut -d' ' -f1)\`) |$"
check "release --resources: ... it shipped what was merged since the tag of 12 (not 12's own, nor 12's record)" \
    bash -c "grep -q 'Shipped: #8 Another feature\. ' <<<\"\$1\" && ! grep -q '#7\|record' <<<\"\$1\"" _ "$row"
check "release --resources: ... the rows in revision order" test "$(on_main srp-026g/client-patches/README.md | sed -n 's/^| Revision \([0-9]*\) .*/\1/p' | tr '\n' ' ')" = "10 11 12 13 4 "
check "release: finds the changed server code" has "$out" "CHANGED   crates/a/src/lib.rs"
check "release: tags the running image by its ID" grep -q "docker tag $running_before srp-026g:before-rev13-[0-9-]*-b" "$T/log"
check "release: builds main's source, staged in the folder" grep -q "docker compose --env-file $T/live/.env -f release-rev13-[0-9-]*-b/src/docker-compose.yml build srp26" "$T/log"
check "release: runs the new image" test "$(cat "$T/docker/running")" != "$running_before"
check "release: the source is in place after" grep -qx "crates 2" "$T/live/crates/a/src/lib.rs"
check "release: the old source is kept in the folder" bash -c "tar -xzOf $(folder 13 b)/old-src.tgz crates/a/src/lib.rs | grep -qx 'crates/a/src/lib.rs 1'"
check "release: the rollback puts the image back" grep -qx "OLD_IMAGE=$running_before" "$(folder 13 b)/rollback.env"
check "release: the rollback has srp.db of before the release" test -s "$(folder 13 b)/$(cat "$(folder 13 b)/srp-db.name")"
check "release: leaves nothing staged (the source either)" nothing_staged
check "release: SRP_MIN_PATCH_REVISION=13" grep -qx "SRP_MIN_PATCH_REVISION=13" "$T/live/.env"
check "release: says it changes what a match computes" has "$out" \
    "replays: this release changes what a match computes (a new kit before the start (#8)): recordings of games before revision 13 are refused from it on"
check "release: ... the server built with \`next\` (it takes SRP_MIN_PATCH_REVISION, 13)" bash -c \
    "git -C $T/origin.git show 026g-rev13:srp-026g/crates/session/src/v026g/simulation_changes.tsv | grep -q \$'^next\t' && grep -q \$'^next\t' $T/live/crates/session/src/v026g/simulation_changes.tsv"
check "release: ... the record writes 13 for it" test "$(on_main srp-026g/crates/session/src/v026g/simulation_changes.tsv | grep -v '^#')" = \
    "$(printf '11\tthe TDM end\n13\ta new kit before the start (#8)')"
check "release: ... and says so in its commit" has "$(git -C "$T/origin.git" log -1 --format=%B 'main^2')" \
    "^It changes what a match computes (a new kit before the start (#8)): 13 in place of"
expect_refused "release: the live server built with \`next\`, not built again, would take the new revision for it" \
    "the live server was built with a \`next\` row of srp-026g/crates/session/src/v026g/simulation_changes.tsv, and the server code is not built (--server-code no)" \
    "$here/release.sh" --server-code no

# Refused in the preflight, before anything.
: >"$T/state/admin.log"
expect_refused "release: a patch already listed" "is in $T/patches.tsv already: run without --resources" \
    "$here/release.sh" --resources "$T/r13/resources.db.patch_13"
expect_refused "release: --revision not above the live exe's" "--revision 13 is not above the live exe's 13" "$here/release.sh" --revision 13
expect_refused "release: --revision below it too" "--revision 12 is not above the live exe's 13" "$here/release.sh" --revision 12
expect_refused "release: --revision is a revision" "--revision 256: a revision is" "$here/release.sh" --revision 256
touch "$T/gh-no-auth"
expect_refused "release: needs gh signed in (the record's PR)" "gh is not signed in" "$here/release.sh"
rm "$T/gh-no-auth"
git -C "$T/repo" config user.useConfigOnly true
git -C "$T/repo" config --unset user.email
expect_refused "release: needs a git user (the record's tag and commit)" "has no user.name and user.email" \
    env GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null "$here/release.sh"
git -C "$T/repo" config user.email owner@example.org
git -C "$T/repo" config --unset user.useConfigOnly
git -C "$T/repo" tag -a 026g-rev14 -m "left by a run" HEAD
expect_refused "release: a tag of its revision left here" "a tag 026g-rev14 is in $T/repo, not on origin" "$here/release.sh"
git -C "$T/repo" tag -d 026g-rev14 >/dev/null
echo "crates 3" >"$src/crates/a/src/lib.rs"
commit "server code again"
mkdir -p "$T/p14/gameplay/items" "$T/r14"
echo "item 14" >"$T/p14/gameplay/items/e.options"
"$bin" pack --dir "$T/p14" --out "$T/r14/resources.db.patch_14" >/dev/null
mkdir -p "$T/r16" && cp "$T/r14/resources.db.patch_14" "$T/r16/resources.db.patch_16"
expect_refused "release: a patch of another revision" "must be called resources.db.patch_14" \
    "$here/release.sh" --resources "$T/r16/resources.db.patch_16"
cp "$T/r14/resources.db.patch_14" "$T/patches/"
expect_refused "release: a patch published but not listed" "already published, these same bytes, but not in" \
    "$here/release.sh" --resources "$T/r14/resources.db.patch_14"
rm "$T/patches/resources.db.patch_14"
expect_refused "release: --yes needs --i-know" "goes with --i-know" "$here/release.sh" --yes
touch "$T/ssh-fails-live"
expect_refused "release: says the live host cannot be reached" "ssh to the live host (live through jump) fails" "$here/release.sh"
rm "$T/ssh-fails-live"
python3 -c 'import time; print(time.time() + 600)' >"$T/state/maint"
expect_refused "release: refuses when a maintenance is planned already" "a maintenance is planned already" "$here/release.sh"
rm "$T/state/maint"
mkdir "$T/lock" && echo "someone@else, since then" >"$T/lock/owner"
expect_refused "release: refuses while another release runs (the lock)" "holds the lock on the jump host: someone@else" "$here/release.sh"
check "release: ... and leaves the other's lock" test -f "$T/lock/owner"
rm -r "$T/lock"
cp "$T/docker/latest" "$T/latest.kept"
echo "sha256:$(echo hand | sha256sum | cut -c1-64)" >"$T/docker/latest"
expect_refused "release: refuses when srp-026g:latest is not the running image" "a restart would run an image no release made" "$here/release.sh"
cp "$T/latest.kept" "$T/docker/latest"
grep -v patch_12 "$T/patches.tsv" >"$T/no12.tsv"
expect_refused "release: revision 14 needs patch_12 (the menus) listed" "needs resources.db.patch_12 (the menus)" \
    env SKHID_RELEASE_ENV="$(changed_env "s|^SKHID_PATCH_LIST=.*|SKHID_PATCH_LIST=$T/no12.tsv|")" "$here/release.sh"
sed "s/^\(resources.db.patch_3\t\)[0-9a-f]*/\1$sha12/" "$T/patches.tsv" >"$T/wrong.tsv"
expect_refused "release: a listed patch that is not so on the patches server" "patch_3 .* is not so on the patches server" \
    env SKHID_RELEASE_ENV="$(changed_env "s|^SKHID_PATCH_LIST=.*|SKHID_PATCH_LIST=$T/wrong.tsv|")" "$here/release.sh"
cat >>"$T/release.env" <<ENV
SKHID_TEST=test-host
SKHID_TEST_DIR=$T/live
SKHID_TEST_SITE=http://127.0.0.1:$site_port
SKHID_TEST_GAME_SERVER=127.0.0.1:2234
ENV
expect_refused "release --test refuses the production patches" "is the production patches URL" \
    env SKHID_RELEASE_ENV="$(changed_env "s|^SKHID_TEST_PATCHES_URL=.*|SKHID_TEST_PATCHES_URL=$P|")" "$here/release.sh" --test --resources "$T/r14/resources.db.patch_14"
expect_refused "release --test needs its own patches server" "SKHID_TEST_PATCHES_DIR is not set" \
    env SKHID_RELEASE_ENV="$(changed_env "/^SKHID_TEST_PATCHES_DIR=/d")" "$here/release.sh" --test
check "release: the refusals planned nothing" test ! -s "$T/state/admin.log"
check "release: ... and published nothing" test ! -e "$T/patches/resources.db.patch_14" -a ! -e "$T/patches-test/resources.db.patch_14"

# The site's pages: built on main from the list since the record of 13. A
# release without new content does not rebuild them; one with it does, in
# its preflight, and a rebuild that fails, or names other patches, stops
# the run there, before anything players see.
: >"$T/build-site.args"
run quiet --dry-run --minutes 2 || fail "release.sh --dry-run, the pages as offered: $out"
check "release: the site's pages built from the list: not rebuilt" bash -c \
    "grep -q 'the Maps tab: built from the patches this release offers' <<<\"\$1\" && grep -q 'the calculator: built from the patches this release offers' <<<\"\$1\" && test ! -s $T/build-site.args" _ "$out"
: >"$T/state/admin.log"
touch "$T/build-site-fails"
expect_refused "release --resources: a failed rebuild of the site's pages stops the preflight" \
    "the Maps tab and the calculator: the rebuild failed (release/build-site; its log: .*): build.py: the Maps tab's build failed" \
    "$here/release.sh" --resources "$T/r14/resources.db.patch_14"
rm "$T/build-site-fails"
check "release: ... before anything players see: no maintenance, nothing published, no lock" bash -c \
    "test ! -s $T/state/admin.log && test ! -e $T/patches/resources.db.patch_14 && ! grep -q patch_14 $T/patches.tsv && test ! -e $T/lock"
check "release: ... no worktree of the rebuild left" bash -c "! git -C $T/repo worktree list | grep -q '/site\$'"
touch "$T/build-site-wrong"
expect_refused "release --resources: rebuilt pages that name other patches stop it too" \
    "the Maps tab, rebuilt: its built-from.tsv does not name the patches this release offers" \
    "$here/release.sh" --resources "$T/r14/resources.db.patch_14"
rm "$T/build-site-wrong"
: >"$T/build-site.args"
expect_refused "release --resources: pages to rebuild need the server code built: --server-code no refused" \
    "the Maps tab and the calculator are rebuilt by this release, and --server-code no builds no server code" \
    "$here/release.sh" --resources "$T/r14/resources.db.patch_14" --server-code no
check "release: ... without the 2-min rebuild" test ! -s "$T/build-site.args"
out=$(setsid -w env SKHID_RELEASE_ENV="$(changed_env 's/^SKHID_LIVE_MIN_FREE_MB=.*/SKHID_LIVE_MIN_FREE_MB=999999999/')" \
    "$here/release.sh" --resources "$T/r14/resources.db.patch_14" </dev/null 2>&1) && fail "release.sh, too little disk: ran ($out)"
check "release --resources: another preflight problem: the pages are not rebuilt (it stops anyway)" bash -c \
    "grep -q 'MISSING  free on the live host: .* (SKHID_LIVE_MIN_FREE_MB)' <<<\"\$1\" && grep -q 'the Maps tab and the calculator: not rebuilt, as the preflight has 1 problem(s) already' <<<\"\$1\" && test ! -s $T/build-site.args" _ "$out"
check "release: ... none of them planned or published anything" bash -c \
    "test ! -s $T/state/admin.log && test ! -e $T/patches/resources.db.patch_14 && test ! -e $T/lock"

# The dry run.
before=$(tree)
: >"$T/build-site.args"
published_before=$(ls "$T/patches")
run quiet --dry-run --resources "$T/r14/resources.db.patch_14" --minutes 2 || fail "release.sh --dry-run: $out"
check "release --dry-run: no file changes on the live host" test "$(tree)" = "$before"
check "release --dry-run: no patch is published" test "$(ls "$T/patches")" = "$published_before"
check "release --dry-run: no row is added" test "$(rows "$T/patches.tsv")" = "resources.db.patch_3 resources.db.patch_12 resources.db.patch_13"
check "release --dry-run: plans no maintenance" test ! -s "$T/state/admin.log"
check "release --dry-run: runs no docker but reads" bash -c "! grep -qE '^docker (compose .* (up|stop|start|build) |tag |run |rmi )' $T/log"
check "release --dry-run: takes no lock" test ! -e "$T/lock"
check "release --dry-run: shows the POST, not its secrets" has "$out" "POST http://127.0.0.1:$site_port/<SRP_ADMIN_PATH> token=<SRP_ADMIN_TOKEN> tab=maint minutes=2"
check "release --dry-run: shows what /patches will offer" has "$out" "  patch_14 [0-9a-f]* $P/resources.db.patch_14"
check "release --dry-run: shows the swap and the restart" has "$out" "docker compose up -d --no-deps --force-recreate srp26"
check "release --dry-run: shows the build" has "$out" "build srp26"
check "release --dry-run: says so" has "$out" "dry run: nothing was planned, uploaded or changed"
check "release --dry-run: prints the rebuild of the site's pages with the new patch" has "$out" \
    "^  \[dry run\] would rebuild the Maps tab and the calculator (about 2 min; .*) in a worktree of [0-9a-f]*: $T/build-site --tree $T/tmp/skhid-release\.[^ ]*/site --with $T/r14/resources.db.patch_14\$"
check "release --dry-run: ... runs none" test ! -s "$T/build-site.args"
check "release --dry-run: ... and the record it would make commits them" has "$out" "release/patches.tsv as offered, the rebuilt Maps tab and calculator"
check "release --dry-run: ... its server-code report has them changed, as the real run's" bash -c \
    "grep -q '^             CHANGED   servers/web-server/maps/ (a real run rebuilds it)\$' <<<\"\$1\" && grep -q '^             CHANGED   servers/web-server/damage/ (a real run rebuilds it)\$' <<<\"\$1\" && grep -q '^  server code: built before the swap' <<<\"\$1\"" _ "$out"
run quiet --dry-run --resources "$T/r14/resources.db.patch_14" --server-code no --minutes 2 && fail "release.sh --dry-run --server-code no: no problem ($out)"
check "release --dry-run --server-code no: refused as a real run is, the pages to rebuild" bash -c \
    "grep -q 'MISSING  the Maps tab and the calculator are rebuilt by this release, and --server-code no builds no server code' <<<\"\$1\" && grep -q 'a real run would stop before the lock' <<<\"\$1\" && test ! -s $T/build-site.args" _ "$out"
git_t -C "$T/repo" rm -q -r "$src/servers/web-server"
commit "no site pages again (the rest of the runs)"

# build-site itself, with stand-ins for nix (a python, java, the dbgrab
# build) and for the two builds (they write built-from.tsv in revision
# order, not the list's + --with's), run from a checkout of its own (the
# main checkout it builds dbgrab in) on a tree of another commit.
mkdir -p "$T/sb/bin" "$T/sb/py/bin" "$T/sb/root/release" "$T/sb/root/tools/vostok-resources-db" "$T/sb/cache/skhid-release/ffdec-26.0.0" "$T/r5"
ln -s "$(command -v python3)" "$T/sb/py/bin/python3"
cp "$here/build-site" "$here/lib.sh" "$T/sb/root/release/"
touch "$T/sb/root/tools/vostok-resources-db/Cargo.toml" "$T/sb/cache/skhid-release/ffdec-26.0.0/ffdec-cli.jar"
cat >"$T/sb/bin/nix" <<'STANDIN'
#!/usr/bin/env bash
# nix build ...: the python; nix shell <pkg> -c <command>: the command;
# nix develop <flake> -c cargo build ... --manifest-path <m> --target-dir <t>:
# a dbgrab in <t>/release that prints the main.rs it was built from (the
# files of its source folder in $TEST_DIR/sb/dbgrab-src).
case $1 in
build) echo "$TEST_DIR/sb/py" ;;
shell) while [[ $1 != -c ]]; do shift; done; shift; exec "$@" ;;
develop)
    manifest='' target=''
    while (($#)); do
        case $1 in
        --manifest-path) manifest=$2; shift 2 ;;
        --target-dir) target=$2; shift 2 ;;
        *) shift ;;
        esac
    done
    dir=$(dirname "$manifest")
    (cd "$dir" && find . -type f | sort | tr '\n' ' ') >"$TEST_DIR/sb/dbgrab-src"
    mkdir -p "${target:-$dir/target}/release"
    printf '#!/bin/sh\necho %q\n' "$(cat "$dir/src/main.rs")" >"${target:-$dir/target}/release/dbgrab"
    chmod +x "${target:-$dir/target}/release/dbgrab" ;;
*) echo "nix $*: not in the stand-in" >&2; exit 1 ;;
esac
STANDIN
chmod +x "$T/sb/bin/nix"
sb_repo() { # <dir> <main.rs>: a checkout whose commit has dbgrab's sources and the two builds
    mkdir -p "$1/tools/re-kit/common/dbgrab/src" "$1/srp-026g/scripts/maps-tab" "$1/srp-026g/scripts/damage-calc"
    echo '[package]' >"$1/tools/re-kit/common/dbgrab/Cargo.toml"
    echo "$2" >"$1/tools/re-kit/common/dbgrab/src/main.rs"
    cat >"$1/srp-026g/scripts/maps-tab/build.py" <<'PY'
import hashlib, os, pathlib, subprocess, sys
opts = dict(zip(sys.argv[1::2], sys.argv[2::2]))
for key in ("--stock", "--patches", "--list", "--dbgrab", "--ffdec", "--with"):
    if key in opts and not pathlib.Path(opts[key]).exists():
        sys.exit(f"{key} {opts[key]}: no such file here ({os.getcwd()})")
rows = [line.split("\t")[:2] for line in pathlib.Path(opts["--list"]).read_text().splitlines() if line.strip() and not line.startswith("#")]
if "--with" in opts and not os.path.exists(os.environ["TEST_DIR"] + "/sb/drop-with"):
    rows.append([pathlib.Path(opts["--with"]).name, hashlib.sha256(pathlib.Path(opts["--with"]).read_bytes()).hexdigest()])
rows.sort(key=lambda row: int(row[0].rsplit("_", 1)[1]))
out = pathlib.Path("srp-026g/servers/web-server", "maps" if sys.argv[0].endswith("/build.py") else "damage")
out.mkdir(parents=True, exist_ok=True)
(out / "built-from.tsv").write_text("# The patches it was built from: name, sha256.\n" + "".join(f"{n}\t{s}\n" for n, s in rows))
(out / "dbgrab.txt").write_text(subprocess.run([opts["--dbgrab"]], capture_output=True, text=True, check=True).stdout)
PY
    cp "$1/srp-026g/scripts/maps-tab/build.py" "$1/srp-026g/scripts/damage-calc/build_site.py"
    git_t -C "$1" init -q && git_t -C "$1" add -A && git_t -C "$1" commit -q -m "$2"
}
sb_repo "$T/sb/root" "the operator's dbgrab"
sb_repo "$T/sb/tree" "the tree's dbgrab"
# A source folder and a target of an earlier build in the cache.
mkdir -p "$T/sb/cache/skhid-release/dbgrab/tools/re-kit/common/dbgrab/src" "$T/sb/cache/skhid-release/dbgrab-target"
echo stale >"$T/sb/cache/skhid-release/dbgrab/tools/re-kit/common/dbgrab/src/old.rs"
echo kept >"$T/sb/cache/skhid-release/dbgrab-target/kept"
# Relative paths, of the folder it is run in ($T): the stock db, the list,
# --with (patch_5, below the list's top).
sb_env=$(changed_env 's|^SKHID_STOCK_DB=.*|SKHID_STOCK_DB=resources.db|; s|^SKHID_PATCH_LIST=.*|SKHID_PATCH_LIST=patches.tsv|')
echo "patch 5" >"$T/r5/resources.db.patch_5"
sb_run() { # <args...>: build-site in $T, its output into $out
    out=$(cd "$T" && setsid -w env -u SKHID_DBGRAB PATH="$T/sb/bin:$PATH" XDG_CACHE_HOME="$T/sb/cache" SKHID_RELEASE_ENV="$sb_env" \
        "$T/sb/root/release/build-site" --tree sb/tree "$@" </dev/null 2>&1)
}
sb_run --with r5/resources.db.patch_5 || fail "build-site: $out"
sb_rows=$( { grep -v '^#' "$T/patches.tsv" | cut -f1,2; printf 'resources.db.patch_5\t%s\n' "$(sha256sum <"$T/r5/resources.db.patch_5" | cut -c1-64)"; } | sort -V)
check "build-site: relative --with, SKHID_STOCK_DB and SKHID_PATCH_LIST: the builds (in the tree) find them" bash -c \
    "for d in maps damage; do test \"\$(grep -v '^#' $T/sb/tree/srp-026g/servers/web-server/\$d/built-from.tsv | sort -V)\" = \"\$1\" || exit 1; done" _ "$sb_rows"
check "build-site: a --with patch below the list's top, the pages naming it in another order: passes" has "$out" \
    "^the Maps tab and the calculator: built in $T/sb/tree/srp-026g/servers/web-server/ from the stock resources.db and $(grep -c . <<<"$sb_rows") patch(es):"
check "build-site: dbgrab built from the tree's commit, not the checkout running it" grep -qx "the tree's dbgrab" "$T/sb/tree/srp-026g/servers/web-server/maps/dbgrab.txt"
check "build-site: ... from a source folder made anew (an earlier one's file gone)" test "$(cat "$T/sb/dbgrab-src")" = "./Cargo.toml ./src/main.rs "
check "build-site: ... its target kept" test -f "$T/sb/cache/skhid-release/dbgrab-target/kept"
touch "$T/sb/drop-with"
sb_run --with r5/resources.db.patch_5 && fail "build-site, the pages without --with's patch: passed ($out)"
check "build-site: pages that leave --with's patch out: fails" has "$out" \
    "built-from.tsv does not name exactly the patches of $T/patches.tsv and resources.db.patch_5"
rm "$T/sb/drop-with"

# The preflight checks every URL of every row, mirrors too: once whole,
# then (the cache, across runs) by their first bytes.
rm -f "$T/cache/skhid-release/patch-urls.checked"
: >"$T/mirror.log"
export SKHID_RELEASE_ENV
SKHID_RELEASE_ENV=$(changed_env "s|^SKHID_PATCH_LIST=.*|SKHID_PATCH_LIST=$T/mirrored.tsv|")
run quiet --dry-run --minutes 2 || fail "release.sh --dry-run, mirrors: $out"
check "release: checks every URL of the list" has "$out" \
    "the offered-patch list: 2 patch(es), each on the patches server (patches) with its sha256, and its 4 URL(s) giving it"
check "release: ... each mirror downloaded whole the first time" \
    bash -c "grep -qx '/files/resources.db.patch_3 -' $T/mirror.log && grep -qx '/hop/5/resources.db.patch_3 -' $T/mirror.log"
check "release: ... and kept in the cache" test "$(grep -c " $M/" "$T/cache/skhid-release/patch-urls.checked")" = 2
check "release: shows the mirrors /patches will offer" has "$out" \
    "  patch_3 $sha3 $P/resources.db.patch_3 $M/hop/5/resources.db.patch_3 $M/files/resources.db.patch_3"
: >"$T/mirror.log"
run quiet --dry-run --minutes 2 || fail "release.sh --dry-run, mirrors again: $out"
check "release: the next run asks the mirrors for their first bytes only" \
    bash -c "test -s $T/mirror.log && ! grep -v ' bytes=0-12\$' $T/mirror.log"
mv "$T/mirror/resources.db.patch_3" "$T/mirror/kept_3"
run quiet --dry-run --minutes 2 || true
check "release: a mirror that no longer gives the patch is a problem" has "$out" \
    "  - $T/mirrored.tsv: resources.db.patch_3: $M/files/resources.db.patch_3 answers 404 now (it gave the file before)"
# Replaced since by a file of another size that starts as a patch does:
# seen by the size the answer gives, a 200's Content-Length or a 206's
# Content-Range.
head -c $((size3 - 1)) "$T/mirror/kept_3" >"$T/mirror/resources.db.patch_3"
run quiet --dry-run --minutes 2 || true
check "release: a mirror whose file is now of another size is a problem (200)" has "$out" \
    "  - $T/mirrored.tsv: resources.db.patch_3: $M/files/resources.db.patch_3 says the file is $((size3 - 1)) bytes now, not $size3 (it gave the file before)"
touch "$T/mirror.ranges"
run quiet --dry-run --minutes 2 || true
check "release: a mirror whose file is now of another size is a problem (206)" has "$out" \
    "  - $T/mirrored.tsv: resources.db.patch_3: $M/files/resources.db.patch_3 says the file is $((size3 - 1)) bytes now, not $size3 (it gave the file before)"
mv "$T/mirror/kept_3" "$T/mirror/resources.db.patch_3"
: >"$T/mirror.log"
run quiet --dry-run --minutes 2 || fail "release.sh --dry-run, mirrors with ranges: $out"
check "release: a mirror that answers its first bytes with a 206 of the row's size is good" has "$out" \
    "the offered-patch list: 2 patch(es), each on the patches server (patches) with its sha256, and its 4 URL(s) giving it"
check "release: ... asked for its first bytes only" \
    bash -c "test -s $T/mirror.log && ! grep -v ' bytes=0-12\$' $T/mirror.log"
rm -f "$T/mirror.ranges"
printf 'resources.db.patch_3\t%s\t%s\t%s %s\nresources.db.patch_12\t%s\t%s\t%s\n' "$sha3" "$size3" "$P/resources.db.patch_3" \
    "$M/page/resources.db.patch_3" "$sha12" "$(stat -c %s "$T/r12/resources.db.patch_12")" "$P/resources.db.patch_12" >"$T/page.tsv"
SKHID_RELEASE_ENV=$(changed_env "s|^SKHID_PATCH_LIST=.*|SKHID_PATCH_LIST=$T/page.tsv|")
run quiet --dry-run --minutes 2 || true
check "release: a mirror that is a web page is a problem" has "$out" \
    "resources.db.patch_3: $M/page/resources.db.patch_3 answers with a web page"
SKHID_RELEASE_ENV=$T/release.env

# A failure before the swap: the build of the image.
touch "$T/build-fails"
run yes --minutes 2 --label c || true
rm "$T/build-fails"
check "release: a failed build stops before the swap" has "$out" "STOPPED before the swap"
check "release: ... cancels the maintenance, planned before the build" test "$(admin_log)" = "plan 2 cancel" -a ! -e "$T/state/maint"
check "release: ... removes the staged files" nothing_staged
check "release: ... puts the image tag back" grep -q "docker tag $(cat "$T/docker/running") srp-026g:latest" "$T/log"
check "release: ... srp-026g:latest is the running image" test "$(cat "$T/docker/latest")" = "$(cat "$T/docker/running")"
check "release: ... leaves the live files as they were" test "$(tree)" = "$before"
check "release: ... restarts nothing" bash -c "! grep -q ' up ' $T/log"
check "release: ... and makes no record: no tag, no branch, main's REVISION as it was" no_record 14
check "release: ... no PR" bash -c "! grep -q record-rev14 $T/gh.log"

# A no at the question before the swap.
: >"$T/state/admin.log"
run no --minutes 1 --label g || true
check "release: a no at the swap cancels the maintenance" test "$(admin_log)" = "plan 1 cancel"
check "release: ... swaps nothing" has "$out" "nothing was swapped"
check "release: ... removes the staged files" nothing_staged
check "release: ... leaves the live files as they were" test "$(tree)" = "$before"

# No answer at the question before the swap.
: >"$T/state/admin.log"
SKHID_SWAP_ANSWER_MINUTES=2 run wait --minutes 1 --label h || true
check "release: no answer at the swap is a no" has "$out" "no answer in 2 min is a no"
check "release: ... swaps nothing" has "$out" "nothing was swapped"
check "release: ... cancels the maintenance" test "$(admin_log)" = "plan 1 cancel"
check "release: ... leaves the live files as they were" test "$(tree)" = "$before"

# srp makes no backup at its start (it never opened srp.db): the release does
# not go on without it, and the rollback keeps srp.db as it is.
: >"$T/state/admin.log"
touch "$T/no-backup"
lines_before=$(grep -c . "$T/volume/srp.db")
run yes --minutes 1 --label k || true
rm "$T/no-backup"
check "release: no new backup of srp.db, no release" has "$out" "srp made no backup of srp.db at its start"
check "release: ... rolled back" has "$out" "rolled back: srp26"
check "release: ... srp.db kept as it is" bash -c "grep -q 'srp.db: kept as it is (the new srp made no backup' <<<\"\$1\" && test \$(grep -c . $T/volume/srp.db) -eq $((lines_before + 2))" _ "$out"
check "release: ... srp26 running" test "$(cat "$T/docker/state")" = running
check "release: ... the ZIP never moved" cmp -s "$T/work/r13.zip" "$T/live/download/Survarium-0.26g-srp-files.zip"
check "release: ... nothing staged" nothing_staged
check "release: ... the setting as it was" grep -qx "SRP_MIN_PATCH_REVISION=13" "$T/live/.env"

# A match that does not end.
: >"$T/state/admin.log"
echo 1000 >"$T/state/busy"
before=$(tree)
SKHID_MATCH_WAIT_MINUTES=2 run yes --minutes 1 --label e || true
check "release: a match still running stops it" has "$out" "match(es) still running 2 min after the planned time"
check "release: ... cancels the maintenance" test "$(admin_log)" = "plan 1 cancel"
check "release: ... removes the staged files" nothing_staged
check "release: ... its folder says stopped" grep -qx stopped "$(folder 14 e)/state"
check "release: ... leaves the live files as they were" test "$(tree)" = "$before"
rm -f "$T/state/busy"

# srp refuses to start on the new data, before it opens srp.db: its log, then
# the rollback of it all (srp.db, untouched, kept).
: >"$T/state/admin.log"
echo "ERROR srp: SRP_MIN_PATCH_REVISION is 13, below resources.db.patch_14 of /srv/srp/download/server-game-data.db. Set SRP_MIN_PATCH_REVISION=14 or more" >"$T/refuse-start"
running_before=$(cat "$T/docker/running")
data_before=$(sha256sum <"$T/live/download/server-game-data.db")
starts=$(cat "$T/docker/starts")
run yes --resources "$T/r14/resources.db.patch_14" --minutes 1 --label f || true
rm -f "$T/refuse-start"
check "release: srp refusing to start fails the release" has "$out" "srp26 is not up: restarting"
check "release: ... shows why, from srp's log" has "$out" "srp refused to start:"
check "release: ... the line that says why" has "$out" "  > ERROR srp: SRP_MIN_PATCH_REVISION is 13"
check "release: ... rolls back" has "$out" "rolled back: srp26 .*/patches .* as before"
check "release: ... the ZIP never moved" cmp -s "$T/work/r13.zip" "$T/live/download/Survarium-0.26g-srp-files.zip"
check "release: ... the server data back" test "$(sha256sum <"$T/live/download/server-game-data.db")" = "$data_before"
check "release: ... the setting back" grep -qx "SRP_MIN_PATCH_REVISION=13" "$T/live/.env"
check "release: ... the image back, by its ID" test "$(cat "$T/docker/running")|$(cat "$T/docker/latest")" = "$running_before|$running_before"
check "release: ... the source back" grep -qx "crates 2" "$T/live/crates/a/src/lib.rs"
check "release: ... srp.db as it was (the refused start never opened it)" bash -c "grep -q '^start $((starts + 1)) $running_before' $T/volume/srp.db && test \$(grep -c '^start ' $T/volume/srp.db) -le $((starts + 1))"
check "release: ... nothing staged" nothing_staged
check "release: ... the folder says rolled-back" grep -qx rolled-back "$(folder 14 f)/state"
check "release: ... the patch stays published and listed" bash -c "test -f $T/patches/resources.db.patch_14 && grep -q '^resources.db.patch_14' $T/patches.tsv"
check "release: ... the lock freed" test ! -e "$T/lock"
check "release: ... and no record (a rolled-back release leaves none)" no_record 14

# srp refuses after it opened srp.db (and backed it up): the rollback puts
# that backup back.
: >"$T/state/admin.log"
echo "ERROR srp: cannot load the 0.26g accounts: the kept accounts name what the server's game data (x) lacks; item 99" >"$T/refuse-start"
starts=$(cat "$T/docker/starts")
run yes --minutes 1 --label m || true
rm -f "$T/refuse-start"
check "release: a refusal after srp.db was opened rolls back too" has "$out" "rolled back: srp26"
check "release: ... with srp's backup of that start" has "$out" "srp.db: as it was before the release (srp-[0-9]*.db)"
check "release: ... srp.db: the rollback's start, after the one before" bash -c "grep -q '^start $((starts + 1)) ' $T/volume/srp.db"

# A match that ends a little after the planned time (patch_14 listed now).
: >"$T/state/admin.log"
echo 3 >"$T/state/busy"
run yes --minutes 1 --label d || fail "release.sh with a match: $out"
rm -f "$T/state/busy"
check "release: waits while a match runs" has "$out" "1 match(es) still running; waiting"
check "release: ... then releases" cmp -s "$T/work/r14.zip" "$T/live/download/Survarium-0.26g-srp-files.zip"
check "release: ... with the new patch" offers patch_14
check "release: ... without cancelling" test "$(admin_log)" = "plan 1"

# release/rollback: the folders, then one rolled back keeping srp.db.
out=$("$here/rollback" 2>&1) || fail "rollback (the list): $out"
check "rollback lists the release folders with their state" has "$out" "release-rev14-[0-9-]*-d  released"
D_FOLDER=$(basename "$(folder 14 d)")
starts=$(cat "$T/docker/starts")
out=$(yes_to "$here/rollback" --keep-db "$D_FOLDER") || fail "rollback --keep-db: $out"
check "rollback --keep-db: the ZIP back" cmp -s "$T/work/r13.zip" "$T/live/download/Survarium-0.26g-srp-files.zip"
check "rollback --keep-db: srp.db as it was" grep -q "^start $starts " "$T/volume/srp.db"
check "rollback --keep-db: the setting back" grep -qx "SRP_MIN_PATCH_REVISION=13" "$T/live/.env"
check "rollback --keep-db: the folder says rolled-back" grep -qx rolled-back "$(folder 14 d)/state"
# Revision 14 went out and came back: the next release is not 14 again.
expect_refused "release after a rollback: main's REVISION says 14 went out" "main's REVISION 14 says revision 14 went out already, and the live exe says 13 .*--revision 15" "$here/release.sh"
expect_refused "release after a rollback: ... and its record is on origin" "the record of revision 14 is on origin already: refs/heads/release/record-rev14 refs/tags/026g-rev14" "$here/release.sh"
run quiet --dry-run --revision 15 --minutes 1 || fail "release.sh --revision 15 --dry-run: $out"
check "release --revision 15 after the rollback: accepted (the dry run)" has "$out" "the release: revision 13 -> 15 to the live server"
check "release --dry-run: prints the record it would make" has "$out" '^\[dry run\] 7. once it went out, the record: git tag -a 026g-rev15'
check "release --dry-run: ... no change of what a match computes, and no exe patch changed: no note" bash -c \
    "! grep -q 'note     the exe.s patches changed' <<<\"\$1\"" _ "$out"
# An exe patch changed since the last release's tag, and nothing marked:
# a note; marked, the record it would make writes the revision for it.
echo "the rule" >"$src/client-patches/a_rule_026g.asm"
commit "an exe patch"
run quiet --dry-run --revision 15 --minutes 1 || fail "release.sh --revision 15 --dry-run: $out"
check "release --dry-run: an exe patch changed, nothing marked: the note names it" has "$out" \
    "note     the exe's patches changed since 026g-rev14 (a_rule_026g.asm), and no change of what a match computes is marked"
printf 'next\tthe rule\n' >>"$sim"
commit "the rule changes what a match computes"
run quiet --dry-run --revision 15 --minutes 1 || fail "release.sh --revision 15 --dry-run: $out"
check "release --dry-run: marked: no note, the release says so" bash -c \
    "! grep -q 'note     the exe.s patches changed' <<<\"\$1\" && grep -q '^  replays: what a match computes changes (the rule): recordings of games before revision 15 are refused from the swap on' <<<\"\$1\"" _ "$out"
check "release --dry-run: ... and the record it would make writes 15 for \`next\`" has "$out" \
    "15 for \`next\` in srp-026g/crates/session/src/v026g/simulation_changes.tsv), pushed as release/record-rev15"
sed -i '/^next\t/d' "$sim"
commit "the rule, not yet"
expect_refused "rollback refuses what is not a release folder" "not the name of a release folder" "$here/rollback" "../etc"
# Rolled back again while players own items the old data lacks: no loop, the
# advice to roll forward, and the way back to the release.
echo "ERROR srp: cannot load the 0.26g accounts: the kept accounts name what the server's game data (x) lacks; item 14" >"$T/refuse-start"
out=$(yes_to "$here/rollback" --keep-db "$D_FOLDER" || true)
rm -f "$T/refuse-start"
check "rollback: srp refusing the old files: roll forward" has "$out" "roll forward with a fixed patch"
check "rollback: ... srp26 stopped, not looping" test "$(cat "$T/docker/state")" = stopped
check "rollback: ... the folder says rollback-refused" grep -qx rollback-refused "$(folder 14 d)/state"
back=$(grep -A1 'To put the release back as it was:' <<<"$out" | tail -n 1)
(bash -c "$back") >/dev/null 2>&1 || fail "the way back to the release: $back"
check "rollback: ... its way back puts the release in place again" bash -c "cmp -s $T/work/r14.zip $T/live/download/Survarium-0.26g-srp-files.zip && grep -qx SRP_MIN_PATCH_REVISION=14 $T/live/.env && test \$(cat $T/docker/state) = running"

# --test: the test environment's compose project and patches server, no copy
# of srp.db to the jump host (it is there).
# Its revision is the test exe's + 1 (here the same site: 15).
mkdir -p "$T/p15/gameplay/items" "$T/r15"
echo "item 15" >"$T/p15/gameplay/items/f.options"
"$bin" pack --dir "$T/p15" --out "$T/r15/resources.db.patch_15" >/dev/null
run quiet --test --dry-run --resources "$T/r15/resources.db.patch_15" || true
check "release --test: its revision is the test exe's + 1" has "$out" "the test exe is revision 14: this release is revision 15"
check "release --test: no record (the dry run says so)" has "$out" '^\[dry run\] 7. no record (--test: no commit, no tag)'
check "release --test: compose project srptest" has "$out" "docker compose -p srptest up -d --no-deps --force-recreate srp26"
check "release --test: the TEST patches server" has "$out" "On the TEST patches server"
check "release --test: no copy of srp.db to the jump host" bash -c "! grep -q 'copied to the jump host' <<<\"\$1\"" _ "$out"

# --no-maintenance: no countdown, the admin page never asked; prepared at
# once, swapped as soon as "matches":0 (and once more right before the stop).
commit "for 15"
: >"$T/state/admin.log"
: >"$T/state/admin-seen.log"
before=$(tree)
run quiet --no-maintenance --dry-run || fail "release.sh --no-maintenance --dry-run: $out"
check "release --no-maintenance --dry-run: the preflight needs no admin page" has "$out" "the admin page: not needed (--no-maintenance)"
check "release --no-maintenance --dry-run: plans no maintenance" has "$out" "no maintenance planned (--no-maintenance: the admin page is never asked)"
check "release --no-maintenance --dry-run: ... shows no POST" bash -c "! grep -q 'POST ' <<<\"\$1\"" _ "$out"
check "release --no-maintenance --dry-run: the swap once \"matches\":0, up to 120 min" has "$out" 'wait for "matches":0 on .*/api/stats (up to 120 min), then y/N'
check "release --no-maintenance --dry-run: \"matches\":0 once more, right before the stop" bash -c \
    "grep -A3 'matches\":0 once more right before the stop' <<<\"\$1\" | grep -q 'compose stop srp26'" _ "$out"
check "release --no-maintenance --dry-run: says the lobby reconnects" has "$out" '~30 s "server sync" and reconnect'
check "release --no-maintenance --dry-run: changes nothing" test "$(tree)" = "$before"
check "release --no-maintenance --dry-run: asks no admin page" test ! -s "$T/state/admin-seen.log"
expect_refused "release: --minutes and --no-maintenance refused together" "and --no-maintenance has none" \
    "$here/release.sh" --no-maintenance --minutes 5
expect_refused "release: --match-wait is a number" "match-wait: a whole number" "$here/release.sh" --no-maintenance --match-wait soon

# The limit passes with a match still running: a clean stop.
echo 1000 >"$T/state/busy"
run yes --no-maintenance --match-wait 2 --label nme || true
rm -f "$T/state/busy"
check "release --no-maintenance: a match past --match-wait stops it" has "$out" "match(es) still running after 2 min of waiting (--match-wait): nothing was swapped"
check "release --no-maintenance: ... removes the staged files" nothing_staged
check "release --no-maintenance: ... its folder says stopped" grep -qx stopped "$(folder 15 nme)/state"
check "release --no-maintenance: ... leaves the live files as they were" test "$(tree)" = "$before"
check "release --no-maintenance: ... restarts nothing" bash -c "! grep -qE ' (up|stop) ' $T/log"
check "release --no-maintenance: ... cancels no maintenance (none was planned)" bash -c "! grep -qE '^maintenance: ' <<<\"\$1\"" _ "$out"
check "release --no-maintenance: ... the clock counts from the go" bash -c "grep -q 'since the go\]' <<<\"\$1\" && ! grep -q 'planned time\]' <<<\"\$1\"" _ "$out"
check "release --no-maintenance: ... frees the lock" test ! -e "$T/lock"
check "release --no-maintenance: ... asked no admin page" test ! -s "$T/state/admin-seen.log"

# A match, then none, then one begun during the y/N: the check before the
# stop waits for it, and the stop comes right after a "matches":0.
printf '1\n1\n0\n2\n0\n' >"$T/state/matches"
args_before=$(grep -c '' "$T/args")
run yes --no-maintenance --label nm || fail "release.sh --no-maintenance: $out"
rm -f "$T/state/matches"
check "release --no-maintenance: waits while a match runs" has "$out" "1 match(es) still running; waiting"
check "release --no-maintenance: ... the check before the stop waits for the new one" bash -c \
    "grep -q '2 match(es) still running; waiting' <<<\"\$1\" && grep -q 'matches\":0 once more: stopping srp26 now' <<<\"\$1\"" _ "$out"
# The ssh to the jump host that stops srp26 (its command quoted: \ before a
# space), and what came just before it.
stop_at=$(tail -n +"$((args_before + 1))" "$T/args" | grep -n 'compose.\{1,2\}stop.\{1,2\}srp26' | head -n 1 | cut -d: -f1)
check "release --no-maintenance: ... the stop right after /api/stats said 0" \
    bash -c "tail -n +$((args_before + 1)) $T/args | sed -n '$((${stop_at:-1} - 1))p' | grep -q '^curl .*/api/stats$'"
check "release --no-maintenance: ... then releases" cmp -s "$T/work/r15.zip" "$T/live/download/Survarium-0.26g-srp-files.zip"
check "release --no-maintenance: ... SRP_MIN_PATCH_REVISION=15" grep -qx "SRP_MIN_PATCH_REVISION=15" "$T/live/.env"
check "release --no-maintenance: ... its folder says released" grep -qx released "$(folder 15 nm)/state"
check "release --no-maintenance: ... asked no admin page" test ! -s "$T/state/admin-seen.log"
check "release --no-maintenance: ... planned nothing" test ! -s "$T/state/admin.log"
check "release --no-maintenance: ... frees the lock" test ! -e "$T/lock"

# The default: the go, then at once the maintenance, planned to start 30 s
# after the last running match ends, as /api/stats says
# (last_match_ends_in), in whole minutes; from an srp without it, the
# launcher page's "Time left", else a whole match. Then, during the
# countdown, everything prepared and staged; the swap at the later of the
# two. The dry run shows that order, and the minutes it would plan now.
commit "for 16"
: >"$T/state/admin.log"
: >"$T/state/admin-seen.log"
: >"$T/state/admin-staged.log"
plan_line() { grep -m1 'plan the maintenance in' <<<"$out" || true; }
line_of() { grep -n -m1 -- "$2" <<<"$1" | cut -d: -f1; } # <text> <pattern>
in_order() { # <text> <pattern>...: each found, each after the one before
    local text=$1 at last=0
    shift
    for pattern; do
        at=$(line_of "$text" "$pattern")
        [[ -n $at ]] && ((at > last)) || return 1
        last=$at
    done
}
echo 300 >"$T/state/ends-in"
run quiet --dry-run || fail "release.sh --dry-run, a match ending in 5 min: $out"
check "release: a match ending in 5 min: the maintenance in 6 min" bash -c "grep -q 'in 6 min (now: /api/stats: the last match ends in 5:00 at the latest (last_match_ends_in 300), then 30 s' <<<\"\$1\"" _ "$(plan_line)"
check "release --dry-run: prints the order: the lock and the go, the plan, the staging, the swap" in_order "$out" \
    '^\[dry run\] 2. take the release lock .*, then wait for the go' '^\[dry run\] 3. right after the go: plan the maintenance' \
    '^\[dry run\] 4. during the countdown, build here' '^\[dry run\] 5. at the later of the planned time and the end of the staging' \
    'docker compose up -d --no-deps --force-recreate srp26'
echo 299 >"$T/state/ends-in"
run quiet --dry-run || true
check "release: ... 4:59 to the last match's end: 6 too (rounded up)" bash -c "grep -q 'in 6 min' <<<\"\$1\"" _ "$(plan_line)"
echo 0 >"$T/state/ends-in"
run quiet --dry-run || true
check "release: no match running: 1 min" bash -c "grep -q 'in 1 min (now: /api/stats: no match runs (last_match_ends_in 0), then 30 s' <<<\"\$1\"" _ "$(plan_line)"
check "release --dry-run: ... shows the POST" has "$out" "tab=maint minutes=1 "
check "release --dry-run: ... asks the admin page nothing but the preflight's GET" test "$(sort -u "$T/state/admin-seen.log")" = GET
rm "$T/state/ends-in"
run quiet --dry-run || true
check "release: an srp without last_match_ends_in, \"matches\":0: 1 min, and it says why" bash -c "grep -q 'in 1 min (now: /api/stats has no last_match_ends_in (an srp from before it); \"matches\":0' <<<\"\$1\"" _ "$(plan_line)"
printf '2\n2\n' >"$T/state/matches"
printf '5\n9\n' >"$T/state/page-left"
run quiet --dry-run || true
check "release: ... matches running: the launcher page's most \"Time left\" + 1" bash -c "grep -q 'in 10 min (now: /api/stats has no last_match_ends_in .*: 2 match(es), the last ending in <= 9 min, then 30 s' <<<\"\$1\"" _ "$(plan_line)"
printf '2\n2\n' >"$T/state/matches"
printf '5\n-\n' >"$T/state/page-left"
run quiet --dry-run || true
check "release: ... one still loading (no time left): a whole match and 30 s, 16 min" bash -c "grep -q 'in 16 min (now: .*(a match with no time left yet (still loading)): a whole match, 15 min, then 30 s' <<<\"\$1\"" _ "$(plan_line)"
printf '3\n3\n' >"$T/state/matches"
printf '5\n9\n' >"$T/state/page-left"
run quiet --dry-run || true
check "release: ... the page lists fewer than run: 16 min" bash -c "grep -q 'in 16 min (now: .*(2 time(s) left for 3 match(es))' <<<\"\$1\"" _ "$(plan_line)"
rm -f "$T/state/matches" "$T/state/page-left"
check "release --dry-run: none of these planned anything" test ! -s "$T/state/admin.log"

# A stop after the plan (build-zip fails, during the countdown): the
# maintenance cancelled.
echo 300 >"$T/state/ends-in"
: >"$T/state/admin-seen.log"
before=$(tree)
touch "$T/build-zip-fails"
run yes --label stop || true
rm "$T/build-zip-fails"
check "release: a stop during the countdown (build-zip failed)" has "$out" "STOPPED before the swap"
check "release: ... the admin page: the preflight's GET, the plan, the cancel" test "$(tr '\n' ' ' <"$T/state/admin-seen.log" | sed 's/POST [^ ]*/POST/g; s/ $//')" = "GET POST POST"
check "release: ... the maintenance planned, then cancelled" test "$(admin_log)" = "plan 6 cancel" -a ! -e "$T/state/maint"
check "release: ... says so" has "$out" "^maintenance: cancelled"
check "release: ... leaves the live files as they were" test "$(tree)" = "$before"
check "release: ... frees the lock" test ! -e "$T/lock"

# Ctrl-C while it waits for the planned time (to bash only, as a trap during
# a sleep of a minute once waited for the sleep): cleaned up in a few
# seconds. A minute is 20 s here, so each wait is 20 s.
: >"$T/state/admin.log"
set -m
SKHID_TEST_SECONDS_PER_MINUTE=20 "$here/release.sh" --yes --i-know --minutes 3 --label int >"$T/int.out" 2>&1 </dev/null &
int_pid=$!
set +m
for _ in $(seq 300); do grep -q '5. waiting for the planned time' "$T/int.out" && break; sleep 0.1; done
sleep 1
kill -INT "$int_pid"
int_at=$SECONDS
for _ in $(seq 150); do kill -0 "$int_pid" 2>/dev/null || break; sleep 0.1; done
int_took=$((SECONDS - int_at))
if kill -0 "$int_pid" 2>/dev/null; then kill -9 "$int_pid"; fi
wait "$int_pid" 2>/dev/null || true
out=$(cat "$T/int.out")
printf '%s\n' "$out" >>"$T/outputs"
check "release: Ctrl-C in the wait for the planned time ends it in a few seconds (${int_took} s)" test "$int_took" -le 4
check "release: ... it was waiting, the maintenance planned" bash -c "grep -q 'maintenance planned' <<<\"\$1\" && grep -q 'interrupted' <<<\"\$1\"" _ "$out"
check "release: ... cancels the maintenance" test "$(admin_log)" = "plan 3 cancel" -a ! -e "$T/state/maint"
check "release: ... removes the staged files" nothing_staged
check "release: ... its folder says stopped" grep -qx stopped "$(folder 16 int)/state"
check "release: ... leaves the live files as they were" test "$(tree)" = "$before"
check "release: ... frees the lock" test ! -e "$T/lock"

# Ctrl-C during the staging (the build of the ZIP), to the whole process
# group as a terminal sends it: the plan made before is cancelled.
: >"$T/state/admin.log"
echo 30 >"$T/build-zip-sleep"
set -m
"$here/release.sh" --yes --i-know --minutes 60 --label stg >"$T/stg.out" 2>&1 </dev/null &
stg_pid=$!
set +m
for _ in $(seq 300); do grep -q '4. building the ZIP' "$T/stg.out" && break; sleep 0.1; done
sleep 1
kill -INT -- "-$stg_pid"
stg_at=$SECONDS
for _ in $(seq 150); do kill -0 "$stg_pid" 2>/dev/null || break; sleep 0.1; done
stg_took=$((SECONDS - stg_at))
if kill -0 "$stg_pid" 2>/dev/null; then kill -9 -- "-$stg_pid"; fi
wait "$stg_pid" 2>/dev/null || true
rm -f "$T/build-zip-sleep"
out=$(cat "$T/stg.out")
printf '%s\n' "$out" >>"$T/outputs"
check "release: Ctrl-C during the staging, after the plan, ends it in a few seconds (${stg_took} s)" test "$stg_took" -le 5
check "release: ... it was building, the maintenance planned" bash -c "grep -q 'maintenance planned' <<<\"\$1\" && grep -q 'interrupted' <<<\"\$1\"" _ "$out"
check "release: ... cancels the maintenance" test "$(admin_log)" = "plan 60 cancel" -a ! -e "$T/state/maint"
check "release: ... says so" has "$out" "^maintenance: cancelled"
check "release: ... staged nothing (no folder yet)" test -z "$(folder 16 stg)"
check "release: ... leaves the live files as they were" test "$(tree)" = "$before"
check "release: ... frees the lock" test ! -e "$T/lock"

# A closed terminal (HUP to the whole group, the release.log copy too) and
# a second Ctrl-C during the cleanup: the cleanup still finishes (the plan
# cancelled, the lock freed); a maintenance left planned would put the
# server down with nothing swapped.
for how in hup twice; do
    : >"$T/state/admin.log"
    echo 30 >"$T/build-zip-sleep"
    set -m
    "$here/release.sh" --yes --i-know --minutes 60 --label "$how" >"$T/$how.out" 2>&1 </dev/null &
    sig_pid=$!
    set +m
    for _ in $(seq 300); do grep -q '4. building the ZIP' "$T/$how.out" && break; sleep 0.1; done
    sleep 1
    if [[ $how == hup ]]; then
        kill -HUP -- "-$sig_pid"
    else
        kill -INT -- "-$sig_pid"
        sleep 0.2
        kill -INT -- "-$sig_pid" 2>/dev/null || true
    fi
    for _ in $(seq 150); do kill -0 "$sig_pid" 2>/dev/null || break; sleep 0.1; done
    if kill -0 "$sig_pid" 2>/dev/null; then kill -9 -- "-$sig_pid"; fi
    wait "$sig_pid" 2>/dev/null || true
    rm -f "$T/build-zip-sleep"
    printf '%s\n' "$(cat "$T/$how.out")" >>"$T/outputs"
    check "release: $how during the staging: the plan is still cancelled" test "$(admin_log)" = "plan 60 cancel" -a ! -e "$T/state/maint"
    check "release: ... and the lock freed" test ! -e "$T/lock"
    check "release: ... the live files as they were" test "$(tree)" = "$before"
done

# Without the go given beforehand: it waits for it right after the
# preflight, having built, staged and asked the admin page nothing; the go
# file lets it plan at once. Then Ctrl-C (the plan is cancelled, as any
# stop).
: >"$T/state/admin.log"
: >"$T/state/admin-seen.log"
echo 300 >"$T/state/ends-in"
set -m
SKHID_RELEASE_GO='' SKHID_TEST_SECONDS_PER_MINUTE=20 "$here/release.sh" --yes --i-know --label go >"$T/go.out" 2>&1 </dev/null &
go_pid=$!
set +m
for _ in $(seq 600); do grep -q "waiting for the go" "$T/go.out" && break; sleep 0.1; done
sleep 3
check "release: without the go given, it waits for it after the preflight" grep -q "2. waiting for the go" "$T/go.out"
check "release: ... having asked the admin page nothing but the preflight's GET" test "$(cat "$T/state/admin-seen.log")" = GET
check "release: ... built and staged nothing" bash -c "! grep -q '4. building' $T/go.out && test -z \"\$(find $T/live -maxdepth 1 -name 'release-rev16-*-go')\""
go_file=$(sed -n "s/^Go: answer y here, or run: touch //p" "$T/go.out" | head -n 1)
check "release: ... names its go file, in the cache" bash -c "[[ \$1 == $T/cache/skhid-release/go-* ]]" _ "$go_file"
touch "$go_file"
for _ in $(seq 300); do grep -q "3. planning the maintenance in 6 min" "$T/go.out" && break; sleep 0.1; done
check "release: ... the go file lets it plan, at once" grep -q "3. planning the maintenance in 6 min" "$T/go.out"
check "release: ... and it takes the file away" test ! -e "$go_file"
for _ in $(seq 300); do grep -q "5. waiting for the planned time" "$T/go.out" && break; sleep 0.1; done
check "release: ... then it stages, during the countdown" in_order "$(cat "$T/go.out")" \
    '^go: ' '3. maintenance planned' '4. staging in the release folder' '5. waiting for the planned time'
kill -INT "$go_pid"
for _ in $(seq 150); do kill -0 "$go_pid" 2>/dev/null || break; sleep 0.1; done
if kill -0 "$go_pid" 2>/dev/null; then kill -9 "$go_pid"; fi
wait "$go_pid" 2>/dev/null || true
printf "%s\n" "$(cat "$T/go.out")" >>"$T/outputs"
check "release: ... stopped after the go, the plan cancelled" test "$(admin_log)" = "plan 6 cancel" -a ! -e "$T/state/maint"
check "release: ... frees the lock" test ! -e "$T/lock"

# Ctrl-C while it waits for the go: nothing at all.
before=$(tree)
: >"$T/state/admin.log"
: >"$T/state/admin-seen.log"
set -m
SKHID_RELEASE_GO='' "$here/release.sh" --yes --i-know --label nogo >"$T/nogo.out" 2>&1 </dev/null &
nogo_pid=$!
set +m
for _ in $(seq 600); do grep -q "waiting for the go" "$T/nogo.out" && break; sleep 0.1; done
kill -INT "$nogo_pid"
for _ in $(seq 150); do kill -0 "$nogo_pid" 2>/dev/null || break; sleep 0.1; done
if kill -0 "$nogo_pid" 2>/dev/null; then kill -9 "$nogo_pid"; fi
wait "$nogo_pid" 2>/dev/null || true
printf "%s\n" "$(cat "$T/nogo.out")" >>"$T/outputs"
check "release: Ctrl-C before the go: nothing planned, nothing cancelled" test ! -s "$T/state/admin.log"
check "release: ... nothing built or staged" bash -c "! grep -qE '4\\. |STOPPED before the swap' $T/nogo.out"
check "release: ... the live files as they were" test "$(tree)" = "$before"
check "release: ... frees the lock" test ! -e "$T/lock"

# The whole default run: a match ending in 5 min. The go, then the plan (6
# min), with nothing staged yet, then the staging, then the swap.
: >"$T/state/admin.log"
: >"$T/state/admin-seen.log"
: >"$T/state/admin-staged.log"
echo 300 >"$T/state/ends-in"
run yes --label auto || fail "release.sh, the default: $out"
check "release: the default plans the maintenance in 6 min (a match ending in 5)" test "$(admin_log)" = "plan 6"
check "release: ... the go, then the plan, then the staging, then the swap" in_order "$out" \
    '^go: given beforehand' '3. planning the maintenance in 6 min' '3. maintenance planned' '4. building the server data' \
    '4. staging in the release folder' '5. the swap'
check "release: ... planned before anything was staged (the admin page saw no staged folder)" test "$(cat "$T/state/admin-staged.log")" = "plan 6 NOT staged"
check "release: ... the admin page: the preflight's GET, then the plan" test "$(tr '\n' ' ' <"$T/state/admin-seen.log" | sed 's/ POST [^ ]*/ POST/; s/ $//')" = "GET POST"
check "release: ... says Play is locked, and that it builds now" has "$out" 'the launcher says "Maintenance in 6 min"; Play is locked; building and staging now'
check "release: ... then releases" cmp -s "$T/work/r16.zip" "$T/live/download/Survarium-0.26g-srp-files.zip"
check "release: ... SRP_MIN_PATCH_REVISION=16" grep -qx "SRP_MIN_PATCH_REVISION=16" "$T/live/.env"
check "release: ... the folder's rollback" test -f "$(folder 16 auto)/rollback.sh" -a -f "$(folder 16 auto)/rollback.env"
rm -f "$T/state/ends-in"

# Staging that ends after the planned time: the swap at once. gh may not
# merge the record's PR (a rule of the repository), and origin refuses the
# tag ($T/tag-push-fails): the PR stays open, the tag is made here only, the
# run says so, with what to do, and ends with 1; the next release waits for
# the merge.
commit "for 17"
: >"$T/state/admin.log"
echo 3 >"$T/build-zip-sleep"
cat >"$T/origin.git/hooks/pre-receive" <<HOOK
#!/usr/bin/env bash
while read -r old new ref; do
    if [[ -e $T/tag-push-fails && \$ref == refs/tags/* ]]; then echo "tags are refused here" >&2; exit 1; fi
done
HOOK
chmod +x "$T/origin.git/hooks/pre-receive"
touch "$T/gh-merge-fails" "$T/tag-push-fails"
run yes --minutes 1 --label late && fail "release.sh, staged late, the PR not merged: ended 0"
rm -f "$T/build-zip-sleep" "$T/gh-merge-fails" "$T/tag-push-fails"
check "release: gh cannot merge the record's PR: said loudly at the end" bash -c \
    "tail -n 4 <<<\"\$1\" | grep -q 'RELEASED, BUT THE RECORD IS NOT COMPLETE' && tail -n 3 <<<\"\$1\" | grep -q 'record.s PR https://github.com/srp/skhid-srp/pull/[0-9]* is OPEN, NOT MERGED: gh could not merge it (GraphQL: Repository rule violations'" _ "$out"
check "release: origin refuses the tag: said, with the push to do by hand (the tag is made here)" bash -c \
    "grep -q '^  - the push of the tag 026g-rev17: .*tags are refused here.*by hand: git -C [^ ]*repo push origin refs/tags/026g-rev17\$' <<<\"\$1\" && ! grep -q 'tag -a 026g-rev17' <<<\"\$1\" && git -C $T/repo rev-parse -q --verify refs/tags/026g-rev17 >/dev/null" _ "$out"
check "release: ... the tag not on origin; the branch there, main's REVISION still 16" bash -c \
    "test -z \"\$(git -C $T/origin.git rev-parse -q --verify refs/tags/026g-rev17)\" && test -n \"\$(git -C $T/origin.git rev-parse -q --verify refs/heads/release/record-rev17)\" && test \"\$1\" = 16" _ "$(main_revision)"
expect_refused "release: the next one waits for the record's merge" "main's REVISION 16 is below the live exe's 17: the record of revision 17 is not on main (its PR still open? the end of that release.s log says what was not done)" "$here/release.sh"
pr17=$(grep -l '^release/record-rev17$' "$T"/gh/*/head | head -n 1)
quiet gh pr merge "https://github.com/srp/skhid-srp/pull/$(basename "$(dirname "$pr17")")" --merge
check "release: ... merged by hand: main has REVISION 17" test "$(main_revision)" = 17
push_tag=$(sed -n 's/^  - the push of the tag 026g-rev17: .*by hand: //p' <<<"$out")
bash -c "$push_tag" >/dev/null 2>&1
check "release: ... the tag pushed by hand as said: on origin, on the commit built" test "$(git -C "$T/origin.git" rev-parse -q --verify '026g-rev17^{commit}')" = "$(git -C "$T/origin.git" rev-parse 'main^1')"
check "release: staged after the planned time: the swap at once" has "$out" "5. staged after the planned time: the swap at once"
check "release: ... no wait for the planned time" bash -c "! grep -q 'waiting for the planned time' <<<\"\$1\"" _ "$out"
check "release: ... then releases" cmp -s "$T/work/r17.zip" "$T/live/download/Survarium-0.26g-srp-files.zip"
check "release: ... without cancelling" test "$(admin_log)" = "plan 1"

# --now: no maintenance, no notice, no wait.
commit "for 18"
expect_refused "release: --now and --minutes refused together" "and --now has none" "$here/release.sh" --now --minutes 5
: >"$T/state/admin.log"
: >"$T/state/admin-seen.log"
before=$(tree)
run quiet --dry-run --now --no-maintenance || fail "release.sh --now --dry-run: $out"
check "release --now: --no-maintenance with it is accepted, and said to be the same" has "$out" "note: --now plans no maintenance anyway"
check "release --now --dry-run: says there is no notice" has "$out" "maintenance: NONE, NO NOTICE (--now)"
check "release --now --dry-run: prints the order: the go, the staging, the swap at once" in_order "$out" \
    '^\[dry run\] 2. take the release lock .*, then wait for the go' '^\[dry run\] 3. no maintenance planned (--now' \
    '^\[dry run\] 4. after the go, build here' '^\[dry run\] 5. once staged: the swap at once, NO wait for matches' \
    'the stop ends without results (--now)' 'compose stop srp26'
check "release --now --dry-run: no POST, no admin page" bash -c "! grep -q 'POST ' <<<\"\$1\" && test ! -s $T/state/admin-seen.log" _ "$out"
check "release --now --dry-run: changes nothing" test "$(tree)" = "$before"
# A refused start with --now: rolled back by itself all the same.
echo "ERROR srp: SRP_MIN_PATCH_REVISION is 13, below resources.db.patch_14. Set SRP_MIN_PATCH_REVISION=14 or more" >"$T/refuse-start"
run yes --now --label nowf || true
rm -f "$T/refuse-start"
check "release --now: srp refusing to start: rolled back" has "$out" "rolled back: srp26 .*/patches .* as before"
check "release --now: ... the ZIP never moved" cmp -s "$T/work/r17.zip" "$T/live/download/Survarium-0.26g-srp-files.zip"
check "release --now: ... no record of 18" no_record 18
# A match running: swapped anyway, and it says how many it ended.
echo 1000 >"$T/state/busy"
run yes --now --label now || fail "release.sh --now: $out"
rm -f "$T/state/busy"
check "release --now: with a match running, swaps anyway" cmp -s "$T/work/r18.zip" "$T/live/download/Survarium-0.26g-srp-files.zip"
check "release --now: ... says, right before the stop, the match it ends" in_order "$out" \
    '5. the swap' '--now: 1 match(es) running: the stop ends them now, WITHOUT RESULTS' 'the server is up on the new data'
check "release --now: ... and at the end how many it ended" has "$out" "--now: the stop ended 1 match(es) without results"
check "release --now: ... kept in the folder's release.log" grep -q -- "--now: the stop ended 1 match(es) without results" "$(folder 18 now)/release.log"
check "release --now: ... never waited for a match" bash -c "! grep -q 'still running; waiting' <<<\"\$1\"" _ "$out"
check "release --now: ... planned nothing, asked no admin page" test ! -s "$T/state/admin.log" -a ! -s "$T/state/admin-seen.log"
check "release --now: ... SRP_MIN_PATCH_REVISION=18" grep -qx "SRP_MIN_PATCH_REVISION=18" "$T/live/.env"
check "release --now: ... frees the lock" test ! -e "$T/lock"
check "release --now: ... its record: REVISION 18 on main, the tag" bash -c "test \"\$1\" = 18 && git -C $T/origin.git rev-parse -q --verify refs/tags/026g-rev18 >/dev/null" _ "$(main_revision)"

# A real --test release: its revision is the test exe's + 1, and it makes
# no record (no tag, no branch, no PR, main as it was).
main_before=$(git -C "$T/origin.git" rev-parse main)
: >"$T/gh.log"
run yes --test --now --label t || fail "release.sh --test: $out"
check "release --test: released revision 19 (the test exe's + 1)" bash -c "grep -qx SRP_MIN_PATCH_REVISION=19 $T/live/.env && cmp -s $T/work/r19.zip $T/live/download/Survarium-0.26g-srp-files.zip"
check "release --test: no record" has "$out" "No record (--test): no commit, no tag."
check "release --test: ... no tag, no branch" no_record 19
check "release --test: ... main as it was, gh never asked" test "$(git -C "$T/origin.git" rev-parse main)|$(cat "$T/gh.log")" = "$main_before|"

# No secret in any output or command line.
check "release: the admin token is in no output" bash -c "! grep -qF -- '$token' $T/outputs"
check "release: the admin path is in no output" bash -c "! grep -qF -- '$admin_path' $T/outputs"
check "release: ... nor in a folder's release.log" bash -c "! grep -qF -e '$token' -e '$admin_path' $T/live/release-rev*/release.log"
check "release: no setting's value is printed" bash -c "! grep -qF 'SRP_DAILY_BONUS=5' $T/outputs"
check "release: neither is in a command line of ssh, curl or docker" bash -c "! grep -qF -e '$token' -e '$admin_path' $T/args $T/log"
check "release: the admin page was asked through curl's stdin, on the jump host" grep -q "^ssh jump curl .* -K -$" "$T/args"
check "release: no other container was ever run" bash -c "! grep -q '^docker run' $T/log"

echo
if ((failures)); then
    echo "$failures FAILED"
    exit 1
fi
echo "all passed"
