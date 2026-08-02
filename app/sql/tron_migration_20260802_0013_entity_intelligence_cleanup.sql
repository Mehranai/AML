ALTER TABLE tron_db.address_relationships
    DROP INDEX IF EXISTS idx_from;

ALTER TABLE tron_db.address_relationships
    DROP INDEX IF EXISTS idx_to;
