"use strict";
// The wiki's tables: a click on a column's title sorts by it (numbers by
// their value), again the other way; a search box filters the tables it names.
(() => {
  const value = td => {
    const v = td.dataset.v !== undefined ? td.dataset.v : td.textContent.trim();
    const n = parseFloat(v);
    return isNaN(n) || !/^-?[\d.]/.test(v) ? v.toLowerCase() : n;
  };
  for (const table of document.querySelectorAll("table.sort")) {
    const heads = table.tHead ? [...table.tHead.rows[0].cells] : [];
    heads.forEach((th, col) => {
      th.tabIndex = 0;
      const sort = () => {
        const down = th.getAttribute("aria-sort") === "ascending";
        heads.forEach(h => h.removeAttribute("aria-sort"));
        th.setAttribute("aria-sort", down ? "descending" : "ascending");
        const body = table.tBodies[0];
        const rows = [...body.rows].map((r, i) => [value(r.cells[col] || r), i, r]);
        rows.sort((a, b) => (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : a[1] - b[1]) * (down ? -1 : 1));
        for (const [, , r] of rows) body.append(r);
      };
      th.addEventListener("click", sort);
      th.addEventListener("keydown", e => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); sort(); } });
    });
  }
  for (const box of document.querySelectorAll("input[data-find]")) {
    const tables = document.querySelectorAll("table." + box.dataset.find);
    box.addEventListener("input", () => {
      const q = box.value.trim().toLowerCase();
      for (const t of tables) for (const r of t.tBodies[0].rows) r.hidden = q !== "" && !r.textContent.toLowerCase().includes(q);
    });
  }
})();
