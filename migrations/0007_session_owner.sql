-- Previous conversations cannot be assigned to an API key retroactively.
-- They remain visible in the desktop history but cannot be resumed by a key.
ALTER TABLE conversations ADD COLUMN owner_key_id TEXT NOT NULL DEFAULT '';
