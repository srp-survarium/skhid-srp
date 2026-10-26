#!/usr/bin/env bash
# The Rust port's whole-game regression: the old exe (main's build) against
# the ported one, headless, no player. What each step checks: README.txt.
set -euo pipefail

HERE=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
REPO=$(git -C "$HERE" rev-parse --show-toplevel)
MAIN=$(dirname -- "$(git -C "$HERE" rev-parse --path-format=absolute --git-common-dir)")
PRIVATE=${SRP_PRIVATE:-$MAIN/private}

# ---- settings ------------------------------------------------------------------
OLD_EXE="" NEW_EXE="" OLD_REV=origin/main NEW_REV=feat/rust-port-all
ORIGINAL=$PRIVATE/function-map/inputs/026g/survarium.exe
CONFIG=$PRIVATE/local-clients/config.json
LC_STATE=$PRIVATE/local-clients/runtime/state.json
REVISION=""                       # --revision for both builds; default: the config's SRP_MIN_PATCH_REVISION
REPLAYS=()                        # default: private/recordings/*.srprec
GAME_MODES=$PRIVATE/port-regression/game-modes
SUR_TOOL=""
FIRST_KEY_FRAME_ONLY=1
REPLAY_SECONDS=240
HASH_CSV=srp_world_hashes.csv
BOT_MATCHES="battery:2:61,tdm:4:126,research:5:145"
BOT_MINUTES=3 BOT_COUNT=2 BOT_CLIENTS=pair
SERVER_BIN="" BOT_BIN=""
LOGIN_CLICK=1110,550 PLAY_CLICK=640,22
FRAMES_OLD="" FRAMES_NEW="" FRAMES_THRESHOLD=0.10 FRAMES_MAX_THRESHOLD=1.0
ONLY=all DRY=0 WAIT=0 WAIT_TIMEOUT=3600 KEEP=0
PORT_BASE=52234 DISPLAY_FIRST=120 WORK=""

usage() {
	cat <<EOF
usage: $0 [options]
  --old-exe PATH | --old-rev REV   the old exe, or the revision to build it from (default $OLD_REV)
  --new-exe PATH | --new-rev REV   the ported exe, or its revision (default $NEW_REV)
  --original PATH                  the original 0.26g survarium.exe (default under private/function-map)
  --revision N                     patch_026g.py --revision=N for both builds (default: the config's
                                   SRP_MIN_PATCH_REVISION, so the local server lets them in)
  --config PATH                    the local-clients config.json (game, wine, prefix template, server)
  --replay PATH                    a .srprec or .sur to replay (repeat; default private/recordings/*.srprec)
  --game-modes DIR                 the game's gameplay/game_modes/*.options (for .srprec -> .sur)
  --sur-tool PATH                  srp-recording-to-sur (default: built in the dev shell)
  --all-key-frames                 convert with every key frame (default: the first only, so the
                                   whole replay is simulated from inputs)
  --replay-seconds N               how long each replay runs (default $REPLAY_SECONDS)
  --bot-matches LIST               label:mode:map,... (default $BOT_MATCHES)
  --bot-minutes N                  minutes of play per match (default $BOT_MINUTES)
  --bot-count N                    srp-bot players per match (default $BOT_COUNT)
  --bot-clients new|pair           the new exe alone, or new and old in one match (default $BOT_CLIENTS)
  --server-bin PATH --bot-bin PATH srp and srp-bot (default: the config's server, srp-bot beside it)
  --login-click X,Y --play-click X,Y   the in-game cursor positions of Enter and Play
  --frames-old-exe PATH --frames-new-exe PATH   patch 71 frame-trace builds (default: the old/new
                                   exes themselves when they carry patch 71)
  --frames-threshold F             allowed p50/p95/p99 growth (default $FRAMES_THRESHOLD)
  --only replay|bots|frames        one step
  --dry-run                        print the plan; launch and build nothing
  --wait [--wait-timeout S]        wait while the owner has a game open (default: refuse)
  --work DIR                       the run's folder (default private/port-regression/run-<time>)
  --keep                           keep the Wine prefix copies
  --port-base N --display N        the first ports / X display number tried
EOF
}

while (($#)); do
	case $1 in
	--old-exe) OLD_EXE=$2; shift ;;
	--new-exe) NEW_EXE=$2; shift ;;
	--old-rev) OLD_REV=$2; shift ;;
	--new-rev) NEW_REV=$2; shift ;;
	--original) ORIGINAL=$2; shift ;;
	--revision) REVISION=$2; shift ;;
	--config) CONFIG=$2; shift ;;
	--replay) REPLAYS+=("$2"); shift ;;
	--game-modes) GAME_MODES=$2; shift ;;
	--sur-tool) SUR_TOOL=$2; shift ;;
	--all-key-frames) FIRST_KEY_FRAME_ONLY=0 ;;
	--replay-seconds) REPLAY_SECONDS=$2; shift ;;
	--hash-csv) HASH_CSV=$2; shift ;;
	--bot-matches) BOT_MATCHES=$2; shift ;;
	--bot-minutes) BOT_MINUTES=$2; shift ;;
	--bot-count) BOT_COUNT=$2; shift ;;
	--bot-clients) BOT_CLIENTS=$2; shift ;;
	--server-bin) SERVER_BIN=$2; shift ;;
	--bot-bin) BOT_BIN=$2; shift ;;
	--login-click) LOGIN_CLICK=$2; shift ;;
	--play-click) PLAY_CLICK=$2; shift ;;
	--frames-old-exe) FRAMES_OLD=$2; shift ;;
	--frames-new-exe) FRAMES_NEW=$2; shift ;;
	--frames-threshold) FRAMES_THRESHOLD=$2; shift ;;
	--only) ONLY=$2; shift ;;
	--dry-run) DRY=1 ;;
	--wait) WAIT=1 ;;
	--wait-timeout) WAIT_TIMEOUT=$2; shift ;;
	--work) WORK=$2; shift ;;
	--keep) KEEP=1 ;;
	--port-base) PORT_BASE=$2; shift ;;
	--display) DISPLAY_FIRST=$2; shift ;;
	-h | --help) usage; exit 0 ;;
	*) echo "unknown option $1" >&2; usage >&2; exit 2 ;;
	esac
	shift
done
case $ONLY in all | replay | bots | frames) ;; *) echo "--only replay|bots|frames" >&2; exit 2 ;; esac
case $BOT_CLIENTS in new | pair) ;; *) echo "--bot-clients new|pair" >&2; exit 2 ;; esac

log() { printf '[%s] %s\n' "$(date +%H:%M:%S)" "$*" >&2; }
die() { log "port-regression: $*"; exit 2; }
want() { [[ $ONLY == all || $ONLY == "$1" ]]; }

# Python 3.10+ for the helper: the system has none on PATH, so the store's.
if command -v python3 >/dev/null && python3 -c 'import sys; sys.exit(sys.version_info < (3, 10))' 2>/dev/null; then
	PYTHON=python3
else
	PYTHON=$(nix build --no-link --print-out-paths nixpkgs#python3.out | head -n1)/bin/python3
fi
H() { "$PYTHON" "$HERE/harness.py" "$@"; }

# ---- the run's folder, results, the processes it started ----------------------------
if ((DRY)); then
	WORK=$(mktemp -d "${TMPDIR:-/tmp}/port-regression-plan.XXXXXX")
else
	WORK=${WORK:-$PRIVATE/port-regression/run-$(date +%Y%m%d-%H%M%S)}
	mkdir -p "$WORK"
	WORK=$(cd -- "$WORK" && pwd)
fi
[[ $WORK != *[[:space:]]* ]] || die "the work folder must have no spaces (the game reads -match_replay= to a space): $WORK"
mkdir -p "$WORK/logs"
RESULTS=$WORK/results.tsv
: >"$RESULTS"
record() {
	printf '%s\t%s\t%s\n' "$1" "$2" "$3" >>"$RESULTS"
	log "$1: $2 $3"
}

PIDS=()     # pid:start:label:signal of everything this harness started
PREFIXES=() # the Wine prefixes our games ran in (wineserver -k is per prefix)

starttime() { # field 22 of /proc/<pid>/stat; fails for a gone or zombie process
	local stat rest
	stat=$(cat "/proc/$1/stat" 2>/dev/null) || return 1
	rest=${stat##*) }
	read -r -a fields <<<"$rest"
	[[ ${fields[0]} != Z ]] || return 1
	echo "${fields[19]}"
}
alive() { [[ -n $2 && "$(starttime "$1" 2>/dev/null || true)" == "$2" ]]; }

launch() { # label signal logfile dir command... ; sets LAST_PID
	local label=$1 signal=$2 logfile=$3 dir=$4
	shift 4
	(cd -- "$dir" && exec setsid "$@") >"$logfile" 2>&1 </dev/null &
	LAST_PID=$!
	PIDS+=("$LAST_PID:$(starttime "$LAST_PID" || true):$label:$signal")
}

stop_entry() { # pid:start:label:signal
	local pid start label signal i
	IFS=: read -r pid start label signal <<<"$1"
	if alive "$pid" "$start"; then
		kill -s "$signal" "$pid" 2>/dev/null || true
		for ((i = 0; i < 100; i++)); do alive "$pid" "$start" || break; sleep 0.1; done
		alive "$pid" "$start" && kill -s KILL "$pid" 2>/dev/null || true
	fi
	wait "$pid" 2>/dev/null || true
}

stop_label() { # stop and forget every process whose label starts with $1
	local kept=() entry label
	for entry in "${PIDS[@]}"; do
		label=$(cut -d: -f3 <<<"$entry")
		if [[ $label == "$1"* ]]; then stop_entry "$entry"; else kept+=("$entry"); fi
	done
	PIDS=("${kept[@]}")
}

wine_kill() { # only the processes of this prefix (never an unscoped wineserver -k)
	[[ -f $1/.srp-port-regression ]] || return 0
	env WINEPREFIX="$1" timeout 15 "$WINESERVER" -k >/dev/null 2>&1 || true
	env WINEPREFIX="$1" timeout 15 "$WINESERVER" -w >/dev/null 2>&1 || true
}

# shellcheck disable=SC2329 # the EXIT trap's
cleanup() {
	local code=$? prefix i
	set +e
	for prefix in "${PREFIXES[@]}"; do wine_kill "$prefix"; done
	for ((i = ${#PIDS[@]} - 1; i >= 0; i--)); do stop_entry "${PIDS[i]}"; done
	if ((DRY)); then
		rm -rf -- "$WORK"
	elif ((!KEEP)) && [[ -d $WORK/prefixes ]]; then
		rm -rf -- "$WORK/prefixes"
	fi
	exit "$code"
}
trap cleanup EXIT
trap 'exit 130' INT TERM

# ---- the owner's game -----------------------------------------------------------
owner_busy() { H busy --ours "$WORK" --state "$LC_STATE"; }
owner_guard() {
	local reasons deadline=$(($(date +%s) + WAIT_TIMEOUT))
	while ! reasons=$(owner_busy); do
		if ((!WAIT)); then
			printf '%s\n' "$reasons" >&2
			die "the owner has a game or a local-clients run open; nothing started (--wait waits)"
		fi
		(($(date +%s) < deadline)) || die "still busy after ${WAIT_TIMEOUT}s: $reasons"
		log "waiting: $reasons"
		sleep 30
	done
}

if ((!DRY)); then
	exec 9>"$PRIVATE/port-regression.lock" || die "cannot open the lock file"
	flock -n 9 || die "another port-regression run holds $PRIVATE/port-regression.lock"
fi

# ---- the configuration ------------------------------------------------------------
[[ -f $CONFIG ]] || die "no local-clients config: $CONFIG"
cfg() { H config "$CONFIG" "$1"; }
GAME_DIR=$(cfg game_dir)
WINE=$(cfg wine)
WINESERVER=$(cfg wineserver)
TEMPLATE=$(cfg prefix_template)
RESOLUTION=$(cfg resolution)
mapfile -d '' CLIENT_ENV < <(cfg client_env)
mapfile -d '' CONFIG_SERVER_ENV < <(cfg server_env)
SERVER_BIN=${SERVER_BIN:-$(cfg server_binary)}
SERVER_WD=$(cfg server_working_dir)
BOT_BIN=${BOT_BIN:-$(dirname -- "${SERVER_BIN:-/nonexistent}")/srp-bot}
config_env() { # a value of the config's server.env
	local item
	for item in "${CONFIG_SERVER_ENV[@]}"; do [[ $item == "$1="* ]] && { echo "${item#*=}"; return; }; done
	echo ""
}
MIN_REVISION=$(config_env SRP_MIN_PATCH_REVISION)
REVISION=${REVISION:-$MIN_REVISION}
if ((${#REPLAYS[@]} == 0)); then
	shopt -s nullglob
	REPLAYS=("$PRIVATE"/recordings/*.srprec)
	shopt -u nullglob
fi

# What a step lacks; empty: it can run.
missing_common() {
	[[ -x $WINE && -x $WINESERVER ]] || { echo "no wine at $WINE"; return; }
	[[ -f $TEMPLATE/system.reg ]] || { echo "no prefix template (system.reg) at $TEMPLATE"; return; }
	[[ -f $GAME_DIR/resources.db ]] || { echo "no game (resources.db) at $GAME_DIR"; return; }
	if [[ -z $OLD_EXE || -z $NEW_EXE ]]; then
		[[ -f $ORIGINAL ]] || echo "no original exe at $ORIGINAL"
	fi
}
missing_bots() {
	[[ -x $SERVER_BIN ]] || { echo "no srp server binary (--server-bin): ${SERVER_BIN:-none in the config}"; return; }
	[[ -x $BOT_BIN ]] || { echo "no srp-bot (--bot-bin): $BOT_BIN"; return; }
	[[ -f $SERVER_WD/certs/survarium_login_server.crt ]] || { echo "the server's working dir has no certs/: $SERVER_WD"; return; }
	local data
	data=$(config_env SRP_GAME_DATA)
	[[ -z $data || -f $data ]] || echo "the config's SRP_GAME_DATA is gone: $data"
}
game_modes_ok() { compgen -G "$GAME_MODES/*.options" >/dev/null; }

# Without --game-modes: the options out of the game's resources.db and then each
# resources.db.patch_<n> in order (a later one wins, as the game mounts them),
# with tools/re-kit's dbgrab (it needs the main checkout's tools/vostok-resources-db).
DBGRAB_SRC=$MAIN/tools/re-kit/common/dbgrab
extract_game_modes() {
	game_modes_ok && return 0
	[[ -d $MAIN/tools/vostok-resources-db ]] || return 1
	local out=$WORK/game-modes-db db
	if ((DRY)); then
		echo "  game modes: dbgrab (built from $DBGRAB_SRC into <work>) get gameplay/game_modes/ from resources.db and each patch"
		return 0
	fi
	log "extracting gameplay/game_modes/*.options from the game"
	(cd -- "$DBGRAB_SRC" && nix develop "$REPO/srp-026g" --command cargo build --release \
		--target-dir "$WORK/cargo-dbgrab") >"$WORK/logs/dbgrab-build.log" 2>&1 || return 1
	mkdir -p "$out"
	while read -r db; do
		[[ -f $db ]] || continue
		"$WORK/cargo-dbgrab/release/dbgrab" "$db" get gameplay/game_modes/ "$out" >>"$WORK/logs/dbgrab.log" 2>&1 ||
			log "dbgrab could not read $db (skipped)"
	done < <(printf '%s\n' "$GAME_DIR"/resources.db "$GAME_DIR"/resources.db.patch_* | sort -V)
	GAME_MODES=$out
	game_modes_ok
}

# ---- the exes -----------------------------------------------------------------------
build_exe() { # label rev out
	local label=$1 rev=$2 out=$3 sha tree
	sha=$(git -C "$REPO" rev-parse --verify --quiet "$rev^{commit}") || die "no such revision: $rev"
	if ((DRY)); then
		echo "  build $label from $rev ($sha): git archive $sha srp-026g/client-patches srp-026g/crates tools/re-kit | tar -x -C <work>/build/$label"
		echo "    nix develop $REPO/srp-026g --command python3 patch_026g.py $ORIGINAL <work>/exe/$label.exe${REVISION:+ --revision=$REVISION}"
		return
	fi
	tree=$WORK/build/$label
	rm -rf -- "$tree"
	mkdir -p "$tree" "$WORK/exe" "$REPO/srp-026g/target-arena"
	git -C "$REPO" archive "$sha" srp-026g/client-patches srp-026g/crates tools/re-kit | tar -x -C "$tree"
	ln -s "$REPO/srp-026g/target-arena" "$tree/srp-026g/target-arena" # the shared sysroot, as equiv_026g.py
	log "building $label from $rev ($sha)"
	# shellcheck disable=SC2016 # the inner bash expands them
	nix develop "$REPO/srp-026g" --command bash -c 'cd "$1" && python3 patch_026g.py "$2" "$3" ${4:+"--revision=$4"}' \
		_ "$tree/srp-026g/client-patches" "$ORIGINAL" "$out" "$REVISION" >"$WORK/logs/build-$label.log" 2>&1 ||
		die "the $label build failed: $WORK/logs/build-$label.log"
	echo "$sha" >"$WORK/exe/$label.rev"
}

# ---- the display and the game -----------------------------------------------------
nix_out() { nix build --no-link --print-out-paths "nixpkgs#$1.out" | head -n1; }
DISPLAY_NUM=""
start_display() {
	[[ -z $DISPLAY_NUM ]] || return 0
	XSERVER=$(nix_out xorg-server)
	DUMMY=$(nix_out xf86-video-dummy)
	OPENBOX=$(nix_out openbox)
	XDOTOOL=$(nix_out xdotool)/bin/xdotool
	DISPLAY_NUM=$(H free-display --first "$DISPLAY_FIRST")
	mkdir -p "$WORK/xorg.conf.d"
	# private/local-clients/weapon-camo-test/xorg-dummy.conf: a 60 Hz mode (Xvfb's 0 Hz crashes DXVK).
	cat >"$WORK/xorg.conf" <<'EOF'
Section "ServerFlags"
  Option "AutoAddDevices" "false"
  Option "DontVTSwitch" "true"
EndSection
Section "Device"
  Identifier "dummy"
  Driver "dummy"
  VideoRam 256000
EndSection
Section "Monitor"
  Identifier "mon"
  HorizSync 5.0 - 1000.0
  VertRefresh 5.0 - 200.0
  Modeline "1280x720_60" 74.50 1280 1344 1472 1664 720 723 728 748 -hsync +vsync
EndSection
Section "Screen"
  Identifier "scr"
  Device "dummy"
  Monitor "mon"
  DefaultDepth 24
  SubSection "Display"
    Depth 24
    Modes "1280x720_60"
  EndSubSection
EndSection
EOF
	log "starting Xorg (dummy, 60 Hz) on :$DISPLAY_NUM"
	launch xorg TERM "$WORK/logs/xorg.out" "$WORK" "$XSERVER/bin/Xorg" ":$DISPLAY_NUM" -ac -noreset -novtswitch \
		-nolisten tcp -config "$WORK/xorg.conf" -configdir "$WORK/xorg.conf.d" \
		-modulepath "$XSERVER/lib/xorg/modules,$DUMMY/lib/xorg/modules" -logfile "$WORK/logs/xorg.log"
	local xorg_pid=$LAST_PID i
	for ((i = 0; i < 200; i++)); do
		[[ -S /tmp/.X11-unix/X$DISPLAY_NUM ]] && break
		kill -0 "$xorg_pid" 2>/dev/null || die "Xorg exited: $WORK/logs/xorg.log"
		sleep 0.1
	done
	[[ -S /tmp/.X11-unix/X$DISPLAY_NUM ]] || die "Xorg did not open :$DISPLAY_NUM"
	launch openbox TERM "$WORK/logs/openbox.log" "$WORK" env DISPLAY=":$DISPLAY_NUM" "$OPENBOX/bin/openbox"
}

FARM=$WORK/game
farm() { # the game folder with our exes (harness.py game-farm)
	local args=() label
	for label in old new frames-old frames-new; do
		local var=EXE_${label//-/_}
		[[ -n ${!var:-} ]] && args+=("$label=${!var}")
	done
	H game-farm --game "$GAME_DIR" --out "$FARM" "${args[@]}"
}

prefix_for() { # who -> PREFIX: ours, made from the template once, its game logs cleared
	PREFIX=$WORK/prefixes/$1
	H prepare-prefix --template "$TEMPLATE" --prefix "$PREFIX" --resolution "$RESOLUTION"
	H clear-documents "$PREFIX"
	[[ " ${PREFIXES[*]} " == *" $PREFIX "* ]] || PREFIXES+=("$PREFIX")
	return 0
}

launch_game() { # label prefix exe logfile args...
	local label=$1 prefix=$2 exe=$3 logfile=$4
	shift 4
	launch "$label" TERM "$logfile" "$FARM/binaries/x86" env -u WAYLAND_DISPLAY -u XDG_SESSION_TYPE \
		"${CLIENT_ENV[@]}" DISPLAY=":$DISPLAY_NUM" WINEPREFIX="$prefix" WINEDEBUG=-all \
		XDG_CACHE_HOME="$WORK/cache" "$WINE" "$exe" "$@"
}

crash_reports() { compgen -G "$FARM/binaries/x86/*.zip" | sort || true; }

collect() { # prefix outdir before-reports [zipdir]: the game's logs to outdir, new crash reports to zipdir
	local prefix=$1 out=$2 before=$3 zips=${4:-$2} docs zip
	mkdir -p "$out/docs" "$zips"
	while read -r docs; do
		[[ -n $docs ]] && cp -a "$docs/." "$out/docs/" 2>/dev/null || true
	done < <(H documents "$prefix")
	rm -rf -- "$out/docs/streamed_textures"
	while read -r zip; do
		[[ -n $zip ]] && mv -- "$zip" "$zips/" || true
	done < <(comm -13 <(printf '%s\n' "$before") <(crash_reports))
	return 0
}

# ---- step 4: replays ----------------------------------------------------------------
SURS=() SUR_NAMES=()
prepare_replays() {
	local input i=0 name out sur modes=1
	case " ${REPLAYS[*]}" in *.srprec*) extract_game_modes || modes=0 ;; esac
	for input in "${REPLAYS[@]}"; do
		i=$((i + 1))
		name=$(basename -- "$input")
		name=${name%.*}
		case $input in
		*.sur)
			((DRY)) && { echo "  replay $name: $input as it is"; SURS+=("$input"); SUR_NAMES+=("$name"); continue; }
			mkdir -p "$WORK/sur"
			cp -- "$input" "$WORK/sur/r$i.sur"
			SURS+=("$WORK/sur/r$i.sur") SUR_NAMES+=("$name")
			;;
		*.srprec)
			if ((!modes)); then
				record "replay:$name" SKIPPED "no game modes to convert it (--game-modes DIR of gameplay/game_modes/*.options, or dbgrab failed: $WORK/logs/dbgrab*.log)"
				continue
			fi
			if ((DRY)); then
				echo "  replay $name: srp-recording-to-sur $input --game-modes <the options, prefix stripped>$( ((FIRST_KEY_FRAME_ONLY)) && echo ' --first-key-frame-only')"
				SURS+=("$input") SUR_NAMES+=("$name")
				continue
			fi
			sur_tool || { record "replay:$name" SKIPPED "srp-recording-to-sur could not be built: $WORK/logs/sur-tool.log"; continue; }
			out=$WORK/sur/r$i
			mkdir -p "$out"
			only=()
			((FIRST_KEY_FRAME_ONLY)) && only=(--first-key-frame-only)
			if ! "$SUR_TOOL" "$input" --game-modes "$WORK/game-modes" --out "$out" "${only[@]}" \
				>"$WORK/logs/sur-$name.log" 2>&1; then
				record "replay:$name" SKIPPED "conversion failed: $(tail -n1 "$WORK/logs/sur-$name.log")"
				continue
			fi
			sur=$(compgen -G "$out/*.sur" | head -n1 || true)
			[[ -n $sur ]] || { record "replay:$name" SKIPPED "the converter wrote no .sur"; continue; }
			mv -- "$sur" "$WORK/sur/r$i.sur"
			SURS+=("$WORK/sur/r$i.sur") SUR_NAMES+=("$name")
			;;
		*) record "replay:$name" SKIPPED "not a .srprec or .sur: $input" ;;
		esac
	done
}

sur_tool() {
	if [[ -z $SUR_TOOL ]]; then
		SUR_TOOL=$REPO/srp-026g/target/debug/srp-recording-to-sur
		if [[ ! -x $SUR_TOOL ]]; then
			log "building srp-recording-to-sur"
			(cd -- "$REPO/srp-026g" && nix develop "$REPO/srp-026g" --command cargo build -p match-server \
				--bin srp-recording-to-sur) >"$WORK/logs/sur-tool.log" 2>&1 || return 1
		fi
	fi
	[[ -x $SUR_TOOL ]] || return 1
	if [[ ! -d $WORK/game-modes ]]; then # the converter wants the files under their own names
		mkdir -p "$WORK/game-modes"
		local file
		for file in "$GAME_MODES"/*.options; do
			ln -sf -- "$file" "$WORK/game-modes/$(basename -- "${file#"$GAME_MODES"/gameplay__game_modes__}")"
		done
	fi
}

# Plays one replay with one exe; writes <out>/result (status words for compare).
play_replay() { # who exe sur out
	local who=$1 exe=$2 sur=$3 out=$4 prefix before pid start t=0 code=none survived=1 winpath
	owner_guard
	start_display
	mkdir -p "$out"
	prefix_for "$who"
	prefix=$PREFIX
	before=$(crash_reports)
	# shellcheck disable=SC1003 # tr's '\\' is one backslash
	winpath="Z:$(tr '/' '\\' <<<"$sur")"
	((${#winpath} <= 259)) || die "the replay's path is over 259 bytes (the game cuts it): $winpath"
	launch_game "replay-$who" "$prefix" "$exe" "$out/wine.log" -localization=english -mt_allocator=768 \
		-minimum_resources_memory_size=768 "-match_replay=$winpath"
	pid=$LAST_PID start=$(cut -d: -f2 <<<"${PIDS[-1]}")
	while ((t < REPLAY_SECONDS)); do
		sleep 5
		t=$((t + 5))
		if ! alive "$pid" "$start"; then
			survived=0
			wait "$pid" && code=0 || code=$?
			break
		fi
		if ((t % 30 == 0)) && ! owner_busy >/dev/null; then
			survived=interrupted
			break
		fi
	done
	wine_kill "$prefix"
	stop_label "replay-$who"
	collect "$prefix" "$out" "$before"
	printf 'survived=%s exit=%s seconds=%s\n' "$survived" "$code" "$t" >"$out/result"
}

replay_ok() { # out -> 0 when it ran to the end (or quit by itself with 0) and nothing crashed
	local out=$1 result
	result=$(cat "$out/result")
	[[ $result == *"survived=interrupted"* ]] && return 2
	[[ $result == *"survived=1"* || $result == *"exit=0 "* ]] || return 1
	H scan-client --wine-log "$out/wine.log" --game-log "$out"/docs/*.log \
		--crash-reports "$(compgen -G "$out/*.zip" | wc -l)" >"$out/scan" || return 1
}

step_replay() {
	local i name sur base added hashes
	if ((DRY)); then
		echo "step replay: ${#REPLAYS[@]} recording(s), ${REPLAY_SECONDS}s each with old and new"
		prepare_replays
		for name in "${SUR_NAMES[@]}"; do
			printf '  replay %s: wine survarium_old.exe -match_replay=Z:\\...\\rN.sur, then survarium_new.exe; compare\n' "$name"
			record "replay:$name:playback" PLANNED "old vs new, ${REPLAY_SECONDS}s each"
			record "replay:$name:hashes" SKIPPED "no exe writes per-tick world hashes in a replay ($HASH_CSV; README)"
		done
		return
	fi
	prepare_replays
	((${#SURS[@]})) || { record replay SKIPPED "no replay to play (none given and none convertible)"; return; }
	for i in "${!SURS[@]}"; do
		sur=${SURS[i]} name=${SUR_NAMES[i]} base=$WORK/replay/$name
		log "replay $name with the old exe"
		play_replay old survarium_old.exe "$sur" "$base/old"
		log "replay $name with the new exe"
		play_replay new survarium_new.exe "$sur" "$base/new"
		if replay_ok "$base/old"; then :; else
			case $? in
			2) record "replay:$name:playback" SKIPPED "the owner started a game; stopped" ;;
			*) record "replay:$name:playback" SKIPPED "the old exe fails on it too ($(cat "$base/old/result"); $(cat "$base/old/scan" 2>/dev/null)): no baseline" ;;
			esac
			continue
		fi
		if replay_ok "$base/new"; then
			if added=$(H error-kinds-new --old "$base"/old/docs/*.log --new "$base"/new/docs/*.log); then
				record "replay:$name:playback" PASS "both ran ($(cat "$base/new/result"))"
			else
				record "replay:$name:playback" FAIL "the new exe logs errors the old does not: $(head -n3 <<<"$added" | paste -sd';')"
			fi
		else
			[[ $? == 2 ]] && { record "replay:$name:playback" SKIPPED "the owner started a game; stopped"; continue; }
			record "replay:$name:playback" FAIL "the new exe: $(cat "$base/new/result"); $(cat "$base/new/scan" 2>/dev/null)"
		fi
		if [[ -f $base/old/docs/$HASH_CSV && -f $base/new/docs/$HASH_CSV ]]; then
			if hashes=$(H compare-hashes "$base/old/docs/$HASH_CSV" "$base/new/docs/$HASH_CSV"); then
				record "replay:$name:hashes" PASS "$hashes"
			else
				record "replay:$name:hashes" FAIL "$hashes"
			fi
		else
			record "replay:$name:hashes" SKIPPED "no per-tick world hashes from a replay: the game sends 0xDF only to a match server and no exe patch writes them to a file (README)"
		fi
	done
}

# ---- step 5: matches against bots ---------------------------------------------------------
wait_log() { # file regex seconds -> 0 when a line matches
	local i
	for ((i = 0; i < $3 * 2; i++)); do
		grep -qE -- "$2" "$1" 2>/dev/null && return 0
		sleep 0.5
	done
	return 1
}

find_window() { # pid -> the game's window on our display
	local win
	win=$(env DISPLAY=":$DISPLAY_NUM" "$XDOTOOL" search --onlyvisible --pid "$1" 2>/dev/null | tail -n1 || true)
	[[ -n $win ]] || win=$(env DISPLAY=":$DISPLAY_NUM" "$XDOTOOL" search --onlyvisible --name 'Survarium' 2>/dev/null | tail -n1 || true)
	echo "$win"
}

click() { # window "X,Y": the game reads relative motion only (HARNESS.txt): to the corner, then there
	local x=${2%,*} y=${2#*,}
	local X=(env DISPLAY=":$DISPLAY_NUM" "$XDOTOOL")
	"${X[@]}" windowactivate --sync "$1" >/dev/null 2>&1 || "${X[@]}" windowfocus --sync "$1" >/dev/null 2>&1 || true
	sleep 0.5
	"${X[@]}" mousemove_relative -- -5000 -5000
	sleep 0.3
	"${X[@]}" mousemove_relative -- "$x" "$y"
	sleep 0.3
	"${X[@]}" mousedown 1
	sleep 0.3
	"${X[@]}" mouseup 1
}

into_lobby() { # pid account serverlog
	local deadline=$(($(date +%s) + 240)) win
	while (($(date +%s) < deadline)); do
		win=$(find_window "$1")
		[[ -n $win ]] && click "$win" "$LOGIN_CLICK"
		wait_log "$3" " $2 \(account [0-9]+\) signed in to the lobby" 15 && return 0
	done
	return 1
}

press_play() { # pid account serverlog
	local deadline=$(($(date +%s) + 180)) win
	sleep 10 # the lobby's movie and the character
	while (($(date +%s) < deadline)); do
		win=$(find_window "$1")
		[[ -n $win ]] && click "$win" "$PLAY_CLICK"
		wait_log "$3" " $2 (pressed Play|queues at|goes to match)" 15 && return 0
	done
	return 1
}

bot_match() { # label mode map
	local label=$1 mode=$2 map=$3 dir=$WORK/bots/$1 ports login bots="" i who acct pw prefix pid
	local -a whos=(new) accounts=() pids=() starts=() senv=()
	[[ $BOT_CLIENTS == pair ]] && whos=(new old)
	for ((i = 1; i <= BOT_COUNT; i++)); do bots+=${bots:+,}bot-$i; done
	if ((DRY)); then
		echo "  match $label: srp SRP_MATCH_MODES=$mode SRP_MATCH_MAPS=$map SRP_BOT_ACCOUNTS=$bots (own ports, data, packet log)"
		echo "    srp-bot --count $BOT_COUNT --drive; clients: ${whos[*]} (Enter at $LOGIN_CLICK, Play at $PLAY_CLICK); ${BOT_MINUTES} min"
		record "bots:$label" PLANNED "mode $mode map $map, clients ${whos[*]}"
		return
	fi
	owner_guard
	start_display
	mkdir -p "$dir"
	ports=$(H free-ports --base "$PORT_BASE")
	login=${ports%%,*}
	mapfile -d '' senv < <(H server-env --config "$CONFIG" --work "$dir" --ports "$ports" \
		--set "SRP_MATCH_MODES=$mode" --set "SRP_MATCH_MAPS=$map" --set SRP_MATCH_GATHER_SECONDS=0 \
		--set SRP_MATCH_SEARCH_STEP_SECONDS=1 --set "SRP_BOT_ACCOUNTS=$bots")
	log "match $label: server on $ports, mode $mode, map $map"
	launch "srv-$label" INT "$dir/server.log" "$SERVER_WD" env -i "${senv[@]}" "$SERVER_BIN"
	for ((i = 0; i < 60; i++)); do
		(exec 3<>"/dev/tcp/127.0.0.1/$login") 2>/dev/null && break
		sleep 0.5
	done
	(exec 3<>"/dev/tcp/127.0.0.1/$login") 2>/dev/null || {
		stop_label "srv-$label"
		record "bots:$label" FAIL "the server did not open its login port: $dir/server.log"
		return
	}
	local before
	before=$(crash_reports)
	for who in "${whos[@]}"; do
		acct=PortReg${who^}
		pw=$(H password)
		prefix_for "$who"
		prefix=$PREFIX
		launch_game "cli-$label-$who" "$prefix" "survarium_$who.exe" "$dir/client-$who.log" "-account_name=$acct" \
			"-account_password=$pw" -localization=english "-server=127.0.0.1:$login" -mt_allocator=768 \
			-minimum_resources_memory_size=768 -splash_screen
		accounts+=("$acct") pids+=("$LAST_PID") starts+=("$(cut -d: -f2 <<<"${PIDS[-1]}")")
		sleep 20 # one window at a time: the input goes to the focused one
		if ! into_lobby "$LAST_PID" "$acct" "$dir/server.log"; then
			bot_match_end "$label" "$dir" "$before" "${whos[*]}"
			record "bots:$label" FAIL "$acct never signed in to the lobby (Enter at $LOGIN_CLICK; $dir/client-$who.log)"
			return
		fi
	done
	launch "bots-$label" TERM "$dir/bots.log" "$dir" "$BOT_BIN" --server "127.0.0.1:$login" --count "$BOT_COUNT" --drive
	for i in "${!whos[@]}"; do
		if ! press_play "${pids[i]}" "${accounts[i]}" "$dir/server.log"; then
			bot_match_end "$label" "$dir" "$before" "${whos[*]}"
			record "bots:$label" FAIL "${accounts[i]} never pressed Play (Play at $PLAY_CLICK)"
			return
		fi
	done
	for acct in "${accounts[@]}"; do
		if ! wait_log "$dir/server.log" " $acct goes to match 0x" 300; then
			bot_match_end "$label" "$dir" "$before" "${whos[*]}"
			record "bots:$label" FAIL "$acct was never sent to a match"
			return
		fi
	done
	if ! wait_log "$dir/server.log" "gave the initial world" 300; then
		bot_match_end "$label" "$dir" "$before" "${whos[*]}"
		record "bots:$label" FAIL "the match never started (no initial world)"
		return
	fi
	log "match $label: playing ${BOT_MINUTES} min"
	local t=0 died="" interrupted=0
	while ((t < BOT_MINUTES * 60)); do
		sleep 5
		t=$((t + 5))
		for i in "${!pids[@]}"; do
			alive "${pids[i]}" "${starts[i]}" || died+="${whos[i]} exited after ${t}s; "
		done
		[[ -z $died ]] || break
		if ((t % 30 == 0)) && ! owner_busy >/dev/null; then interrupted=1; break; fi
	done
	bot_match_end "$label" "$dir" "$before" "${whos[*]}"
	if ((interrupted)); then
		record "bots:$label" SKIPPED "the owner started a game; stopped after ${t}s"
		return
	fi
	local problems="" server scan df ids reports
	server=$(H scan-server "$dir/server.log") || problems+="server: $server; "
	for who in "${whos[@]}"; do
		scan=$(H scan-client --wine-log "$dir/client-$who.log" --game-log "$dir/$who"/docs/*.log) ||
			problems+="$who: $scan; "
	done
	reports=$(compgen -G "$dir/crash-reports/*.zip" | wc -l || true)
	((reports == 0)) || problems+="$reports crash report(s) in $dir/crash-reports; "
	[[ -z $died ]] || problems+="$died"
	ids=$({ grep -oE "PortReg(New|Old) goes to match 0x[0-9a-f]+" "$dir/server.log" || true; } |
		awk '{print $NF}' | sort -u | wc -l)
	if [[ $BOT_CLIENTS == pair ]]; then
		if ((ids != 1)); then
			record "bots:$label:hashes" SKIPPED "the two clients were not put in one match"
		elif df=$(H compare-df "$dir/packets"); then
			record "bots:$label:hashes" PASS "old and new agree tick by tick: $df"
		else
			record "bots:$label:hashes" FAIL "old and new differ: $df"
		fi
	fi
	if [[ -n $problems ]]; then
		record "bots:$label" FAIL "$problems"
	else
		record "bots:$label" PASS "${t}s of play; $server"
	fi
}

bot_match_end() { # label dir before-reports whos: stop everything of this match, keep the logs
	local label=$1 dir=$2 before=$3 who
	for who in $4; do wine_kill "$WORK/prefixes/$who"; done
	stop_label "cli-$label"
	stop_label "bots-$label"
	stop_label "srv-$label"
	for who in $4; do collect "$WORK/prefixes/$who" "$dir/$who" "$before" "$dir/crash-reports"; done
	return 0
}

step_bots() {
	local spec label mode map reason
	reason=$(missing_bots)
	[[ -z $reason ]] || { record bots SKIPPED "$reason"; return; }
	((DRY)) && echo "step bots: $BOT_MATCHES"
	IFS=, read -r -a specs <<<"$BOT_MATCHES"
	for spec in "${specs[@]}"; do
		IFS=: read -r label mode map <<<"$spec"
		[[ -n $label && -n $mode && -n $map ]] || { record "bots:$spec" SKIPPED "not label:mode:map"; continue; }
		bot_match "$label" "$mode" "$map"
	done
}

# ---- step 6: frame times -------------------------------------------------------------
step_frames() {
	local sur out verdict status detail
	if [[ -z ${EXE_frames_old:-} || -z ${EXE_frames_new:-} ]]; then
		record frames SKIPPED "no patch 71 frame-trace build of both trees: patch 71 is on feat/freeze-detector only (not main, not the port); pass --frames-old-exe/--frames-new-exe"
		return
	fi
	if ((DRY)); then
		echo "step frames: the first replay, ${REPLAY_SECONDS}s with each frame-trace exe; p50/p95/p99/max of frame_us (states 5,6)"
		record frames PLANNED "threshold $FRAMES_THRESHOLD"
		return
	fi
	((${#SURS[@]})) || prepare_replays
	((${#SURS[@]})) || { record frames SKIPPED "no replay to time (the workload is a replay)"; return; }
	sur=${SURS[0]} out=$WORK/frames
	play_replay frames-old survarium_frames-old.exe "$sur" "$out/old"
	play_replay frames-new survarium_frames-new.exe "$sur" "$out/new"
	if [[ ! -f $out/old/docs/srp_frames.csv || ! -f $out/new/docs/srp_frames.csv ]]; then
		record frames SKIPPED "a run wrote no srp_frames.csv (srp_trace 0, or the game did not start)"
		return
	fi
	verdict=$(H frames-compare "$out/old/docs/srp_frames.csv" "$out/new/docs/srp_frames.csv" --states 5,6 \
		--threshold "$FRAMES_THRESHOLD" --max-threshold "$FRAMES_MAX_THRESHOLD") || true
	IFS=$'\t' read -r status detail <<<"$verdict"
	record frames "${status:-FAIL}" "${detail:-no verdict}"
}

# ---- the run ----------------------------------------------------------------------
log "work: $WORK"
if ! reasons=$(owner_busy); then
	if ((DRY)); then log "note: busy now (a real run would refuse or --wait): $reasons"; else owner_guard; fi
fi
reason=$(missing_common)
[[ -z $reason ]] || die "$reason"

((DRY)) && echo "exes:"
EXE_old=$OLD_EXE EXE_new=$NEW_EXE
if [[ -z $EXE_old ]]; then EXE_old=$WORK/exe/old.exe; build_exe old "$OLD_REV" "$EXE_old"; fi
if [[ -z $EXE_new ]]; then EXE_new=$WORK/exe/new.exe; build_exe new "$NEW_REV" "$EXE_new"; fi
((DRY)) && echo "  old: $EXE_old" && echo "  new: $EXE_new"
if [[ -z $OLD_EXE && -z $NEW_EXE ]] &&
	[[ $(git -C "$REPO" rev-parse "$OLD_REV^{commit}") == $(git -C "$REPO" rev-parse "$NEW_REV^{commit}") ]]; then
	log "warning: $OLD_REV and $NEW_REV are the same commit: the comparison proves nothing about the port"
fi
EXE_frames_old=$FRAMES_OLD EXE_frames_new=$FRAMES_NEW
if ((!DRY)); then
	[[ -f $EXE_old && -f $EXE_new ]] || die "an exe is missing: $EXE_old $EXE_new"
	[[ -n $EXE_frames_old ]] || { H has-frames "$EXE_old" && EXE_frames_old=$EXE_old || true; }
	[[ -n $EXE_frames_new ]] || { H has-frames "$EXE_new" && EXE_frames_new=$EXE_new || true; }
	farm
fi
if ((DRY)); then
	echo "display: Xorg :<free from $DISPLAY_FIRST> (dummy driver, 1280x720 60 Hz, -nolisten tcp) + openbox; WAYLAND_DISPLAY/XDG_SESSION_TYPE unset"
	echo "prefixes: copies of $TEMPLATE under <work>/prefixes; game: $GAME_DIR linked into <work>/game"
	echo "busy check: $LC_STATE + every survarium*.exe process outside <work>"
fi

want replay && step_replay
want bots && step_bots
want frames && step_frames

echo
H summary "$RESULTS" && code=0 || code=$?
((DRY)) || log "logs: $WORK"
exit "$code"
