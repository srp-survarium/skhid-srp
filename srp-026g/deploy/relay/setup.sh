#!/usr/bin/env bash
# On Debian/Ubuntu, as root. Does not alter or restart the game server.
# prepare: install dependencies and generate a host-local key; print PUBLIC key.
# configure relay|origin <peer-public-key> <relay-public-ip>: install tunnel,
# firewall and (relay only) forwarder service. The forwarder is left stopped.
set -euo pipefail
cd -- "$(dirname -- "$0")"
[[ $(id -u) == 0 ]] || { echo 'run as root' >&2; exit 1; }
case ${1:-} in
prepare)
    apt-get update -qq
    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends wireguard-tools nftables python3
    install -d -m 700 /etc/wireguard /etc/skhid-relay
    if [[ ! -f /etc/wireguard/skhid-relay.key ]]; then
        (umask 077; wg genkey > /etc/wireguard/skhid-relay.key)
    fi
    wg pubkey < /etc/wireguard/skhid-relay.key
    ;;
configure)
    (($# == 4)) || { echo 'configure relay|origin peer-public-key relay-public-ip' >&2; exit 1; }
    role=$2 peer=$3 public=$4
    [[ $role == relay || $role == origin ]] || exit 1
    [[ $peer =~ ^[A-Za-z0-9+/]{43}=$ ]] || { echo 'invalid public key' >&2; exit 1; }
    python3 -c 'import ipaddress,sys; ipaddress.IPv4Address(sys.argv[1])' "$public"
    [[ -f /etc/wireguard/skhid-relay.key ]] || { echo 'run prepare first' >&2; exit 1; }
    if [[ $role == relay ]]; then own=10.77.0.2 other=10.77.0.1; else own=10.77.0.1 other=10.77.0.2; fi
    umask 077
    # Own table only. Atomic replacement; no flush of Docker/host firewall rules.
    {
        echo 'add table inet skhid_relay'
        echo 'flush table inet skhid_relay'
        echo 'add chain inet skhid_relay ingress { type filter hook prerouting priority -310; policy accept; }'
        echo "add rule inet skhid_relay ingress ip saddr $other iifname != \"skhid-relay\" drop"
        echo "add rule inet skhid_relay ingress iifname \"skhid-relay\" ip saddr != $other drop"
        echo "add rule inet skhid_relay ingress iifname \"skhid-relay\" ip daddr != $own drop"
        echo 'add rule inet skhid_relay ingress iifname "skhid-relay" ip protocol icmp accept'
        if [[ $role == origin ]]; then
            echo 'add rule inet skhid_relay ingress iifname "skhid-relay" tcp dport { 443, 1234 } accept'
            echo 'add rule inet skhid_relay ingress iifname "skhid-relay" udp dport 1235-1237 accept'
        else
            # Responses to this host's connected sockets only, never new
            # connections from the origin into SSH or the public forwarder.
            # conntrack runs AFTER raw prerouting, so source port + destination
            # ephemeral port are bounded here; WG authenticates the origin.
            echo 'add rule inet skhid_relay ingress iifname "skhid-relay" tcp sport { 443, 1234 } tcp dport 1024-65535 accept'
            echo 'add rule inet skhid_relay ingress iifname "skhid-relay" udp sport 1235-1237 udp dport 1024-65535 accept'
        fi
        echo 'add rule inet skhid_relay ingress iifname "skhid-relay" drop'
    } > /etc/skhid-relay/tunnel.nft
    nft --check --file /etc/skhid-relay/tunnel.nft
    {
        echo '[Interface]'
        printf 'Address = %s/32\n' "$own"
        printf 'PrivateKey = %s\n' "$(cat /etc/wireguard/skhid-relay.key)"
        [[ $role != relay ]] || echo 'ListenPort = 51820'
        echo 'MTU = 1380'
        echo 'PreUp = nft --file /etc/skhid-relay/tunnel.nft'
        echo '[Peer]'
        printf 'PublicKey = %s\nAllowedIPs = %s/32\n' "$peer" "$other"
        if [[ $role == origin ]]; then
            printf 'Endpoint = %s:51820\nPersistentKeepalive = 25\n' "$public"
        fi
    } > /etc/wireguard/skhid-relay.conf
    if [[ $role == relay ]]; then
        install -d -m 755 /opt/skhid-relay
        install -m 644 forwarder.py /opt/skhid-relay/forwarder.py
        printf 'RELAY_PUBLIC=%s\nRELAY_BACKEND=10.77.0.1\n' "$public" > /etc/skhid-relay/relay.env
        install -m 644 relay.service /etc/systemd/system/skhid-relay.service
        install -m 644 relay-web.service /etc/systemd/system/skhid-relay-web.service
        systemctl daemon-reload
    fi
    # Refuse to bounce an existing tunnel; configuration changes need an
    # explicit planned restart and a fresh end-to-end check.
    if systemctl is-active --quiet wg-quick@skhid-relay; then
        # One transaction of this table alone: no packet meets half of the rules.
        nft --file /etc/skhid-relay/tunnel.nft
        echo 'tunnel already running: firewall reloaded, tunnel config written for its next planned restart'
    else
        systemctl enable --now wg-quick@skhid-relay
    fi
    echo 'tunnel configured; forwarder remains stopped until backend checks pass'
    ;;
*) echo 'usage: setup.sh prepare | configure relay|origin peer-public-key relay-public-ip' >&2; exit 1 ;;
esac
