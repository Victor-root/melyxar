-- Why the analyser could not describe a file.
--
-- A file nobody could read sits in the library with a card of its own and
-- fails the moment somebody presses play. Its name was already reported; the
-- reason lived in a log line nobody keeps, so the only way to learn it was to
-- be watching a scan go by in a terminal.
--
-- Cleared the moment the file is described, and the moment it changes on disk:
-- a reason outliving what caused it sends whoever reads it the wrong way.
ALTER TABLE media_sources ADD COLUMN analysis_failure TEXT;
