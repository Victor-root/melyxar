//! The cameras of a library of cameras: what the administration declares of
//! each one, its password kept sealed as it was handed in.

use melyxar_core::id::{CameraId, LibraryId};
use sqlx::Row;

use crate::convert::{parse_id, timestamp_to_text};
use crate::{Database, DatabaseError, Result};

/// One camera, as kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Camera {
    pub id: CameraId,
    pub library_id: LibraryId,
    pub name: String,
    /// The RTSP address, without the account it is opened with.
    pub address: String,
    pub username: String,
    /// The password sealed with the server's key, when there is one.
    pub sealed_password: Option<Vec<u8>>,
}

/// What the administration says of a camera, its password apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CameraFields<'a> {
    pub name: &'a str,
    pub address: &'a str,
    pub username: &'a str,
}

const WHAT_A_CAMERA_IS: &str = "id, library_id, name, address, username, sealed_password";

fn camera_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Camera> {
    Ok(Camera {
        id: parse_id(&row.try_get::<String, _>("id")?)?,
        library_id: parse_id(&row.try_get::<String, _>("library_id")?)?,
        name: row.try_get("name")?,
        address: row.try_get("address")?,
        username: row.try_get("username")?,
        sealed_password: row.try_get("sealed_password")?,
    })
}

impl Database {
    /// Declares a camera in a library, and answers it as kept.
    pub async fn add_camera(
        &self,
        library: LibraryId,
        fields: CameraFields<'_>,
        sealed_password: Option<&[u8]>,
    ) -> Result<Camera> {
        let id = CameraId::new();
        sqlx::query(
            "INSERT INTO cameras (id, library_id, name, address, username, sealed_password, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id.to_db_string())
        .bind(library.to_db_string())
        .bind(fields.name)
        .bind(fields.address)
        .bind(fields.username)
        .bind(sealed_password)
        .bind(timestamp_to_text(melyxar_core::time::now()))
        .execute(self.writer())
        .await?;
        self.camera(id)
            .await?
            .ok_or_else(|| DatabaseError::Corrupt("a camera just written is missing".to_string()))
    }

    pub async fn camera(&self, id: CameraId) -> Result<Option<Camera>> {
        let row = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT {WHAT_A_CAMERA_IS} FROM cameras WHERE id = ?"
        )))
        .bind(id.to_db_string())
        .fetch_optional(self.writer())
        .await?;
        row.as_ref().map(camera_from_row).transpose()
    }

    /// The cameras of a library, by name.
    pub async fn cameras_of(&self, library: LibraryId) -> Result<Vec<Camera>> {
        let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT {WHAT_A_CAMERA_IS} FROM cameras WHERE library_id = ? ORDER BY name, id"
        )))
        .bind(library.to_db_string())
        .fetch_all(self.reader())
        .await?;
        rows.iter().map(camera_from_row).collect()
    }

    pub async fn count_cameras(&self, library: LibraryId) -> Result<i64> {
        Ok(
            sqlx::query_scalar("SELECT COUNT(*) FROM cameras WHERE library_id = ?")
                .bind(library.to_db_string())
                .fetch_one(self.reader())
                .await?,
        )
    }

    /// Says something else of a camera, its password left as it is. Answers
    /// whether there was such a camera.
    pub async fn update_camera(&self, id: CameraId, fields: CameraFields<'_>) -> Result<bool> {
        let done =
            sqlx::query("UPDATE cameras SET name = ?, address = ?, username = ? WHERE id = ?")
                .bind(fields.name)
                .bind(fields.address)
                .bind(fields.username)
                .bind(id.to_db_string())
                .execute(self.writer())
                .await?;
        Ok(done.rows_affected() > 0)
    }

    /// Gives a camera another password, sealed, or none.
    pub async fn set_camera_password(&self, id: CameraId, sealed: Option<&[u8]>) -> Result<bool> {
        let done = sqlx::query("UPDATE cameras SET sealed_password = ? WHERE id = ?")
            .bind(sealed)
            .bind(id.to_db_string())
            .execute(self.writer())
            .await?;
        Ok(done.rows_affected() > 0)
    }

    /// Takes a camera away. Answers whether there was such a camera.
    pub async fn remove_camera(&self, id: CameraId) -> Result<bool> {
        let done = sqlx::query("DELETE FROM cameras WHERE id = ?")
            .bind(id.to_db_string())
            .execute(self.writer())
            .await?;
        Ok(done.rows_affected() > 0)
    }
}

#[cfg(test)]
mod tests {
    use melyxar_core::library::LibraryKind;

    use super::*;

    const DOOR: CameraFields<'static> = CameraFields {
        name: "Front door",
        address: "rtsp://192.168.1.20:554/stream1",
        username: "viewer",
    };

    async fn a_library_of_cameras() -> (Database, LibraryId) {
        let database = Database::open_in_memory().await.expect("database opens");
        let library = database
            .create_library("Cameras", LibraryKind::Cameras, "en", &[])
            .await
            .expect("library created");
        (database, library.id)
    }

    #[tokio::test]
    async fn a_camera_is_kept_as_declared_and_listed_by_name() {
        let (database, library) = a_library_of_cameras().await;
        let door = database.add_camera(library, DOOR, Some(b"sealed")).await.expect("added");
        database
            .add_camera(
                library,
                CameraFields { name: "Back garden", ..DOOR },
                None,
            )
            .await
            .expect("added");

        assert_eq!(door.name, "Front door");
        assert_eq!(door.address, "rtsp://192.168.1.20:554/stream1");
        assert_eq!(door.username, "viewer");
        assert_eq!(door.sealed_password.as_deref(), Some(&b"sealed"[..]));
        let names: Vec<String> = database
            .cameras_of(library)
            .await
            .expect("listed")
            .into_iter()
            .map(|camera| camera.name)
            .collect();
        assert_eq!(names, ["Back garden", "Front door"]);
        assert_eq!(database.count_cameras(library).await.expect("counted"), 2);
    }

    #[tokio::test]
    async fn a_camera_changed_keeps_its_password_until_it_is_given_another() {
        let (database, library) = a_library_of_cameras().await;
        let door = database.add_camera(library, DOOR, Some(b"first")).await.expect("added");

        let moved = CameraFields { address: "rtsp://192.168.1.21/live", ..DOOR };
        assert!(database.update_camera(door.id, moved).await.expect("updated"));
        let kept = database.camera(door.id).await.expect("read").expect("there");
        assert_eq!(kept.address, "rtsp://192.168.1.21/live");
        assert_eq!(kept.sealed_password.as_deref(), Some(&b"first"[..]));

        assert!(database.set_camera_password(door.id, None).await.expect("cleared"));
        let kept = database.camera(door.id).await.expect("read").expect("there");
        assert_eq!(kept.sealed_password, None);

        assert!(!database.update_camera(CameraId::new(), DOOR).await.expect("nothing"));
    }

    #[tokio::test]
    async fn a_camera_goes_alone_or_with_its_library() {
        let (database, library) = a_library_of_cameras().await;
        let door = database.add_camera(library, DOOR, None).await.expect("added");
        let garden = database
            .add_camera(library, CameraFields { name: "Garden", ..DOOR }, None)
            .await
            .expect("added");

        assert!(database.remove_camera(door.id).await.expect("removed"));
        assert!(!database.remove_camera(door.id).await.expect("already gone"));
        assert!(database.camera(garden.id).await.expect("read").is_some());

        database.delete_library(library).await.expect("library removed");
        assert!(database.camera(garden.id).await.expect("read").is_none());
    }
}
