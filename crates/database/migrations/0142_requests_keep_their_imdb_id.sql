-- The title a request is for, as IMDb names it, so the administrator can
-- open its page there as well as at the provider. Known when the request is
-- made; a request made before is given it the first time it is read for
-- deciding.
ALTER TABLE title_requests ADD COLUMN imdb_id TEXT;
