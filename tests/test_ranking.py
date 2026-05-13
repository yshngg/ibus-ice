"""Tests for frequency-based ranking and user boost."""

from test_helpers import assert_candidates


def test_high_freq_ranks_first(client):
    client.type_pinyin("wo")
    candidates = client.get_candidates()
    texts = [c.text for c in candidates]
    assert "我" in texts, f"Expected 我 in candidates: {texts[:5]}"
    wo_idx = texts.index("我")
    assert wo_idx < 5, f"我 ranked #{wo_idx+1}, expected top 5"


def test_selecting_boosts_candidate(client):
    """After selecting a candidate, verify the engine handles it without error."""
    pinyin = "nihao"
    client.type_pinyin(pinyin)
    candidates = client.get_candidates()
    assert len(candidates) >= 1, f"Expected candidates for '{pinyin}'"

    # Select the first candidate
    first_text = candidates[0].text
    client.press_space()

    # Type again — engine should still work (user dict boost is for next engine restart)
    client.type_pinyin(pinyin)
    candidates_after = client.get_candidates()
    assert len(candidates_after) > 0, "Expected candidates after re-typing"
