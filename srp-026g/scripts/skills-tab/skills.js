"use strict";
// The skills' tooltip: one box out of the page's flow (position: fixed), filled from the skill's
// own text. A mouse shows it while hovering; a tap opens it, and a tap anywhere (the same skill
// too) or a scroll closes it; the keyboard shows it on focus, Escape and blur hide it, and a
// scroll moves it with its skill. Below the skill when it fits the window, above it otherwise,
// kept inside the window; on a phone it is docked at the bottom (CSS).
//
// The planner: a click adds a rank, a right click (or a long press) takes one back; a tapped
// skill's tooltip has − and + for it; the keyboard's + / Enter and − / Backspace do the same. A
// rank is taken only when the build stays one the server accepts (the rules of
// crates/survarium/src/v026g/skills.rs, `validate`): no more than the skill's ranks, no more
// points than the last level has, no skill with points next to one it excludes (either way), and
// every skill with points in a row its branch unlocks. The bar above the trees counts the points,
// so the level that has them, and what that level is in matches and hours (the server's levels
// section). The build is kept in the address (#101.5,103.2) to share it.
const box = document.querySelector(".skill-float");
const docked = matchMedia("(max-width:560px)");
let shown = null, touch = false;

// The most branch points the lobby's walk takes (skills.rs MAX_BRANCH_POINTS): a skill needing
// more never opens.
const MOST_BRANCH_POINTS = 25;
const skills = new Map();
for (const el of document.querySelectorAll(".skill")) {
  const id = Number(el.dataset.skill), pips = el.querySelector(".skill-pips");
  skills.set(id, {
    id, el, branch: Math.floor(id / 100), ranks: Number(el.dataset.ranks), needs: Number(el.dataset.needs),
    row: Number(el.dataset.row), excludes: (el.dataset.excludes || "").split(" ").filter(Boolean).map(Number),
    pips, empty: pips && pips.getAttribute("src"), fills: pips ? pips.dataset.fills.split(" ") : [],
  });
}
// Each branch's skills in the walk's order: by row, then id.
const branches = new Map();
for (const s of [...skills.values()].sort((a, b) => a.row - b.row || a.id - b.id)) {
  if (!branches.has(s.branch)) branches.set(s.branch, []);
  branches.get(s.branch).push(s);
}
const levels = document.querySelector(".levels")?.dataset ?? {};
const lastLevel = Number(levels.max) || Infinity;
const bar = document.querySelector(".skill-plan");
const lang = document.documentElement.lang || "en";
const number = (n, digits = 0) => n.toLocaleString(lang, {minimumFractionDigits: digits, maximumFractionDigits: digits});

// The build: skill id -> ranks taken (never 0).
let build = new Map();

/** The branch points `taken` unlocks in `branch`: the lobby's walk (getBranchUnlockedLevel). */
function unlocked(branch, taken) {
  let before = 0, open = 0;
  for (const s of branches.get(branch) ?? []) {
    if (s.needs > MOST_BRANCH_POINTS) continue;
    if (s.needs > before) break;
    open = s.needs;
    before += taken.get(s.id) ?? 0;
  }
  return open;
}

const excluded = (s, taken) => s.excludes.some(x => taken.has(x)) ||
  [...taken.keys()].some(id => skills.get(id).excludes.includes(s.id));

/** Whether the server would take `taken`. */
function valid(taken) {
  let spent = 0;
  for (const [id, n] of taken) {
    const s = skills.get(id);
    if (!s || n > s.ranks || excluded(s, taken) || s.needs > unlocked(s.branch, taken)) return false;
    spent += n;
  }
  return spent <= lastLevel;
}

/** `build` with `step` ranks more (or fewer) of `s`, if the server would take it. */
function stepped(s, step) {
  const taken = new Map(build), n = (taken.get(s.id) ?? 0) + step;
  if (n < 0) return null;
  if (n) taken.set(s.id, n); else taken.delete(s.id);
  return valid(taken) ? taken : null;
}

function change(s, step) {
  const taken = stepped(s, step);
  if (!taken) {
    // Refused: the disc shakes.
    s.el.classList.remove("refused");
    void s.el.offsetWidth;
    s.el.classList.add("refused");
    return;
  }
  build = taken;
  draw();
  const hash = [...build].map(([id, n]) => `${id}.${n}`).join(",");
  history.replaceState(null, "", hash ? `#${hash}` : location.pathname + location.search);
  if (shown === s.el) show(s.el);
}

function draw() {
  const spent = new Map();
  let points = 0;
  for (const [id, n] of build) {
    const branch = skills.get(id).branch;
    spent.set(branch, (spent.get(branch) ?? 0) + n);
    points += n;
  }
  const open = new Map([...branches.keys()].map(b => [b, unlocked(b, build)]));
  for (const s of skills.values()) {
    const n = build.get(s.id) ?? 0;
    s.el.classList.toggle("taken", n > 0);
    s.el.classList.toggle("closed", s.needs > open.get(s.branch));
    s.el.classList.toggle("barred", excluded(s, build));
    if (s.pips) s.pips.src = n ? s.fills[n - 1] : s.empty;
  }
  for (const section of document.querySelectorAll(".skill-branch")) {
    const branch = skills.get(Number(section.querySelector(".skill").dataset.skill)).branch;
    section.querySelector(".skill-branch-points").textContent = spent.get(branch) ? number(spent.get(branch)) : "";
  }
  bar.querySelector('[data-plan="points"]').textContent = number(points);
  bar.querySelector('[data-plan="level"]').textContent = number(points);
  bar.querySelector('[data-plan="reset"]').disabled = !points;
  // The level's experience (account::level_start: first × L + step / 2 × L × (L − 1)), and so
  // the matches and hours it is at what a match is now.
  const play = bar.querySelector('[data-plan="play"]'), exp = Number(levels.matchExperience);
  if (points && exp) {
    const total = Number(levels.first) * points + Number(levels.step) / 2 * points * (points - 1);
    const tenths = Math.ceil(total * Number(levels.matchSeconds) * 10 / (exp * 3600));
    const hours = tenths < 100 ? number(tenths / 10, 1) : number(Math.ceil(tenths / 10));
    play.textContent = `≈ ${number(Math.ceil(total / exp))} ${play.dataset.matches} · ≈ ${hours} ${play.dataset.hours}`;
  } else {
    play.textContent = "";
  }
}

function show(skill) {
  shown = skill;
  const s = skills.get(Number(skill.dataset.skill)), n = build.get(s.id) ?? 0;
  const tip = skill.querySelector(".skill-tip");
  box.innerHTML = tip.innerHTML;
  box.classList.toggle("wrap", tip.classList.contains("wrap"));
  // The ranks taken, and the "Current value" of them.
  box.querySelectorAll(".skill-now").forEach((line, i) => { if (i !== n - 1) line.remove(); });
  if (s.ranks) box.querySelector("b").insertAdjacentHTML("beforeend", ` <span class="skill-rank">${n} / ${s.ranks}</span>`);
  // A tapped skill's tooltip has its − and +.
  const controls = skill.classList.contains("open") && s.ranks > 0;
  box.classList.toggle("controls", controls);
  if (controls) {
    box.insertAdjacentHTML("beforeend", '<div class="skill-steps"><button type="button" data-step="-1">−</button>' +
      '<button type="button" data-step="1">+</button></div>');
    box.querySelector('[data-step="-1"]').disabled = !stepped(s, -1);
    box.querySelector('[data-step="1"]').disabled = !stepped(s, 1);
  }
  box.hidden = false;
  if (docked.matches) { box.style.left = box.style.top = ""; return; }
  const at = skill.getBoundingClientRect(), width = box.offsetWidth, height = box.offsetHeight, gap = 4;
  const below = at.bottom + gap + height <= innerHeight || at.top - gap - height < 0;
  box.style.top = `${below ? at.bottom + gap : at.top - gap - height}px`;
  box.style.left = `${Math.max(8, Math.min(at.left + at.width / 2 - width / 2, innerWidth - width - 8))}px`;
}

function hide() {
  if (shown) shown.classList.remove("open");
  shown = null;
  box.hidden = true;
}

for (const skill of document.querySelectorAll(".skill")) {
  const s = skills.get(Number(skill.dataset.skill));
  skill.addEventListener("pointerenter", event => { if (event.pointerType !== "touch") show(skill); });
  skill.addEventListener("pointerleave", event => { if (event.pointerType !== "touch" && shown === skill) hide(); });
  skill.addEventListener("focus", () => { if (!touch) show(skill); });
  skill.addEventListener("blur", () => { if (shown === skill) hide(); });
  skill.addEventListener("contextmenu", event => { event.preventDefault(); change(s, -1); });
  skill.addEventListener("keydown", event => {
    if (["+", "=", "Enter", " "].includes(event.key)) change(s, 1);
    else if (["-", "Backspace", "Delete"].includes(event.key)) change(s, -1);
    else return;
    event.preventDefault();
  });
}
document.addEventListener("pointerdown", event => {
  touch = event.pointerType === "touch";
  // A mouse or pen press does not focus the skill (it would stay shown).
  if (!touch && event.target.closest(".skill")) event.preventDefault();
});
document.addEventListener("click", event => {
  const skill = event.target.closest(".skill");
  if (!touch) {
    if (skill) change(skills.get(Number(skill.dataset.skill)), 1);
    return;
  }
  const step = event.target.closest(".skill-float [data-step]");
  if (step) { if (shown) change(skills.get(Number(shown.dataset.skill)), Number(step.dataset.step)); return; }
  if (event.target.closest(".skill-float")) return;
  const again = skill && skill === shown;
  hide();
  if (skill && !again) { skill.classList.add("open"); show(skill); }
});
addEventListener("scroll", () => { if (touch) hide(); else if (shown) show(shown); }, {passive: true});
document.addEventListener("keydown", event => {
  if (event.key === "Escape" && shown) { const skill = shown; hide(); skill.blur(); }
});
bar.querySelector('[data-plan="reset"]').addEventListener("click", () => {
  build = new Map();
  draw();
  history.replaceState(null, "", location.pathname + location.search);
});

// A shared build (#101.5,103.2), when the server would take it.
const shared = new Map(location.hash.slice(1).split(",").filter(Boolean).map(part => part.split(".").map(Number)));
if (shared.size && [...shared].every(([id, n]) => skills.has(id) && n > 0) && valid(shared)) build = shared;
const hint = bar.querySelector(".skill-plan-hint");
if (matchMedia("(hover:none)").matches) hint.textContent = hint.dataset.touch;
bar.hidden = false;
draw();
