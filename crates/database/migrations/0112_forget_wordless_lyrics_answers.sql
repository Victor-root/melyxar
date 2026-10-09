-- The answers kept as "a song without words": until the choice of an entry
-- put words before no words, a song LRCLIB holds both with and without words
-- could be kept as one without. Asked again, those songs are told apart.
DELETE FROM music_lyrics
 WHERE plain IS NULL AND synced IS NULL AND instrumental = 1;
