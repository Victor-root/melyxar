-- The answers kept when LRCLIB knew nothing of a song: asked for by artist,
-- title, album and length only, a song it holds under another album was
-- answered "unknown" and never asked again. Searched for by artist and title
-- as well from now on, those songs are asked again.
DELETE FROM music_lyrics
 WHERE plain IS NULL AND synced IS NULL AND instrumental = 0;
