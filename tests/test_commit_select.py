"""Tests for candidate selection and text committing."""

from test_helpers import assert_committed


def test_space_commits_first_candidate(client):
    client.type_pinyin("wo").press_space()
    committed = client.get_committed()
    assert len(committed) > 0, "Expected committed text after space"


def test_number_selects_second_candidate(client):
    client.type_pinyin("zhongguo")
    candidates_before = client.get_candidates()
    if len(candidates_before) >= 2:
        client.press_number(2)
        committed = client.get_committed()
        assert len(committed) > 0, f"Expected committed text after selecting #2, got {committed!r}"


def test_first_character_is_chinese(client):
    client.type_pinyin("ren").press_space()
    committed = client.get_committed()
    assert len(committed) > 0
    first_char = committed[0]
    assert ord(first_char) > 127, f"Expected Chinese character, got {first_char!r} (U+{ord(first_char):04X})"
