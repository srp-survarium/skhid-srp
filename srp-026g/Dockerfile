# syntax=docker/dockerfile:1
# srp for the Survarium 0.26g client (login TCP, lobby UDP, match UDP and the
# web page in one process), as one small non-root image.
#   docker build -t srp-026g .
# Run it through docker-compose.yml (see docs/DEPLOY.md).
# It is made to run beside the live 0.21d server without touching it: its own
# image name, compose project, volume and ports.

FROM rust:1-slim-bookworm AS build
RUN apt-get update \
 && apt-get install -y --no-install-recommends pkg-config libssl-dev build-essential \
 && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY servers servers
COPY scripts scripts
COPY resources/binary-config-examples resources/binary-config-examples
COPY resources/game-026g resources/game-026g
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p srp \
 && cp target/release/srp /srp

FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends libssl3 ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --no-create-home --shell /usr/sbin/nologin srp \
 && mkdir -p /srv/srp/data \
 && chown srp /srv/srp/data
WORKDIR /srv/srp
COPY --from=build /srp /usr/local/bin/srp
# The login server's key and certificate (srp is run from the folder that
# holds ./certs). The certificate is the game's own, issued again with the
# same key because the original ran out in 2014: every player's client must
# have this very file as resources/ssl/survarium_login_server.crt
# (client-patches/make_client_026g.py puts it there). They are in
# the repository, so they are not secrets.
COPY certs ./certs

USER srp
# The ports the client is told are the ports srp binds (the login answer
# carries the lobby's port, the lobby's Play answer the match server's), so a
# published port must be the same number inside and outside: the compose
# file sets SRP_LOGIN_PORT / SRP_LOBBY_PORT / SRP_MATCH_PORT and publishes
# those. The defaults here are the game's own.
ENV SRP_BIND_HOST=0.0.0.0 \
    SRP_LOGIN_PORT=1234 \
    SRP_LOBBY_PORT=1235 \
    SRP_MATCH_PORT=1236 \
    SRP_LOCAL_PEERS=off \
    SRP_WEB_BIND=0.0.0.0 \
    SRP_WEB_PORT=8088 \
    SRP_LOG_FILES=off \
    SRP_SNAPSHOT_DIR=off \
    SRP_DATA_DIR=/srv/srp/data
VOLUME /srv/srp/data
# Login (TLS over TCP), lobby (UDP), match (UDP), voice (UDP, the match port
# + 1, SRP_VOICE), web page (HTTP).
EXPOSE 1234/tcp 1235/udp 1236/udp 1237/udp 8088/tcp
STOPSIGNAL SIGTERM
ENTRYPOINT ["srp"]
