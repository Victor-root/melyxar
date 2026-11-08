-- An account holds either a colour it chose or the server's. Those still on
-- the red every account was given follow the server from now on, which shows
-- them the same red for as long as the server keeps it.
UPDATE user_preferences SET accent_color = 'server' WHERE lower(accent_color) = '#c81e1e';
