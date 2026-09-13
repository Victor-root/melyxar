-- Why the last look up did not name a work.
--
-- no_match, provider_unreachable, provider_busy or provider_unreadable. Null
-- for a work nobody has looked up yet, and cleared the moment a provider names
-- it: a reason that outlives its cause is a lie on a screen.
ALTER TABLE works ADD COLUMN identification_note TEXT;
