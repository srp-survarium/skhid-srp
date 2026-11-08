// Free batteries after a world load, as 0.26g steps them, stock and with
// patch 121 (the contact manifolds saved with the world): Bullet 2.79 of the
// 0.10b decompilation, the game's world (bullet_physics_world::initialize
// 0x74B20: btAxisSweep3, gravity -10, 2 solver iterations, solver mode
// SOLVER_SIMD), its step (tick 0x75550: stepSimulation(0,0,1/60) then
// (0.05,3,1/60)), its near callback (0x8F860: the body nearer the world's
// origin first, the pair's algorithm made again when that order flips) and the
// battery's body (a box, mass 8, CCD 0.01/0.01).
//
// A world is what victory_item_core::serialize keeps of a battery (in the
// world or carried; transform, velocities, deactivation time, activation
// state), and with the fix also every contact manifold of a battery that has
// points: its two bodies by key, which of them the near callback passes first,
// and the points.
//
// Games: A steps straight (its first battery taken out of the world and put
// back before the match, as a carry does, so its broadphase and arrays have a
// history of their own); B is A's twin that every `every` ticks loads its own
// world of `back` ticks ago (a rollback); C starts at tick `join` from A's
// world (a joiner or a resync: a fresh physics world). With `drop`, the last
// battery is carried until that tick, then falls on the first (a carrier's
// death). Each tick compares B and C with A: every body bit for bit, and with
// the fix the contacts too (they are in the world and its CRC).
//
// build (from ~/Projects/survarium/vostok/sources/bullet):
//   F="-O2 -w -fpermissive -DBT_USE_SSE -include xmmintrin.h -I."
//   g++ -std=gnu++11 $F contacts_sim.cpp $(find LinearMath BulletCollision BulletDynamics -name '*.cpp') -o csim
// usage: csim <fix 0|1> <seed> [batteries] [every] [back] [join] [drop]
// (NOSORT=1: the fix without its solveGroup order)
//
// 300 seeds each (batteries 1-3, a rollback every 2-5 ticks of 1-6 ticks, a
// join at 8-67; then 2-3 batteries, the last dropped on the first at 95 and a
// join at 40-119), 2026-11-08: stock, the rollback twin apart in 299 and 300,
// the joiner in 240 and 293; fixed, 0 and 0, and 0 and 0; the fix without the
// order, 27 and 5, 11 and 13.
#include "btBulletDynamicsCommon.h"
#include <algorithm>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <vector>

static unsigned seed = 1;
static float frand() { seed = seed * 1664525u + 1013904223u; return (seed >> 8) / 16777216.0f; }

static bool FIX = false;

// A body's key: a battery by its origin's bits (-0 as 0), the level by a tag.
struct Key { unsigned tag, a, b, c; };
static bool operator==(const Key& x, const Key& y) { return x.tag == y.tag && x.a == y.a && x.b == y.b && x.c == y.c; }
static bool operator<(const Key& x, const Key& y) {
    if (x.tag != y.tag) return x.tag < y.tag; if (x.a != y.a) return x.a < y.a; if (x.b != y.b) return x.b < y.b; return x.c < y.c;
}
static unsigned bits(float f) { unsigned u; memcpy(&u, &f, 4); return u == 0x80000000u ? 0 : u; }
static Key keyOf(const btCollisionObject* o) {
    if (o->isStaticObject()) { Key k = {2, 0, 0, 0}; return k; }
    const btVector3& p = o->getWorldTransform().getOrigin();
    Key k = {1, bits(p.x()), bits(p.y()), bits(p.z())}; return k;
}

// A manifold's record: its bodies in its order, whether the near callback
// passes body 0 first, its points.
struct Record { Key k0, k1; bool first0; int n; btManifoldPoint p[4]; };

// What a load leaves to the step: the records no manifold took yet, each
// once (`into`: the manifold a getNewManifold put it in, which the
// algorithm's constructor may empty again).
struct Pending {
    std::vector<Record> r; std::vector<bool> used; bool fresh;
    void clear() { r.clear(); used.clear(); fresh = false; }
};

static void fill(btPersistentManifold* m, const Record& r) {
    m->clearManifold();
    for (int k = 0; k < r.n; k++) m->addManifoldPoint(r.p[k]);
}
static bool samePair(btPersistentManifold* m, const btCollisionObject* a, const btCollisionObject* b) {
    return (m->getBody0() == a && m->getBody1() == b) || (m->getBody0() == b && m->getBody1() == a);
}
static btPersistentManifold* manifoldOf(btDispatcher* d, const btCollisionObject* a, const btCollisionObject* b) {
    for (int j = d->getNumManifolds() - 1; j >= 0; j--)
        if (samePair(d->getManifoldByIndexInternal(j), a, b)) return d->getManifoldByIndexInternal(j);
    return 0;
}
static const btCollisionObject* firstOf(const btBroadphasePair* p) {
    return (const btCollisionObject*)(p->m_internalInfo1 ? p->m_pProxy1 : p->m_pProxy0)->m_clientObject;
}
// The near callback's order now: the body nearer the world's origin first.
static int swappedNow(const btBroadphasePair* p) {
    const btCollisionObject* o0 = (const btCollisionObject*)p->m_pProxy0->m_clientObject;
    const btCollisionObject* o1 = (const btCollisionObject*)p->m_pProxy1->m_clientObject;
    return o0->getWorldTransform().getOrigin().length2() > o1->getWorldTransform().getOrigin().length2() ? 1 : 0;
}

static Pending* current = 0;

// The pair whose processCollision runs (the near callback's, 0x8F9C2 on).
static const btBroadphasePair* processing = 0;

struct Dispatcher : btCollisionDispatcher {
    Dispatcher(btCollisionConfiguration* c) : btCollisionDispatcher(c) {}
    // a manifold a processCollision makes (btConvexConvexAlgorithm makes its
    // own in its first): the record of its bodies in that order, when the
    // near callback's order is the record's (body 0 first: it is then)
    virtual btPersistentManifold* getNewManifold(void* b0, void* b1) {
        btPersistentManifold* m = btCollisionDispatcher::getNewManifold(b0, b1);
        if (FIX && current && processing) {
            Key k0 = keyOf((btCollisionObject*)b0), k1 = keyOf((btCollisionObject*)b1);
            for (size_t i = 0; i < current->r.size(); i++)
                if (!current->used[i] && current->r[i].k0 == k0 && current->r[i].k1 == k1) {
                    current->used[i] = true;
                    if (current->r[i].first0) fill(m, current->r[i]);
                    break;
                }
        }
        return m;
    }
};

// Before a pair's processCollision (0x8F9C2): a record of its two bodies goes
// into their manifold (the algorithm's, made now or kept) when the near
// callback's order and the manifold's are the record's; else it is spent (the
// straight game made its algorithm again then: empty).
static void injectPair(btCollisionDispatcher& d, btBroadphasePair& pair) {
    if (!FIX || !current || current->r.empty()) return;
    btCollisionObject* o0 = (btCollisionObject*)pair.m_pProxy0->m_clientObject;
    btCollisionObject* o1 = (btCollisionObject*)pair.m_pProxy1->m_clientObject;
    Key k0 = keyOf(o0), k1 = keyOf(o1);
    for (size_t i = 0; i < current->r.size(); i++) {
        const Record& r = current->r[i];
        if (current->used[i] || !((r.k0 == k0 && r.k1 == k1) || (r.k0 == k1 && r.k1 == k0))) continue;
        btPersistentManifold* m = manifoldOf(&d, o0, o1);
        if (!m) continue;   // made in processCollision: getNewManifold's
        current->used[i] = true;
        bool first0 = keyOf(firstOf(&pair)) == r.k0;
        if (keyOf((btCollisionObject*)m->getBody0()) == r.k0 && first0 == r.first0) fill(m, r);
    }
}

// The near callback of 0x8F860.
static void nearCallback(btBroadphasePair& pair, btCollisionDispatcher& d, const btDispatcherInfo& info) {
    btCollisionObject* o0 = (btCollisionObject*)pair.m_pProxy0->m_clientObject;
    btCollisionObject* o1 = (btCollisionObject*)pair.m_pProxy1->m_clientObject;
    if (!d.needsCollision(o0, o1)) return;
    int swapped = swappedNow(&pair);
    if (pair.m_algorithm && pair.m_internalInfo1 != (void*)(size_t)swapped) {
        pair.m_algorithm->~btCollisionAlgorithm(); d.freeCollisionAlgorithm(pair.m_algorithm); pair.m_algorithm = 0;
    }
    btCollisionObject* first = swapped ? o1 : o0; btCollisionObject* second = swapped ? o0 : o1;
    if (!pair.m_algorithm) { pair.m_internalInfo1 = (void*)(size_t)swapped; pair.m_algorithm = d.findAlgorithm(first, second); }
    if (!pair.m_algorithm) return;
    injectPair(d, pair);
    processing = &pair;
    btManifoldResult result(first, second);
    pair.m_algorithm->processCollision(first, second, info, &result);
    processing = 0;
}

// Every island's manifolds in the keys' order (solveGroup's entry).
static bool manifoldLess(btPersistentManifold* x, btPersistentManifold* y) {
    Key a0 = keyOf((btCollisionObject*)x->getBody0()), a1 = keyOf((btCollisionObject*)x->getBody1());
    Key b0 = keyOf((btCollisionObject*)y->getBody0()), b1 = keyOf((btCollisionObject*)y->getBody1());
    if (!(a0 == b0)) return a0 < b0; return a1 < b1;
}
static bool SORT = true;
struct Solver : btSequentialImpulseConstraintSolver {
    virtual btScalar solveGroup(btCollisionObject** bodies, int nb, btPersistentManifold** m, int nm, btTypedConstraint** c, int nc,
                                const btContactSolverInfo& info, btIDebugDraw* dd, btStackAlloc* sa, btDispatcher* d) {
        if (FIX && SORT) std::stable_sort(m, m + nm, manifoldLess);
        return btSequentialImpulseConstraintSolver::solveGroup(bodies, nb, m, nm, c, nc, info, dd, sa, d);
    }
};

struct World_ : btDiscreteDynamicsWorld {
    btCollisionDispatcher* d; Pending* pending;
    World_(btCollisionDispatcher* dd, btBroadphaseInterface* b, btConstraintSolver* s, btCollisionConfiguration* c)
        : btDiscreteDynamicsWorld(dd, b, s, c), d(dd), pending(0) {}
    btCollisionObject* byKey(const Key& k) {
        for (int i = 0; i < getNumCollisionObjects(); i++) if (keyOf(getCollisionObjectArray()[i]) == k) return getCollisionObjectArray()[i];
        return 0;
    }
    btBroadphasePair* pairOf(const btCollisionObject* a, const btCollisionObject* b) {
        if (!a->getBroadphaseHandle() || !b->getBroadphaseHandle()) return 0;
        return getBroadphase()->getOverlappingPairCache()->findPair(a->getBroadphaseHandle(), b->getBroadphaseHandle());
    }
    // A pair's algorithm made as the near callback makes it, in the record's
    // order, and the record in its manifold. False when the algorithm makes
    // its manifold in its first processCollision (getNewManifold's then).
    bool make(btBroadphasePair* pair, const Record& r, btCollisionObject* b0, btCollisionObject* b1) {
        btCollisionObject* first = r.first0 ? b0 : b1; btCollisionObject* second = r.first0 ? b1 : b0;
        pair->m_internalInfo1 = (void*)(size_t)(pair->m_pProxy0->m_clientObject == first ? 0 : 1);
        // awake while it is made: an algorithm whose constructor makes its
        // manifold makes none for two sleeping bodies (needsCollision)
        int s0 = b0->getActivationState(), s1 = b1->getActivationState();
        b0->forceActivationState(ACTIVE_TAG); b1->forceActivationState(ACTIVE_TAG);
        pair->m_algorithm = d->findAlgorithm(first, second);
        b0->forceActivationState(s0); b1->forceActivationState(s1);
        btPersistentManifold* m = pair->m_algorithm ? manifoldOf(d, b0, b1) : 0;
        if (!m) return false;
        if (m->getBody0() == b0) fill(m, r);
        return true;
    }
    // After the first collision detection since a load (before the islands):
    // a record whose pair the near callback did not take (asleep) goes into
    // its algorithm, of the record's order (made again when it is not).
    virtual void calculateSimulationIslands() {
        if (FIX && pending && pending->fresh) {
            pending->fresh = false;
            for (size_t i = 0; i < pending->r.size(); i++) {
                if (pending->used[i]) continue;
                const Record& r = pending->r[i];
                btCollisionObject *b0 = byKey(r.k0), *b1 = byKey(r.k1);
                btBroadphasePair* pair = b0 && b1 ? pairOf(b0, b1) : 0;
                if (!pair) continue;   // the bodies apart: the straight game's pair is gone too
                btPersistentManifold* m = pair->m_algorithm ? manifoldOf(d, b0, b1) : 0;
                if (m && m->getBody0() == b0 && (firstOf(pair) == b0) == r.first0) { fill(m, r); pending->used[i] = true; continue; }
                if (pair->m_algorithm) {   // of the other order: made again in the record's
                    pair->m_algorithm->~btCollisionAlgorithm(); d->freeCollisionAlgorithm(pair->m_algorithm); pair->m_algorithm = 0;
                }
                pending->used[i] = make(pair, r, b0, b1);
            }
        }
        btDiscreteDynamicsWorld::calculateSimulationIslands();
    }
};

struct BodyState { btTransform tr; btVector3 v, w; float dt; int act; bool in; };
struct World { std::vector<BodyState> b; std::vector<Record> contacts; };

static bool sameRecord(const Record& x, const Record& y) {
    if (!(x.k0 == y.k0) || !(x.k1 == y.k1) || x.first0 != y.first0 || x.n != y.n) return false;
    for (int i = 0; i < x.n; i++) {
        const btManifoldPoint &p = x.p[i], &q = y.p[i];
        if (memcmp(&p.m_localPointA, &q.m_localPointA, 12) || memcmp(&p.m_localPointB, &q.m_localPointB, 12) ||
            memcmp(&p.m_positionWorldOnA, &q.m_positionWorldOnA, 12) || memcmp(&p.m_positionWorldOnB, &q.m_positionWorldOnB, 12) ||
            memcmp(&p.m_normalWorldOnB, &q.m_normalWorldOnB, 12) || p.m_distance1 != q.m_distance1 || p.m_lifeTime != q.m_lifeTime ||
            p.m_appliedImpulse != q.m_appliedImpulse || p.m_combinedFriction != q.m_combinedFriction) return false;
    }
    return true;
}

struct Game {
    btDefaultCollisionConfiguration cfg;
    Dispatcher dispatcher;
    btAxisSweep3 broadphase;
    Solver solver;
    World_ world;
    btTriangleMesh mesh;
    btBvhTriangleMeshShape* level;
    btRigidBody* levelBody;
    btBoxShape box;
    std::vector<btRigidBody*> body;
    std::vector<bool> in;
    Pending pending;
    int carried, dropAt;
    Game(const float* h, int n, float step, int batteries)
        : dispatcher(&cfg), broadphase(btVector3(-1000, -1000, -1000), btVector3(1000, 1000, 1000), 32766),
          world(&dispatcher, &broadphase, &solver, &cfg), box(btVector3(0.15f, 0.12f, 0.22f)), carried(-1), dropAt(0)
    {
        world.setGravity(btVector3(0, -10, 0));
        world.getSolverInfo().m_numIterations = 2; world.getSolverInfo().m_solverMode = 0x100;
        dispatcher.setNearCallback(nearCallback);
        world.pending = &pending; pending.fresh = false;
        for (int j = 0; j < n - 1; j++)
            for (int i = 0; i < n - 1; i++) {
                float x = -10 + i * step, z = -10 + j * step;
                btVector3 a(x, h[j * n + i], z), b(x + step, h[j * n + i + 1], z);
                btVector3 c(x, h[(j + 1) * n + i], z + step), d(x + step, h[(j + 1) * n + i + 1], z + step);
                mesh.addTriangle(a, b, c); mesh.addTriangle(b, d, c);
            }
        level = new btBvhTriangleMeshShape(&mesh, true);
        levelBody = new btRigidBody(0, 0, level);
        world.addRigidBody(levelBody, 8, 0x10 | 0x800);
        btVector3 inertia; box.calculateLocalInertia(8.f, inertia);
        for (int i = 0; i < batteries; i++) {
            btRigidBody::btRigidBodyConstructionInfo info(8.f, 0, &box, inertia);
            btRigidBody* r = new btRigidBody(info);
            r->setCcdMotionThreshold(0.01f); r->setCcdSweptSphereRadius(0.01f);
            body.push_back(r); in.push_back(true);
            world.addRigidBody(r, 0xA18, 0x418);
        }
    }
    void setIn(size_t i, bool want) {
        if (in[i] == want) return;
        if (want) world.addRigidBody(body[i], 0xA18, 0x418); else world.removeRigidBody(body[i]);
        in[i] = want;
    }
    // victory_item_core's reader (0x301660): in or out of the world, then set_transform, velocities, +E8h, +E4h
    void place(const std::vector<BodyState>& s) {
        for (size_t i = 0; i < body.size(); i++) {
            btRigidBody* r = body[i];
            setIn(i, s[i].in);
            r->setCenterOfMassTransform(s[i].tr);
            r->setLinearVelocity(s[i].v); r->setAngularVelocity(s[i].w);
            r->setDeactivationTime(s[i].dt); r->forceActivationState(s[i].act);
        }
    }
    static bool moving(const btCollisionObject* o) { return !o->isStaticObject(); }
    World save() {
        World w;
        for (size_t i = 0; i < body.size(); i++) {
            BodyState s; btRigidBody* r = body[i];
            s.tr = r->getWorldTransform(); s.v = r->getLinearVelocity(); s.w = r->getAngularVelocity();
            s.dt = r->getDeactivationTime(); s.act = r->getActivationState(); s.in = in[i];
            w.b.push_back(s);
        }
        if (FIX) {
            for (int i = 0; i < dispatcher.getNumManifolds(); i++) {
                btPersistentManifold* m = dispatcher.getManifoldByIndexInternal(i);
                btCollisionObject* b0 = (btCollisionObject*)m->getBody0();
                btCollisionObject* b1 = (btCollisionObject*)m->getBody1();
                if (!m->getNumContacts() || !(moving(b0) || moving(b1))) continue;
                Record r; r.k0 = keyOf(b0); r.k1 = keyOf(b1); r.n = m->getNumContacts();
                btBroadphasePair* pair = world.pairOf(b0, b1);
                r.first0 = !pair || firstOf(pair) == b0;
                for (int k = 0; k < r.n; k++) { r.p[k] = m->getContactPoint(k); r.p[k].m_userPersistentData = 0; }
                w.contacts.push_back(r);
            }
            // the records no manifold took yet are the world's too
            for (size_t i = 0; i < pending.r.size(); i++) if (!pending.used[i]) w.contacts.push_back(pending.r[i]);
            std::sort(w.contacts.begin(), w.contacts.end(), [](const Record& x, const Record& y) {
                if (!(x.k0 == y.k0)) return x.k0 < y.k0; return x.k1 < y.k1; });
        }
        pending.clear();
        return w;
    }
    // the end of a world's load (game_world_core::deserialize 0x2D2D5D)
    void load(const World& w) {
        place(w.b);
        if (!FIX) return;
        pending.clear();
        pending.r = w.contacts; pending.used.assign(w.contacts.size(), false);
        pending.fresh = true;
        // every manifold of a moving body empty
        for (int i = 0; i < dispatcher.getNumManifolds(); i++) {
            btPersistentManifold* m = dispatcher.getManifoldByIndexInternal(i);
            if (moving((btCollisionObject*)m->getBody0()) || moving((btCollisionObject*)m->getBody1())) m->clearManifold();
        }
    }
    void tick(int t) {
        if (carried >= 0 && t == dropAt) {   // the carrier dies: the battery falls on the first one
            btTransform tr = body[0]->getWorldTransform(); tr.setOrigin(tr.getOrigin() + btVector3(0.05f, 1.2f, -0.03f));
            setIn(carried, true);
            body[carried]->setCenterOfMassTransform(tr);
            body[carried]->setLinearVelocity(btVector3(0, -2, 0)); body[carried]->setAngularVelocity(btVector3(1, 0, 0.5f));
            body[carried]->setDeactivationTime(0); body[carried]->forceActivationState(1);
        }
        current = &pending;
        world.stepSimulation(0, 0, 1.f / 60); world.stepSimulation(0.05f, 3, 1.f / 60);
    }
};

static bool sameBodies(const World& x, const World& y) {
    for (size_t i = 0; i < x.b.size(); i++) {
        const BodyState &p = x.b[i], &q = y.b[i];
        if (p.in != q.in) return false;
        for (int r = 0; r < 3; r++) if (memcmp(&p.tr.getBasis()[r], &q.tr.getBasis()[r], 12)) return false;
        if (memcmp(&p.tr.getOrigin(), &q.tr.getOrigin(), 12) || memcmp(&p.v, &q.v, 12) || memcmp(&p.w, &q.w, 12) || p.dt != q.dt || p.act != q.act) return false;
    }
    return true;
}
static bool sameContacts(const World& x, const World& y) {
    if (x.contacts.size() != y.contacts.size()) return false;
    for (size_t i = 0; i < x.contacts.size(); i++) if (!sameRecord(x.contacts[i], y.contacts[i])) return false;
    return true;
}

int main(int argc, char** argv)
{
    FIX = argc > 1 && atoi(argv[1]);
    SORT = !getenv("NOSORT");
    int seed0 = argc > 2 ? atoi(argv[2]) : 1; seed = seed0;
    int batteries = argc > 3 ? atoi(argv[3]) : 2, every = argc > 4 ? atoi(argv[4]) : 3, back = argc > 5 ? atoi(argv[5]) : 4;
    int join = argc > 6 ? atoi(argv[6]) : 12, drop = argc > 7 ? atoi(argv[7]) : 0;
    const float step = 1.0f; const int n = 21;
    std::vector<float> h(n * n); for (int i = 0; i < n * n; i++) h[i] = 0.3f * (frand() * 2 - 1);
    std::vector<BodyState> start;
    for (int i = 0; i < batteries; i++) {   // close together, so that they meet
        BodyState s; s.tr.setIdentity();
        s.tr.setOrigin(btVector3(frand() * 0.6f - 0.3f + i * 0.35f, 1.0f + frand() * 2 + i * 0.4f, frand() * 0.6f - 0.3f));
        s.tr.setRotation(btQuaternion(btVector3(frand() + 0.1f, frand(), frand()).normalized(), frand() * 6.28f));
        s.v = btVector3(frand() * 6 - 3, -frand() * 6, frand() * 6 - 3); s.w = btVector3(frand() * 8 - 4, frand() * 8 - 4, frand() * 8 - 4);
        s.dt = 0; s.act = 1; s.in = true; start.push_back(s);
    }
    if (drop && batteries > 1) start[batteries - 1].in = false;
    Game a(&h[0], n, step, batteries), b(&h[0], n, step, batteries);
    a.world.removeRigidBody(a.body[0]); a.world.addRigidBody(a.body[0], 0xA18, 0x418);   // A's history
    a.carried = b.carried = drop && batteries > 1 ? batteries - 1 : -1; a.dropAt = b.dropAt = drop;
    a.place(start); b.place(start);
    std::vector<World> hb; hb.push_back(b.save());
    Game* c = 0; World aw;
    int firstB = -1, firstC = -1, ticks = 160;
    for (int t = 1; t <= ticks; t++) {
        a.tick(t); aw = a.save();
        b.tick(t); hb.push_back(b.save());
        if (t % every == 0 && t > back) {
            int from = t - back;
            b.load(hb[from]); hb.resize(from + 1);
            for (int r = from + 1; r <= t; r++) { b.tick(r); hb.push_back(b.save()); }
        }
        if (t == join) { c = new Game(&h[0], n, step, batteries); c->carried = a.carried; c->dropAt = drop; c->load(aw); }
        else if (c) c->tick(t);
        World cw; if (c) cw = (t == join) ? aw : c->save();
        bool sb = sameBodies(aw, hb[t]) && (!FIX || sameContacts(aw, hb[t]));
        bool sc = !c || (sameBodies(aw, cw) && (!FIX || sameContacts(aw, cw)));
        if (!sb && firstB < 0) firstB = t;
        if (!sc && firstC < 0) firstC = t;
        if (t == ticks) {
            int awake = 0; for (size_t i = 0; i < aw.b.size(); i++) awake += aw.b[i].in && aw.b[i].act != 2;
            printf("fix %d seed %d batteries %d drop %d: rollback first differs at tick %d, joiner (tick %d) first differs at tick %d; contacts at end %d; awake at end %d\n",
                   FIX, seed0, batteries, drop, firstB, join, firstC, (int)aw.contacts.size(), awake);
        }
    }
    return 0;
}
