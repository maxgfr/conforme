#!/usr/bin/env python3
"""Check that every tool's copy of every skill matches the source after a sync.

Usage: skills_conformity.py <project_root> [--source <skills dir>]

The source skills directory defaults to `.claude/skills`. Every other directory
holding `<name>/SKILL.md` folders (`.cursor/skills`, `.agents/skills`,
`.gemini/skills`, ...) is a copy. For each source skill and each copy root:

- MISSING      the copy has no `<name>/SKILL.md`
- NAME         the copy's `name` is not its folder name
- DESCRIPTION  the copy has no `description`, or one over 1024 characters
- BODY         the copy's instructions differ from the source's
- BUNDLE       a bundled file (script, reference) is missing or differs
- EXTRA        the copy bundles a file the source does not have
- STALE        the copy root has a skill the source does not (warning: it may
               be one written by hand in that tool)

Exits 1 when any finding but STALE is reported. Standard library only.
"""

import os
import re
import sys

SKIP_DIRS = {".git", "node_modules", "target", "worktrees"}
METADATA = {"agents/openai.yaml"}
MAX_DESCRIPTION = 1024


def kebab(name):
    return re.sub(r"[^a-z0-9]+", "-", name.lower()).strip("-")


def split_frontmatter(text):
    """Return (fields, body) for a `---` frontmatter; values are plain strings."""
    lines = text.split("\n")
    if not lines or lines[0].strip() != "---":
        return {}, text.strip()
    try:
        end = next(i for i in range(1, len(lines)) if lines[i].strip() == "---")
    except StopIteration:
        return {}, text.strip()
    fields, key = {}, None
    for line in lines[1:end]:
        match = re.match(r"^([A-Za-z0-9_-]+):\s*(.*)$", line)
        if match and not line.startswith((" ", "\t")):
            key, value = match.group(1), match.group(2).strip()
            fields[key] = "" if value in (">", ">-", "|", "|-") else value
        elif key and line.strip():
            fields[key] = (fields[key] + " " + line.strip()).strip()
    for key, value in fields.items():
        if len(value) >= 2 and value[0] == value[-1] and value[0] in "'\"":
            fields[key] = value[1:-1]
    return fields, "\n".join(lines[end + 1 :]).strip()


def bundled(skill_dir):
    """Text files bundled in a skill folder, by relative path."""
    files = {}
    for dirpath, dirnames, filenames in os.walk(skill_dir):
        dirnames[:] = [
            d for d in dirnames if not os.path.exists(os.path.join(dirpath, d, "SKILL.md"))
        ]
        for filename in filenames:
            path = os.path.join(dirpath, filename)
            rel = os.path.relpath(path, skill_dir).replace(os.sep, "/")
            if rel == "SKILL.md" or rel in METADATA or filename == ".DS_Store":
                continue
            try:
                with open(path, encoding="utf-8") as handle:
                    files[rel] = handle.read()
            except UnicodeDecodeError:
                pass
    return files


def skills_in(root):
    """`{folder name: folder path}` for the `<name>/SKILL.md` folders of a root."""
    found = {}
    if os.path.isdir(root):
        for entry in sorted(os.listdir(root)):
            path = os.path.join(root, entry)
            if os.path.isfile(os.path.join(path, "SKILL.md")):
                found[entry] = path
    return found


def copy_roots(project, source):
    roots = set()
    for dirpath, dirnames, filenames in os.walk(project):
        dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS]
        if "SKILL.md" in filenames:
            root = os.path.dirname(dirpath)
            if os.path.basename(root) == "skills":
                roots.add(root)
            dirnames[:] = []
    roots.discard(source)
    return sorted(roots)


def main(argv):
    if len(argv) < 2:
        print(__doc__)
        return 2
    project = os.path.abspath(argv[1])
    source_rel = argv[argv.index("--source") + 1] if "--source" in argv else ".claude/skills"
    source = os.path.join(project, source_rel)
    source_skills = skills_in(source)
    if not source_skills:
        print(f"no skill in {source_rel}")
        return 2

    expected = {}
    for folder, path in source_skills.items():
        with open(os.path.join(path, "SKILL.md"), encoding="utf-8") as handle:
            fields, body = split_frontmatter(handle.read())
        expected[kebab(fields.get("name") or folder)] = (body, bundled(path))

    errors, warnings = [], []
    roots = copy_roots(project, source)
    for root in roots:
        rel_root = os.path.relpath(root, project)
        copies = skills_in(root)
        for name, (body, files) in expected.items():
            where = f"{rel_root}/{name}"
            if name not in copies:
                errors.append(f"MISSING      {where}/SKILL.md")
                continue
            with open(os.path.join(copies[name], "SKILL.md"), encoding="utf-8") as handle:
                fields, copy_body = split_frontmatter(handle.read())
            if fields.get("name", name) != name:
                errors.append(f"NAME         {where}: name '{fields.get('name')}'")
            description = fields.get("description", "")
            if not description or len(description) > MAX_DESCRIPTION:
                errors.append(f"DESCRIPTION  {where}: {len(description)} characters")
            if copy_body != body:
                errors.append(f"BODY         {where}: instructions differ from the source")
            copy_files = bundled(copies[name])
            for rel, content in files.items():
                if copy_files.get(rel) != content:
                    errors.append(f"BUNDLE       {where}/{rel}: missing or different")
            for rel in sorted(set(copy_files) - set(files)):
                errors.append(f"EXTRA        {where}/{rel}: not in the source")
        for name in sorted(set(copies) - set(expected)):
            warnings.append(f"STALE        {rel_root}/{name}: not in the source")

    print(f"source {source_rel}: {len(expected)} skill(s); {len(roots)} copy root(s)")
    for root in roots:
        print(f"  {os.path.relpath(root, project)}")
    for line in errors + warnings:
        print(line)
    if not errors and not warnings:
        print("every copy matches the source")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
