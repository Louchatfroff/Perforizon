/* Perforizon wiki: palettes, keyboard, search. window.PALETTES is prepended by build.py. */
(function () {
  var KEY = "perforizon-palette", root = document.documentElement;
  var P = window.PALETTES, btn = document.getElementById("palette");
  function find(n) { for (var i = 0; i < P.length; i++) if (P[i].name === n) return i; return 0; }
  function show() { btn.textContent = P[find(root.dataset.palette)].label; }
  function setPalette(i) {
    root.dataset.palette = P[i].name;
    try { localStorage.setItem(KEY, P[i].name); } catch (e) {}
    show();
  }
  function cycle(d) { setPalette((find(root.dataset.palette) + (d || 1) + P.length) % P.length); }
  root.dataset.palette = P[find(root.dataset.palette)].name; // unknown saved name -> grayscale
  show();
  btn.addEventListener("click", function () { cycle(1); });

  // sidebar panels start collapsed on narrow screens
  var mq = window.matchMedia("(max-width:820px)");
  function fold() { [].forEach.call(document.querySelectorAll(".side details"), function (d) { d.open = !mq.matches; }); }
  fold();
  if (mq.addEventListener) mq.addEventListener("change", fold);

  // copy buttons
  document.addEventListener("click", function (e) {
    var b = e.target.closest("[data-copy]");
    if (!b) return;
    var t = b.closest(".code").querySelector("code").textContent;
    var done = function () { b.textContent = "[ copied ]"; setTimeout(function () { b.textContent = "[ copy ]"; }, 1500); };
    try { navigator.clipboard.writeText(t).then(done, function () {}); } catch (err) {}
  });

  // keyboard
  var chord = 0;
  function typing(t) { return t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable); }
  document.addEventListener("keydown", function (e) {
    if (e.ctrlKey && !e.altKey && !e.metaKey && (e.key === "p" || e.key === "P")) {
      e.preventDefault(); cycle(e.shiftKey ? -1 : 1); return;
    }
    if (e.key === "Escape") {
      if (typing(document.activeElement)) document.activeElement.blur();
      else if (document.referrer.indexOf(location.origin) === 0) history.back();
      return;
    }
    if (typing(e.target) || e.ctrlKey || e.metaKey || e.altKey) return;
    var step = Math.round(window.innerHeight * 0.12);
    if (e.key === "/") {
      e.preventDefault();
      (document.getElementById("q") || document.getElementById("sq")).focus();
    } else if (e.key === "j") window.scrollBy(0, step);
    else if (e.key === "k") window.scrollBy(0, -step);
    else if (e.key === "g") chord = Date.now();
    else if (e.key === "h" && Date.now() - chord < 1000) { chord = 0; location.href = "index.html"; }
  });

  // search page
  var q = document.getElementById("q"), out = document.getElementById("results");
  if (!q || !out) return;
  var status = document.getElementById("status"), idx = window.WIKI_INDEX || [];
  function esc(s) { return s.replace(/[&<>"]/g, function (c) { return { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]; }); }
  function rx(s) { return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"); }
  function run() {
    var terms = q.value.toLowerCase().split(/\s+/).filter(Boolean), hits = [];
    out.innerHTML = "";
    var empty = document.querySelector(".empty");
    if (!terms.length) { empty.hidden = false; status.textContent = "Type to search the wiki."; return; }
    idx.forEach(function (p) {
      var hay = (p.title + " " + p.summary).toLowerCase(), body = p.text.toLowerCase(), s = 0;
      for (var i = 0; i < terms.length; i++) {
        var inHead = hay.indexOf(terms[i]) >= 0, inBody = body.indexOf(terms[i]) >= 0;
        if (!inHead && !inBody) return;
        s += (p.title.toLowerCase().indexOf(terms[i]) >= 0 ? 10 : 0) + (inHead ? 3 : 0) + 1;
      }
      var at = body.indexOf(terms[0]), snip = at < 0 ? p.summary : p.text.slice(Math.max(0, at - 50), at + 110);
      hits.push({ p: p, s: s, snip: (at > 50 ? "... " : "") + snip });
    });
    hits.sort(function (a, b) { return b.s - a.s; });
    empty.hidden = hits.length > 0;
    status.textContent = "No page matches that.";
    var re = new RegExp("(" + terms.map(rx).join("|") + ")", "gi");
    out.innerHTML = hits.map(function (h) {
      return '<li><a href="' + h.p.url + '">' + esc(h.p.title) + '</a><span class="sec">' + esc(h.p.section) +
        "</span><p>" + esc(h.snip).replace(re, "<mark>$1</mark>") + "</p></li>";
    }).join("");
  }
  var init = new URLSearchParams(location.search).get("q");
  if (init) q.value = init;
  q.addEventListener("input", run);
  run();
  if (!init) q.focus();
})();
