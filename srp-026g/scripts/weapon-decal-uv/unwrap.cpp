// Generate a decal atlas for one 0.26g skinned render surface. xatlas splits
// vertices along chart boundaries. Copy the complete source vertex at each
// split so bone weights, tangent frame and bind-pose position remain exact.
#include "xatlas.h"
#include "atlas_layout.hpp"

#include <algorithm>
#include <array>
#include <cmath>
#include <cstdint>
#include <cstring>
#include <fstream>
#include <iostream>
#include <iterator>
#include <map>
#include <memory>
#include <stdexcept>
#include <tuple>
#include <vector>

using Bytes = std::vector<uint8_t>;
using AtlasPtr = std::unique_ptr<xatlas::Atlas, decltype(&xatlas::Destroy)>;

// The native weapon bake's texture is 2048 texels square. Charts keep 8
// texels of padding and sit on 4x4 compression blocks of that texture.
constexpr uint32_t Resolution = 2048;
constexpr uint32_t Padding = 8;
constexpr uint32_t Stride = 36; // the native one-weight vertex

static uint32_t read32(const Bytes &bytes, size_t offset) {
    if (offset > bytes.size() || bytes.size() - offset < 4)
        throw std::runtime_error("truncated converted_model");
    uint32_t value;
    std::memcpy(&value, bytes.data() + offset, sizeof(value));
    return value;
}

template <typename T> static void write(std::ostream &out, const T &value) {
    out.write(reinterpret_cast<const char *>(&value), sizeof(value));
}

// Pack the atlas's charts into exactly one Resolution² atlas, at the highest
// density (from `density` down, 3% a step) at which they all fit. The output
// texel coordinates are then those of the baked texture, so xatlas's padding
// and block alignment are the texture's.
static void pack(xatlas::Atlas *atlas, float density) {
    xatlas::PackOptions options;
    options.resolution = Resolution;
    options.padding = Padding;
    options.blockAlign = true;
    for (options.texelsPerUnit = density;; options.texelsPerUnit *= 0.97f) {
        xatlas::PackCharts(atlas, options);
        if (atlas->atlasCount == 1)
            return;
    }
}

// A face xatlas leaves out of every chart (no area in the model) has its
// corners at 0: zero decal UV area, which separate() moves to a chart of its own.
static Uv quantize(const xatlas::Vertex &v) {
    if (!(v.uv[0] >= 0 && v.uv[0] <= Resolution && v.uv[1] >= 0 && v.uv[1] <= Resolution))
        throw std::runtime_error("decal UV outside the atlas");
    return {std::lround(v.uv[0] * 65535.0 / Resolution), std::lround(v.uv[1] * 65535.0 / Resolution)};
}

// Some donor islands fold back onto themselves, and quantizing a thin face
// can make it overlap a neighbour. Each face that conflicts() rejects becomes
// a chart of its own; xatlas packs those and the first pass's charts again,
// whole and padded, until no face shares decal texels with another.
static std::vector<Triangle> separate(const xatlas::Mesh &first, float density,
                                      const std::vector<std::array<Position, 3>> &faces) {
    std::vector<Triangle> result(faces.size());
    for (size_t i = 0; i < first.indexCount; ++i)
        result[i / 3][i % 3] = quantize(first.vertexArray[first.indexArray[i]]);
    std::vector<bool> detached(faces.size());
    for (auto bad = conflicts(result); !bad.empty(); bad = conflicts(result)) {
        bool grew = false;
        for (auto face : bad) {
            grew |= !detached[face];
            detached[face] = true;
        }
        if (!grew)
            throw std::runtime_error("a face in a chart of its own still shares decal texels");
        // The first pass's vertices for the faces kept, three new ones for each
        // face moved. Its own material keeps a moved face out of other charts.
        std::vector<Texel> uvs;
        std::vector<uint32_t> indices, materials;
        for (uint32_t v = 0; v < first.vertexCount; ++v)
            uvs.push_back({first.vertexArray[v].uv[0], first.vertexArray[v].uv[1]});
        for (uint32_t face = 0; face < faces.size(); ++face) {
            if (!detached[face]) {
                for (int k = 0; k < 3; ++k)
                    indices.push_back(first.indexArray[3 * face + k]);
                materials.push_back(0);
                continue;
            }
            const float *a = first.vertexArray[first.indexArray[3 * face]].uv;
            const float *b = first.vertexArray[first.indexArray[3 * face + 1]].uv;
            const float *c = first.vertexArray[first.indexArray[3 * face + 2]].uv;
            double area = std::abs((b[0] - a[0]) * double(c[1] - a[1]) - (b[1] - a[1]) * double(c[0] - a[0])) / 2;
            for (auto corner : face_chart(faces[face], area, density)) {
                indices.push_back(uint32_t(uvs.size()));
                uvs.push_back(corner);
            }
            materials.push_back(face + 1);
        }
        AtlasPtr atlas(xatlas::Create(), xatlas::Destroy);
        xatlas::UvMeshDecl mesh;
        mesh.vertexUvData = uvs.data();
        mesh.vertexStride = sizeof(Texel);
        mesh.vertexCount = uint32_t(uvs.size());
        mesh.indexData = indices.data();
        mesh.indexFormat = xatlas::IndexFormat::UInt32;
        mesh.indexCount = uint32_t(indices.size());
        mesh.faceMaterialData = materials.data();
        auto error = xatlas::AddUvMesh(atlas.get(), mesh);
        if (error != xatlas::AddMeshError::Success)
            throw std::runtime_error(xatlas::StringForEnum(error));
        xatlas::ComputeCharts(atlas.get());
        // The UV mesh is in the first pass's texels: a density of 1 keeps its
        // charts' size where they still fit.
        pack(atlas.get(), 1);
        const auto &packed = atlas->meshes[0];
        for (size_t i = 0; i < packed.indexCount; ++i)
            result[i / 3][i % 3] = quantize(packed.vertexArray[packed.indexArray[i]]);
    }
    return result;
}

static void unwrap(const char *source, const char *target) {
    std::ifstream input(source, std::ios::binary);
    if (!input)
        throw std::runtime_error("cannot open source mesh");
    Bytes bytes{std::istreambuf_iterator<char>(input), {}};
    uint32_t vertexBytes = read32(bytes, 4), vertexCount = read32(bytes, 8);
    size_t indexChunk = size_t(vertexBytes) + 8;
    uint32_t indexCount = read32(bytes, indexChunk + 8);
    if (read32(bytes, 0) != 3 || vertexCount == 0 || vertexBytes != 4 + uint64_t(vertexCount) * Stride)
        throw std::runtime_error("dual UV requires a native one-weight vertex chunk");
    if (read32(bytes, indexChunk) != 4 || indexCount == 0 || indexCount % 3 != 0 ||
        read32(bytes, indexChunk + 4) != 4 + uint64_t(indexCount) * 2 ||
        indexChunk + 12 + uint64_t(indexCount) * 2 != bytes.size())
        throw std::runtime_error("unsupported converted_model shape");
    std::vector<uint16_t> indices(indexCount);
    std::memcpy(indices.data(), bytes.data() + indexChunk + 12, size_t(indexCount) * 2);
    xatlas::MeshDecl mesh;
    mesh.vertexCount = vertexCount;
    // xatlas tests for zero triangle area in the units it receives. Some
    // separate sight surfaces are only fractions of a millimetre across.
    // Normalize the working copy; the game still gets the original vertices.
    std::vector<Position> positions(vertexCount);
    {
        Position lo{}, hi{};
        for (size_t i = 0; i < vertexCount; ++i) {
            std::memcpy(positions[i].data(), bytes.data() + 12 + i * Stride, 12);
            for (size_t axis = 0; axis < 3; ++axis) {
                const float x = positions[i][axis];
                if (!std::isfinite(x))
                    throw std::runtime_error("nonfinite position");
                lo[axis] = i ? std::min(lo[axis], x) : x;
                hi[axis] = i ? std::max(hi[axis], x) : x;
            }
        }
        const float extent = std::max({hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]});
        if (!(extent > 0))
            throw std::runtime_error("surface has no extent");
        for (auto &p : positions)
            for (size_t axis = 0; axis < 3; ++axis)
                p[axis] = (p[axis] - lo[axis]) * (1000.f / extent);
        mesh.vertexPositionData = positions.data();
        mesh.vertexPositionStride = sizeof(positions[0]);
    }
    mesh.vertexUvData = bytes.data() + 12 + Stride - 8;
    mesh.vertexUvStride = Stride;
    mesh.indexData = indices.data();
    mesh.indexCount = indexCount;
    AtlasPtr atlas(xatlas::Create(), xatlas::Destroy);
    auto error = xatlas::AddMesh(atlas.get(), mesh);
    if (error != xatlas::AddMeshError::Success)
        throw std::runtime_error(xatlas::StringForEnum(error));
    xatlas::ChartOptions charts;
    charts.useInputMeshUvs = true;
    xatlas::ComputeCharts(atlas.get(), charts);
    // From xatlas's estimate of the density that fills the atlas, the
    // highest that fits it in one, 3% a step.
    xatlas::PackOptions estimate;
    estimate.resolution = Resolution;
    xatlas::PackCharts(atlas.get(), estimate);
    pack(atlas.get(), atlas->texelsPerUnit);
    const auto &result = atlas->meshes[0];
    if (result.indexCount != indexCount)
        throw std::runtime_error("atlas changed the triangle count");
    for (size_t i = 0; i < indices.size(); ++i)
        if (result.vertexArray[result.indexArray[i]].xref != indices[i])
            throw std::runtime_error("atlas changed the triangle stream");
    std::vector<std::array<Position, 3>> faces(indexCount / 3);
    for (size_t i = 0; i < indexCount; ++i)
        faces[i / 3][i % 3] = positions[indices[i]];
    auto triangles = separate(result, atlas->texelsPerUnit, faces);
    using Key = std::tuple<uint16_t, uint16_t, uint16_t>;
    std::map<Key, uint32_t> byCorner;
    std::vector<std::array<uint8_t, Stride>> vertices;
    std::vector<uint16_t> outputIndices;
    for (size_t i = 0; i < indexCount; ++i) {
        auto uv = triangles[i / 3][i % 3];
        auto [entry, added] = byCorner.emplace(Key{indices[i], uv.x, uv.y}, vertices.size());
        if (added) {
            if (vertices.size() == 65536)
                throw std::runtime_error("mesh exceeds the native index format");
            std::array<uint8_t, Stride> v;
            std::memcpy(v.data(), bytes.data() + 12 + size_t(indices[i]) * Stride, v.size());
            if (v[13] || v[14] || v[15] || v[19] != 127 || v[23] != 255 || v[27] != 255)
                throw std::runtime_error("unexpected native one-weight vertex padding");
            // Only the XYZ tangent frame and first blend index are live in
            // 0.26g's one-weight vertex programs. The shader checks both tags.
            v[13] = uint8_t(uv.x);
            v[14] = uint8_t(uv.x >> 8);
            v[15] = uint8_t(uv.y);
            v[23] = uint8_t(uv.y >> 8);
            v[19] = 'S';
            v[27] = 'D';
            vertices.push_back(v);
        }
        outputIndices.push_back(uint16_t(entry->second));
    }
    std::ofstream out(target, std::ios::binary);
    out.exceptions(std::ios::badbit | std::ios::failbit);
    write(out, uint32_t(3));
    write(out, uint32_t(4 + vertices.size() * Stride));
    write(out, uint32_t(vertices.size()));
    for (const auto &v : vertices)
        out.write(reinterpret_cast<const char *>(v.data()), v.size());
    write(out, uint32_t(4));
    write(out, uint32_t(4 + 2 * indexCount));
    write(out, indexCount);
    for (auto index : outputIndices)
        write(out, index);
    out.close();
}

int main(int argc, char **argv) {
    try {
        if (argc != 3)
            throw std::runtime_error("usage: unwrap converted_model output");
        unwrap(argv[1], argv[2]);
    } catch (const std::exception &error) {
        std::cerr << error.what() << '\n';
        return 1;
    }
}
