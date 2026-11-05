// A battery dropped at its carrier's death, against the level's collision, as
// 0.26g sets it up (../docs/notes/battery_ccd.md). Built like grenade_ccd_sim.cpp
// (Bullet 2.79 of the 0.10b decompilation, every file with BT_USE_SSE):
//
//   cd ~/Projects/survarium/vostok/sources/bullet
//   F="-std=gnu++98 -O2 -w -fpermissive -DBT_USE_SSE -include xmmintrin.h -I."
//   g++ $F <this file> $(find LinearMath BulletCollision BulletDynamics -name '*.cpp') -o sim
//   ./sim 0.01 0.01 2>&1 >/dev/null; ./sim <swept> <threshold> 2>&1 >/dev/null
//
// The game's values: the world and its step as grenade_ccd_sim.cpp; the battery
// (victory_item_core 0x3000A0): a box of half extents 0.9 x its config's
// collision mesh scale (gameplay/victory_items/default.options: 0.3 0.12 0.16),
// mass 10, friction 1, restitution 0, sleeping thresholds 2 and 4, ccd threshold
// and swept sphere 0.01 (0x300468, argv); activate_physics 0x301CB0: group A18h,
// mask 418h, at rest where the carrier's hands held it (drop 0x300E50).
#include "btBulletDynamicsCommon.h"
#include <cmath>
#include <cstdio>
#include <cstdlib>

static unsigned seed = 4242;
static float frand() { seed = seed * 1664525u + 1013904223u; return (seed >> 8) / 16777216.0f; }
static const float PI = 3.14159265f;

struct Scene { int fell, total; double slide, tilt; };

// ground: cells of `step` m, heights in +-bump, the whole tilted by `slope`
// degrees about z; a drop falls from `hmin`..`hmax` over the ground at rest.
static Scene run(float swept, float thresh, float step, float bump, float slope, float hmin, float hmax, int drops)
{
    btDefaultCollisionConfiguration cfg;
    btCollisionDispatcher dispatcher(&cfg);
    btAxisSweep3 broadphase(btVector3(-1000, -1000, -1000), btVector3(1000, 1000, 1000), 32766);
    btSequentialImpulseConstraintSolver solver;
    btDiscreteDynamicsWorld world(&dispatcher, &broadphase, &solver, &cfg);
    world.setGravity(btVector3(0, -10, 0));
    const int n = (int)(40 / step) + 1;
    const float k = tanf(slope * PI / 180);
    float* h = new float[n * n];
    for (int j = 0; j < n; j++)
        for (int i = 0; i < n; i++) h[j * n + i] = bump * (frand() * 2 - 1) + k * (-20 + i * step);
    btTriangleMesh mesh;
    for (int j = 0; j < n - 1; j++)
        for (int i = 0; i < n - 1; i++) {
            float x = -20 + i * step, z = -20 + j * step;
            btVector3 a(x, h[j * n + i], z), b(x + step, h[j * n + i + 1], z);
            btVector3 c(x, h[(j + 1) * n + i], z + step), d(x + step, h[(j + 1) * n + i + 1], z + step);
            mesh.addTriangle(a, b, c); mesh.addTriangle(b, d, c);
        }
    btBvhTriangleMeshShape level(&mesh, true);
    btRigidBody levelBody(0, 0, &level);
    world.addRigidBody(&levelBody, 8, 16);
    btBoxShape box(btVector3(0.3f, 0.12f, 0.16f) * 0.9f);
    btVector3 inertia; box.calculateLocalInertia(10, inertia);
    Scene s = {0, 0, 0, 0};
    for (int t = 0; t < drops; t++) {
        btRigidBody::btRigidBodyConstructionInfo info(10, 0, &box, inertia);
        info.m_friction = 1; info.m_restitution = 0;
        info.m_linearSleepingThreshold = 2; info.m_angularSleepingThreshold = 4;
        btRigidBody body(info);
        float x = frand() * 10 - 5, z = frand() * 10 - 5;
        float i = (x + 20) / step, j = (z + 20) / step;
        int ci = (int)i, cj = (int)j;                                  // the cell's highest corner
        float ground = btMax(btMax(h[cj * n + ci], h[cj * n + ci + 1]), btMax(h[(cj + 1) * n + ci], h[(cj + 1) * n + ci + 1]));
        btTransform tr; tr.setIdentity();
        tr.setOrigin(btVector3(x, ground + hmin + frand() * (hmax - hmin), z));
        tr.setRotation(btQuaternion(btVector3(0, 1, 0), frand() * 2 * PI)
                       * btQuaternion(btVector3(1, 0, 0), (frand() * 2 - 1) * 0.6f)
                       * btQuaternion(btVector3(0, 0, 1), (frand() * 2 - 1) * 0.6f));
        body.setWorldTransform(tr);
        body.setCcdMotionThreshold(thresh); body.setCcdSweptSphereRadius(swept);
        world.addRigidBody(&body, 0xA18, 0x418);
        btVector3 start = tr.getOrigin();
        bool fell = false;
        for (int tick = 0; tick < 100 && !fell; tick++) {
            world.stepSimulation(0, 0, 1.f / 60);
            world.stepSimulation(0.05f, 3, 1.f / 60);
            btVector3 o = body.getWorldTransform().getOrigin();
            int oi = (int)((o.x() + 20) / step), oj = (int)((o.z() + 20) / step);
            bool inside = oi >= 0 && oj >= 0 && oi < n - 1 && oj < n - 1;
            fell = inside && o.y() < h[oj * n + oi] - 1 - bump - k * step;     // a metre under the ground there
        }
        btVector3 o = body.getWorldTransform().getOrigin();
        world.removeRigidBody(&body);
        if (!fell && (fabsf(o.x()) > 19 || fabsf(o.z()) > 19)) continue;   // slid off the mesh's edge
        s.total++;
        if (fell) { s.fell++; continue; }
        btVector3 d = o - start; d.setY(0);
        s.slide += d.length();
        s.tilt += acosf(btMin(1.f, fabsf(body.getWorldTransform().getBasis().getColumn(1).y()))) * 180 / PI;
    }
    delete[] h;
    return s;
}

int main(int argc, char** argv)
{
    float swept = argc > 1 ? (float)atof(argv[1]) : 0.01f;
    float thresh = argc > 2 ? (float)atof(argv[2]) : 0.01f;
    if (argc > 3) seed = (unsigned)atoi(argv[3]);                       // another set of drops
    struct { const char* what; float step, bump, slope, hmin, hmax; } cases[] = {
        {"flat floor, 4 m triangles", 4, 0, 0, 0.9f, 1.3f},
        {"rough ground, 1 m triangles", 1, 0.3f, 0, 0.9f, 1.3f},
        {"crouched carrier, rough", 1, 0.3f, 0, 0.5f, 0.8f},
        {"a 25 degree slope", 2, 0.1f, 25, 0.9f, 1.3f},
        {"from a ledge, 3 m", 4, 0, 0, 3, 3.5f},
    };
    for (unsigned c = 0; c < sizeof cases / sizeof cases[0]; c++) {
        Scene s = run(swept, thresh, cases[c].step, cases[c].bump, cases[c].slope, cases[c].hmin, cases[c].hmax, 1000);
        int kept = s.total - s.fell;
        fprintf(stderr, "swept %.3f threshold %.3f  %-28s %4d of %4d under the ground (%4.1f %%)  slide %.2f m  tilt %4.1f deg\n",
                swept, thresh, cases[c].what, s.fell, s.total, 100.0 * s.fell / s.total,
                kept ? s.slide / kept : 0, kept ? s.tilt / kept : 0);
    }
}
