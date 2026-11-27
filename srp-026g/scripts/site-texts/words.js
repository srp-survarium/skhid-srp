// The page's own words in the reader's language (site-texts/README.md): the web server fills the
// page's <script type="application/json" id="page-words"> with every text its script asks for
// with T, Tc (a context first) and Tf (a template and its places), from words/<code>.tsv. Each
// takes its English text as a literal (the server and the words test find them so); English is
// what a text has no row for.
const PAGE_WORDS = JSON.parse(document.getElementById("page-words").textContent);
// (None calls another: each call in a script is a text's.)
const Tc = (context, english) => PAGE_WORDS[context]?.[english] ?? english;
const T = english => PAGE_WORDS[""]?.[english] ?? english;
const Tf = (english, values) => (PAGE_WORDS[""]?.[english] ?? english)
  .replace(/\{(\w+)\}/g, (place, name) => (name in values ? String(values[name]) : place));
