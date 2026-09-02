-- Migration number: 0005 2026-09-02T00:00:00.000Z

-- NULL message id retains the old timestamp-only meaning: all rows at that
-- timestamp are processed.
ALTER TABLE channel ADD COLUMN last_summarized_message_id TEXT;
ALTER TABLE channel ADD COLUMN pending_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE channel ADD COLUMN first_pending_at TEXT;
ALTER TABLE channel ADD COLUMN first_pending_message_id TEXT;
ALTER TABLE channel ADD COLUMN last_pending_at TEXT;
ALTER TABLE channel ADD COLUMN last_pending_message_id TEXT;
ALTER TABLE message ADD COLUMN summary_pending INTEGER NOT NULL DEFAULT 0;

CREATE INDEX idx_message_channel_id_timestamp_message_id
    ON message (channel_id, timestamp, message_id);
CREATE INDEX idx_message_channel_summary_pending_timestamp_message_id
    ON message (channel_id, summary_pending, timestamp, message_id);

-- Existing rows use the legacy timestamp-only cursor for their initial state.
UPDATE message AS m
SET summary_pending = 1
WHERE EXISTS (
    SELECT 1 FROM channel AS c
    WHERE c.channel_id = m.channel_id AND (
        c.last_summarized_at IS NULL OR m.timestamp > c.last_summarized_at OR
        (c.last_summarized_message_id IS NOT NULL AND m.timestamp = c.last_summarized_at
         AND m.message_id > c.last_summarized_message_id)
    )
);

UPDATE channel AS c
SET pending_count = (
        SELECT COUNT(*) FROM message AS m
        WHERE m.channel_id = c.channel_id AND m.summary_pending = 1
    ),
    (first_pending_at, first_pending_message_id) = (
        SELECT m.timestamp, m.message_id FROM message AS m
        WHERE m.channel_id = c.channel_id AND m.summary_pending = 1
        ORDER BY m.timestamp, m.message_id LIMIT 1
    ),
    (last_pending_at, last_pending_message_id) = (
        SELECT m.timestamp, m.message_id FROM message AS m
        WHERE m.channel_id = c.channel_id AND m.summary_pending = 1
        ORDER BY m.timestamp DESC, m.message_id DESC LIMIT 1
    );

-- Every row first seen after migration is pending, even when delayed delivery
-- gives it a timestamp behind the established cursor. Duplicate message_id
-- upserts update content only and do not fire this INSERT trigger.
CREATE TRIGGER message_summary_state_after_insert AFTER INSERT ON message
BEGIN
    UPDATE message SET summary_pending = 1 WHERE message_id = NEW.message_id;
    UPDATE channel SET
        pending_count = pending_count + 1,
        first_pending_at = CASE WHEN first_pending_at IS NULL OR NEW.timestamp < first_pending_at
            OR (NEW.timestamp = first_pending_at AND NEW.message_id < first_pending_message_id)
            THEN NEW.timestamp ELSE first_pending_at END,
        first_pending_message_id = CASE WHEN first_pending_at IS NULL OR NEW.timestamp < first_pending_at
            OR (NEW.timestamp = first_pending_at AND NEW.message_id < first_pending_message_id)
            THEN NEW.message_id ELSE first_pending_message_id END,
        last_pending_at = CASE WHEN last_pending_at IS NULL OR NEW.timestamp > last_pending_at
            OR (NEW.timestamp = last_pending_at AND NEW.message_id > last_pending_message_id)
            THEN NEW.timestamp ELSE last_pending_at END,
        last_pending_message_id = CASE WHEN last_pending_at IS NULL OR NEW.timestamp > last_pending_at
            OR (NEW.timestamp = last_pending_at AND NEW.message_id > last_pending_message_id)
            THEN NEW.message_id ELSE last_pending_message_id END
    WHERE channel_id = NEW.channel_id;
END;

CREATE TRIGGER message_summary_state_after_delete AFTER DELETE ON message
WHEN OLD.summary_pending = 1
BEGIN
    UPDATE channel SET
        pending_count = MAX(0, pending_count - 1),
        (first_pending_at, first_pending_message_id) = (
            SELECT m.timestamp, m.message_id FROM message AS m
            WHERE m.channel_id = OLD.channel_id AND m.summary_pending = 1
            ORDER BY m.timestamp, m.message_id LIMIT 1
        ),
        (last_pending_at, last_pending_message_id) = (
            SELECT m.timestamp, m.message_id FROM message AS m
            WHERE m.channel_id = OLD.channel_id AND m.summary_pending = 1
            ORDER BY m.timestamp DESC, m.message_id DESC LIMIT 1
        )
    WHERE channel_id = OLD.channel_id;
END;

-- Progress confirms only IDs actually returned to the app.
CREATE TRIGGER message_summary_state_after_confirm
AFTER UPDATE OF summary_pending ON message
WHEN OLD.summary_pending = 1 AND NEW.summary_pending = 0
BEGIN
    UPDATE channel SET
        pending_count = MAX(0, pending_count - 1),
        (first_pending_at, first_pending_message_id) = (
            SELECT m.timestamp, m.message_id FROM message AS m
            WHERE m.channel_id = OLD.channel_id AND m.summary_pending = 1
            ORDER BY m.timestamp, m.message_id LIMIT 1
        ),
        (last_pending_at, last_pending_message_id) = (
            SELECT m.timestamp, m.message_id FROM message AS m
            WHERE m.channel_id = OLD.channel_id AND m.summary_pending = 1
            ORDER BY m.timestamp DESC, m.message_id DESC LIMIT 1
        )
    WHERE channel_id = OLD.channel_id;
END;

-- Covers both the new pair update and an old Worker updating timestamp alone.
CREATE TRIGGER channel_summary_state_after_progress
AFTER UPDATE OF last_summarized_at, last_summarized_message_id ON channel
WHEN OLD.last_summarized_at IS NOT NEW.last_summarized_at
  OR OLD.last_summarized_message_id IS NOT NEW.last_summarized_message_id
BEGIN
    UPDATE channel SET
        pending_count = (
            SELECT COUNT(*) FROM message AS m
            WHERE m.channel_id = NEW.channel_id AND m.summary_pending = 1
        ),
        (first_pending_at, first_pending_message_id) = (
            SELECT m.timestamp, m.message_id FROM message AS m
            WHERE m.channel_id = NEW.channel_id AND m.summary_pending = 1
            ORDER BY m.timestamp, m.message_id LIMIT 1
        ),
        (last_pending_at, last_pending_message_id) = (
            SELECT m.timestamp, m.message_id FROM message AS m
            WHERE m.channel_id = NEW.channel_id AND m.summary_pending = 1
            ORDER BY m.timestamp DESC, m.message_id DESC LIMIT 1
        )
    WHERE channel_id = NEW.channel_id;
END;
