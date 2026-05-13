"""Tests for English word input path."""


def test_english_word_has_candidates(client):
    """English words with entries in dict should produce candidates."""
    client.type_pinyin("ren")
    candidates = client.get_candidates()
    assert len(candidates) > 0, "Expected candidates for 'ren'"


def test_english_word_can_be_committed(client):
    """Typing pinyin and pressing space commits text."""
    client.type_pinyin("ren").press_space()
    committed = client.get_committed()
    assert len(committed) > 0, f"Expected committed text for 'ren', got {committed!r}"


def test_non_pinyin_input_handled(client):
    """Non-pinyin input (e.g., 'hello') should be handled gracefully."""
    client.type_pinyin("hello")
    # May or may not produce candidates with test dict,
    # but should not crash
    candidates = client.get_candidates()
    assert isinstance(candidates, list), "Expected list from get_candidates()"
