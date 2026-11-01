-- Stable, explicit search identities survive VACUUM and status transitions.
-- Keep this mapping for every memory, including inactive history. Never derive
-- the FTS identity from memories' implicit rowid (its primary key is TEXT).
CREATE TABLE memory_search_ids (
    search_id INTEGER PRIMARY KEY AUTOINCREMENT,
    memory_id TEXT NOT NULL UNIQUE REFERENCES memories(id)
        ON DELETE CASCADE ON UPDATE CASCADE
);
INSERT INTO memory_search_ids(memory_id) SELECT id FROM memories ORDER BY id;

DROP TRIGGER trg_memories_search_insert;
DROP TRIGGER trg_memories_search_delete;
DROP TRIGGER trg_memories_search_update;
DROP TABLE memory_search;

-- Retain owned content and the existing trigram/search columns. Both the core
-- secure_delete pragma and FTS secure-delete are needed to erase old postings.
-- Bundled SQLite supports this persistent setting (available since 3.42.0).
CREATE VIRTUAL TABLE memory_search USING fts5 (
    memory_id UNINDEXED,
    content,
    memory_key,
    tokenize = 'trigram'
);
INSERT INTO memory_search(memory_search, rank) VALUES('secure-delete', 1);
INSERT INTO memory_search(rowid, memory_id, content, memory_key)
SELECT s.search_id, m.id, m.content, IFNULL(m.memory_key, '')
FROM memories m JOIN memory_search_ids s ON s.memory_id = m.id
WHERE m.status = 'active';

CREATE TRIGGER trg_memories_search_insert
AFTER INSERT ON memories
BEGIN
    INSERT INTO memory_search_ids(memory_id) VALUES(NEW.id);
    INSERT INTO memory_search(rowid, memory_id, content, memory_key)
    SELECT search_id, NEW.id, NEW.content, IFNULL(NEW.memory_key, '')
    FROM memory_search_ids WHERE memory_id = NEW.id AND NEW.status = 'active';
END;

-- Cascade cleanup owns the old search_id, so no trigger ordering assumption
-- about when the parent row or its mapping is removed is needed.
CREATE TRIGGER trg_memory_search_ids_delete
AFTER DELETE ON memory_search_ids
BEGIN
    DELETE FROM memory_search WHERE rowid = OLD.search_id;
END;

CREATE TRIGGER trg_memories_search_update
AFTER UPDATE OF id, content, memory_key, status ON memories
WHEN (OLD.status = 'active' OR NEW.status = 'active')
 AND (OLD.id IS NOT NEW.id OR OLD.content IS NOT NEW.content
      OR IFNULL(OLD.memory_key, '') IS NOT IFNULL(NEW.memory_key, '')
      OR OLD.status IS NOT NEW.status)
BEGIN
    DELETE FROM memory_search
    WHERE rowid = (SELECT search_id FROM memory_search_ids WHERE memory_id = NEW.id);
    INSERT INTO memory_search(rowid, memory_id, content, memory_key)
    SELECT search_id, NEW.id, NEW.content, IFNULL(NEW.memory_key, '')
    FROM memory_search_ids WHERE memory_id = NEW.id AND NEW.status = 'active';
END;

INSERT INTO memory_search(memory_search) VALUES('integrity-check');
