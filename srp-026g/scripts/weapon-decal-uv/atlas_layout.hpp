// Exact intersection tests on the coordinates that will reach the shader, and
// the chart of a face moved out of its xatlas chart.
#pragma once
#include <algorithm>
#include <array>
#include <cmath>
#include <cstdint>
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

// Keep one owner for each overlapping region: the faces returned are the
// later of each overlapping pair and those of zero area. A 64x64 grid bounds
// the candidate pairs. All intersection decisions use integer edge equations.
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
using Texel = std::array<float, 2>;

inline double distance2(Position a, Position b) {
    double sum = 0;
    for (int i = 0; i < 3; ++i) {
        double d = double(a[i]) - b[i];
        sum += d * d;
    }
    return sum;
}

// A face moved into a chart of its own: its shape in the model, its longest
// side along x, scaled to the texel area its xatlas chart gave it (or to the
// atlas's texel density where that area is 0), at least a texel each way.
inline std::array<Texel, 3> face_chart(std::array<Position, 3> p, double uvArea, double density) {
    int a = 0, b = 1, c = 2;
    if (distance2(p[1], p[2]) > distance2(p[a], p[b]))
        a = 1, b = 2, c = 0;
    if (distance2(p[2], p[0]) > distance2(p[a], p[b]))
        a = 2, b = 0, c = 1;
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
    double scale = uvArea > 0 && length * height > 0 ? std::sqrt(2 * uvArea / (length * height)) : density;
    double width = std::max(1.0, length * scale), high = std::max(1.0, height * scale);
    std::array<Texel, 3> chart;
    chart[a] = {0, 0};
    chart[b] = {float(width), 0};
    chart[c] = {float(length > 0 ? width * along / length : 0), float(high)};
    return chart;
}
