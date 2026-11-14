-- Keep ON DELETE SET NULL lookups proportional to actual dependents.
CREATE INDEX idx_memories_supersedes ON memories (supersedes_id);
CREATE INDEX idx_candidates_accepted_memory ON memory_candidates (accepted_memory_id);
