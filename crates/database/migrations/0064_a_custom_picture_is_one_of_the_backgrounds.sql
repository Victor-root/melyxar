-- The picture behind the sign in screen becomes one of the backgrounds to
-- choose from rather than something standing over whichever was chosen. A
-- server that has one keeps showing it: it is now its chosen background.
UPDATE server_settings SET login_background_style = 'picture'
 WHERE login_background_path IS NOT NULL;
