ALTER TABLE queued_items ADD COLUMN claimed_turn_id TEXT;

CREATE UNIQUE INDEX queued_items_claimed_turn_idx
    ON queued_items(thread_id, claimed_turn_id)
    WHERE claimed_turn_id IS NOT NULL;
