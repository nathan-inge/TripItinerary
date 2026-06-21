-- Trip-level metadata: external album / map links and a checklist.
-- todos_json stores a JSON array of {id, text, done, sort_order}.

ALTER TABLE trips ADD COLUMN photos_url TEXT;
ALTER TABLE trips ADD COLUMN maps_url   TEXT;
ALTER TABLE trips ADD COLUMN todos_json TEXT NOT NULL DEFAULT '[]';
