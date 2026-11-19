-- Repo lifecycle for URL-registered (cloned) repositories.
-- status: cloning | ready | failed
ALTER TABLE repositories ADD COLUMN status TEXT NOT NULL DEFAULT 'ready';
ALTER TABLE repositories ADD COLUMN error TEXT;
