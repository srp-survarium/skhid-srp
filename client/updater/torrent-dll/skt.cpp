// skt.cpp: skhid_torrent.dll, the C API of skt.h over libtorrent-rasterbar 2.0.
// What a session is, by the updater's rules (client/updater/README.md,
// "BitTorrent"): TCP on IPv4, the DHT on, no LSD, UPnP, NAT-PMP, uTP or PEX;
// the extensions smart_ban and ut_metadata alone; one upload limit for every
// peer, local ones too; a torrent's own trackers and web seeds never used.
#define SKT_BUILD
#include "skt.h"

#include <libtorrent/add_torrent_params.hpp>
#include <libtorrent/alert_types.hpp>
#include <libtorrent/bdecode.hpp>
#include <libtorrent/bencode.hpp>
#include <libtorrent/extensions/smart_ban.hpp>
#include <libtorrent/extensions/ut_metadata.hpp>
#include <libtorrent/ip_filter.hpp>
#include <libtorrent/load_torrent.hpp>
#include <libtorrent/session.hpp>
#include <libtorrent/session_params.hpp>
#include <libtorrent/settings_pack.hpp>
#include <libtorrent/torrent_handle.hpp>
#include <libtorrent/torrent_info.hpp>
#include <libtorrent/torrent_status.hpp>
#include <libtorrent/version.hpp>

#include <algorithm>
#include <array>
#include <cstring>
#include <map>
#include <memory>
#include <string>
#include <vector>

namespace lt = libtorrent;

namespace {
struct torrent {
    lt::torrent_handle handle;
    // skt_add's list: the file of the torrent each is.
    std::vector<lt::file_index_t> files;
    // skt_add_magnet's: the info, once it came.
    std::vector<char> info;
};
} // namespace

struct skt_session {
    std::unique_ptr<lt::session> ses;
    std::vector<torrent> torrents;
    std::vector<lt::alert *> pending;
    size_t next = 0;
    bool loopback = false;
    lt::ip_filter banned;
};

namespace {
void put(char *out, int len, std::string const &s) {
    if (!out || len <= 0) return;
    size_t n = std::min(s.size(), size_t(len - 1));
    std::memcpy(out, s.data(), n);
    out[n] = 0;
}

torrent *find(skt_session *s, int t) {
    if (!s || t < 0 || size_t(t) >= s->torrents.size() || !s->torrents[t].handle.is_valid()) return nullptr;
    return &s->torrents[t];
}

int id_of(skt_session *s, lt::torrent_handle const &h) {
    for (size_t i = 0; i < s->torrents.size(); ++i)
        if (s->torrents[i].handle == h) return int(i);
    return -1;
}

std::string interfaces(bool loopback, int port) {
    return (loopback ? "127.0.0.1:" : "0.0.0.0:") + std::to_string(port);
}

lt::settings_pack settings(skt_config const &c) {
    lt::settings_pack p;
    p.set_str(lt::settings_pack::listen_interfaces, interfaces(c.loopback != 0, c.listen_port));
    // A port that is busy is busy: not another one instead.
    p.set_bool(lt::settings_pack::listen_system_port_fallback, false);
    p.set_int(lt::settings_pack::max_retry_port_bind, 0);
    p.set_int(lt::settings_pack::upload_rate_limit, c.upload_limit);
    p.set_bool(lt::settings_pack::enable_dht, true);
    if (c.dht_bootstrap) p.set_str(lt::settings_pack::dht_bootstrap_nodes, c.dht_bootstrap);
    p.set_bool(lt::settings_pack::enable_lsd, false);
    p.set_bool(lt::settings_pack::enable_upnp, false);
    p.set_bool(lt::settings_pack::enable_natpmp, false);
    p.set_bool(lt::settings_pack::enable_outgoing_utp, false);
    p.set_bool(lt::settings_pack::enable_incoming_utp, false);
    // Every tracker of the list asked, not the first that answers: the
    // public ones come and go.
    p.set_bool(lt::settings_pack::announce_to_all_tiers, true);
    p.set_bool(lt::settings_pack::announce_to_all_trackers, true);
    p.set_str(lt::settings_pack::user_agent, "Skhid");
    p.set_str(lt::settings_pack::peer_fingerprint, "-SK0200-");
    // The updater lets go of its port before another one runs.
    p.set_int(lt::settings_pack::stop_tracker_timeout, 1);
    p.set_int(lt::settings_pack::alert_mask, lt::alert_category::error | lt::alert_category::status
        | lt::alert_category::storage | lt::alert_category::dht | lt::alert_category::connect
        | lt::alert_category::port_mapping);
    if (c.loopback) {
        p.set_bool(lt::settings_pack::allow_multiple_connections_per_ip, true);
        p.set_bool(lt::settings_pack::dht_restrict_routing_ips, false);
        p.set_bool(lt::settings_pack::dht_restrict_search_ips, false);
        p.set_bool(lt::settings_pack::dht_prefer_verified_node_ids, false);
        p.set_bool(lt::settings_pack::dht_ignore_dark_internet, false);
    }
    return p;
}

// Port 0: the TCP socket's port is the one asked for then, so that the DHT's
// UDP socket is on it too (each would pick its own).
void listen(skt_session *s, int port) {
    lt::settings_pack p;
    p.set_str(lt::settings_pack::listen_interfaces, interfaces(s->loopback, port));
    s->ses->apply_settings(std::move(p));
    if (port == 0) {
        int const picked = s->ses->listen_port();
        if (picked > 0) {
            lt::settings_pack again;
            again.set_str(lt::settings_pack::listen_interfaces, interfaces(s->loopback, picked));
            s->ses->apply_settings(std::move(again));
        }
    }
}

std::string name_of(lt::file_storage const &fs, lt::file_index_t i) {
    std::string path = fs.file_path(i);
    std::replace(path.begin(), path.end(), '\\', '/');
    std::string const prefix = fs.name() + "/";
    if (path.compare(0, prefix.size(), prefix) == 0) path.erase(0, prefix.size());
    return path;
}

std::vector<lt::announce_entry> trackers_of(const char *const *trackers, size_t n) {
    std::vector<lt::announce_entry> out;
    for (size_t i = 0; i < n; ++i) {
        lt::announce_entry e(trackers[i]);
        e.tier = std::uint8_t(i);
        out.push_back(e);
    }
    return out;
}
} // namespace

extern "C" {

SKT_API const char *SKT_CALL skt_version(void) { return lt::version(); }

SKT_API skt_session *SKT_CALL skt_session_new(const skt_config *cfg, char *err, int errlen) try {
    skt_config c{};
    if (cfg) c = *cfg;
    // No extension but the two added below (session_params' default has PEX).
    lt::session_params params(settings(c), {});
    auto s = std::make_unique<skt_session>();
    s->loopback = c.loopback != 0;
    s->ses = std::make_unique<lt::session>(std::move(params));
    s->ses->add_extension(&lt::create_smart_ban_plugin);
    s->ses->add_extension(&lt::create_ut_metadata_plugin);
    // Every peer in the global class: its upload limit holds for peers of
    // the local network too (by default they have none).
    lt::ip_filter classes;
    classes.add_rule(lt::make_address("0.0.0.0"), lt::make_address("255.255.255.255"),
                     1u << static_cast<std::uint32_t>(lt::session::global_peer_class_id));
    classes.add_rule(lt::make_address("::"), lt::make_address("ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff"),
                     1u << static_cast<std::uint32_t>(lt::session::global_peer_class_id));
    s->ses->set_peer_class_filter(classes);
    if (c.listen_port == 0) listen(s.get(), 0);
    return s.release();
} catch (std::exception const &e) {
    put(err, errlen, e.what());
    return nullptr;
}

SKT_API void SKT_CALL skt_session_free(skt_session *s) {
    try { delete s; } catch (...) {}
}

SKT_API void SKT_CALL skt_listen(skt_session *s, uint16_t port) try {
    listen(s, port);
} catch (...) {}

SKT_API void SKT_CALL skt_set_upload_limit(skt_session *s, int32_t limit) try {
    lt::settings_pack p;
    p.set_int(lt::settings_pack::upload_rate_limit, limit);
    s->ses->apply_settings(std::move(p));
} catch (...) {}

SKT_API int SKT_CALL skt_ban(skt_session *s, const char *ip) try {
    lt::address const a = lt::make_address(ip);
    s->banned.add_rule(a, a, lt::ip_filter::blocked);
    s->ses->set_ip_filter(s->banned);
    return 0;
} catch (...) { return -1; }

SKT_API int SKT_CALL skt_add(skt_session *s, const uint8_t *bytes, size_t len, const char *save_path,
                             const skt_file *files, size_t nfiles, const char *const *trackers,
                             size_t ntrackers, char *err, int errlen) try {
    lt::add_torrent_params atp = lt::load_torrent_buffer(
        lt::span<char const>(reinterpret_cast<char const *>(bytes), std::ptrdiff_t(len)));
    atp.ti->clear_trackers();
    atp.ti->set_web_seeds({});
    atp.url_seeds.clear();
    atp.trackers.clear();
    atp.tracker_tiers.clear();
    for (size_t i = 0; i < ntrackers; ++i) {
        atp.trackers.emplace_back(trackers[i]);
        atp.tracker_tiers.push_back(int(i));
    }
    atp.save_path = save_path;
    atp.flags &= ~(lt::torrent_flags::auto_managed | lt::torrent_flags::paused);
    atp.flags |= lt::torrent_flags::disable_lsd | lt::torrent_flags::disable_pex;

    lt::file_storage const &fs = atp.ti->files();
    std::map<std::string, lt::file_index_t> by_name;
    for (auto const i : fs.file_range())
        if (!fs.pad_file_at(i)) by_name.emplace(name_of(fs, i), i);
    torrent t;
    atp.file_priorities.assign(std::size_t(fs.num_files()), lt::dont_download);
    for (size_t k = 0; k < nfiles; ++k) {
        auto const found = by_name.find(files[k].name);
        if (found == by_name.end()) {
            put(err, errlen, std::string("the torrent has no ") + files[k].name);
            return -1;
        }
        lt::file_index_t const i = found->second;
        t.files.push_back(i);
        atp.renamed_files[i] = files[k].path;
        atp.file_priorities[std::size_t(static_cast<int>(i))] = files[k].fetch ? lt::default_priority : lt::dont_download;
    }
    // The pieces of the trusted files are had; libtorrent checks from the
    // first piece of the first file that is not (to the end: a shorter
    // have-list says the check stopped there).
    int const pieces = atp.ti->num_pieces();
    auto first = [&](lt::file_index_t i) { return static_cast<int>(fs.map_file(i, 0, 0).piece); };
    auto last = [&](lt::file_index_t i) {
        return fs.file_size(i) == 0 ? first(i) - 1
            : static_cast<int>(fs.map_file(i, fs.file_size(i) - 1, 0).piece);
    };
    int check_from = pieces;
    bool any = false;
    for (size_t k = 0; k < nfiles; ++k) {
        if (files[k].trusted && !files[k].fetch) any = true;
        else check_from = std::min(check_from, first(t.files[k]));
    }
    if (any && check_from > 0) {
        atp.have_pieces.resize(check_from, false);
        for (size_t k = 0; k < nfiles; ++k) {
            if (!files[k].trusted || files[k].fetch) continue;
            for (int p = first(t.files[k]); p <= last(t.files[k]) && p < check_from; ++p)
                atp.have_pieces.set_bit(lt::piece_index_t(p));
        }
    }
    t.handle = s->ses->add_torrent(std::move(atp));
    s->torrents.push_back(std::move(t));
    return int(s->torrents.size() - 1);
} catch (std::exception const &e) {
    put(err, errlen, e.what());
    return -1;
}

SKT_API int SKT_CALL skt_add_magnet(skt_session *s, const uint8_t v2[32], const char *save_path,
                                    const char *const *trackers, size_t ntrackers, char *err, int errlen) try {
    lt::add_torrent_params atp;
    atp.info_hashes.v2 = lt::sha256_hash(reinterpret_cast<char const *>(v2));
    atp.save_path = save_path;
    for (size_t i = 0; i < ntrackers; ++i) {
        atp.trackers.emplace_back(trackers[i]);
        atp.tracker_tiers.push_back(int(i));
    }
    atp.flags &= ~(lt::torrent_flags::auto_managed | lt::torrent_flags::paused);
    // The info only: no piece is fetched.
    atp.flags |= lt::torrent_flags::upload_mode | lt::torrent_flags::disable_lsd | lt::torrent_flags::disable_pex;
    torrent t;
    t.handle = s->ses->add_torrent(std::move(atp));
    s->torrents.push_back(std::move(t));
    return int(s->torrents.size() - 1);
} catch (std::exception const &e) {
    put(err, errlen, e.what());
    return -1;
}

SKT_API int SKT_CALL skt_info(skt_session *s, int t, uint8_t *out, int outlen) try {
    auto *x = find(s, t);
    if (!x || x->info.empty()) return 0;
    int const n = int(x->info.size());
    if (n > outlen) return -n;
    std::memcpy(out, x->info.data(), x->info.size());
    return n;
} catch (...) { return 0; }

SKT_API int SKT_CALL skt_status_of(skt_session *s, int t, skt_status *o) try {
    auto *x = find(s, t);
    if (!x || !o) return -1;
    lt::torrent_status const st = x->handle.status();
    std::memset(o, 0, sizeof *o);
    o->checking = st.state == lt::torrent_status::checking_files
        || st.state == lt::torrent_status::checking_resume_data;
    o->progress_ppm = st.progress_ppm;
    o->wanted = st.total_wanted;
    o->wanted_done = st.total_wanted_done;
    o->peers = st.num_peers;
    o->uploaded = st.all_time_upload;
    if (st.errc) put(o->error, sizeof o->error, st.errc.message());
    return 0;
} catch (...) { return -1; }

SKT_API int SKT_CALL skt_file_progress(skt_session *s, int t, int64_t *out, size_t n) try {
    auto *x = find(s, t);
    if (!x || n != x->files.size()) return -1;
    std::vector<std::int64_t> const done = x->handle.file_progress(lt::torrent_handle::piece_granularity);
    for (size_t k = 0; k < n; ++k) out[k] = done.at(std::size_t(static_cast<int>(x->files[k])));
    return 0;
} catch (...) { return -1; }

SKT_API int SKT_CALL skt_remove(skt_session *s, int t) try {
    auto *x = find(s, t);
    if (!x) return -1;
    s->ses->remove_torrent(x->handle);
    *x = torrent();
    return 0;
} catch (...) { return -1; }

SKT_API int SKT_CALL skt_poll(skt_session *s, skt_event *out, int max, int wait_ms) try {
    int n = 0;
    while (n < max) {
        if (s->next >= s->pending.size()) {
            if (n > 0) break;
            if (wait_ms > 0) s->ses->wait_for_alert(lt::milliseconds(wait_ms));
            s->ses->pop_alerts(&s->pending);
            s->next = 0;
            wait_ms = 0;
            if (s->pending.empty()) break;
        }
        lt::alert *a = s->pending[s->next++];
        skt_event &e = out[n];
        std::memset(&e, 0, sizeof e);
        e.torrent = -1;
        if (auto *ta = dynamic_cast<lt::torrent_alert *>(a)) e.torrent = id_of(s, ta->handle);
        if (auto *x = lt::alert_cast<lt::listen_succeeded_alert>(a)) {
            if (x->socket_type != lt::socket_type_t::tcp) continue;
            e.type = SKT_EV_LISTEN_OK;
            e.a = x->port;
        } else if (auto *x = lt::alert_cast<lt::listen_failed_alert>(a)) {
            if (x->socket_type != lt::socket_type_t::tcp) continue;
            e.type = SKT_EV_LISTEN_FAILED;
            e.a = x->port;
            put(e.text, sizeof e.text, x->error.message());
        } else if (auto *x = lt::alert_cast<lt::hash_failed_alert>(a)) {
            e.type = SKT_EV_HASH_FAILED;
            e.a = static_cast<int>(x->piece_index);
        } else if (auto *x = lt::alert_cast<lt::peer_disconnected_alert>(a)) {
            // Banned for a piece that failed: of it alone (libtorrent), or
            // found out by smart_ban (which says nothing else).
            if (x->error != lt::errors::too_many_corrupt_pieces && x->error != lt::errors::peer_banned) continue;
            e.type = SKT_EV_PEER_BANNED;
            put(e.text, sizeof e.text, x->endpoint.address().to_string());
        } else if (lt::alert_cast<lt::torrent_checked_alert>(a)) {
            e.type = SKT_EV_CHECKED;
        } else if (lt::alert_cast<lt::torrent_finished_alert>(a)) {
            e.type = SKT_EV_FINISHED;
        } else if (auto *x = lt::alert_cast<lt::metadata_received_alert>(a)) {
            auto *t = find(s, e.torrent);
            auto const ti = t ? x->handle.torrent_file() : nullptr;
            if (!ti) continue;
            auto const info = ti->info_section();
            t->info.assign(info.begin(), info.end());
            e.type = SKT_EV_METADATA;
            e.a = std::int64_t(t->info.size());
        } else if (auto *x = lt::alert_cast<lt::dht_mutable_item_alert>(a)) {
            e.type = SKT_EV_DHT_ITEM;
            e.a = x->seq;
            std::memcpy(e.sig, x->signature.data(), 64);
            std::vector<char> v;
            lt::bencode(std::back_inserter(v), x->item);
            if (v.size() > sizeof e.data) continue;
            e.len = int(v.size());
            std::memcpy(e.data, v.data(), v.size());
        } else if (auto *x = lt::alert_cast<lt::dht_put_alert>(a)) {
            e.type = SKT_EV_DHT_PUT;
            e.a = x->num_success;
        } else if (a->category() & lt::alert_category::error) {
            e.type = SKT_EV_ERROR;
            put(e.text, sizeof e.text, a->message());
        } else {
            continue;
        }
        ++n;
    }
    return n;
} catch (...) { return -1; }

SKT_API void SKT_CALL skt_dht_get(skt_session *s, const uint8_t key[32], const uint8_t *salt, size_t saltlen) try {
    std::array<char, 32> k;
    std::memcpy(k.data(), key, 32);
    s->ses->dht_get_item(k, std::string(reinterpret_cast<char const *>(salt), saltlen));
} catch (...) {}

SKT_API int SKT_CALL skt_dht_put(skt_session *s, const uint8_t key[32], const uint8_t sig[64], int64_t seq,
                                 const uint8_t *salt, size_t saltlen, const uint8_t *value, size_t len) try {
    std::array<char, 32> k;
    std::memcpy(k.data(), key, 32);
    std::array<char, 64> g;
    std::memcpy(g.data(), sig, 64);
    lt::entry const v = lt::bdecode(lt::span<char const>(reinterpret_cast<char const *>(value), std::ptrdiff_t(len)));
    s->ses->dht_put_item(k, [v, g, seq](lt::entry &item, std::array<char, 64> &signature,
                                        std::int64_t &sequence, std::string const &) {
        // A newer item where it is stays (it is put again as it is).
        if (sequence > seq) return;
        item = v;
        signature = g;
        sequence = seq;
    }, std::string(reinterpret_cast<char const *>(salt), saltlen));
    return 0;
} catch (...) { return -1; }

} // extern "C"
