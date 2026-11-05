"use strict";
// The skills' tooltip: one box out of the page's flow (position: fixed), filled from the skill's
// own text. A mouse shows it while hovering, a click pins nothing; a tap opens it, and a tap
// anywhere (the same skill too) or a scroll closes it; the keyboard shows it on focus, Escape and
// blur hide it. Below the skill when it fits the window, above it otherwise, kept inside the
// window; on a phone it is docked at the bottom (CSS), over the page.
const box = document.querySelector(".skill-float");
const docked = matchMedia("(max-width:560px)");
let shown = null, touch = false;

function show(skill) {
  shown = skill;
  const tip = skill.querySelector(".skill-tip");
  box.innerHTML = tip.innerHTML;
  box.classList.toggle("wrap", tip.classList.contains("wrap"));
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
  skill.addEventListener("pointerenter", event => { if (event.pointerType !== "touch") show(skill); });
  skill.addEventListener("pointerleave", event => { if (event.pointerType !== "touch" && shown === skill) hide(); });
  skill.addEventListener("focus", () => { if (!touch) show(skill); });
  skill.addEventListener("blur", () => { if (shown === skill) hide(); });
}
document.addEventListener("pointerdown", event => {
  touch = event.pointerType === "touch";
  // A mouse or pen press does not focus the skill (it would stay shown).
  if (!touch && event.target.closest(".skill")) event.preventDefault();
});
document.addEventListener("click", event => {
  if (!touch) return;
  const skill = event.target.closest(".skill"), again = skill && skill === shown;
  hide();
  if (skill && !again) { skill.classList.add("open"); show(skill); }
});
addEventListener("scroll", () => { if (touch) hide(); }, {passive: true});
document.addEventListener("keydown", event => {
  if (event.key === "Escape" && shown) { const skill = shown; hide(); skill.blur(); }
});
