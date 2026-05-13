"""Tests for English word input path."""

from test_helpers import assert_candidates


def test_english_word_has_candidates(client):
    client.type_pinyin("hello")
    candidates = client.get_candidates()
    assert len(candidates) > 0, "Expected candidates for English word 'hello'"


def test_english_word_can_be_committed(client):
    client.type_pinyin("hello").press_space()
    committed = client.get_committed()
    assert len(committed) > 0, f"Expected committed text for 'hello', got {committed!r}"


def test_mixed_input_fallbacks(client):
    client.type_pinyin("apple")
    candidates = client.get_candidates()
    assert len(candidates) > 0, "Expected candidates for 'apple'"
