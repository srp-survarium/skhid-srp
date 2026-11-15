/* skhid-dht: the release's BEP 46 item put on the DHT, or read back, with
   the updater's own libtorrent session (skt.h). Built for the host by
   client/updater/torrent-dll/default.nix; release/publish-dht runs it. It
   holds no private key: the item comes signed.

     skhid-dht put <key hex> <salt> <seq> <value hex> <signature hex> [options]
     skhid-dht get <key hex> <salt> [options]       prints "<seq> <value hex>"
   options: --bootstrap host:port,...  --loopback  --seconds <n> (default 120)

   put: exit 0 once some nodes stored it (and says how many), 1 when none
   did in time. get: exit 0 with the highest seq that came, 1 when none. */
#include "skt.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static int unhex(const char *text, uint8_t *out, size_t max) {
    size_t n = strlen(text);
    if (n % 2 || n / 2 > max) return -1;
    for (size_t i = 0; i < n / 2; ++i) {
        unsigned v;
        if (sscanf(text + 2 * i, "%2x", &v) != 1) return -1;
        out[i] = (uint8_t)v;
    }
    return (int)(n / 2);
}

static void print_hex(const uint8_t *data, int n) {
    for (int i = 0; i < n; ++i) printf("%02x", data[i]);
}

int main(int argc, char **argv) {
    if (argc < 4) {
        fprintf(stderr, "usage: skhid-dht put <key> <salt> <seq> <value> <signature> | get <key> <salt> [--bootstrap ..] [--loopback] [--seconds n]\n");
        return 2;
    }
    int put = strcmp(argv[1], "put") == 0;
    int first_option = put ? 7 : 4;
    if (argc < first_option || (!put && strcmp(argv[1], "get") != 0)) {
        fprintf(stderr, "skhid-dht: put or get, and their arguments\n");
        return 2;
    }
    skt_config config = {0};
    config.dht_bootstrap = "dht.libtorrent.org:25401,router.bittorrent.com:6881,router.utorrent.com:6881,dht.transmissionbt.com:6881";
    long seconds = 120;
    for (int i = first_option; i < argc; ++i) {
        if (strcmp(argv[i], "--bootstrap") == 0 && i + 1 < argc) config.dht_bootstrap = argv[++i];
        else if (strcmp(argv[i], "--loopback") == 0) config.loopback = 1;
        else if (strcmp(argv[i], "--seconds") == 0 && i + 1 < argc) seconds = atol(argv[++i]);
        else {
            fprintf(stderr, "skhid-dht: %s?\n", argv[i]);
            return 2;
        }
    }
    uint8_t key[32], sig[64], value[1000];
    const char *salt = argv[3];
    int value_len = 0;
    long long seq = 0;
    if (unhex(argv[2], key, 32) != 32) {
        fprintf(stderr, "skhid-dht: the key is 64 hex digits\n");
        return 2;
    }
    if (put) {
        seq = atoll(argv[4]);
        value_len = unhex(argv[5], value, sizeof value);
        if (value_len <= 0 || unhex(argv[6], sig, 64) != 64) {
            fprintf(stderr, "skhid-dht: the value (hex) and the signature (128 hex digits)\n");
            return 2;
        }
    }
    char err[160] = {0};
    skt_session *s = skt_session_new(&config, err, sizeof err);
    if (!s) {
        fprintf(stderr, "skhid-dht: %s\n", err);
        return 1;
    }
    static skt_event events[32];
    time_t until = time(NULL) + seconds;
    time_t asked = 0;
    int code = 1;
    long long best = -1;
    while (time(NULL) < until) {
        /* Again every 10 s: the DHT is found meanwhile. */
        if (time(NULL) - asked >= 10) {
            asked = time(NULL);
            if (put) skt_dht_put(s, key, sig, seq, (const uint8_t *)salt, strlen(salt), value, (size_t)value_len);
            else skt_dht_get(s, key, (const uint8_t *)salt, strlen(salt));
        }
        int n = skt_poll(s, events, 32, 500);
        for (int i = 0; i < n; ++i) {
            skt_event *e = &events[i];
            if (put && e->type == SKT_EV_DHT_PUT && e->a > 0) {
                printf("stored on %lld nodes\n", (long long)e->a);
                code = 0;
            } else if (!put && e->type == SKT_EV_DHT_ITEM && e->len > 0 && e->a > best) {
                best = e->a;
                value_len = e->len;
                memcpy(value, e->data, (size_t)e->len);
                code = 0;
            }
        }
        if (code == 0 && put) break;
        /* A get waits a little for higher ones after the first. */
        if (code == 0 && !put && until - time(NULL) > 5) until = time(NULL) + 5;
    }
    if (!put && code == 0) {
        printf("%lld ", best);
        print_hex(value, value_len);
        printf("\n");
    }
    skt_session_free(s);
    if (code != 0) fprintf(stderr, "skhid-dht: %s in %ld s\n", put ? "stored on no node" : "no item", seconds);
    return code;
}
