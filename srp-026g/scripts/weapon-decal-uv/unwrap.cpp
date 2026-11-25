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
#include <tuple>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>

using Bytes = std::vector<uint8_t>;

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

static void unwrap(const char *source, const char *target, unsigned resolution) {
    std::ifstream input(source, std::ios::binary);
    if (!input)
        throw std::runtime_error("cannot open source mesh");
    Bytes bytes{std::istreambuf_iterator<char>(input), {}};
    uint32_t vertexBytes = read32(bytes, 4), vertexCount = read32(bytes, 8);
    if (read32(bytes, 0) != 3 || vertexCount == 0 || vertexBytes < 4 || (vertexBytes - 4) % vertexCount != 0)
        throw std::runtime_error("invalid vertex chunk");
    uint32_t stride = (vertexBytes - 4) / vertexCount;
    size_t indexChunk = size_t(vertexBytes) + 8;
    uint32_t indexCount = read32(bytes, indexChunk + 8);
    if (stride < 20 || stride % 4 != 0 || read32(bytes, indexChunk) != 4 || indexCount == 0 ||
        indexCount % 3 != 0 || read32(bytes, indexChunk + 4) != 4 + uint64_t(indexCount) * 2 ||
        indexChunk + 12 + uint64_t(indexCount) * 2 != bytes.size())
        throw std::runtime_error("unsupported converted_model shape");
    if (stride != 36)
        throw std::runtime_error("dual UV requires a native one-weight vertex");
    std::vector<uint16_t> indices(indexCount);
    std::memcpy(indices.data(), bytes.data() + indexChunk + 12, size_t(indexCount) * 2);
    xatlas::MeshDecl mesh;
    mesh.vertexCount = vertexCount;
    // xatlas tests for zero triangle area in the units it receives. Some
    // separate sight surfaces are only fractions of a millimetre across.
    // Normalize the working copy; the game still gets the original vertices.
    std::vector<std::array<float, 3>> positions(vertexCount);
    {
        std::array<float, 3> lo{}, hi{};
        for (size_t i = 0; i < vertexCount; ++i) {
            std::memcpy(positions[i].data(), bytes.data() + 12 + i * stride, 12);
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
    mesh.vertexUvData = bytes.data() + 12 + stride - 8;
    mesh.vertexUvStride = stride;
    mesh.indexData = indices.data();
    mesh.indexCount = indexCount;
    std::unique_ptr<xatlas::Atlas, decltype(&xatlas::Destroy)> atlas(xatlas::Create(), xatlas::Destroy);
    auto error = xatlas::AddMesh(atlas.get(), mesh);
    if (error != xatlas::AddMeshError::Success)
        throw std::runtime_error(xatlas::StringForEnum(error));
    xatlas::ChartOptions charts;
    charts.useInputMeshUvs = true;
    xatlas::PackOptions pack;
    pack.resolution = resolution;
    pack.padding = 8;
    pack.blockAlign = true;
    xatlas::Generate(atlas.get(), charts, pack);
    if (atlas->meshCount != 1 || atlas->atlasCount != 1 || !atlas->width || !atlas->height)
        throw std::runtime_error("expected one UV atlas");
    const auto &result = atlas->meshes[0];
    if (result.indexCount != indexCount)
        throw std::runtime_error("atlas changed the triangle count");
    for (size_t i = 0; i < indices.size(); ++i) {
        auto index = result.indexArray[i];
        if (index >= result.vertexCount || result.vertexArray[index].xref != indices[i])
            throw std::runtime_error("atlas changed the triangle stream");
    }
    std::vector<Triangle> triangles(indexCount / 3);
    for (size_t i = 0; i < indexCount; ++i) {
        const auto &v = result.vertexArray[result.indexArray[i]];
        double u = v.uv[0] / atlas->width, t = v.uv[1] / atlas->height;
        if (!(u >= 0 && u <= 1 && t >= 0 && t <= 1))
            throw std::runtime_error("decal UV outside the atlas");
        triangles[i / 3][i % 3] = {std::lround(u * 65535), std::lround(t * 65535)};
    }
    triangles = separate(std::move(triangles), indices, positions, resolution);
    using Key = std::tuple<uint16_t, uint16_t, uint16_t>;
    std::map<Key, uint32_t> byCorner;
    std::vector<std::array<uint8_t, 36>> vertices;
    std::vector<uint16_t> outputIndices;
    for (size_t i = 0; i < indexCount; ++i) {
        auto uv = triangles[i / 3][i % 3];
        auto [entry, added] = byCorner.emplace(Key{indices[i], uv.x, uv.y}, vertices.size());
        if (added) {
            if (vertices.size() == 65536)
                throw std::runtime_error("mesh exceeds the native index format");
            std::array<uint8_t, 36> v;
            std::memcpy(v.data(), bytes.data() + 12 + size_t(indices[i]) * stride, v.size());
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
    write(out, uint32_t(4 + vertices.size() * stride));
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
        if (argc != 4)
            throw std::runtime_error("usage: unwrap converted_model output resolution");
        if (std::string(argv[3]) != "2048")
            throw std::runtime_error("the native weapon bake requires a 2048-texel atlas");
        unwrap(argv[1], argv[2], 2048);
    } catch (const std::exception &error) {
        std::cerr << error.what() << '\n';
        return 1;
    }
}
