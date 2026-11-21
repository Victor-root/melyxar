-- Whether the tag manager of a library of music may search the covers of
-- MusicBrainz and Deezer to choose one by eye. Off until an administrator
-- turns it on, like every source online.
ALTER TABLE music_library_options ADD COLUMN cover_search_online INTEGER NOT NULL DEFAULT 0;
