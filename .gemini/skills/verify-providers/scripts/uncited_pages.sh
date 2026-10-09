#!/bin/bash
# List the pages of each vendor's docs index (llms.txt or sitemap.xml) that are
# on a topic conforme cares about and that no docs/providers file cites yet.
#
# Usage: uncited_pages.sh <project_root> <output_dir>
# Writes <output_dir>/<site>.all (every page) and <site>.new (candidates), and
# prints the counts. Candidates are leads: open each before citing it.
set -u
root=${1:?project root}
out=${2:?output directory}
mkdir -p "$out"

grep -oh 'https://[^ )>,`"]*' "$root"/docs/providers/*.md | sed 's/[.,]$//; s:/$::' | sort -u > "$out/cited.txt"

topics='rule|skill|agent|mcp|config|setting|instruction|memory|steering|context|custom|mode|hook|command|prompt|trust|permission|plugin|extension|cli'
skip='/(zh|ja|ko|fr|de|es|pt|ru|it|tr|id|pl|vi|uk|zh-cn|zh-tw|pt-br)(/|$)|blog|pricing'

# One docs index per vendor; add a line when an adapter is added.
while read -r site url; do
  curl -sS -A 'Mozilla/5.0' --max-time 60 -L "$url" 2>/dev/null \
    | grep -oE 'https://[^ <>"()]*' | sed 's/[.,]$//; s:/$::' | sort -u > "$out/$site.all"
  grep -iE "$topics" "$out/$site.all" | grep -viE "$skip" | grep -vxFf "$out/cited.txt" > "$out/$site.new"
  printf '%-10s %5s pages, %4s uncited on topic\n' "$site" "$(wc -l < "$out/$site.all")" "$(wc -l < "$out/$site.new")"
done <<'SITES'
claude https://code.claude.com/docs/llms.txt
cursor https://cursor.com/llms.txt
codex https://developers.openai.com/codex/llms.txt
opencode https://opencode.ai/sitemap.xml
kilo https://kilo.ai/docs/llms.txt
gemini https://geminicli.com/llms.txt
copilot https://docs.github.com/llms.txt
vscode https://code.visualstudio.com/sitemap.xml
zed https://zed.dev/docs/llms.txt
kiro https://kiro.dev/llms.txt
devin https://docs.devin.ai/llms.txt
zoocode https://docs.zoocode.dev/sitemap.xml
vibe https://docs.mistral.ai/sitemap.xml
SITES
echo "DeepSeek Harness has no docs index: browse its developer docs and the repository's docs/ tree."
