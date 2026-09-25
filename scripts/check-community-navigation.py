#!/usr/bin/env python3
"""SSR acceptance checks for the fediverse.kr community navigation IA.

This standard-library-only checker validates HTTP status and server-rendered
HTML. It deliberately does not claim browser hydration or visual verification.
"""
import argparse
from dataclasses import dataclass, field
from html.parser import HTMLParser
import json
from pathlib import Path
import sys
from typing import Optional
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen


EXTERNAL_URLS = ("https://fedidev.kr", "https://joinfediverse.kr")
REQUIRED_ROUTES = ("/", "/community", "/people", "/operate", "/servers", "/start/delivery")
VOID_ELEMENTS = {"area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr"}
UNCHECKED = [
    "hydration behavior (requires real browser)",
    "visual/layout verification (requires real browser)",
]


@dataclass
class Page:
    status: Optional[int]
    html: str
    error: Optional[str] = None


@dataclass
class Node:
    tag: str
    attrs: dict[str, str]
    parent: Optional["Node"] = None
    children: list["Node"] = field(default_factory=list)
    text_parts: list[str] = field(default_factory=list)

    @property
    def text(self) -> str:
        return " ".join(" ".join(self.text_parts).split())


class DocumentParser(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.root = Node("root", {})
        self.stack = [self.root]

    def handle_starttag(self, tag, attrs):
        node = Node(tag, {name: value or "" for name, value in attrs}, self.stack[-1])
        self.stack[-1].children.append(node)
        if tag not in VOID_ELEMENTS:
            self.stack.append(node)

    def handle_startendtag(self, tag, attrs):
        self.handle_starttag(tag, attrs)
        self.handle_endtag(tag)

    def handle_endtag(self, tag):
        for index in range(len(self.stack) - 1, 0, -1):
            if self.stack[index].tag == tag:
                del self.stack[index:]
                return

    def handle_data(self, data):
        for node in self.stack:
            node.text_parts.append(data)


def descendants(node):
    for child in node.children:
        yield child
        yield from descendants(child)


def classes(node):
    return set(node.attrs.get("class", "").split())


def is_primary_nav(node):
    return node.tag == "nav" and node.attrs.get("aria-label") == "주요 메뉴"


def analyze_html(html):
    parser = DocumentParser()
    parser.feed(html)
    parser.close()
    nodes = [parser.root, *descendants(parser.root)]
    navs = [node for node in nodes if is_primary_nav(node)]
    primary_links = []
    primary_containers = []
    for nav in navs:
        links = [node for node in descendants(nav) if node.tag == "a"]
        if links:
            primary_containers.append(links)
            primary_links.extend(links)
    mains = [node for node in nodes if node.tag == "main"]
    main_links = [link for main in mains for link in descendants(main) if link.tag == "a"]
    return {
        "primary_nav_labels": [link.text for link in primary_links],
        "primary_nav_links": [link.attrs for link in primary_links],
        "community_nav_links": [link.attrs for link in primary_links if link.text == "커뮤니티"],
        "primary_nav_containers": [[link.text for link in links] for links in primary_containers],
        "primary_nav_active_labels": [link.text for link in primary_links if "active" in classes(link)],
        "main_text": " ".join(main.text for main in mains),
        "main_links": [link.attrs for link in main_links],
        "main_ids": [node.attrs["id"] for main in mains for node in descendants(main) if "id" in node.attrs],
        "h1": [node.text for node in nodes if node.tag == "h1"],
    }


def assertion(assertions, assertion_id, passed, detail):
    assertions.append({"id": assertion_id, "passed": bool(passed), "detail": detail})


def safe_external_link(links, url):
    for link in links:
        if link.get("href") != url:
            continue
        rel_tokens = set(link.get("rel", "").split())
        if link.get("target") == "_blank" and {"noopener", "noreferrer"} <= rel_tokens:
            return True
    return False


def evaluate_pages(pages):
    assertions = []
    transport_errors = []
    for path, page in pages.items():
        if page.error:
            transport_errors.append({"path": path, "error": page.error})
            continue
        assertion(assertions, f"{path}.status-200", page.status == 200, "HTTP status must be 200")

    home = pages.get("/")
    if home and not home.error and home.status == 200:
        document = analyze_html(home.html)
        navs = document["primary_nav_containers"]
        assertion(assertions, "home.primary-nav-rendered", bool(navs), "at least one primary nav container is rendered")
        assertion(assertions, "home.primary-nav-community", bool(navs) and all("커뮤니티" in nav for nav in navs), "each rendered primary nav contains 커뮤니티")
        assertion(assertions, "home.primary-nav-people-absent", all("사람 찾기" not in nav for nav in navs), "사람 찾기 is absent only from primary nav")
        labels = document["primary_nav_labels"]
        assertion(assertions, "home.primary-nav-retains-server-operate", "서버 찾기" in labels and "서버 운영" in labels, "server discovery and operation navigation remain")
        hrefs = [link.get("href", "") for link in document["main_links"]]
        assertion(assertions, "home.no-old-community-hero", "#community-resources" not in hrefs, "home main has no old community hero anchor")
        assertion(assertions, "home.no-old-community-section", "community-resources" not in document["main_ids"], "home main has no old community section ID")
        assertion(assertions, "home.no-external-community-content", not any(url in hrefs for url in EXTERNAL_URLS), "home main has no external community URLs")

    community = pages.get("/community")
    if community and not community.error and community.status == 200:
        document = analyze_html(community.html)
        assertion(assertions, "community.exact-h1", document["h1"] == ["한국어 연합우주 커뮤니티"], "community h1 is exact")
        assertion(assertions, "community.independent-projects", "독립" in document["main_text"] and "프로젝트" in document["main_text"], "content states independent projects")
        assertion(assertions, "community.fedidev-safe-external-link", safe_external_link(document["main_links"], EXTERNAL_URLS[0]), "fedidev link is target=_blank with noopener noreferrer")
        assertion(assertions, "community.joinfediverse-safe-external-link", safe_external_link(document["main_links"], EXTERNAL_URLS[1]), "joinfediverse link is target=_blank with noopener noreferrer")
        assertion(assertions, "community.primary-nav-active", "커뮤니티" in document["primary_nav_active_labels"], "primary nav uses existing active class convention")

    for path, name in [("/", "home"), ("/community", "community")]:
        page = pages.get(path)
        if page and not page.error and page.status == 200:
            links = analyze_html(page.html)["community_nav_links"]
            assertion(assertions, f"{name}.community-nav-exact-href", bool(links) and all(link.get("href") == "/community" for link in links), "every primary-nav community label links exactly to /community")

    return {
        "assertions": assertions,
        "failed_assertions": sum(not item["passed"] for item in assertions),
        "transport_errors": transport_errors,
        "unchecked": UNCHECKED,
    }


def fetch_page(base_url, path):
    url = base_url.rstrip("/") + path
    request = Request(url, headers={"User-Agent": "fediverse-kr-community-navigation-check/1"})
    try:
        with urlopen(request, timeout=20) as response:
            charset = response.headers.get_content_charset() or "utf-8"
            return Page(response.status, response.read().decode(charset, errors="replace"))
    except HTTPError as error:
        return Page(error.code, "")
    except (URLError, OSError, ValueError) as error:
        return Page(None, "", type(error).__name__)


def main(argv=None):
    parser = argparse.ArgumentParser(description="Check SSR community navigation acceptance criteria.")
    parser.add_argument("--base-url", required=True, help="Candidate root, e.g. http://127.0.0.1:8080")
    parser.add_argument("--output", type=Path, help="Write the JSON report to this path")
    args = parser.parse_args(argv)
    pages = {path: fetch_page(args.base_url, path) for path in REQUIRED_ROUTES}
    report = evaluate_pages(pages)
    report["base_url"] = args.base_url.rstrip("/")
    report["checked_routes"] = list(REQUIRED_ROUTES)
    report["outcome"] = "transport-error" if report["transport_errors"] else ("failed" if report["failed_assertions"] else "passed")
    encoded = json.dumps(report, ensure_ascii=False, indent=2) + "\n"
    if args.output:
        args.output.write_text(encoded, encoding="utf-8")
    print(f"community navigation SSR check: {report['outcome']}; assertions={len(report['assertions'])}; failed={report['failed_assertions']}; transport_errors={len(report['transport_errors'])}")
    print("unchecked: " + "; ".join(UNCHECKED))
    return 2 if report["transport_errors"] else (1 if report["failed_assertions"] else 0)


if __name__ == "__main__":
    sys.exit(main())
