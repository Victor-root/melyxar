-- The wave now rises to three quarters until it is turned up or down. Accounts
-- that still stand at the old starting point, which no one could have chosen
-- before the slider existed, move to the new one.
UPDATE music_preferences SET spectrum_amplitude = 75 WHERE spectrum_amplitude = 100;
