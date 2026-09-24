-- The identifier a browser gave itself once and for all, sent when it signs
-- in. A new sign in of the same account from the same browser takes the
-- place of the session it had there, rather than leaving one behind that
-- nobody holds any more. Absent for a session opened before this, and for a
-- browser that could keep nothing.

ALTER TABLE devices ADD COLUMN client_id TEXT;

CREATE INDEX devices_by_user_and_client ON devices (user_id, client_id);
