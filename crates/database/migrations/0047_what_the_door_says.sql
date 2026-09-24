-- The line the sign in screen says under the server's name, as the
-- administrator wrote it. Nothing for the one Melyxar says, in the language
-- of whoever is looking.
ALTER TABLE server_settings ADD COLUMN door_slogan TEXT;
