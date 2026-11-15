/* skt.h: the C API of skhid_torrent.dll, libtorrent-rasterbar 2.0 behind it
   (client/updater/README.md, "BitTorrent"). The updater loads the DLL at
   run time (LoadLibraryW, GetProcAddress): exe/src/libtorrent.rs is its one
   caller. All text is UTF-8; every call is made from one thread at a time
   (the updater's: libtorrent's own threads stay inside); no call throws;
   what comes back is copied into the caller's buffers. */
#ifndef SKT_H
#define SKT_H
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
#if !defined(_WIN32)
#define SKT_API __attribute__((visibility("default")))
#define SKT_CALL
#elif defined(SKT_BUILD)
#define SKT_API __declspec(dllexport)
#define SKT_CALL __cdecl
#else
#define SKT_API __declspec(dllimport)
#define SKT_CALL __cdecl
#endif

typedef struct skt_session skt_session;

typedef struct {
    uint16_t listen_port;          /* 0: the system picks one */
    int32_t upload_limit;          /* bytes a second, every peer (local ones too) */
    const char *dht_bootstrap;     /* "host:port,host:port" */
    int32_t loopback;              /* tests: 127.0.0.1 only, peers of one address, DHT nodes on it */
} skt_config;

/* One file of a torrent as this run has it: `name` its path in the torrent
   ('/' between the parts, without the torrent's name), `path` where it is
   (absolute). fetch 1: fetched into `path`, what is there checked first;
   0: shared from `path`, never written. trusted 1: its every piece was
   checked before and it has not changed since (the caller's record): not
   hashed again. A file of the torrent not listed is neither. */
typedef struct {
    const char *name;
    const char *path;
    int32_t fetch;
    int32_t trusted;
} skt_file;

enum {
    SKT_EV_NONE = 0,
    SKT_EV_LISTEN_OK,     /* a: the port */
    SKT_EV_LISTEN_FAILED, /* a: the port; text */
    SKT_EV_HASH_FAILED,   /* torrent; a: the piece */
    SKT_EV_PEER_BANNED,   /* torrent; text: the address */
    SKT_EV_CHECKED,       /* torrent: its files checked */
    SKT_EV_FINISHED,      /* torrent: every wanted piece */
    SKT_EV_METADATA,      /* torrent (skt_add_magnet's): a: the info's bytes (skt_info) */
    SKT_EV_ERROR,         /* torrent or -1; text */
    SKT_EV_DHT_ITEM,      /* a: seq; sig; data/len: the bencoded value (signature checked by libtorrent) */
    SKT_EV_DHT_PUT        /* a: the nodes that stored it */
};

typedef struct {
    int32_t type;       /* SKT_EV_* */
    int32_t torrent;    /* the id skt_add gave, -1: the session's */
    int64_t a;
    char text[160];
    uint8_t sig[64];
    int32_t len;
    uint8_t data[1000];
} skt_event;

typedef struct {
    int32_t checking;       /* its files being checked */
    int32_t progress_ppm;   /* of the check while checking */
    int64_t wanted, wanted_done;
    int32_t peers;
    int64_t uploaded;
    char error[160];
} skt_status;

SKT_API const char *SKT_CALL skt_version(void);
SKT_API skt_session *SKT_CALL skt_session_new(const skt_config *cfg, char *err, int errlen);
SKT_API void SKT_CALL skt_session_free(skt_session *s); /* stops everything, waits for it */
/* Listen on another port (0: any); a LISTEN_OK or LISTEN_FAILED follows. */
SKT_API void SKT_CALL skt_listen(skt_session *s, uint16_t port);
SKT_API void SKT_CALL skt_set_upload_limit(skt_session *s, int32_t bytes_per_second);
/* No connection to or from the address for the session. */
SKT_API int SKT_CALL skt_ban(skt_session *s, const char *ip);

/* A torrent from its bytes; save_path: a folder where nothing of it is.
   trackers replace the torrent's own (its announce-list and web seeds are
   never used). Returns the torrent's id (>= 0) or -1 and err. */
SKT_API int SKT_CALL skt_add(skt_session *s, const uint8_t *torrent, size_t len, const char *save_path,
                             const skt_file *files, size_t nfiles, const char *const *trackers,
                             size_t ntrackers, char *err, int errlen);
/* Only the info of a torrent, from its swarm (ut_metadata): a METADATA
   follows when it came (libtorrent has checked it is the info hash's). */
SKT_API int SKT_CALL skt_add_magnet(skt_session *s, const uint8_t v2[32], const char *save_path,
                                    const char *const *trackers, size_t ntrackers, char *err, int errlen);
/* The info a METADATA said: its length, or -length when out is too short. */
SKT_API int SKT_CALL skt_info(skt_session *s, int t, uint8_t *out, int outlen);
SKT_API int SKT_CALL skt_status_of(skt_session *s, int t, skt_status *out);
/* Per file of skt_add's list: its bytes checked (whole pieces). */
SKT_API int SKT_CALL skt_file_progress(skt_session *s, int t, int64_t *out, size_t n);
SKT_API int SKT_CALL skt_remove(skt_session *s, int t); /* the files stay */

/* Up to max events, waiting up to wait_ms for the first. */
SKT_API int SKT_CALL skt_poll(skt_session *s, skt_event *out, int max, int wait_ms);

/* BEP 46: read the item of key and salt (a DHT_ITEM for each answer);
   put a signed one as it is (no private key here: the signature is the
   release's). */
SKT_API void SKT_CALL skt_dht_get(skt_session *s, const uint8_t key[32], const uint8_t *salt, size_t saltlen);
SKT_API int SKT_CALL skt_dht_put(skt_session *s, const uint8_t key[32], const uint8_t sig[64], int64_t seq,
                                 const uint8_t *salt, size_t saltlen, const uint8_t *value, size_t len);
#ifdef __cplusplus
}
#endif
#endif
