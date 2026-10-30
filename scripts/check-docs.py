"""Check repository Markdown links/anchors; sibling planning sources are optional in CI."""

from pathlib import Path
import re
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parent.parent
failures, external = [], 0
for source in [ROOT / "README.md", ROOT / "AGENTS.md", *sorted((ROOT / "docs").rglob("*.md"))]:
    text = source.read_text(encoding="utf-8")
    for reference in re.findall(r"\]\(([^)\n]+)\)", text):
        reference = reference.strip("<>")
        if urlsplit(reference).scheme:
            continue
        path, _, anchor = reference.partition("#")
        target = (source.parent / unquote(path)).resolve() if path else source
        if not target.is_relative_to(ROOT) and not target.exists():
            external += 1
            continue
        if not target.exists():
            failures.append(f"{source.relative_to(ROOT)}: missing {reference}")
            continue
        if anchor and target.suffix == ".md":
            content = target.read_text(encoding="utf-8")
            headings = re.findall(r"^#{1,6}\s+(.+?)\s*$", content, re.MULTILINE)
            anchors = {re.sub(r"\s", "-", re.sub(r"[^\w\s-]", "", heading.lower())) for heading in headings}
            anchors.update(re.findall(r'\bid="([^"]+)"', content))
            if unquote(anchor) not in anchors:
                failures.append(f"{source.relative_to(ROOT)}: missing anchor {reference}")
if failures:
    raise SystemExit("\n".join(failures))
print(f"Repository document links/anchors verified; {external} unavailable sibling-planning links deferred to canonical URLs.")
