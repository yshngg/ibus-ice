"""Tests for edge cases: empty input, rapid typing, etc."""

from test_helpers import assert_preedit


def test_rapid_typing_does_not_crash(client):
    client.type_pinyin("zhongguo")
    client.press_backspace()
    client.type_pinyin("ren")
    candidates = client.get_candidates()
    assert len(candidates) > 0, "Expected candidates after rapid typing"


def test_long_input_does_not_crash(client):
    long_pinyin = "zhongguorenmindaxue"
    client.type_pinyin(long_pinyin)
    candidates = client.get_candidates()
    assert isinstance(candidates, list), "Expected list from get_candidates()"


def test_uppercase_input_lowered(client):
    """Uppercase letters are lowered when typed via type_pinyin."""
    client.type_pinyin("WO")
    assert_preedit(client, "wo")


def test_control_keys_ignored(client):
    client.type_pinyin("zhong")
    before = client.get_preedit()
    client.press_key(ord('a'), 1 << 2)  # Control mask
    assert_preedit(client, before)


def test_single_char_has_candidates(client):
    client.type_pinyin("w")
    candidates = client.get_candidates()
    assert len(candidates) > 0, "Expected candidates for single char 'w'"
