-- Which libraries an account may see, said out loud.
--
-- It used to be read from the absence of rows: an account with no row in the
-- table of granted libraries saw every library, which is the ordinary case and
-- what saves granting each new library to everybody.
--
-- It also meant that an account granted one library, whose library was later
-- taken away, quietly came to see every library on the server. A grant goes
-- with the library it names, so removing the last library removes the last
-- grant, and no grants meant everything. Nobody would ever have been told.
--
-- Now the two are different things. An account either sees every library, or
-- it sees the ones it was granted and nothing besides, including nothing at
-- all. Every account that exists today saw everything, which is what the
-- default says; the ones that were granted a library go on seeing only it.
ALTER TABLE users
    ADD COLUMN sees_every_library INTEGER NOT NULL DEFAULT 1;

UPDATE users
   SET sees_every_library = 0
 WHERE EXISTS (SELECT 1 FROM user_library_access WHERE user_id = users.id);
