// Exact intersection tests on the coordinates that will reach the shader.
#pragma once
#include <algorithm>
#include <array>
#include <cmath>
#include <cstdint>
#include <stdexcept>
#include <vector>

struct Uv {
    int64_t x, y;
};
using Triangle = std::array<Uv, 3>;

inline int64_t side(Uv a, Uv b, Uv p) { return (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x); }

// Each triangle is counter-clockwise. Edge contact has no shared interior.
inline bool separated(const Triangle &a, const Triangle &b) {
    for (int i = 0; i < 3; ++i) {
        auto outside = [&](Uv p) { return side(a[i], a[(i + 1) % 3], p) <= 0; };
        if (std::all_of(b.begin(), b.end(), outside))
            return true;
    }
    return false;
}

// Keep one owner for each overlapping region. A 64x64 grid bounds the
// candidate pairs. All intersection decisions use integer edge equations.
inline std::vector<uint32_t> conflicts(std::vector<Triangle> triangles) {
    constexpr int cells = 64;
    std::vector<std::vector<uint32_t>> grid(cells * cells);
    std::vector<int> visited(triangles.size(), -1);
    std::vector<uint32_t> rejected;
    for (uint32_t id = 0; id < triangles.size(); ++id) {
        auto &t = triangles[id];
        auto area = side(t[0], t[1], t[2]);
        if (area == 0) {
            rejected.push_back(id);
            continue;
        }
        if (area < 0)
            std::swap(t[1], t[2]);
        int x0 = std::min({t[0].x, t[1].x, t[2].x}) / 1024, x1 = std::max({t[0].x, t[1].x, t[2].x}) / 1024;
        int y0 = std::min({t[0].y, t[1].y, t[2].y}) / 1024, y1 = std::max({t[0].y, t[1].y, t[2].y}) / 1024;
        bool hit = false;
        for (int y = y0; y <= y1 && !hit; ++y)
            for (int x = x0; x <= x1 && !hit; ++x) {
                for (auto other : grid[y * cells + x]) {
                    if (visited[other] == int(id))
                        continue;
                    visited[other] = int(id);
                    if (!separated(t, triangles[other]) && !separated(triangles[other], t)) {
                        hit = true;
                        break;
                    }
                }
            }
        if (hit) {
            rejected.push_back(id);
            continue;
        }
        for (int y = y0; y <= y1; ++y)
            for (int x = x0; x <= x1; ++x)
                grid[y * cells + x].push_back(id);
    }
    return rejected;
}


using Position = std::array<float, 3>;

inline double distance2(Position a, Position b) {
    double sum = 0;
    for (int i = 0; i < 3; ++i) {
        double d = double(a[i]) - b[i];
        sum += d * d;
    }
    return sum;
}

struct FaceChart {
    uint32_t face;
    Triangle uv;
    int64_t width, height;
};

// Put a detached face in its own chart. Place its longest side horizontally;
// the third vertex retains its position along that edge and its altitude.
// The uncompressed source positions/UVs are still used by the shader.
inline FaceChart face_chart(uint32_t face, const std::vector<uint16_t> &indices,
                            const std::vector<Position> &positions, const Triangle &source, double density,
                            int64_t padding) {
    std::array<Position, 3> p;
    for (int i = 0; i < 3; ++i)
        p[i] = positions[indices[3 * face + i]];
    int a = 0, b = 1, c = 2;
    if (distance2(p[1], p[2]) > distance2(p[a], p[b])) {
        a = 1;
        b = 2;
        c = 0;
    }
    if (distance2(p[2], p[0]) > distance2(p[a], p[b])) {
        a = 2;
        b = 0;
        c = 1;
    }
    double length = std::sqrt(distance2(p[a], p[b]));
    double along = 0, height2 = 0;
    if (length > 0) {
        for (int k = 0; k < 3; ++k)
            along += (p[c][k] - double(p[a][k])) * (p[b][k] - double(p[a][k])) / length;
        for (int k = 0; k < 3; ++k) {
            double orthogonal = p[c][k] - double(p[a][k]) - along * (p[b][k] - double(p[a][k])) / length;
            height2 += orthogonal * orthogonal;
        }
    }
    double height = std::sqrt(height2);
    double uvArea = std::abs(side(source[0], source[1], source[2]));
    double scale = (uvArea > 0 && length * height > 0) ? std::sqrt(uvArea / (length * height)) : density;
    // A one-texel minimum also gives an originally collapsed face a valid
    // footprint. Its geometric area can be zero in the donor data.
    int64_t width = std::clamp<int64_t>(std::llround(length * scale), 32, 65535 - 2 * padding);
    int64_t high = std::max<int64_t>(32, std::llround(length ? width * height / length : 0));
    FaceChart chart{face, {}, width, high};
    chart.uv[a] = {0, 0};
    chart.uv[b] = {width, 0};
    chart.uv[c] = {std::clamp<int64_t>(std::llround(length ? width * along / length : 0), 0, width), high};
    return chart;
}

// Most faces keep the charts produced by xatlas. Some donor islands fold
// back onto themselves; even quantizing a thin face can introduce overlap.
// Move just one owner of each conflict into a padded strip of separate
// charts. Recheck after quantization; each iteration detaches new faces.
inline std::vector<Triangle> separate(std::vector<Triangle> source, const std::vector<uint16_t> &indices,
                                      const std::vector<Position> &positions, unsigned resolution) {
    auto bad = conflicts(source);
    if (bad.empty())
        return source;
    const int64_t pad = (8 * 65535 + resolution - 1) / resolution;
    const double density = 65535. / 1000.; // working mesh's longest axis is 1000
    std::vector<bool> detached(source.size());
    std::vector<FaceChart> charts;
    do {
        for (auto i : bad) {
            if (detached[i])
                throw std::runtime_error("detached chart intersects another face");
            detached[i] = true;
            charts.push_back(face_chart(i, indices, positions, source[i], density, pad));
        }
        std::sort(charts.begin(), charts.end(), [](const auto &a, const auto &b) {
            return a.height != b.height ? a.height > b.height : a.face < b.face;
        });
        std::vector<Triangle> result = source;
        int64_t x = pad, y = pad, row = 0;
        for (const auto &chart : charts) {
            if (x + chart.width + pad > 65535) {
                x = pad;
                y += row + 2 * pad;
                row = 0;
            }
            for (int k = 0; k < 3; ++k)
                result[chart.face][k] = {x + chart.uv[k].x, y + chart.uv[k].y};
            x += chart.width + 2 * pad;
            row = std::max(row, chart.height);
        }
        int64_t strip = y + row + pad;
        if (strip >= 65535 - pad)
            throw std::runtime_error("detached charts exceed the atlas");
        for (size_t i = 0; i < result.size(); ++i)
            for (auto &p : result[i]) {
                if (detached[i])
                    p.y += 65535 - strip;
                else
                    p.y = (p.y * (65535 - strip - pad) + 32767) / 65535;
            }
        bad = conflicts(result);
        if (bad.empty())
            return result;
        // conflicts() keeps the first owner; detached and original bands
        // are disjoint, so any new owner belongs to the compressed band.
    } while (true);
}
