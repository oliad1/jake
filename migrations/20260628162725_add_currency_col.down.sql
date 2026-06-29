ALTER TABLE applications
DROP COLUMN currency;

ALTER TABLE applications
DROP CONSTRAINT application_concurrency_check;
