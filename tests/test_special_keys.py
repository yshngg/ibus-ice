"""Tests for special key handling: backspace, escape, enter, apostrophe."""

from test_helpers import assert_candidates, assert_preedit


def test_backspace_removes_characters(client):
    client.type_pinyin("zhongguo")
    assert_preedit(client, "zhongguo")
    client.press_backspace()
    assert_preedit(client, "zhonggu")


def test_backspace_to_empty(client):
    client.type_pinyin("a")
    assert_preedit(client, "a")
    client.press_backspace()
    assert_preedit(client, "")


def test_escape_resets_state(client):
    client.type_pinyin("zhongguo")
    assert len(client.get_candidates()) > 0
    client.press_escape()
    candidates = client.get_candidates()
    assert len(candidates) == 0, "Expected no candidates after escape"
    assert_preedit(client, "")


def test_enter_commits_raw_pinyin(client):
    client.type_pinyin("hello").press_enter()
    committed = client.get_committed()
    assert "hello" in committed.lower(), f"Expected raw pinyin in committed: {committed!r}"


def test_apostrophe_separator(client):
    client.type_pinyin("xi")
    client.press_apostrophe()
    client.type_pinyin("an")
    assert_preedit(client, "xi'an")
    candidates = client.get_candidates()
    assert len(candidates) > 0, "Expected candidates for xi'an"
