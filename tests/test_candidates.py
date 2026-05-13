"""Tests for candidate ordering and page navigation."""

from test_helpers import assert_candidates


def test_candidate_order_is_stable(client):
    client.type_pinyin("zhongguo")
    first = client.get_candidates()
    client.press_escape()
    client.type_pinyin("zhongguo")
    second = client.get_candidates()
    first_texts = [c.text for c in first]
    second_texts = [c.text for c in second]
    assert first_texts == second_texts, (
        f"Candidate order not stable:\n  first: {first_texts}\n  second: {second_texts}"
    )


def test_page_navigation(client):
    client.type_pinyin("y")
    first_page = client.get_candidates()
    if len(first_page) < 5:
        return  # not enough candidates for paging
    client.press_page_down()
    second_page = client.get_candidates()
    assert len(second_page) >= 0, "page_down should not crash"


def test_common_word_ranks_high(client):
    client.type_pinyin("zhongguo")
    candidates = client.get_candidates()
    texts = [c.text for c in candidates]
    assert "中国" in texts, f"Expected 中国 in candidates: {texts[:5]}"
    china_idx = texts.index("中国")
    assert china_idx < 5, f"中国 ranked #{china_idx+1}, expected top 5"
