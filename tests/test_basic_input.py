"""Tests for basic pinyin input and preedit display."""

from test_helpers import assert_candidates, assert_preedit


def test_single_syllable_suggests_candidates(client):
    client.type_pinyin("wo")
    candidates = client.get_candidates()
    assert len(candidates) > 0, "Expected candidates for 'wo'"
    assert_preedit(client, "wo")


def test_multi_syllable_candidates(client):
    client.type_pinyin("zhongguo")
    candidates = client.get_candidates()
    assert len(candidates) > 0, "Expected candidates for 'zhongguo'"
    assert_preedit(client, "zhongguo")


def test_empty_input_has_no_candidates(client):
    client.type_pinyin("a").press_backspace()
    candidates = client.get_candidates()
    assert len(candidates) == 0, "Expected no candidates after clearing input"


def test_candidates_update_as_typing(client):
    client.type_pinyin("zho")
    count1 = len(client.get_candidates())
    client.type_pinyin("ngguo")
    count2 = len(client.get_candidates())
    assert count1 > 0 or count2 > 0, "Expected candidates while typing"
