#!/usr/bin/env python3
"""Render the atopile design as diagrams, from the SOURCES.

    tools/ato-diagram.py              # -> hardware/elec/CIRCUIT.md
    tools/ato-diagram.py --html out.html

# Why read the sources rather than ask atopile

atopile's own views — `pinout`, `power-tree`, `2d-image`, `glb` — are build
artifacts emitted *after* the part picker, and on 0.15.8+ the picker requires an
interactive sign-in that cannot be automated (see
docs/hardware/ATOPILE-TOOLCHAIN.md). So on an unauthenticated machine, and in
CI, there is no way to get a picture out of atopile at all.

The `.ato` files, though, already state the structure: which modules exist, what
they instantiate, and what connects to what. That is the part a person wants to
*look at* when reviewing a circuit, and it is available offline.

# What this is NOT

Not a netlist, and not authoritative. It is a faithful reading of the source
text, which means:

* It shows connections as WRITTEN. atopile's solver may unify nets through
  bridges and interface sub-signals in ways a textual reading does not follow.
* It does not know footprints, designators, pin numbers or physical placement.
  Those live in the `.kicad_pcb`.
* A `~` between two interfaces stands for every sub-signal in them. This draws
  one edge, because the point of a block diagram is the one edge.

For the authoritative view, sign in and run `ato build -b default`, which emits
the pinout, power tree and 3D model. This exists for the other 90% of the time.
"""

from __future__ import annotations

import argparse
import html
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SRC = ROOT / "hardware/elec/src"

BLOCK_RE = re.compile(r"^(module|component|interface)\s+(\w+)\s*:", re.M)
NEW_RE = re.compile(r"^\s{4,}(\w+)\s*=\s*new\s+([\w.]+)(?:\[(\d+)\])?", re.M)
SIGNAL_RE = re.compile(r"^\s{4,}(?:signal|pin)\s+(\w+)", re.M)
# `a.b ~ c.d`, and bridges `a ~> b ~> c`. Several may share a line, separated
# by `;`, which this design does for pairs of load capacitors.
CONN_RE = re.compile(r"([\w.\[\]]+)\s*(~>|<~|~)\s*([\w.\[\]]+)")
IMPORT_RE = re.compile(r'^(?:from\s+"([^"]+)"\s+)?import\s+(\w+)', re.M)
ASSERT_RE = re.compile(r"^\s+assert\s+(.+?)\s*$", re.M)
PARAM_RE = re.compile(r"^\s{4,}(\w+(?:\.\w+)*)\s*=\s*([0-9][^\n#]*)", re.M)

#: Instance names whose type is a decoupling/passive support part. Collapsed in
#: the per-module diagrams, because forty capacitors named c_vdd_N tell a reader
#: nothing and hide the six edges that matter.
PASSIVE_TYPES = {"Capacitor", "Resistor", "Inductor", "Diode", "LED", "Fuse"}


class Block:
    def __init__(self, kind: str, name: str, file: pathlib.Path, body: str):
        self.kind = kind
        self.name = name
        self.file = file
        self.body = body
        self.instances: dict[str, tuple[str, int | None]] = {
            m.group(1): (m.group(2), int(m.group(3)) if m.group(3) else None)
            for m in NEW_RE.finditer(body)
        }
        self.signals: list[str] = SIGNAL_RE.findall(body)
        self.asserts: list[str] = [a.strip() for a in ASSERT_RE.findall(body)]
        self.connections: list[tuple[str, str, str]] = []
        for line in body.splitlines():
            code = line.split("#", 1)[0]
            for m in CONN_RE.finditer(code):
                self.connections.append((m.group(1), m.group(3), m.group(2)))
        self.params: dict[str, str] = {}
        for m in PARAM_RE.finditer(body):
            if m.group(1) not in ("frequency",) or True:
                self.params[m.group(1)] = m.group(2).strip()

    @property
    def passives(self) -> dict[str, tuple[str, int | None]]:
        return {k: v for k, v in self.instances.items() if v[0] in PASSIVE_TYPES}

    @property
    def actives(self) -> dict[str, tuple[str, int | None]]:
        return {k: v for k, v in self.instances.items() if v[0] not in PASSIVE_TYPES}


def parse_all() -> dict[str, Block]:
    blocks: dict[str, Block] = {}
    for f in sorted(SRC.rglob("*.ato")):
        text = f.read_text()
        marks = list(BLOCK_RE.finditer(text))
        for i, m in enumerate(marks):
            end = marks[i + 1].start() if i + 1 < len(marks) else len(text)
            blocks[m.group(2)] = Block(m.group(1), m.group(2), f, text[m.start():end])
    return blocks


def _node(name: str) -> str:
    """Mermaid-safe node id."""
    return re.sub(r"\W", "_", name)


def top_level_diagram(blocks: dict[str, Block], top: str) -> str:
    """Block diagram of the top module: its children and what links them."""
    b = blocks.get(top)
    if b is None:
        return ""
    lines = ["flowchart LR"]
    for inst, (ty, _) in b.actives.items():
        lines.append(f'  {_node(inst)}["{inst}<br/><i>{ty}</i>"]')

    # Collapse the edges: `power.power_3v3 ~ mcu.power_3v3` six times over is one
    # fact about the power tree, not six.
    seen: dict[tuple[str, str], set[str]] = {}
    for a, c, _op in b.connections:
        ia, ic = a.split(".")[0], c.split(".")[0]
        if ia not in b.actives or ic not in b.actives or ia == ic:
            continue
        key = tuple(sorted((ia, ic)))
        port = a.split(".", 1)[1] if "." in a else a
        seen.setdefault(key, set()).add(port)
    for (ia, ic), ports in sorted(seen.items()):
        label = ", ".join(sorted(ports)[:3])
        if len(ports) > 3:
            label += f", +{len(ports) - 3}"
        lines.append(f"  {_node(ia)} ---|{label}| {_node(ic)}")
    return "\n".join(lines)


def module_diagram(b: Block) -> str:
    """One module's internals: active parts and the nets between them.

    Passives are counted rather than drawn. A module with 34 decoupling
    capacitors is not clarified by 34 boxes.
    """
    lines = ["flowchart LR"]
    ports = [s for s in b.signals] + [
        i for i, (t, _) in b.instances.items() if t.endswith(("Power", "Bus", "Port", "Source"))
    ]
    for p in dict.fromkeys(ports):
        lines.append(f'  {_node(p)}(["{p}"])')
    for inst, (ty, n) in b.actives.items():
        if inst in ports:
            continue
        suffix = f" x{n}" if n else ""
        lines.append(f'  {_node(inst)}["{inst}{suffix}<br/><i>{ty}</i>"]')

    drawn = set()
    for a, c, _op in b.connections:
        ia, ic = a.split(".")[0], c.split(".")[0]
        if ia in b.passives or ic in b.passives:
            continue
        if ia == ic or (ia, ic) in drawn or (ic, ia) in drawn:
            continue
        if ia not in b.instances and ia not in b.signals:
            continue
        if ic not in b.instances and ic not in b.signals:
            continue
        drawn.add((ia, ic))
        lines.append(f"  {_node(ia)} --- {_node(ic)}")
    return "\n".join(lines)


def markdown(blocks: dict[str, Block], top: str) -> str:
    out: list[str] = []
    out.append("# Circuit — SoulAudio DAP\n")
    out.append(
        "**Generated** from `hardware/elec/src/**/*.ato` by `tools/ato-diagram.py`. "
        "Do not edit.\n"
    )
    out.append(
        "This is a reading of the SOURCES, not a netlist. It shows connections as "
        "written, knows nothing about footprints, designators or placement, and draws "
        "one edge for an interface-to-interface connection that carries many signals. "
        "For the authoritative pinout, power tree and 3D model, sign in "
        "(`ato auth login`) and run `ato build -b default` — see "
        "`docs/hardware/ATOPILE-TOOLCHAIN.md` for why that needs a person.\n"
    )

    out.append("\n## Board\n")
    out.append("```mermaid\n" + top_level_diagram(blocks, top) + "\n```\n")

    tb = blocks[top]
    out.append(f"`{top}` instantiates {len(tb.actives)} modules "
               f"with {len(tb.connections)} connection statements between them.\n")

    out.append("\n## Modules\n")
    for name, b in sorted(blocks.items()):
        if b.kind != "module" or name == top or not b.instances:
            continue
        rel = b.file.relative_to(ROOT)
        out.append(f"\n### `{name}`\n")
        out.append(f"<sub>{rel}</sub>\n")
        counts = []
        if b.actives:
            counts.append(f"{len(b.actives)} part(s)")
        if b.passives:
            counts.append(f"{len(b.passives)} passive(s)")
        if b.asserts:
            counts.append(f"{len(b.asserts)} assert(s)")
        if counts:
            out.append(", ".join(counts) + "\n")
        diag = module_diagram(b)
        if diag.count("\n") > 1:
            out.append("```mermaid\n" + diag + "\n```\n")
        if b.passives:
            out.append(
                f"\n<details><summary>{len(b.passives)} passives (collapsed)</summary>\n\n"
            )
            for inst, (ty, _) in sorted(b.passives.items()):
                val = next(
                    (v for k, v in b.params.items() if k.startswith(inst + ".")), ""
                )
                out.append(f"- `{inst}` — {ty}{' = ' + val if val else ''}\n")
            out.append("\n</details>\n")
        if b.asserts:
            out.append("\n<details><summary>design rules (asserts)</summary>\n\n")
            for a in b.asserts:
                out.append(f"- `{a}`\n")
            out.append(
                "\n</details>\n\n> These are NOT checked by the offline build — they are "
                "solver constraints evaluated after part picking.\n"
            )
    return "".join(out)


def _esc(t: str) -> str:
    return html.escape(str(t), quote=True)


def html_page(blocks: dict[str, Block], top: str) -> str:
    """A viewable page, built from the blocks rather than from the markdown.

    Mermaid is emitted as `<pre class="mermaid">`, which the artifact host renders
    natively — no CDN, which a strict CSP would block anyway.
    """
    tb = blocks[top]
    mods = [
        (n, b)
        for n, b in sorted(blocks.items())
        if b.kind == "module" and n != top and b.instances
    ]
    n_parts = sum(len(b.actives) for _, b in mods)
    n_pass = sum(len(b.passives) for _, b in mods)
    n_assert = sum(len(b.asserts) for _, b in mods)

    o: list[str] = []
    o.append("<title>Circuit — SoulAudio DAP</title>")
    o.append("""<style>
  /* Palette from the subject's own materials: cool soldermask ink, silkscreen
     white, ENIG gold for the accent, probe teal for the secondary. Neutrals are
     biased cool so they read as chosen rather than inherited. */
  :root {
    --ink:      #11161c;
    --paper:    #f6f7f5;
    --bg:       var(--paper);
    --surface:  #ffffff;
    --surface-2:#eceeea;
    --line:     #d6d9d3;
    --text:     #1a2026;
    --text-dim: #5d666e;
    --accent:   #9a6a2f;
    --accent-2: #2f6672;
    --grid:     #e4e7e2;
    --warn-bg:  #f6efe2;
    --warn-line:#cdab74;
    --radius: 3px;
    --mono: ui-monospace, "SF Mono", "Cascadia Mono", "Roboto Mono", Menlo, monospace;
    --sans: system-ui, -apple-system, "Segoe UI", Roboto, sans-serif;
    color-scheme: light dark;
  }
  @media (prefers-color-scheme: dark) {
    :root {
      --bg: var(--ink); --surface: #171d24; --surface-2: #1d242c;
      --line: #2b343d; --text: #dfe4e8; --text-dim: #8e99a3;
      --accent: #c0894a; --accent-2: #5fa3b1; --grid: #1b2229;
      --warn-bg: #241e13; --warn-line: #6b5429;
    }
  }
  :root[data-theme="dark"] {
    --bg: var(--ink); --surface: #171d24; --surface-2: #1d242c;
    --line: #2b343d; --text: #dfe4e8; --text-dim: #8e99a3;
    --accent: #c0894a; --accent-2: #5fa3b1; --grid: #1b2229;
    --warn-bg: #241e13; --warn-line: #6b5429;
  }
  :root[data-theme="light"] {
    --bg: var(--paper); --surface: #ffffff; --surface-2: #eceeea;
    --line: #d6d9d3; --text: #1a2026; --text-dim: #5d666e;
    --accent: #9a6a2f; --accent-2: #2f6672; --grid: #e4e7e2;
    --warn-bg: #f6efe2; --warn-line: #cdab74;
  }

  body { background: var(--bg); color: var(--text); font: 15px/1.65 var(--sans);
         margin: 0; }
  .shell { max-width: 78rem; margin: 0 auto; padding: 0 1.25rem 7rem;
           display: grid; gap: 2.5rem; }
  @media (min-width: 60rem) {
    .shell { grid-template-columns: 13rem minmax(0, 1fr); align-items: start; }
    .masthead, .board { grid-column: 1 / -1; }
  }

  .masthead { border-bottom: 1px solid var(--line); padding: 3rem 0 1.5rem;
              display: grid; gap: .6rem; }
  .eyebrow { font: 600 11px/1 var(--mono); letter-spacing: .14em;
             text-transform: uppercase; color: var(--accent); }
  h1 { font: 700 clamp(1.7rem, 4vw, 2.4rem)/1.1 var(--sans);
       letter-spacing: -.022em; margin: 0; text-wrap: balance; }
  .lede { color: var(--text-dim); max-width: 60ch; margin: 0; }

  .caveat { background: var(--warn-bg); border: 1px solid var(--warn-line);
            border-left-width: 3px; border-radius: var(--radius);
            padding: .85rem 1rem; font-size: 13.5px; max-width: 70ch;
            display: grid; gap: .4rem; }
  .caveat b { font-weight: 650; }

  .stats { display: flex; flex-wrap: wrap; gap: 0 2.2rem; margin: 0; padding: 0;
           list-style: none; }
  .stats div { display: grid; gap: .1rem; }
  .stats dt { font: 600 10.5px/1 var(--mono); letter-spacing: .12em;
              text-transform: uppercase; color: var(--text-dim); }
  .stats dd { margin: 0; font: 650 1.45rem/1.1 var(--sans);
              font-variant-numeric: tabular-nums; letter-spacing: -.02em; }

  /* Diagrams sit on a drawing sheet: faint grid, hard border, scrolls alone. */
  .sheet { border: 1px solid var(--line); border-radius: var(--radius);
           background: var(--surface);
           background-image: linear-gradient(var(--grid) 1px, transparent 1px),
                             linear-gradient(90deg, var(--grid) 1px, transparent 1px);
           background-size: 22px 22px; overflow-x: auto; padding: 1.25rem; }
  .sheet pre.mermaid { margin: 0; background: none; }

  nav.index { position: sticky; top: 1.5rem; font-size: 13.5px;
              display: grid; gap: .1rem; align-content: start; }
  nav.index .label { font: 600 10.5px/1 var(--mono); letter-spacing: .12em;
                     text-transform: uppercase; color: var(--text-dim);
                     padding-bottom: .5rem; }
  nav.index a { color: var(--text-dim); text-decoration: none; font-family: var(--mono);
                font-size: 12.5px; padding: .2rem 0; border-left: 2px solid transparent;
                padding-left: .6rem; }
  nav.index a:hover, nav.index a:focus-visible {
      color: var(--accent); border-left-color: var(--accent); }

  section.mod { padding-top: .5rem; display: grid; gap: .9rem;
                scroll-margin-top: 1.5rem; }
  section.mod + section.mod { border-top: 1px solid var(--line);
                              margin-top: 2.2rem; padding-top: 2.2rem; }
  section.mod h2 { font: 650 1.12rem/1.2 var(--mono); margin: 0;
                   letter-spacing: -.01em; }
  .path { font: 11.5px/1 var(--mono); color: var(--text-dim); }
  .chips { display: flex; flex-wrap: wrap; gap: .4rem; }
  .chip { font: 600 11px/1 var(--mono); letter-spacing: .04em;
          border: 1px solid var(--line); background: var(--surface-2);
          color: var(--text-dim); border-radius: 999px; padding: .3rem .6rem; }
  .chip.ac { color: var(--accent); border-color: var(--accent); }

  details { border: 1px solid var(--line); border-radius: var(--radius);
            background: var(--surface); padding: .55rem .9rem; }
  summary { cursor: pointer; font: 600 12.5px/1.5 var(--mono);
            color: var(--text-dim); }
  summary:hover, summary:focus-visible { color: var(--accent); }
  details ul { margin: .7rem 0 .3rem; padding-left: 1.1rem; }
  details li { font-family: var(--mono); font-size: 12.5px; line-height: 1.75; }
  details .note { font-family: var(--sans); font-size: 12.5px;
                  color: var(--text-dim); margin: .5rem 0 .2rem; }

  code { font-family: var(--mono); font-size: .92em; }
  a { color: var(--accent); }
  footer { grid-column: 1 / -1; border-top: 1px solid var(--line);
           padding-top: 1.5rem; color: var(--text-dim); font-size: 13.5px;
           display: grid; gap: .6rem; max-width: 72ch; }
  footer pre { background: var(--surface-2); border: 1px solid var(--line);
               border-radius: var(--radius); padding: .8rem 1rem; overflow-x: auto;
               font-size: 12.5px; }
  :focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  @media (prefers-reduced-motion: reduce) { * { animation: none !important;
                                                transition: none !important; } }
</style>""")

    o.append('<div class="shell">')
    o.append('<header class="masthead">')
    o.append('<p class="eyebrow">atopile &middot; hardware/elec/src</p>')
    o.append(f"<h1>{_esc(top)}</h1>")
    o.append(
        '<p class="lede">The board as the sources describe it: eight modules, what '
        "they instantiate, and what connects them.</p>"
    )
    o.append(
        '<div class="caveat"><span><b>Source-derived, not a netlist.</b> '
        "Connections are shown as written; one edge stands for an interface "
        "carrying many signals. Footprints, designators and placement live in the "
        "<code>.kicad_pcb</code> and are not represented here.</span>"
        "<span>The authoritative pinout, power tree and 3D model are build "
        "artifacts that require a signed-in part picker &mdash; see "
        "<code>docs/hardware/ATOPILE-TOOLCHAIN.md</code>.</span></div>"
    )
    o.append('<dl class="stats">')
    for label, value in (
        ("modules", len(tb.actives)),
        ("top-level nets", len(tb.connections)),
        ("parts", n_parts),
        ("passives", n_pass),
        ("design rules", n_assert),
    ):
        o.append(f"<div><dt>{label}</dt><dd>{value}</dd></div>")
    o.append("</dl>")
    o.append("</header>")

    o.append('<div class="board sheet">')
    o.append(f'<pre class="mermaid">{_esc(top_level_diagram(blocks, top))}</pre>')
    o.append("</div>")

    o.append('<nav class="index" aria-label="Modules"><span class="label">Modules</span>')
    for n, _ in mods:
        o.append(f'<a href="#mod-{_esc(n)}">{_esc(n)}</a>')
    o.append("</nav>")

    o.append("<main>")
    for n, b in mods:
        o.append(f'<section class="mod" id="mod-{_esc(n)}">')
        o.append(f"<h2>{_esc(n)}</h2>")
        o.append(f'<p class="path">{_esc(b.file.relative_to(ROOT))}</p>')
        o.append('<div class="chips">')
        if b.actives:
            o.append(f'<span class="chip ac">{len(b.actives)} part'
                     f'{"s" if len(b.actives) != 1 else ""}</span>')
        if b.passives:
            o.append(f'<span class="chip">{len(b.passives)} passives</span>')
        if b.asserts:
            o.append(f'<span class="chip">{len(b.asserts)} design rules</span>')
        o.append("</div>")
        diag = module_diagram(b)
        if diag.count("\n") > 1:
            o.append(f'<div class="sheet"><pre class="mermaid">{_esc(diag)}</pre></div>')
        if b.passives:
            o.append(f"<details><summary>{len(b.passives)} passives</summary><ul>")
            for inst, (ty, _) in sorted(b.passives.items()):
                val = next((v for k, v in b.params.items() if k.startswith(inst + ".")), "")
                o.append(f"<li>{_esc(inst)} &mdash; {_esc(ty)}"
                         f"{' = ' + _esc(val) if val else ''}</li>")
            o.append("</ul></details>")
        if b.asserts:
            o.append(f"<details><summary>{len(b.asserts)} design rules "
                     f"(asserts)</summary><ul>")
            for a in b.asserts:
                o.append(f"<li>{_esc(a)}</li>")
            o.append('</ul><p class="note">Not checked by the offline build: these are '
                     "solver constraints, evaluated after part picking.</p></details>")
        o.append("</section>")
    o.append("</main>")

    o.append("<footer>")
    o.append("<p>Regenerate with <code>tools/ato-diagram.py</code>. "
             "For the authoritative views, sign in once and build:</p>")
    o.append("<pre>ato auth login\n"
             "ato build -b default            # pinout, power tree, BOM, datasheets\n"
             "ato build -b default -t glb     # 3D model (needs KiCad)</pre>")
    o.append("</footer>")
    o.append("</div>")
    return "\n".join(o)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--top", default="SoulAudioDAP")
    ap.add_argument("--out", default=str(ROOT / "hardware/elec/CIRCUIT.md"))
    ap.add_argument("--html", help="also write a standalone HTML copy here")
    ap.add_argument(
        "--check",
        action="store_true",
        help="exit non-zero if the committed CIRCUIT.md is stale (for CI and the hook)",
    )
    args = ap.parse_args()

    blocks = parse_all()
    if args.top not in blocks:
        print(f"no top-level module {args.top!r}; found: {sorted(blocks)}", file=sys.stderr)
        return 1
    md = markdown(blocks, args.top)
    out = pathlib.Path(args.out)
    if args.check:
        have = out.read_text() if out.exists() else ""
        if have != md:
            print(
                f"{out} is STALE — it is generated from hardware/elec/src/**/*.ato.\n"
                f"Run tools/ato-diagram.py and commit the result.",
                file=sys.stderr,
            )
            return 1
        print(f"{out.name} is current ({len(blocks)} blocks)")
        return 0
    out.write_text(md)
    print(f"wrote {args.out} ({len(blocks)} blocks parsed)")
    if args.html:
        pathlib.Path(args.html).write_text(html_page(blocks, args.top))
        print(f"wrote {args.html}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
