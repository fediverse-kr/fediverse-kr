#!/usr/bin/env python3
"""Fixture-only parser contracts for check-community-navigation.py.

These tests never call a live service. They prove parser boundaries only; the
CLI itself reports live assertions separately.
"""
import importlib.util
from pathlib import Path
import unittest


SCRIPT = Path(__file__).with_name("check-community-navigation.py")
SPEC = importlib.util.spec_from_file_location("community_navigation", SCRIPT)
checker = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(checker)


HOME_OK = """
<html><body>
<nav class="site-nav wrap" aria-label="주요 메뉴"><div class="nav-links">
<a href="/start">연합우주 이해하기</a><a href="/servers">서버 찾기</a>
<a href="/community">커뮤니티</a><a href="/operate">서버 운영</a></div></nav>
<dialog><nav class="mobile-navigation-links" aria-label="주요 메뉴">
<a href="/community">커뮤니티</a><a href="/operate">서버 운영</a></nav></dialog>
<main id="content"><a href="/people">사람 찾기 안내 문서</a><p>일반 본문</p></main>
</body></html>
"""
COMMUNITY_OK = """
<html><body>
<nav aria-label="주요 메뉴"><a href="/community" class="active">커뮤니티</a></nav>
<main><h1>한국어 연합우주 커뮤니티</h1><p>서로 독립적으로 운영되는 프로젝트입니다.</p>
<a href="https://fedidev.kr" target="_blank" rel="noopener noreferrer">개발자 모임</a>
<a href="https://joinfediverse.kr" target="_blank" rel="noreferrer noopener">둘러보기</a></main>
</body></html>
"""


class CommunityNavigationParserFixtureTest(unittest.TestCase):
    """Fixture-only checks; these are not a substitute for a live PASS."""

    def test_primary_nav_ignores_people_text_in_ordinary_main_content(self):
        document = checker.analyze_html(HOME_OK)
        self.assertEqual(document["primary_nav_labels"], ["연합우주 이해하기", "서버 찾기", "커뮤니티", "서버 운영", "커뮤니티", "서버 운영"])
        self.assertIn("사람 찾기 안내 문서", document["main_text"])

    def test_void_element_inside_link_does_not_merge_following_nav_labels(self):
        document = checker.analyze_html(
            '<nav aria-label="주요 메뉴"><a href="/community"><img src="mark.svg">커뮤니티</a><a href="/servers">서버 찾기</a></nav>'
        )
        self.assertEqual(document["primary_nav_labels"], ["커뮤니티", "서버 찾기"])

    def test_home_evaluation_requires_two_community_primary_nav_links(self):
        report = checker.evaluate_pages({"/": checker.Page(200, HOME_OK)})
        failures = [item["id"] for item in report["assertions"] if not item["passed"]]
        self.assertNotIn("home.primary-nav-community", failures)
        self.assertNotIn("home.primary-nav-people-absent", failures)

    def test_home_rejects_external_community_link_or_hero_anchor(self):
        unsafe = HOME_OK.replace("<main id=\"content\">", '<main id="content"><a href="#community-resources">커뮤니티</a><a href="https://fedidev.kr">외부</a>')
        report = checker.evaluate_pages({"/": checker.Page(200, unsafe)})
        failures = {item["id"] for item in report["assertions"] if not item["passed"]}
        self.assertIn("home.no-old-community-hero", failures)
        self.assertIn("home.no-external-community-content", failures)

    def test_every_community_nav_label_requires_exact_destination(self):
        for path, html in [("/", HOME_OK), ("/community", COMMUNITY_OK)]:
            count = html.count('href="/community"')
            for index in range(count):
                for href in ["/people", "/community/", "#community", ""]:
                    with self.subTest(path=path, index=index, href=href):
                        parts = html.split('href="/community"')
                        mutated = 'href="/community"'.join(parts[:index + 1]) + f'href="{href}"' + 'href="/community"'.join(parts[index + 1:])
                        report = checker.evaluate_pages({path: checker.Page(200, mutated)})
                        self.assertGreater(report["failed_assertions"], 0)

    def test_community_accepts_safe_external_links_and_active_primary_nav(self):
        report = checker.evaluate_pages({"/community": checker.Page(200, COMMUNITY_OK)})
        failures = [item["id"] for item in report["assertions"] if not item["passed"]]
        self.assertEqual(failures, [])

    def test_home_rejects_leftover_community_section_without_old_links(self):
        html = HOME_OK.replace('</main>', '<section id="community-resources"><h2>남겨진 섹션</h2></section></main>')
        report = checker.evaluate_pages({"/": checker.Page(200, html)})
        failures = {item["id"] for item in report["assertions"] if not item["passed"]}
        self.assertIn("home.no-old-community-section", failures)

    def test_community_rejects_missing_or_unsafe_external_link(self):
        unsafe = COMMUNITY_OK.replace(' rel="noopener noreferrer"', ' rel="noopener"', 1).replace('https://joinfediverse.kr', 'https://example.invalid')
        report = checker.evaluate_pages({"/community": checker.Page(200, unsafe)})
        failures = {item["id"] for item in report["assertions"] if not item["passed"]}
        self.assertIn("community.fedidev-safe-external-link", failures)
        self.assertIn("community.joinfediverse-safe-external-link", failures)

    def test_transport_error_is_distinct_from_assertion_failure(self):
        report = checker.evaluate_pages({"/": checker.Page(None, "", "timeout")})
        self.assertEqual(report["transport_errors"], [{"path": "/", "error": "timeout"}])
        self.assertEqual(report["failed_assertions"], 0)


if __name__ == "__main__":
    unittest.main()
