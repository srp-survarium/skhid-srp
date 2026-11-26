// Verify the emitted one-weight mesh independently of the chart generator.
#include <algorithm>
#include <array>
#include <cmath>
#include <cstdint>
#include <cstring>
#include <fstream>
#include <iostream>
#include <iterator>
#include <map>
#include <optional>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>

using Bytes = std::vector<uint8_t>;
constexpr int Resolution = 2048;

template <typename T> T field(const Bytes &data, size_t offset) {
    if (offset > data.size() || sizeof(T) > data.size() - offset)
        throw std::runtime_error("truncated mesh");
    T value;
    std::memcpy(&value, data.data() + offset, sizeof(T));
    return value;
}

struct Mesh {
    Bytes bytes;
    size_t vertexCount, indexCount, indices;
    explicit Mesh(const char *path) {
        std::ifstream file(path, std::ios::binary);
        if (!file)
            throw std::runtime_error(std::string("cannot read ") + path);
        bytes.assign(std::istreambuf_iterator<char>(file), {});
        vertexCount = field<uint32_t>(bytes, 8);
        indices = 24 + vertexCount * 36;
        if (field<uint32_t>(bytes, 0) != 3 || field<uint32_t>(bytes, 4) != 4 + 36 * vertexCount ||
            field<uint32_t>(bytes, indices - 12) != 4)
            throw std::runtime_error("invalid one-weight mesh");
        indexCount = field<uint32_t>(bytes, indices - 4);
        if (indexCount % 3 || indices + indexCount * 2 != bytes.size() ||
            field<uint32_t>(bytes, indices - 8) != 4 + 2 * indexCount)
            throw std::runtime_error("invalid triangle stream");
    }
    uint16_t index(size_t i) const {
        uint16_t v = field<uint16_t>(bytes, indices + 2 * i);
        if (v >= vertexCount)
            throw std::runtime_error("index outside mesh");
        return v;
    }
    const uint8_t *corner(size_t i) const { return bytes.data() + 12 + 36 * index(i); }
};

struct Point {
    int64_t x, y;
};

static int64_t edge(Point a, Point b, Point p) { return (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x); }

// Charts are the triangles joined through shared corners: the same model
// position at the same decal UV. xatlas's charts join faces across
// split-but-colocal source vertices; distinct charts are padded apart.
struct Charts {
    std::vector<uint32_t> parent;
    explicit Charts(size_t corners) : parent(corners) {
        for (uint32_t i = 0; i < corners; ++i)
            parent[i] = i;
    }
    uint32_t find(uint32_t v) {
        while (parent[v] != v)
            v = parent[v] = parent[parent[v]];
        return v;
    }
    void join(uint32_t a, uint32_t b) { parent[find(a)] = find(b); }
};

// Each chart's texels (every texel its triangles touch at all, so every texel
// a bilinear sample on the chart can read) lie in 4x4 compression blocks of
// their own, and two charts' blocks are at least Gap blocks apart: with
// xatlas's 8 texels of padding, at least two empty blocks lie between them.
// A decal painted on one chart then reaches no texel, and no block, that
// another chart samples.
constexpr int Blocks = Resolution / 4, Gap = 3;

// A triangle of the decal atlas: its first corner's index, and its corners.
using Placed = std::pair<uint32_t, std::array<Point, 3>>;

static void separate_charts(const Mesh &baked, const std::vector<Placed> &triangles) {
    // Join each corner to the first corner of the same position and decal UV.
    Charts charts(baked.indexCount);
    std::map<std::array<uint8_t, 16>, uint32_t> first;
    for (uint32_t i = 0; i < baked.indexCount; ++i) {
        const uint8_t *v = baked.corner(i);
        std::array<uint8_t, 16> key{};
        std::memcpy(key.data(), v, 12);
        key[12] = v[13], key[13] = v[14], key[14] = v[15], key[15] = v[23];
        auto [at, added] = first.emplace(key, i);
        if (!added)
            charts.join(i, at->second);
        if (i % 3)
            charts.join(i, i - i % 3);
    }
    std::vector<std::optional<uint32_t>> owner(Blocks * Blocks);
    for (const auto &[corner, p] : triangles) {
        // p: counter-clockwise, in units of 1 / (65535 * Resolution) of the atlas
        const uint32_t chart = charts.find(corner);
        auto texel = [](int64_t unit) { return int(std::min<int64_t>(Resolution - 1, unit / (int64_t(65535)))); };
        int x0 = texel(std::min({p[0].x, p[1].x, p[2].x})), x1 = texel(std::max({p[0].x, p[1].x, p[2].x}));
        int y0 = texel(std::min({p[0].y, p[1].y, p[2].y})), y1 = texel(std::max({p[0].y, p[1].y, p[2].y}));
        for (int y = y0; y <= y1; ++y)
            for (int x = x0; x <= x1; ++x) {
                // The texel's square is clear of the triangle when all four
                // corners lie strictly outside one of its edges.
                std::array<Point, 4> square{{{x * 65535, y * 65535}, {(x + 1) * 65535, y * 65535},
                                             {x * 65535, (y + 1) * 65535}, {(x + 1) * 65535, (y + 1) * 65535}}};
                bool clear = false;
                for (int e = 0; e < 3 && !clear; ++e)
                    clear = std::all_of(square.begin(), square.end(),
                                        [&](Point c) { return edge(p[e], p[(e + 1) % 3], c) < 0; });
                if (clear)
                    continue;
                auto &block = owner[(y / 4) * Blocks + x / 4];
                if (block && *block != chart)
                    throw std::runtime_error("two charts share a 4x4 block");
                block = chart;
            }
    }
    for (int by = 0; by < Blocks; ++by)
        for (int bx = 0; bx < Blocks; ++bx) {
            const auto chart = owner[by * Blocks + bx];
            if (!chart)
                continue;
            for (int y = std::max(0, by - Gap + 1); y <= std::min(Blocks - 1, by + Gap - 1); ++y)
                for (int x = std::max(0, bx - Gap + 1); x <= std::min(Blocks - 1, bx + Gap - 1); ++x)
                    if (owner[y * Blocks + x] && owner[y * Blocks + x] != chart)
                        throw std::runtime_error("two charts closer than the atlas padding");
        }
}

static void verify(const Mesh &original, const Mesh &baked) {
    if (baked.indexCount != original.indexCount)
        throw std::runtime_error("triangle count changed");
    std::vector<bool> occupied(Resolution * Resolution);
    size_t covered = 0, collisions = 0, flat = 0;
    std::vector<Placed> triangles;
    for (size_t i = 0; i < baked.indexCount; i += 3) {
        std::array<Point, 3> p;
        for (size_t j = 0; j < 3; ++j) {
            auto *old = original.corner(i + j), *v = baked.corner(i + j);
            if (std::memcmp(old, v, 13) || std::memcmp(old + 16, v + 16, 3) || std::memcmp(old + 20, v + 20, 3) ||
                std::memcmp(old + 24, v + 24, 3) || std::memcmp(old + 28, v + 28, 8))
                throw std::runtime_error("position, bone, tangent frame or source UV changed");
            if (v[19] != 'S' || v[27] != 'D')
                throw std::runtime_error("missing decal UV tag");
            p[j] = {v[13] + 256 * v[14], v[15] + 256 * v[23]};
        }
        const int64_t area = edge(p[0], p[1], p[2]);
        if (!area) {
            ++flat;
            continue;
        }
        if (area < 0)
            std::swap(p[1], p[2]);
        int x0 = std::min({p[0].x, p[1].x, p[2].x}) * Resolution / 65535;
        int y0 = std::min({p[0].y, p[1].y, p[2].y}) * Resolution / 65535;
        int x1 = std::min<int>(Resolution - 1, std::max({p[0].x, p[1].x, p[2].x}) * Resolution / 65535);
        int y1 = std::min<int>(Resolution - 1, std::max({p[0].y, p[1].y, p[2].y}) * Resolution / 65535);
        triangles.push_back({uint32_t(i), {{{p[0].x * Resolution, p[0].y * Resolution},
                                               {p[1].x * Resolution, p[1].y * Resolution},
                                               {p[2].x * Resolution, p[2].y * Resolution}}}});
        for (auto &point : p) {
            point.x *= 2 * Resolution;
            point.y *= 2 * Resolution;
        }
        for (int y = y0; y <= y1; ++y) {
            for (int x = x0; x <= x1; ++x) {
                Point sample{(2 * x + 1) * 65535, (2 * y + 1) * 65535};
                if (edge(p[0], p[1], sample) <= 0 || edge(p[1], p[2], sample) <= 0 ||
                    edge(p[2], p[0], sample) <= 0)
                    continue;
                size_t texel = y * Resolution + x;
                if (occupied[texel])
                    ++collisions;
                occupied[texel] = true;
                ++covered;
            }
        }
    }
    std::cout << baked.indexCount / 3 << " triangles, " << covered << " interior samples, " << collisions
              << " shared, " << flat << " zero UV area\n";
    if (flat)
        throw std::runtime_error("triangle has zero decal UV area");
    if (collisions)
        throw std::runtime_error("distinct triangle interiors share decal texels");
    separate_charts(baked, triangles);
}

int main(int argc, char **argv) {
    try {
        if (argc != 3)
            throw std::runtime_error("usage: check_layout original_mesh dual_uv_mesh");
        verify(Mesh(argv[1]), Mesh(argv[2]));
    } catch (const std::exception &error) {
        std::cerr << error.what() << '\n';
        return 1;
    }
}
