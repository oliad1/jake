CREATE TABLE cities (
  id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY, 
  display_name TEXT NOT NULL, -- city name
  region TEXT NOT NULL, -- state/province
  country CHAR(2) NOT NULL, -- CA
  UNIQUE (display_name, region, country)
);

CREATE TABLE application_cities (
  application_id BIGINT NOT NULL,
  city_id BIGINT NOT NULL,
  PRIMARY KEY (application_id, city_id)
);

CREATE INDEX idx_application_cities_city_id ON application_cities(city_id);

ALTER TABLE applications
DROP COLUMN location;
