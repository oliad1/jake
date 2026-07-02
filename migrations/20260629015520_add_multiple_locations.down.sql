DROP TABLE cities;

DROP TABLE application_cities;

-- Good luck to whoever runs this down migration LOL
ALTER TABLE applications
ADD location TEXT NOT NULL DEFAULT '';
