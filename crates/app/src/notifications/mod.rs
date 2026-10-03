//! Everything the server tells somebody: what deserves an administrator's
//! look today, and what is to come (notifications for every account, with
//! their history). Nothing outside this module knows how a notification is
//! made, kept or shown.

pub mod attention;
