// Thrown grenades against the level's collision, as 0.26g sets them up
// (patch 105, ../docs/notes/grenade_ccd.md). Built against the Bullet 2.79 of
// the 0.10b decompilation (~/Projects/survarium/vostok/sources/bullet: its
// integrateTransforms keeps vostok's CCD branch), every file with BT_USE_SSE
// as the game's MSVC build has it:
//
//   cd ~/Projects/survarium/vostok/sources/bullet
//   F="-std=gnu++98 -O2 -w -fpermissive -DBT_USE_SSE -include xmmintrin.h -I."
//   g++ $F <this file> $(find LinearMath BulletCollision BulletDynamics -name '*.cpp') -o sim
//   ./sim 0.01 2>&1 >/dev/null; ./sim 0.04 2>&1 >/dev/null   (stdout: the branch's printf)
//
// The game's values: the world (bullet_physics_world::initialize 0x74B20:
// btAxisSweep3 +-1000, gravity -10), its step (tick 0x75550: stepSimulation(0,
// 0, 1/60), then (ms / 1000, 3, 1/60), ms 50), the grenade (grenade_core::load
// 0x303890: capsule r 0.05 h 0.1, mass 0.3, friction 0.5, restitution 0,
// ccd threshold 0.03, swept sphere argv[1]; throw 0x303C10: group 818h mask 8,
// no spin) and the level's collision bullets hit (group 8, mask 10h, a
// triangle mesh).
#include "btBulletDynamicsCommon.h"
#include <cmath>
#include <cstdio>
#include <cstdlib>

static unsigned seed = 12345;
static float frand() { seed = seed * 1664525u + 1013904223u; return (seed >> 8) / 16777216.0f; }
static const float PI = 3.14159265f;

struct Scene { int fell, through, total; };

// cells x cells of `step` m, heights in +-bump; with a wall, the plane x = 4
// over a flat floor. A throw fell when it ends 2 cm under the floor inside it,
// went through when it ends past the wall.
static Scene run(float swept, float step, float bump, bool wall, float speed, float pmin, float pmax, int throws)
{
    btDefaultCollisionConfiguration cfg;
    btCollisionDispatcher dispatcher(&cfg);
    btAxisSweep3 broadphase(btVector3(-1000, -1000, -1000), btVector3(1000, 1000, 1000), 32766);
    btSequentialImpulseConstraintSolver solver;
    btDiscreteDynamicsWorld world(&dispatcher, &broadphase, &solver, &cfg);
    world.setGravity(btVector3(0, -10, 0));
    const int n = (int)(60 / step) + 1;
    float* h = new float[n * n];
    for (int i = 0; i < n * n; i++) h[i] = wall ? 0 : bump * (frand() * 2 - 1);
    btTriangleMesh mesh;
    for (int j = 0; j < n - 1; j++)
        for (int i = 0; i < n - 1; i++) {
            float x = -30 + i * step, z = -30 + j * step;
            btVector3 a(x, h[j * n + i], z), b(x + step, h[j * n + i + 1], z);
            btVector3 c(x, h[(j + 1) * n + i], z + step), d(x + step, h[(j + 1) * n + i + 1], z + step);
            mesh.addTriangle(a, b, c); mesh.addTriangle(b, d, c);
        }
    if (wall)
        for (int j = 0; j < 10; j++)
            for (int i = -30; i < 30; i++) {
                btVector3 a(4, j, i), b(4, j, i + 1), c(4, j + 1, i), d(4, j + 1, i + 1);
                mesh.addTriangle(a, b, c); mesh.addTriangle(b, d, c);
            }
    btBvhTriangleMeshShape level(&mesh, true);
    btRigidBody levelBody(0, 0, &level);
    world.addRigidBody(&levelBody, 8, 16);
    btCapsuleShape capsule(0.05f, 0.1f);
    btVector3 inertia; capsule.calculateLocalInertia(0.3f, inertia);
    Scene s = {0, 0, 0};
    for (int t = 0; t < throws; t++) {
        btRigidBody::btRigidBodyConstructionInfo info(0.3f, 0, &capsule, inertia);
        info.m_friction = 0.5f; info.m_restitution = 0;
        btRigidBody body(info);
        float yaw = wall ? (frand() * 90 - 45) * PI / 180 : frand() * 2 * PI;
        float pitch = (pmin + frand() * (pmax - pmin)) * PI / 180;
        btVector3 start(wall ? 0 : frand() * 4 - 2, 0, frand() * 4 - 2);
        start.setY(1.6f + (wall ? 0 : h[(int)((start.z() + 30) / step) * n + (int)((start.x() + 30) / step)]));
        btTransform tr; tr.setIdentity(); tr.setOrigin(start);
        tr.setRotation(btQuaternion(btVector3(1, 0, 0), frand() * 2 * PI) * btQuaternion(btVector3(0, 0, 1), frand() * 2 * PI));
        body.setWorldTransform(tr);
        body.setCcdMotionThreshold(0.03f); body.setCcdSweptSphereRadius(swept);
        world.addRigidBody(&body, 0x818, 8);
        body.setLinearVelocity(btVector3(cosf(pitch) * cosf(yaw), sinf(pitch), cosf(pitch) * sinf(yaw)) * speed);
        bool fell = false, through = false;
        for (int tick = 0; tick < 100 && !fell && !through; tick++) {
            world.stepSimulation(0, 0, 1.f / 60);
            world.stepSimulation(0.05f, 3, 1.f / 60);
            btVector3 o = body.getWorldTransform().getOrigin();
            bool inside = fabsf(o.x()) < 29 && fabsf(o.z()) < 29;
            fell = inside && o.y() < -1.5f;                        // a metre under the lowest ground
            through = wall && inside && o.x() > 4.1f;
        }
        btVector3 o = body.getWorldTransform().getOrigin();
        world.removeRigidBody(&body);
        if (!fell && !through && (fabsf(o.x()) > 29 || fabsf(o.z()) > 29)) continue;   // left over the edge
        s.total++;
        s.fell += fell;
        s.through += through;
    }
    delete[] h;
    return s;
}

int main(int argc, char** argv)
{
    float swept = argc > 1 ? (float)atof(argv[1]) : 0.01f;
    struct { const char* what; float step, bump; bool wall; float speed, pmin, pmax; } cases[] = {
        {"flat ground, 4 m triangles", 4, 0, false, 17, -40, 40},
        {"rough ground, 1 m triangles", 1, 0.3f, false, 17, -40, 40},
        {"rough ground, thrown down", 1, 0.3f, false, 17, -70, -20},
        {"rough ground, a short lob", 1, 0.3f, false, 6, -10, 30},
        {"a wall 4 m ahead", 1, 0, true, 17, -20, 30},
    };
    for (unsigned i = 0; i < sizeof cases / sizeof cases[0]; i++) {
        Scene s = run(swept, cases[i].step, cases[i].bump, cases[i].wall, cases[i].speed, cases[i].pmin, cases[i].pmax, 1000);
        fprintf(stderr, "swept sphere %.2f m, %-28s %4d of %4d under the ground (%4.1f %%), %3d through the wall\n",
                swept, cases[i].what, s.fell, s.total, 100.0 * s.fell / s.total, s.through);
    }
}
