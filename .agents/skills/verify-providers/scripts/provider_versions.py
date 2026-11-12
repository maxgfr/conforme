#!/usr/bin/env python3
"""Each provider's latest upstream version, against its `Last verified online` line.

Usage:
  provider_versions.py <project_root>                  report every provider
  provider_versions.py <project_root> --stamp <tool>   restamp one line (or `all`)
      [--against "<text>"] [--sources "<text>"]

`check` (the default) fetches every version a registry publishes (npm, PyPI,
Homebrew, GitHub releases, VS Code's update API) and prints one row per
`docs/providers/<tool>.md`: CURRENT when each latest version already appears in
the line, STALE when one does not, MANUAL for a version no registry publishes
(Cursor, Devin, Kiro: read their changelog, the URL is printed), ERROR when a
fetch failed. It exits 1 when any row is STALE or ERROR, so the audit knows
which providers moved since the last run.

`--stamp <tool>` rewrites that provider's line to today's date. With no
`--against`, the text comes from the fetched versions (a MANUAL provider needs
`--against`); the parenthesised sources are kept unless `--sources` replaces
them, and whatever follows them is kept too. Stamp a provider only once its
fact sheet was re-verified online in this run: the line is a claim that it was.

Only the standard library is used. Set GITHUB_TOKEN (or GH_TOKEN) to lift
GitHub's anonymous rate limit.
"""
import datetime
import json
import os
import re
import sys
import urllib.request

LINE = re.compile(
    r"^\*\*Last verified online:\*\* (?P<date>\d{4}-\d{2}-\d{2}), against "
    r"(?P<against>.*?) \((?P<sources>[^)]*)\)(?P<rest>.*)$",
    re.M,
)


def get_json(url):
    headers = {"User-Agent": "conforme-verify-providers", "Accept": "application/json"}
    token = os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN")
    if token and url.startswith("https://api.github.com/"):
        headers["Authorization"] = f"Bearer {token}"
    with urllib.request.urlopen(urllib.request.Request(url, headers=headers), timeout=30) as r:
        return json.load(r)


def npm(package):
    return lambda: get_json(f"https://registry.npmjs.org/{package}/latest")["version"]


def pypi(package):
    return lambda: get_json(f"https://pypi.org/pypi/{package}/json")["info"]["version"]


def brew(formula):
    return lambda: get_json(f"https://formulae.brew.sh/api/formula/{formula}.json")["versions"]["stable"]


def github(repo, tag=r"v?\d", prerelease=False):
    """The newest release whose tag matches `tag` (a repository also tags
    other packages: `jetbrains/…`, `process-daemon-v…`), without its prefix
    (`v`, `rust-v`, `dsh-v`). GitHub's own "latest" flag is not used: OpenCode
    sets it on its OpenCode 1 line."""

    def fetch():
        for release in get_json(f"https://api.github.com/repos/{repo}/releases?per_page=50"):
            name = release["tag_name"]
            if release["draft"] or (release["prerelease"] and not prerelease):
                continue
            if re.match(tag, name):
                return re.sub(r"^[A-Za-z-]*?v(?=\d)", "", name)
        raise LookupError(f"no release of {repo} with a tag matching {tag}")

    return fetch


def vscode():
    return get_json("https://update.code.visualstudio.com/api/releases/stable")[0]


# Per provider doc: the versions to fetch, and the `against` text they fill.
# A provider whose version no registry publishes lists its changelog instead.
PROVIDERS = {
    "antigravity": ({"cli": github("google-antigravity/antigravity-cli")}, "Antigravity CLI {cli}"),
    "claude-code": ({"cli": npm("@anthropic-ai/claude-code")}, "Claude Code {cli}"),
    "codex": ({"cli": npm("@openai/codex")}, "Codex CLI {cli}"),
    "copilot": (
        {"cli": npm("@github/copilot"), "vscode": vscode},
        "Copilot CLI {cli} and VS Code {vscode}",
    ),
    "cursor": ("https://cursor.com/changelog, https://cursor.com/docs/cli/changelog", None),
    "deepseek": (
        {"harness": github("deepseek-ai/deepseek-harness", tag=r"(dsh-)?v\d", prerelease=True)},
        "DeepSeek Harness v{harness}",
    ),
    "devin": ("https://docs.devin.ai/desktop/changelog, https://docs.devin.ai/cli/changelog/stable", None),
    "gemini": ({"cli": npm("@google/gemini-cli")}, "Gemini CLI v{cli}"),
    "kilo": ({"cli": github("Kilo-Org/kilocode", tag=r"v\d")}, "Kilo Code {cli}"),
    "kiro": ("https://kiro.dev/changelog/", None),
    "opencode": ({"cli": github("anomalyco/opencode", tag=r"v2\.")}, "OpenCode 2 {cli}"),
    "vibe": ({"cli": pypi("mistral-vibe")}, "Mistral Vibe {cli}"),
    "zed": ({"app": github("zed-industries/zed")}, "Zed {app}"),
    "zoocode": ({"ext": github("Zoo-Code-Org/Zoo-Code")}, "Zoo Code {ext}"),
}


def latest(tool):
    """`(versions, against)`: the fetched versions and the text they fill;
    `(changelog, None)` for a MANUAL provider. Raises on a failed fetch."""
    spec, template = PROVIDERS[tool]
    if template is None:
        return spec, None
    versions = {key: fetch() for key, fetch in spec.items()}
    return versions, template.format(**versions)


def doc_path(root, tool):
    return os.path.join(root, "docs", "providers", f"{tool}.md")


def read_line(root, tool):
    text = open(doc_path(root, tool), encoding="utf-8").read()
    match = LINE.search(text)
    if not match:
        sys.exit(f"{tool}: no `Last verified online` line in {doc_path(root, tool)}")
    return text, match


def check(root):
    tools = sorted(f[:-3] for f in os.listdir(os.path.join(root, "docs", "providers")) if f.endswith(".md"))
    unknown = [t for t in tools if t not in PROVIDERS]
    failed = False
    for tool in tools:
        if tool in unknown:
            print(f"{'ERROR':8} {tool:12} no entry in provider_versions.PROVIDERS: add one")
            failed = True
            continue
        _, match = read_line(root, tool)
        line = match.group("against") + " " + match.group("sources")
        try:
            versions, against = latest(tool)
        except Exception as error:  # a registry down or renamed
            print(f"{'ERROR':8} {tool:12} {error}")
            failed = True
            continue
        if against is None:
            print(f"{'MANUAL':8} {tool:12} last {match.group('date')}: {match.group('against')}; read {versions}")
            continue
        stale = [v for v in versions.values() if v not in line]
        state = "STALE" if stale else "CURRENT"
        failed |= bool(stale)
        print(f"{state:8} {tool:12} last {match.group('date')}: {match.group('against')}; latest: {against}")
    return 1 if failed else 0


def stamp(root, tool, against, sources):
    text, match = read_line(root, tool)
    if against is None:
        _, against = latest(tool)
        if against is None:
            sys.exit(f"{tool}: no registry publishes its version; pass --against \"<product> <version>\"")
    line = (
        f"**Last verified online:** {datetime.date.today().isoformat()}, against {against} "
        f"({sources or match.group('sources')}){match.group('rest')}"
    )
    open(doc_path(root, tool), "w", encoding="utf-8").write(text[: match.start()] + line + text[match.end() :])
    print(line)


def main(argv):
    if not argv or argv[0].startswith("-"):
        sys.exit(__doc__)
    root, rest = argv[0], argv[1:]
    if not rest:
        return check(root)
    options = dict(zip(rest[::2], rest[1::2]))
    if "--stamp" not in options or set(options) - {"--stamp", "--against", "--sources"}:
        sys.exit(__doc__)
    targets = sorted(PROVIDERS) if options["--stamp"] == "all" else [options["--stamp"]]
    for tool in targets:
        if tool not in PROVIDERS:
            sys.exit(f"unknown provider {tool}")
        if options["--stamp"] == "all" and PROVIDERS[tool][1] is None:
            print(f"skipped {tool}: stamp it with --against once its changelog is read")
            continue
        stamp(root, tool, options.get("--against"), options.get("--sources"))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
