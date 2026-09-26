-- Only a work with somewhere to carry on from keeps a position: one watched to
-- the point the rules call watched, or never really started, lets go of it,
-- so putting it back to unwatched later starts it from the beginning rather
-- than from the credits.
UPDATE playback_progress SET position_ms = 0
 WHERE (state = 'watched' AND marked_manually = 0) OR state = 'not_started';

-- What is carried on is what has a position, watched or not, read straight
-- off this index in the order it was played.
CREATE INDEX playback_progress_to_carry_on
    ON playback_progress (user_id, last_played_at DESC) WHERE position_ms > 0;
