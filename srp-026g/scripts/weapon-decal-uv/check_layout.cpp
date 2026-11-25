// Verify the emitted one-weight mesh independently of the chart generator.
#include <algorithm>
#include <array>
#include <cmath>
#include <cstdint>
#include <cstring>
#include <fstream>
#include <iostream>
#include <iterator>
#include <stdexcept>
#include <string>
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
    const uint8_t *corner(size_t i) const {
        uint16_t v = field<uint16_t>(bytes, indices + 2 * i);
        if (v >= vertexCount)
            throw std::runtime_error("index outside mesh");
        return bytes.data() + 12 + 36 * v;
    }
};

struct Point {
    int64_t x, y;
};

static int64_t edge(Point a, Point b, Point p) { return (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x); }

static void verify(const Mesh &original, const Mesh &baked) {
    if (baked.indexCount != original.indexCount)
        throw std::runtime_error("triangle count changed");
    std::vector<bool> occupied(Resolution * Resolution);
    size_t covered = 0, collisions = 0, flat = 0;
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
