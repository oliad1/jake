ALTER TABLE applications
ADD currency TEXT NOT NULL DEFAULT 'USD';

ALTER TABLE applications
ADD CONSTRAINT application_concurrency_check CHECK (currency IN ('USD', 'CAD'));
