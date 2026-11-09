#!/usr/bin/env python3
"""Start isolated native 0.26g clients under Wine, optionally with a local server."""
from __future__ import annotations

import argparse
from contextlib import contextmanager
from dataclasses import dataclass
import fcntl
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import struct
import subprocess
import sys
import time
import uuid

DEFAULT_CONFIG = Path(__file__).resolve().parents[3] / "private/local-clients/config.json"
# The world corpus (srp-026g/docs/notes/packet-validation.md, "The world corpus"): the local
# server records every match into local/<the configuration's folder>/ there, so tools/check's
# world-check and world-drift check a feature's own test games too.
WORLD_CORPUS = Path(__file__).resolve().parents[3] / "private/world-corpus"


def recording_env(config: Config) -> dict[str, str]:
    """On by default; the configuration's server env may set "SRP_RECORD_DIR": "off"."""
    return {"SRP_RECORD_DIR": str(WORLD_CORPUS / "local" / config.path.parent.name),
            "SRP_RECORD_MIN_PLAYERS": "1", "SRP_RECORD_MAX_MB": "2000"}


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(".tmp")
    with temporary.open("w") as stream:
        os.chmod(temporary, 0o600)
        json.dump(value, stream, indent=2)
        stream.write("\n")
    temporary.replace(path)


@dataclass(frozen=True)
class Account:
    name: str
    password: str
    executable: str = "survarium_skhid.exe"

    @classmethod
    def parse(cls, data: dict) -> Account:
        name, password = data["name"], data["password"]
        if not isinstance(name, str) or not name.strip() or len(name.encode()) > 31:
            raise ValueError("account name must contain 1–31 UTF-8 bytes")
        if not isinstance(password, str) or not 1 <= len(password.encode()) <= 47:
            raise ValueError("password must contain 1–47 UTF-8 bytes")
        if any(c in name + password for c in "\0\r\n"):
            raise ValueError("credentials cannot contain NUL or line breaks")
        executable = data.get("executable", "survarium_skhid.exe")
        if (not isinstance(executable, str) or not executable.lower().endswith(".exe")
                or any(c in executable for c in "/\\:\0\r\n")):
            raise ValueError("executable must be an .exe filename inside binaries/x86")
        return cls(name, password, executable)


@dataclass(frozen=True)
class Config:
    path: Path
    identity: str
    runtime: Path
    game: Path
    wine: Path
    wineserver: Path
    template: Path
    accounts: tuple[Account, ...]
    ports: tuple[int, int, int, int]
    resolution: str
    client_env: dict[str, str]
    server: dict | None
    # Wine's virtual desktop around each window. Off: under some Wayland
    # sessions (niri, 2026-10-16) the game exits inside it and only a blue
    # desktop stays, while started directly it runs.
    virtual_desktop: bool = False
    # -localization=<this> at every start; null: none, so the game's own
    # choice (Settings, user.cfg, the Windows locale) is used.
    localization: str | None = "english"
    # r_fullscreen at every start. Under Wine's Wayland driver (niri) a
    # window never captures the mouse; fullscreen at the monitor's own
    # resolution does (owner, 2026-11-05), so the owner's tests use it.
    fullscreen: bool = False

    @classmethod
    def read(cls, path: Path) -> Config:
        path = path.resolve()
        data = json.loads(path.read_text())
        resolve = lambda value: (path.parent / value).resolve()
        accounts = tuple(Account.parse(a) for a in data["accounts"])
        if not 1 <= len(accounts) <= 16 or len({a.name.casefold() for a in accounts}) != len(accounts):
            raise ValueError("configure 1–16 accounts with distinct names")
        ports = tuple(data.get("ports", [1234, 1235, 1236, 8088]))
        if len(ports) != 4 or any(type(p) is not int or not 1024 <= p <= 65535 for p in ports) or len(set(ports)) != 4:
            raise ValueError("ports must be four distinct integers in 1024–65535 (login/lobby/match/web)")
        dimensions = data.get("resolution", "1280x720").split("x")
        if len(dimensions) != 2 or not all(v.isdecimal() and 640 <= int(v) <= 7680 for v in dimensions):
            raise ValueError("resolution must be WIDTHxHEIGHT, dimensions 640–7680")
        client_env = data.get("client_env", {})
        virtual_desktop = data.get("virtual_desktop", False)
        if type(virtual_desktop) is not bool:
            raise ValueError("virtual_desktop must be true or false")
        localization = data.get("localization", "english")
        if localization is not None and (type(localization) is not str or not localization.isalpha()):
            raise ValueError("localization must be a language name or null")
        fullscreen = data.get("fullscreen", False)
        if type(fullscreen) is not bool:
            raise ValueError("fullscreen must be true or false")
        server = data.get("server")
        for values in [client_env, server.get("env", {}) if server else {}]:
            if not isinstance(values, dict) or not all(isinstance(k, str) and isinstance(v, str) for k, v in values.items()):
                raise ValueError("environment overrides must map strings to strings")
        if server:
            server = dict(server, binary=resolve(server["binary"]), working_dir=resolve(server["working_dir"]))
        identity = str(uuid.UUID(data["id"]))
        runtime = path.parent / "runtime"
        template = resolve(data["prefix_template"])
        if template == runtime or runtime in template.parents or template in runtime.parents:
            raise ValueError("prefix template and managed runtime must not contain one another")
        return cls(path, identity, runtime, resolve(data["game_dir"]), resolve(data["wine"]),
                   resolve(data["wineserver"]), template, accounts, ports, "x".join(dimensions), client_env, server,
                   virtual_desktop, localization, fullscreen)

    def prefix(self, index: int) -> Path:
        return self.runtime / f"client-{index + 1}"

    def client_command(self, index: int) -> list[str]:
        account = self.accounts[index]
        game = [account.executable, f"-account_name={account.name}", f"-account_password={account.password}",
                f"-server=127.0.0.1:{self.ports[0]}",
                "-minimum_resources_memory_size=768", "-splash_screen"]
        if self.localization:
            game.insert(3, f"-localization={self.localization}")
        if self.virtual_desktop:
            return [str(self.wine), "explorer", f"/desktop=SRP-{index + 1},{self.resolution}", *game]
        return [str(self.wine), *game]

    def client_environment(self, index: int) -> dict[str, str]:
        # run.py's own environment goes on to the clients: SKHID_MEMORY_PROFILE=on there
        # profiles them (patch 127, docs/notes/memory-profile.md); unset, nothing is set.
        return dict(os.environ, **(self.client_env | {
            "WINEPREFIX": str(self.prefix(index)), "WINEDEBUG": "-all",
            # Full-memory crash dumps (client/bugtrap/README.md, "Dump contents").
            "SKHID_CRASH_DUMP": "full",
        }))


def process_start(pid: int) -> str | None:
    try:
        fields = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()
        return None if fields[0] == "Z" else fields[19]
    except (FileNotFoundError, ProcessLookupError):
        return None


def alive(record: dict) -> bool:
    return process_start(record["pid"]) == record["started"] and record["started"] is not None


def launch(command: list[str], cwd: Path, env: dict, log: Path) -> dict:
    with log.open("ab") as stream:
        os.chmod(log, 0o600)
        child = subprocess.Popen(command, cwd=cwd, env=env, stdout=stream, stderr=subprocess.STDOUT,
                                 stdin=subprocess.DEVNULL, start_new_session=True)
    return {"pid": child.pid, "started": process_start(child.pid)}


@contextmanager
def locked(config: Config):
    config.runtime.mkdir(parents=True, exist_ok=True, mode=0o700)
    with (config.runtime / "launcher.lock").open("w") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        yield


def state_path(config: Config) -> Path:
    return config.runtime / "state.json"


def read_state(config: Config) -> dict:
    path = state_path(config)
    state = json.loads(path.read_text()) if path.exists() else {"id": config.identity, "clients": {}}
    if state["id"] != config.identity:
        raise ValueError("runtime belongs to another configuration")
    return state


def prepare_prefix(config: Config, index: int) -> None:
    prefix = config.prefix(index)
    marker = prefix / ".srp-local-client"
    if prefix.exists():
        if not marker.exists() or marker.read_text() != config.identity:
            raise ValueError(f"refusing to use an unmanaged Wine prefix: {prefix}")
    else:
        staging = config.runtime / f"preparing-{uuid.uuid4()}"
        try:
            subprocess.run(["cp", "-a", "--reflink=auto", str(config.template), str(staging)], check=True,
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            (staging / ".srp-local-client").write_text(config.identity)
            staging.rename(prefix)
        finally:
            if staging.exists():
                shutil.rmtree(staging)
    # Keep Documents in the clone even when the template redirects it outside Wine.
    for user in (prefix / "drive_c/users").iterdir():
        if not user.is_dir() or user.name in ("Public", "All Users"):
            continue
        for name in ("Documents", "My Documents"):
            documents = user / name
            if documents.is_symlink():
                documents.unlink()
            documents.mkdir(exist_ok=True)
            game_settings = documents / "Survarium"
            game_settings.mkdir(exist_ok=True)
            cfg = game_settings / "user.cfg"
            lines = cfg.read_text().splitlines() if cfg.exists() else []
            # account_digest is machine-key XORed, NOT plain SHA-1. Always
            # prefill through the native account_name/account_password options.
            managed = {"account_name", "account_password", "account_digest", "r_fullscreen", "r_resolution", "store_user_password"}
            lines = [line for line in lines if (line.split(maxsplit=1) or [""])[0] not in managed]
            cfg.write_text("\n".join(lines + ["store_user_password on", f"r_fullscreen {'on' if config.fullscreen else 'off'}",
                                     f"r_resolution {config.resolution}"]) + "\n")


def preflight(config: Config, with_server: bool) -> None:
    required = [config.wine, config.wineserver, config.template / "system.reg",
                config.game / "resources.db",
                config.game / "resources/ssl/survarium_login_server.crt"]
    required += [config.game / "binaries/x86" / account.executable for account in config.accounts]
    if with_server:
        if config.server is None:
            raise ValueError("configure a server or use --clients-only")
        required += [config.server["binary"], config.server["working_dir"] / "certs/survarium_login_server.crt"]
    for path in required:
        if not path.is_file():
            raise ValueError(f"missing required file: {path}")
    for index, port in enumerate(config.ports if with_server else []):
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM if index in (1, 2) else socket.SOCK_STREAM) as sock:
            if index not in (1, 2):
                # Match the server's restart semantics: old TCP TIME_WAIT
                # connections do not mean another listener owns the port.
                sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            sock.bind(("127.0.0.1", port))


# Where client-patches/patch_026g.py writes the patch revision the game signs in with
# (REVISION_VA, revision_site: `mov byte [esp+27h], <revision>` / `nop`), and the original's bytes.
REVISION_VA = 0x380862
REVISION_ORIGINAL = bytes.fromhex("8a0d9eae9d00")


def exe_revision(exe: Path) -> int | None:
    """The patch revision a 0.26g game exe signs in with; None for the unpatched original."""
    data = exe.read_bytes()
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    if data[pe:pe + 4] != b"PE\0\0":
        raise ValueError(f"{exe}: not a PE executable")
    sections, optional_size = struct.unpack_from("<H", data, pe + 6)[0], struct.unpack_from("<H", data, pe + 20)[0]
    image_base = struct.unpack_from("<I", data, pe + 24 + 28)[0]
    rva = REVISION_VA - image_base
    for at in range(pe + 24 + optional_size, pe + 24 + optional_size + 40 * sections, 40):
        virtual_size, virtual_address, raw_size, raw_offset = struct.unpack_from("<IIII", data, at + 8)
        if virtual_address <= rva < virtual_address + max(virtual_size, raw_size):
            site = data[raw_offset + rva - virtual_address:][:6]
            if site == REVISION_ORIGINAL:
                return None
            if site[:4] == bytes.fromhex("c6442427") and site[5] == 0x90:
                return site[4]
            raise ValueError(f"{exe}: no 0.26g sign-in revision at {REVISION_VA:#x}: another build?")
    raise ValueError(f"{exe}: {REVISION_VA:#x} is in no section")


def check_revisions(config: Config, count: int) -> list[tuple[Account, int | None]]:
    """Each client's exe and revision; refused before anything starts when one is older than the
    server's SRP_MIN_PATCH_REVISION (the login server would turn it away only at its sign-in, after
    the owner clicked Enter: a configuration whose accounts point at another test's exe)."""
    raw = (config.server or {}).get("env", {}).get("SRP_MIN_PATCH_REVISION")
    least = int(raw) if raw is not None else None
    found = [(account, exe_revision(config.game / "binaries/x86" / account.executable))
             for account in config.accounts[:count]]
    stale = [f"{account.name} plays {account.executable} (patch revision {revision})"
             for account, revision in found if least is not None and (revision or 0) < least]
    if stale:
        raise ValueError("the server takes patch revision " + str(least) + " and later, but "
                         + "; ".join(stale) + ": point each account's executable at a revision "
                         + str(least) + " build")
    return found


def stop(config: Config, state: dict) -> None:
    for key in list(state["clients"]):
        index = int(key)
        marker = config.prefix(index) / ".srp-local-client"
        if not marker.exists() or marker.read_text() != config.identity:
            raise ValueError("refusing to stop an unmanaged Wine prefix")
        # Wine returns 1 when the profile has already stopped. The wait below
        # still verifies that its server has exited before clearing ownership.
        killed = subprocess.run([str(config.wineserver), "-k"], env=config.client_environment(index), check=False,
                                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=10)
        if killed.returncode not in (0, 1):
            killed.check_returncode()
        subprocess.run([str(config.wineserver), "-w"], env=config.client_environment(index), check=True,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=10)
        del state["clients"][key]
        write_json(state_path(config), state)
    server = state.get("server")
    if server and alive(server):
        os.kill(server["pid"], signal.SIGINT)
        deadline = time.monotonic() + 10
        while alive(server) and time.monotonic() < deadline:
            time.sleep(.1)
        if alive(server):
            raise RuntimeError("local server did not stop; left state intact for another stop attempt")
    state.pop("server", None)
    write_json(state_path(config), state)


def start(config: Config, state: dict, count: int, clients_only: bool) -> None:
    if not 1 <= count <= len(config.accounts):
        raise ValueError(f"count must be between 1 and {len(config.accounts)}")
    if state["clients"] or state.get("server"):
        raise ValueError("a run is already recorded; use restart or stop first")
    preflight(config, not clients_only)
    revisions = check_revisions(config, count)
    for index in range(count):
        prepare_prefix(config, index)
    try:
        if not clients_only:
            server = config.server
            env = {**os.environ, **recording_env(config), **server.get("env", {})}
            # Fixed loopback settings win over inherited shell/config values.
            env.update(SRP_DATA_DIR=str(config.path.parent / "data"), SRP_DB="on", SRP_REGISTRATION="open",
                       SRP_REGISTRATIONS_PER_DAY="100", SRP_TUTORIAL="on", SRP_LOG_FILES="off",
                       SRP_BIND_HOST="127.0.0.1", SRP_PUBLIC_HOST="127.0.0.1", SRP_WEB_BIND="127.0.0.1",
                       SRP_LOCAL_PEERS="on", SRP_PACKET_LOG=str(config.runtime / "packets"),
                       SRP_SNAPSHOT_DIR=str(config.runtime / "snapshots"))
            for service, port in zip(("LOGIN", "LOBBY", "MATCH", "WEB"), config.ports):
                env[f"SRP_{service}_PORT"] = str(port)
                env[f"SRP_{service}_BIND"] = "127.0.0.1"
                env[f"SRP_{service}_PUBLIC_HOST"] = "127.0.0.1"
            state["server"] = launch([str(server["binary"])], server["working_dir"], env, config.runtime / "server.log")
            write_json(state_path(config), state)
            deadline = time.monotonic() + 10
            while True:
                if not alive(state["server"]):
                    raise RuntimeError("server exited; see runtime/server.log")
                try:
                    with socket.create_connection(("127.0.0.1", config.ports[0]), timeout=.2):
                        break
                except OSError:
                    if time.monotonic() >= deadline:
                        raise RuntimeError("server did not open its login port; see runtime/server.log")
                    time.sleep(.1)
        else:
            with socket.create_connection(("127.0.0.1", config.ports[0]), timeout=2):
                pass
        for index in range(count):
            # Record prefix ownership before spawning so partial starts can be stopped.
            state["clients"][str(index)] = {}
            write_json(state_path(config), state)
            state["clients"][str(index)] = launch(config.client_command(index), config.game / "binaries/x86",
                                                 config.client_environment(index), config.runtime / f"client-{index + 1}.log")
            write_json(state_path(config), state)
            account, revision = revisions[index]
            print(f"Client {index + 1}: {account.name} (login prefilled), {account.executable}, "
                  + (f"patch revision {revision}" if revision is not None else "the unpatched original"))
        time.sleep(1)
        if any(not alive(record) for record in state["clients"].values()):
            raise RuntimeError("a client launcher exited early; see runtime/client-N.log")
    except BaseException:
        stop(config, state)
        raise
    print(f"Local login: 127.0.0.1:{config.ports[0]}; logs: {config.runtime}")


def initialize(args) -> None:
    if args.config.exists():
        raise ValueError("configuration already exists; edit it to add accounts or change paths")
    wine = Path(args.wine or shutil.which("wine") or "wine").resolve()
    server = None if not args.server_bin else {"binary": str(Path(args.server_bin).resolve()),
                                              "working_dir": str(Path(args.server_workdir).resolve()), "env": {}}
    if not 1 <= args.accounts <= 16:
        raise ValueError("accounts must be between 1 and 16")
    data = {"id": str(uuid.uuid4()), "game_dir": str(Path(args.game_dir).resolve()),
            "wine": str(wine), "wineserver": str(wine.with_name("wineserver")),
            "prefix_template": str(Path(args.prefix_template).resolve()), "resolution": "1280x720",
            "ports": [1234, 1235, 1236, 8088], "server": server,
            "client_env": {"WINEDLLOVERRIDES": "mscoree,mshtml,qwave=;winemenubuilder.exe=d;d3d11,dxgi=n,b"},
            "accounts": [{"name": f"Local{i + 1}", "password": "LocalTest123"} for i in range(args.accounts)]}
    write_json(args.config, data)
    print(f"Created {args.config}. Edit paths/accounts there; passwords are prefilled at launch.")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", type=Path, default=DEFAULT_CONFIG)
    sub = parser.add_subparsers(dest="command", required=True)
    init = sub.add_parser("init", help="write a private local configuration")
    init.add_argument("--game-dir", required=True, help="game directory containing binaries/ and resources.db")
    init.add_argument("--prefix-template", required=True, help="inactive Wine prefix with working DXVK")
    init.add_argument("--wine")
    init.add_argument("--server-bin")
    init.add_argument("--server-workdir", default=str(Path(__file__).resolve().parents[2]))
    init.add_argument("--accounts", type=int, default=4)
    for command in ("start", "restart"):
        action = sub.add_parser(command)
        action.add_argument("--count", type=int, default=2)
        action.add_argument("--clients-only", action="store_true", help="use an existing local server")
    sub.add_parser("stop")
    sub.add_parser("status")
    args = parser.parse_args()
    try:
        if args.command == "init":
            initialize(args)
            return 0
        config = Config.read(args.config)
        with locked(config):
            state = read_state(config)
            if args.command == "status":
                print("Server:", "running" if state.get("server") and alive(state["server"]) else "not managed or stopped")
                for key, record in state["clients"].items():
                    print(f"Client {int(key) + 1}:", "running" if record and alive(record) else "launcher exited (stop clears prefix)")
            elif args.command == "stop":
                stop(config, state)
                print("Stopped managed clients and server. Accounts and profiles retained.")
            else:
                if args.command == "restart":
                    stop(config, state)
                start(config, state, args.count, args.clients_only)
        return 0
    except (OSError, ValueError, KeyError, TypeError, RuntimeError, subprocess.SubprocessError) as error:
        # Do not print CalledProcessError: its argv contains the test password.
        detail = "subprocess failed; inspect runtime logs" if isinstance(error, subprocess.SubprocessError) else str(error)
        print(f"Error: {detail}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
