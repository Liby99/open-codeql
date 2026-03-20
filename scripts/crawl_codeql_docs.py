#!/usr/bin/env python3
"""
Crawl CodeQL documentation from codeql.github.com and the github/codeql repository.

Saves all pages as markdown files under docs/crawled/ with an organized directory structure.
Creates a manifest.json listing all downloaded files and their source URLs.

Usage:
    pip install requests beautifulsoup4 markdownify
    python3 scripts/crawl_codeql_docs.py

Features:
    - Comprehensive coverage of QL language reference, language guides, query help
    - Fetches raw .dbscheme files from GitHub
    - Rate limiting (1 second between requests)
    - Resume support (skips already-downloaded files)
    - Progress reporting
"""

import json
import os
import re
import sys
import time
from pathlib import Path
from urllib.parse import urljoin

try:
    import requests
    from bs4 import BeautifulSoup
    try:
        from markdownify import markdownify as md
    except ImportError:
        md = None
except ImportError:
    print("Required packages not found. Install with:")
    print("  pip install requests beautifulsoup4 markdownify")
    sys.exit(1)

# Project root
PROJECT_ROOT = Path(__file__).parent.parent
CRAWL_DIR = PROJECT_ROOT / "docs" / "crawled"
MANIFEST_PATH = CRAWL_DIR / "manifest.json"

# Rate limiting
REQUEST_DELAY = 1.0  # seconds between requests

# Base URLs
DOCS_BASE = "https://codeql.github.com/docs/"
QUERY_HELP_BASE = "https://codeql.github.com/codeql-query-help/"
RAW_GITHUB = "https://raw.githubusercontent.com/github/codeql/main/"

# ──────────────────────────────────────────────────────────────────
# URL Registry: All pages to crawl, organized by section
# ──────────────────────────────────────────────────────────────────

PAGES = {
    # QL Language Reference
    "ql-language-reference": {
        "output_dir": "ql-language-reference",
        "urls": [
            ("about-the-ql-language", f"{DOCS_BASE}ql-language-reference/about-the-ql-language/"),
            ("predicates", f"{DOCS_BASE}ql-language-reference/predicates/"),
            ("queries", f"{DOCS_BASE}ql-language-reference/queries/"),
            ("types", f"{DOCS_BASE}ql-language-reference/types/"),
            ("modules", f"{DOCS_BASE}ql-language-reference/modules/"),
            ("signatures", f"{DOCS_BASE}ql-language-reference/signatures/"),
            ("aliases", f"{DOCS_BASE}ql-language-reference/aliases/"),
            ("variables", f"{DOCS_BASE}ql-language-reference/variables/"),
            ("expressions", f"{DOCS_BASE}ql-language-reference/expressions/"),
            ("formulas", f"{DOCS_BASE}ql-language-reference/formulas/"),
            ("annotations", f"{DOCS_BASE}ql-language-reference/annotations/"),
            ("recursion", f"{DOCS_BASE}ql-language-reference/recursion/"),
            ("lexical-syntax", f"{DOCS_BASE}ql-language-reference/lexical-syntax/"),
            ("name-resolution", f"{DOCS_BASE}ql-language-reference/name-resolution/"),
            ("evaluation-of-ql-programs", f"{DOCS_BASE}ql-language-reference/evaluation-of-ql-programs/"),
            ("ql-language-specification", f"{DOCS_BASE}ql-language-reference/ql-language-specification/"),
        ],
    },
    # CodeQL Overview
    "codeql-overview": {
        "output_dir": "codeql-overview",
        "urls": [
            ("about-codeql", f"{DOCS_BASE}codeql-overview/about-codeql/"),
            ("supported-languages-and-frameworks", f"{DOCS_BASE}codeql-overview/supported-languages-and-frameworks/"),
            ("system-requirements", f"{DOCS_BASE}codeql-overview/system-requirements/"),
            ("codeql-changelog", f"{DOCS_BASE}codeql-overview/codeql-changelog/"),
            ("codeql-tools", f"{DOCS_BASE}codeql-overview/codeql-tools/"),
            ("codeql-glossary", f"{DOCS_BASE}codeql-overview/codeql-glossary/"),
        ],
    },
    # Writing CodeQL Queries
    "writing-codeql-queries": {
        "output_dir": "writing-codeql-queries",
        "urls": [
            ("codeql-queries", f"{DOCS_BASE}writing-codeql-queries/codeql-queries/"),
            ("ql-tutorials", f"{DOCS_BASE}writing-codeql-queries/ql-tutorials/"),
            ("running-codeql-queries", f"{DOCS_BASE}writing-codeql-queries/running-codeql-queries/"),
        ],
    },
    # C/C++ Language Guides
    "language-guides-cpp": {
        "output_dir": "languages/cpp/guides",
        "urls": [
            ("codeql-for-cpp", f"{DOCS_BASE}codeql-language-guides/codeql-for-cpp/"),
            ("basic-query-for-cpp-code", f"{DOCS_BASE}codeql-language-guides/basic-query-for-cpp-code/"),
            ("codeql-library-for-cpp", f"{DOCS_BASE}codeql-language-guides/codeql-library-for-cpp/"),
            ("functions-in-cpp", f"{DOCS_BASE}codeql-language-guides/functions-in-cpp/"),
            ("expressions-types-and-statements-in-cpp", f"{DOCS_BASE}codeql-language-guides/expressions-types-and-statements-in-cpp/"),
            ("conversions-and-classes-in-cpp", f"{DOCS_BASE}codeql-language-guides/conversions-and-classes-in-cpp/"),
            ("analyzing-data-flow-in-cpp", f"{DOCS_BASE}codeql-language-guides/analyzing-data-flow-in-cpp/"),
            ("refining-a-query-to-account-for-edge-cases", f"{DOCS_BASE}codeql-language-guides/refining-a-query-to-account-for-edge-cases/"),
            ("detecting-a-potential-buffer-overflow", f"{DOCS_BASE}codeql-language-guides/detecting-a-potential-buffer-overflow/"),
            ("using-the-guards-library-in-cpp", f"{DOCS_BASE}codeql-language-guides/using-the-guards-library-in-cpp/"),
            ("using-range-analysis-in-cpp", f"{DOCS_BASE}codeql-language-guides/using-range-analsis-in-cpp/"),
            ("hash-consing-and-value-numbering", f"{DOCS_BASE}codeql-language-guides/hash-consing-and-value-numbering/"),
            ("advanced-dataflow-scenarios-cpp", f"{DOCS_BASE}codeql-language-guides/advanced-dataflow-scenarios-cpp/"),
            ("customizing-library-models-for-cpp", f"{DOCS_BASE}codeql-language-guides/customizing-library-models-for-cpp/"),
        ],
    },
    # Java Language Guides
    "language-guides-java": {
        "output_dir": "languages/java/guides",
        "urls": [
            ("codeql-for-java", f"{DOCS_BASE}codeql-language-guides/codeql-for-java/"),
            ("basic-query-for-java-code", f"{DOCS_BASE}codeql-language-guides/basic-query-for-java-code/"),
            ("codeql-library-for-java", f"{DOCS_BASE}codeql-language-guides/codeql-library-for-java/"),
            ("analyzing-data-flow-in-java", f"{DOCS_BASE}codeql-language-guides/analyzing-data-flow-in-java/"),
            ("types-in-java", f"{DOCS_BASE}codeql-language-guides/types-in-java/"),
            ("overflow-prone-comparisons-in-java", f"{DOCS_BASE}codeql-language-guides/overflow-prone-comparisons-in-java/"),
            ("navigating-the-call-graph", f"{DOCS_BASE}codeql-language-guides/navigating-the-call-graph/"),
            ("annotations-in-java", f"{DOCS_BASE}codeql-language-guides/annotations-in-java/"),
            ("javadoc", f"{DOCS_BASE}codeql-language-guides/javadoc/"),
            ("working-with-source-locations", f"{DOCS_BASE}codeql-language-guides/working-with-source-locations/"),
            ("ast-classes-for-java", f"{DOCS_BASE}codeql-language-guides/abstract-syntax-tree-classes-for-working-with-java-programs/"),
            ("customizing-library-models-for-java", f"{DOCS_BASE}codeql-language-guides/customizing-library-models-for-java-and-kotlin/"),
        ],
    },
    # Query Help
    "query-help": {
        "output_dir": "query-help",
        "urls": [
            ("cpp-index", f"{QUERY_HELP_BASE}cpp/"),
            ("java-index", f"{QUERY_HELP_BASE}java/"),
        ],
    },
}

# Raw files to download (not HTML, saved as-is)
RAW_FILES = {
    "dbscheme-cpp": {
        "url": f"{RAW_GITHUB}cpp/ql/lib/semmlecode.cpp.dbscheme",
        "output": "languages/cpp/semmlecode.cpp.dbscheme",
    },
    "dbscheme-java": {
        "url": f"{RAW_GITHUB}java/ql/lib/config/semmlecode.dbscheme",
        "output": "languages/java/semmlecode.dbscheme",
    },
}


def html_to_markdown(html_content: str, url: str) -> str:
    """Convert HTML page content to clean markdown."""
    soup = BeautifulSoup(html_content, "html.parser")

    # Try to find the main content area
    main = (
        soup.find("main")
        or soup.find("article")
        or soup.find("div", class_="content")
        or soup.find("div", {"role": "main"})
        or soup.find("div", class_="markdown-body")
    )

    if main is None:
        main = soup.find("body") or soup

    # Remove navigation, footer, sidebar elements
    for tag in main.find_all(["nav", "footer", "aside", "header"]):
        tag.decompose()
    for tag in main.find_all(class_=re.compile(r"(sidebar|nav|footer|header|breadcrumb|toc)")):
        tag.decompose()
    for tag in main.find_all("script"):
        tag.decompose()
    for tag in main.find_all("style"):
        tag.decompose()

    if md is not None:
        content = md(str(main), heading_style="ATX", code_language_callback=lambda el: "ql")
    else:
        # Fallback: extract text with basic formatting
        content = extract_text_fallback(main)

    # Clean up excessive whitespace
    content = re.sub(r"\n{3,}", "\n\n", content)
    content = content.strip()

    # Add source URL header
    header = f"<!-- Source: {url} -->\n<!-- Crawled for open-cql project -->\n\n"
    return header + content


def extract_text_fallback(element) -> str:
    """Fallback text extraction when markdownify is not available."""
    lines = []
    for tag in element.find_all(["h1", "h2", "h3", "h4", "h5", "h6", "p", "pre", "code", "li", "td", "th"]):
        text = tag.get_text(strip=True)
        if not text:
            continue
        if tag.name.startswith("h"):
            level = int(tag.name[1])
            lines.append(f"\n{'#' * level} {text}\n")
        elif tag.name == "pre":
            code = tag.get_text()
            lines.append(f"\n```\n{code}\n```\n")
        elif tag.name == "li":
            lines.append(f"- {text}")
        else:
            lines.append(text)
    return "\n".join(lines)


def fetch_page(url: str, session: requests.Session) -> str | None:
    """Fetch a URL and return its content."""
    try:
        response = session.get(url, timeout=30)
        response.raise_for_status()
        return response.text
    except requests.RequestException as e:
        print(f"  ERROR fetching {url}: {e}")
        return None


def crawl_html_pages(session: requests.Session, manifest: dict) -> int:
    """Crawl all HTML documentation pages. Returns count of pages crawled."""
    total = sum(len(section["urls"]) for section in PAGES.values())
    crawled = 0
    skipped = 0

    for section_name, section in PAGES.items():
        output_dir = CRAWL_DIR / section["output_dir"]
        output_dir.mkdir(parents=True, exist_ok=True)

        for page_name, url in section["urls"]:
            output_path = output_dir / f"{page_name}.md"

            if output_path.exists():
                print(f"  [{crawled + skipped + 1}/{total}] SKIP (exists): {page_name}")
                manifest[str(output_path.relative_to(CRAWL_DIR))] = {
                    "url": url,
                    "section": section_name,
                    "status": "skipped",
                }
                skipped += 1
                continue

            print(f"  [{crawled + skipped + 1}/{total}] Fetching: {page_name}")
            html = fetch_page(url, session)

            if html:
                markdown = html_to_markdown(html, url)
                output_path.write_text(markdown, encoding="utf-8")
                manifest[str(output_path.relative_to(CRAWL_DIR))] = {
                    "url": url,
                    "section": section_name,
                    "status": "ok",
                    "size": len(markdown),
                }
                crawled += 1
            else:
                manifest[str(output_path.relative_to(CRAWL_DIR))] = {
                    "url": url,
                    "section": section_name,
                    "status": "error",
                }

            time.sleep(REQUEST_DELAY)

    return crawled


def crawl_raw_files(session: requests.Session, manifest: dict) -> int:
    """Download raw files (dbscheme, etc). Returns count downloaded."""
    crawled = 0

    for name, info in RAW_FILES.items():
        output_path = CRAWL_DIR / info["output"]
        output_path.parent.mkdir(parents=True, exist_ok=True)

        if output_path.exists():
            print(f"  SKIP (exists): {name}")
            manifest[str(output_path.relative_to(CRAWL_DIR))] = {
                "url": info["url"],
                "section": "raw",
                "status": "skipped",
            }
            continue

        print(f"  Fetching raw: {name}")
        content = fetch_page(info["url"], session)

        if content:
            output_path.write_text(content, encoding="utf-8")
            manifest[str(output_path.relative_to(CRAWL_DIR))] = {
                "url": info["url"],
                "section": "raw",
                "status": "ok",
                "size": len(content),
            }
            crawled += 1
        else:
            manifest[str(output_path.relative_to(CRAWL_DIR))] = {
                "url": info["url"],
                "section": "raw",
                "status": "error",
            }

        time.sleep(REQUEST_DELAY)

    return crawled


def crawl_query_help_links(session: requests.Session, manifest: dict) -> int:
    """Crawl individual query help pages for C++ and Java."""
    crawled = 0

    for lang, lang_dir in [("cpp", "languages/cpp/query-help"), ("java", "languages/java/query-help")]:
        index_url = f"{QUERY_HELP_BASE}{lang}/"
        print(f"\n  Fetching query help index for {lang}...")
        html = fetch_page(index_url, session)
        if not html:
            continue

        soup = BeautifulSoup(html, "html.parser")
        links = []
        for a in soup.find_all("a", href=True):
            href = a["href"]
            if href.startswith(f"/codeql-query-help/{lang}/") and href != f"/codeql-query-help/{lang}/":
                full_url = f"https://codeql.github.com{href}"
                page_name = href.rstrip("/").split("/")[-1]
                if page_name and page_name != lang:
                    links.append((page_name, full_url))

        # Deduplicate
        seen = set()
        unique_links = []
        for name, url in links:
            if name not in seen:
                seen.add(name)
                unique_links.append((name, url))

        output_dir = CRAWL_DIR / lang_dir
        output_dir.mkdir(parents=True, exist_ok=True)

        print(f"  Found {len(unique_links)} query help pages for {lang}")

        for i, (page_name, url) in enumerate(unique_links):
            output_path = output_dir / f"{page_name}.md"

            if output_path.exists():
                manifest[str(output_path.relative_to(CRAWL_DIR))] = {
                    "url": url,
                    "section": f"query-help-{lang}",
                    "status": "skipped",
                }
                continue

            print(f"  [{i+1}/{len(unique_links)}] {page_name}")
            page_html = fetch_page(url, session)
            if page_html:
                markdown = html_to_markdown(page_html, url)
                output_path.write_text(markdown, encoding="utf-8")
                manifest[str(output_path.relative_to(CRAWL_DIR))] = {
                    "url": url,
                    "section": f"query-help-{lang}",
                    "status": "ok",
                    "size": len(markdown),
                }
                crawled += 1

            time.sleep(REQUEST_DELAY)

    return crawled


def main():
    print("=" * 60)
    print("open-cql: CodeQL Documentation Crawler")
    print("=" * 60)

    CRAWL_DIR.mkdir(parents=True, exist_ok=True)

    # Load existing manifest if resuming
    manifest = {}
    if MANIFEST_PATH.exists():
        with open(MANIFEST_PATH) as f:
            manifest = json.load(f)

    session = requests.Session()
    session.headers.update({
        "User-Agent": "open-cql-docs-crawler/1.0 (academic research project)"
    })

    # Phase 1: HTML documentation pages
    print("\n--- Phase 1: HTML Documentation Pages ---")
    html_count = crawl_html_pages(session, manifest)
    print(f"  Crawled {html_count} new HTML pages")

    # Phase 2: Raw files (dbscheme, etc.)
    print("\n--- Phase 2: Raw Files ---")
    raw_count = crawl_raw_files(session, manifest)
    print(f"  Downloaded {raw_count} new raw files")

    # Phase 3: Query help individual pages
    print("\n--- Phase 3: Query Help Pages ---")
    qh_count = crawl_query_help_links(session, manifest)
    print(f"  Crawled {qh_count} new query help pages")

    # Save manifest
    with open(MANIFEST_PATH, "w") as f:
        json.dump(manifest, f, indent=2, sort_keys=True)

    # Summary
    total_files = len([f for f in manifest.values() if f["status"] in ("ok", "skipped")])
    print(f"\n{'=' * 60}")
    print(f"Done! {total_files} total files in manifest.")
    print(f"  New downloads: {html_count + raw_count + qh_count}")
    print(f"  Output directory: {CRAWL_DIR}")
    print(f"  Manifest: {MANIFEST_PATH}")
    print(f"{'=' * 60}")


if __name__ == "__main__":
    main()
