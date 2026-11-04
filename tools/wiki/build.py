#!/usr/bin/env python3
"""Build the Perforizon wiki: wiki/pages/*.md -> wiki/_site (static, relative links only).

Needs: pip install markdown. Env WIKI_BASE (e.g. /Perforizon/) is used only by 404.html,
which GitHub Pages serves from any path.
"""
import html, json, os, re, shutil, sys
from pathlib import Path
import markdown

ROOT = Path(__file__).resolve().parents[2]
PAGES = ROOT / "wiki" / "pages"
THEME = ROOT / "wiki" / "theme"
SITE = ROOT / "wiki" / "_site"
REPO = "https://github.com/Louchatfroff/Perforizon"
FONT = ROOT / "crates/horizon-theme/fonts/GeistMono-Regular.ttf"
BRAND = ROOT / "docs/media/brand"
LABELS = ["bg", "bg-alt", "border", "border-focus", "fg", "fg-dim", "accent", "accent-dim",
          "ok", "warn", "err", "selection-bg", "selection-fg"]
esc = html.escape
LIGHT = "grayscale"


def palettes():
    """Parse launcher/src/palette.rs so the site cannot drift from the launcher."""
    src = (ROOT / "launcher/src/palette.rs").read_text(encoding="utf-8")
    rx = re.compile(r'p\("([^"]+)",\s*"([^"]+)",\s*(true|false),\s*\[([^\]]+)\]\)')
    out = []
    for name, label, dark, nums in rx.findall(src):
        vals = [int(n.strip(), 16) for n in nums.split(",") if n.strip()]
        assert len(vals) == 13, name
        out.append({"name": name, "label": label, "dark": dark == "true",
                    "c": ["#%06x" % v for v in vals]})
    assert len(out) == 16, len(out)
    return out


def palette_css(pals):
    blocks = []
    for i, p in enumerate(pals):
        sel = f'[data-palette="{p["name"]}"]' + (",:root" if i == 0 else "")
        body = ";".join(f"--{k}:{v}" for k, v in zip(LABELS, p["c"]))
        mark = "--mark-dark:inline;--mark-light:none" if p["dark"] else "--mark-dark:none;--mark-light:inline"
        scheme = "dark" if p["dark"] else "light"
        blocks.append(f"{sel}{{{body};{mark};color-scheme:{scheme}}}")
    return "\n".join(blocks) + "\n"


def front(text, path):
    m = re.match(r"---\r?\n(.*?)\r?\n---\r?\n?", text, re.S)
    if not m:
        sys.exit(f"{path}: missing front matter")
    meta = {}
    for line in m.group(1).splitlines():
        k, _, v = line.partition(":")
        meta[k.strip()] = v.strip()
    for k in ("title", "section", "summary"):
        if k not in meta:
            sys.exit(f"{path}: front matter lacks {k}")
    return meta, text[m.end():]


class Assets:
    """Copies images a page references into _site/assets and returns the new relative URL."""
    def __init__(self):
        self.done = {}

    def find(self, src, page_dir):
        rel = src.split("?")[0].split("#")[0]
        cands = [(page_dir / rel).resolve(), (ROOT / rel.lstrip("/")).resolve()]
        for base in (ROOT / "wiki/assets", ROOT / "docs/media"):
            if base.exists():
                cands.append((base / rel).resolve())
                cands += list(base.rglob(Path(rel).name))
        return next((c for c in cands if c.is_file()), None)

    def add(self, src, page_dir):
        if src in self.done:
            return self.done[src]
        f = self.find(src, page_dir)
        if not f:
            print(f"warning: asset not found: {src}", file=sys.stderr)
            return src
        name = f.name
        dest = SITE / "assets" / name
        if dest.exists() and dest.read_bytes() != f.read_bytes():
            name = f"{f.parent.name}-{name}"
            dest = SITE / "assets" / name
        shutil.copy2(f, dest)
        self.done[src] = "assets/" + name
        return self.done[src]


def rewrite(htm, page_dir, slugs, assets):
    def img(m):
        src = m.group(1)
        if re.match(r"(https?:|data:|//)", src):
            return m.group(0)
        return f'src="{assets.add(src, page_dir)}"'
    htm = re.sub(r'src="([^"]+)"', img, htm)

    def link(m):
        href = m.group(1)
        if re.match(r"(https?:|mailto:|#|//)", href):
            return m.group(0)
        path, _, frag = href.partition("#")
        frag = "#" + frag if frag else ""
        stem = re.sub(r"\.(md|html)$", "", Path(path).name.rstrip("/"))
        if stem in slugs:
            return f'href="{slugs[stem]}{frag}"'
        target = (page_dir / path).resolve()
        try:
            rel = target.relative_to(ROOT).as_posix()
        except ValueError:
            rel = None
        if rel and target.exists():
            kind = "tree" if target.is_dir() else "blob"
            return f'href="{REPO}/{kind}/main/{rel}{frag}"'
        print(f"warning: unresolved link: {href}", file=sys.stderr)
        return m.group(0)
    return re.sub(r'href="([^"]*)"', link, htm)


def decorate(htm):
    # fenced code -> bordered panel with a title row
    def code(m):
        lang = m.group(1) or "text"
        return (f'<div class="code"><div class="code-title"><span>{lang}</span>'
                f'<button type="button" class="copy" data-copy>[ copy ]</button></div>'
                f'<pre><code>{m.group(2)}</code></pre></div>')
    htm = re.sub(r'<pre><code(?: class="language-([\w+-]+)")?>(.*?)</code></pre>', code, htm, flags=re.S)
    htm = htm.replace("<table>", '<div class="tablewrap"><table>').replace("</table>", "</table></div>")
    return htm.replace("<hr />", '<div class="hatch hatch-line" aria-hidden="true"></div>')


def toc_items(tokens, out=None):
    out = [] if out is None else out
    for t in tokens:
        if t["level"] in (2, 3):
            out.append(t)
        toc_items(t["children"], out)
    return out


def strip_text(htm):
    return re.sub(r"\s+", " ", html.unescape(re.sub(r"<[^>]+>", " ", htm))).strip()


def main():
    files = sorted(PAGES.glob("*.md"))
    if not files:
        sys.exit("wiki/pages has no pages")
    SITE.mkdir(exist_ok=True)
    for old in SITE.iterdir():  # empty it in place (a dev server may hold the folder open)
        shutil.rmtree(old) if old.is_dir() else old.unlink()
    (SITE / "assets").mkdir()
    pals = palettes()
    global LIGHT
    LIGHT = next((p["name"] for p in pals if not p["dark"]), "grayscale")

    pages = []
    for i, f in enumerate(files):
        meta, body = front(f.read_text(encoding="utf-8"), f)
        slug = re.sub(r"^\d+-", "", f.stem)
        pages.append(dict(meta, file=f, slug=slug, body=body,
                          url="index.html" if i == 0 else f"{slug}.html"))
    slugs = {p["slug"]: p["url"] for p in pages}
    slugs.update({p["file"].stem: p["url"] for p in pages})
    sections = []
    for p in pages:
        if p["section"] not in sections:
            sections.append(p["section"])
    sec_first = {s: next(p["url"] for p in pages if p["section"] == s) for s in sections}
    home_sec = pages[0]["section"]

    assets = Assets()
    index = []
    for p in pages:
        md = markdown.Markdown(extensions=["tables", "fenced_code", "toc", "admonition", "attr_list"],
                               extension_configs={"toc": {"toc_depth": "2-3"}})
        htm = decorate(rewrite(md.convert(p["body"]), p["file"].parent, slugs, assets))
        p["html"], p["toc"] = htm, toc_items(md.toc_tokens)
        index.append({"title": p["title"], "section": p["section"], "summary": p["summary"],
                      "url": p["url"], "text": strip_text(htm)})

    def tabs_html(active_url=None, search=False, home=False):
        t = f'<a class="tab{" on" if home else ""}" href="index.html">Home</a>'
        for s in sections:
            if s != home_sec:
                on = " on" if active_url and active_url["section"] == s and not home else ""
                t += f'<a class="tab{on}" href="{sec_first[s]}">{esc(s)}</a>'
        return t

    def side_html(cur=None):
        side = ""
        for s in sections:
            side += f'<div class="sec">{esc(s)}</div><ul>'
            for q in pages:
                if q["section"] == s:
                    here = ' aria-current="page" class="here"' if q is cur else ""
                    side += f'<li><a href="{q["url"]}"{here}>{esc(q["title"])}</a></li>'
            side += "</ul>"
        return side

    for n, p in enumerate(pages):
        toc = "".join(f'<li class="l{t["level"]}"><a href="#{t["id"]}">{esc(t["name"])}</a></li>' for t in p["toc"])
        toc_panel = (f'<details class="panel" open><summary class="label">ON THIS PAGE</summary><ul>{toc}</ul></details>'
                     if toc else "")
        pn = ""
        if n:
            pn += f'<a class="btn" href="{pages[n-1]["url"]}">[ &lsaquo; {esc(pages[n-1]["title"])} ]</a>'
        if n + 1 < len(pages):
            pn += f'<a class="btn next" href="{pages[n+1]["url"]}">[ {esc(pages[n+1]["title"])} &rsaquo; ]</a>'
        edit = f'{REPO}/edit/main/wiki/pages/{p["file"].name}'
        main_html = (f'<h1 class="title"><span class="caret" aria-hidden="true">&rsaquo;</span> {esc(p["title"])}</h1>'
                     f'<p class="subtitle">{esc(p["summary"])}</p>'
                     f'<div class="prose">{p["html"]}</div>'
                     f'<div class="hatch hatch-line" aria-hidden="true"></div>'
                     f'<nav class="pn" aria-label="Previous and next">{pn}</nav>'
                     f'<p><a class="btn" href="{edit}">[ edit this page ]</a></p>')
        write_page(p["url"], p["title"], p["summary"], tabs_html(p, home=n == 0), side_html(p), toc_panel, main_html)

    empty = ('<div class="empty"><div class="hatch" aria-hidden="true"></div>'
             '<p id="status" role="status">Type to search the wiki.</p></div>')
    main_html = ('<h1 class="title"><span class="caret" aria-hidden="true">&rsaquo;</span> Search</h1>'
                 '<p class="subtitle">Search every page of the wiki.</p>'
                 '<form class="searchbox" action="search.html" role="search"><label for="q" class="dim">search</label>'
                 '<input id="q" name="q" type="search" autocomplete="off" placeholder="type, then enter">'
                 '</form><ol id="results" class="results"></ol>' + empty)
    write_page("search.html", "Search", "Search the wiki", tabs_html(), side_html(), "", main_html, search=True)

    nf = ('<h1 class="title"><span class="caret" aria-hidden="true">&rsaquo;</span> Page not found</h1>'
          '<p class="subtitle">Nothing lives at this address.</p>'
          '<div class="empty"><div class="hatch" aria-hidden="true"></div><p>404</p></div>'
          '<p><a class="btn" href="index.html">[ Back to the wiki home ]</a></p>')
    write_page("404.html", "Not found", "Page not found", tabs_html(), side_html(), "", nf,
               base=os.environ.get("WIKI_BASE", ""))

    (SITE / "search-index.js").write_text("window.WIKI_INDEX=" + json.dumps(index, ensure_ascii=False) + ";", encoding="utf-8")
    css = (THEME / "style.css").read_text(encoding="utf-8").replace("__HATCH__", "/" * 700)
    (SITE / "assets" / "style.css").write_text(css + "\n" + palette_css(pals), encoding="utf-8")
    pj = [{k: p[k] for k in ("name", "label", "dark")} for p in pals]
    (SITE / "assets" / "wiki.js").write_text(
        "window.PALETTES=" + json.dumps(pj) + ";\n" + (THEME / "wiki.js").read_text(encoding="utf-8"), encoding="utf-8")
    shutil.copy2(FONT, SITE / "assets" / "GeistMono-Regular.ttf")
    shutil.copy2(BRAND / "web/favicon.svg", SITE / "assets" / "favicon.svg")
    shutil.copy2(BRAND / "mark.svg", SITE / "assets" / "mark.svg")
    shutil.copy2(BRAND / "mark-dark.svg", SITE / "assets" / "mark-dark.svg")
    (SITE / ".nojekyll").write_text("")
    print(f"built {len(pages)} pages + search + 404 -> {SITE}")


def write_page(url, title, summary, tabs, side, toc_panel, main_html, search=False, base=None):
    bh = f'<base href="{esc(base)}">' if base else ""
    out = f"""<!doctype html>
<html lang="en" data-palette="grayscale">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
{bh}<title>{esc(title)} - Perforizon wiki</title>
<meta name="description" content="{esc(summary)}">
<link rel="icon" type="image/svg+xml" href="assets/favicon.svg">
<link rel="stylesheet" href="assets/style.css">
<script>try{{var p=localStorage.getItem("perforizon-palette");if(!p&&matchMedia("(prefers-color-scheme: light)").matches)p="{LIGHT}";if(p)document.documentElement.dataset.palette=p}}catch(e){{}}</script>
</head>
<body>
<a class="skip" href="#content">Skip to content</a>
<header class="bar">
<a class="brand" href="index.html" aria-label="Perforizon wiki home"><img class="mark mark-dark" src="assets/mark-dark.svg" alt="" width="20" height="20"><img class="mark mark-light" src="assets/mark.svg" alt="" width="20" height="20"><span class="word">PERFORIZON</span></a>
<nav class="tabs" aria-label="Sections">{tabs}</nav>
<a class="tab right{' on' if search else ''}" href="search.html">Search</a>
</header>
<div class="layout">
<aside class="side">
<form class="searchbox mini" action="search.html" role="search"><label for="sq" class="dim">search</label><input id="sq" name="q" type="search" placeholder="/" autocomplete="off"></form>
<details class="panel" open><summary class="label">PAGES</summary>{side}</details>
{toc_panel}
</aside>
<main id="content" tabindex="-1">
{main_html}
</main>
</div>
<footer class="keys">
<span class="hints"><span><b>tab</b> next</span> <span><b>shift-tab</b> previous</span> <span><b>enter</b> select</span> <span><b>esc</b> back</span> <span><b>^p</b> palette</span> <span><b>/</b> search</span> <span><b>g h</b> home</span> <span><b>j/k</b> scroll</span></span>
<button type="button" id="palette" title="Cycle palette (Ctrl+P)">Grayscale</button>
</footer>
<script src="search-index.js"></script>
<script src="assets/wiki.js"></script>
</body>
</html>
"""
    (SITE / url).write_text(out, encoding="utf-8")


if __name__ == "__main__":
    main()
