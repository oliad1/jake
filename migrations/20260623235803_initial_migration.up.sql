CREATE TABLE terms (
  id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  display_name TEXT NOT NULL UNIQUE,
  state TEXT NOT NULL DEFAULT 'ACTIVE', -- ACTIVE, INACTIVE
  create_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT term_state_check CHECK (state IN ('ACTIVE', 'INACTIVE'))
);

CREATE TABLE companies (
  id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  display_name TEXT NOT NULL,
  url TEXT NOT NULL UNIQUE,
  hex_code TEXT NOT NULL,
  icon_url TEXT NOT NULL,
  state TEXT NOT NULL DEFAULT 'ACTIVE', -- ACTIVE, INACTIVE
  create_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT company_state_check CHECK (state IN ('ACTIVE', 'INACTIVE'))
);

CREATE TABLE applications (
  id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  term_id BIGINT NOT NULL,
  company_id BIGINT NOT NULL,
  job_title TEXT NOT NULL,
  location TEXT NOT NULL, -- Should be City, State format (i.e. Palo Alto, CA)
  url TEXT NOT NULL UNIQUE,
  page_content TEXT NOT NULL,
  lower_wage_cents SMALLINT NOT NULL,
  upper_wage_cents SMALLINT NULL, -- There could be no range
  state TEXT NOT NULL DEFAULT 'ACTIVE', -- ACTIVE, SUBMITTED, REJECTED, DELETED, IGNORED
  create_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT application_state_check CHECK (state IN ('ACTIVE', 'SUBMITTED', 'REJECTED', 'DELETED', 'IGNORED'))
);

CREATE TABLE application_events (
  id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  application_id BIGINT NOT NULL,
  before_state TEXT NULL,
  after_state TEXT NOT NULL,
  create_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT application_event_state_check
  CHECK (
    after_state IN ('ACTIVE', 'SUBMITTED', 'REJECTED', 'DELETED', 'IGNORED') AND  -- Valid after state and..
    (
      before_state IS NULL OR							  -- Before state is null or valid
      before_state IN ('ACTIVE', 'SUBMITTED', 'REJECTED', 'DELETED', 'IGNORED')
    )
  )
);

-- Insert Fall 26 term
INSERT INTO terms (display_name) VALUES ('F26');
