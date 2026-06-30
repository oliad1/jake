CREATE TABLE application_terms (
  application_id BIGINT NOT NULL,
  term_id BIGINT NOT NULL,
  PRIMARY KEY (application_id, term_id)
);

CREATE INDEX idx_application_terms_term_id ON application_terms(term_id);

ALTER TABLE applications
DROP COLUMN term_id;
