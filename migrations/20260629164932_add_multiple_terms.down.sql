DROP TABLE application_terms;

-- Another crazy migration
ALTER TABLE applications
ADD term_id BIGINT NOT NULL DEFAULT 0;
