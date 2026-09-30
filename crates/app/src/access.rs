//! How the server is reached: in the clear behind a proxy, or encrypted by
//! the server itself with a certificate it signs, one it is given, or one it
//! obtains.
//!
//! What is chosen is kept in the database and applied to the next connection
//! without a restart. Nothing here ever closes the way in: a certificate that
//! cannot be read leaves the server answering in the clear, and says why, and
//! a plain connection is always accepted on the same port, which is how a
//! proxy in front keeps working whatever is chosen. See the decision in
//! `docs/architecture/README.md`.

use std::path::Path;
use std::sync::{Arc, RwLock};

use melyxar_core::time::Timestamp;
use melyxar_database::settings::Access as Stored;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::ServerConfig;

use crate::{AppError, AppState, Result};

/// How long the certificate this server signs itself lasts. A browser warns
/// about it whatever its length, so it is made long enough never to need
/// making again.
const SELF_SIGNED_FOR: time::Duration = time::Duration::days(3650);

const SELF_SIGNED_CERTIFICATE: &str = "self-signed.crt";
const SELF_SIGNED_KEY: &str = "self-signed.key";

/// How the server is reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// In the clear only: a proxy in front, such as nginx, encrypts.
    #[default]
    Proxy,
    /// Encrypted with a certificate the server signs itself.
    SelfSigned,
    /// Encrypted with a certificate the administrator put on the server.
    Provided,
    /// Encrypted with a certificate obtained for a domain.
    Automatic,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Proxy => "proxy",
            Self::SelfSigned => "self_signed",
            Self::Provided => "provided",
            Self::Automatic => "automatic",
        }
    }

    pub fn from_word(word: &str) -> Option<Self> {
        match word {
            "proxy" => Some(Self::Proxy),
            "self_signed" => Some(Self::SelfSigned),
            "provided" => Some(Self::Provided),
            "automatic" => Some(Self::Automatic),
            _ => None,
        }
    }
}

/// Why a certificate could not be put to use, as a word the client words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    /// No certificate was found in the file, or the file could not be read.
    CertificateUnreadable,
    /// No private key was found in the file, or the file could not be read.
    KeyUnreadable,
    /// The key is not the one of the certificate.
    NotAPair,
    /// The certificate this server signs itself could not be made.
    NotMade,
    /// Not offered by this version yet.
    NotYetOffered,
}

impl Problem {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CertificateUnreadable => "certificate_unreadable",
            Self::KeyUnreadable => "key_unreadable",
            Self::NotAPair => "not_a_pair",
            Self::NotMade => "not_made",
            Self::NotYetOffered => "not_yet_offered",
        }
    }
}

/// What a certificate says of itself, for the administrator to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Certificate {
    /// The names it answers for.
    pub names: Vec<String>,
    /// Who vouches for it: the same as who it names when self-signed.
    pub issuer: String,
    pub not_before: Timestamp,
    pub not_after: Timestamp,
}

/// Where the server stands: what was chosen, and what came of it.
#[derive(Debug, Clone, Default)]
pub struct Status {
    pub mode: Mode,
    pub certificate_path: Option<String>,
    pub private_key_path: Option<String>,
    /// The certificate in use, when connections are being encrypted.
    pub certificate: Option<Certificate>,
    /// Why they are not, when they should be.
    pub problem: Option<Problem>,
}

/// What the listener asks at every connection.
#[derive(Default)]
pub struct Current {
    held: RwLock<Held>,
}

#[derive(Default)]
struct Held {
    status: Status,
    encryption: Option<Arc<ServerConfig>>,
}

impl Current {
    fn read(&self) -> std::sync::RwLockReadGuard<'_, Held> {
        self.held.read().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// What encrypts a connection, when the server encrypts.
pub fn encryption(state: &AppState) -> Option<Arc<ServerConfig>> {
    state.access().read().encryption.clone()
}

/// Whether a request that arrived in the clear is sent to the encrypted
/// address: only when the server encrypts, and not behind a proxy.
pub fn sends_plain_to_encrypted(state: &AppState) -> bool {
    let held = state.access().read();
    held.status.mode != Mode::Proxy && held.encryption.is_some()
}

/// Where the server stands.
pub fn status(state: &AppState) -> Status {
    state.access().read().status.clone()
}

/// Puts what the database holds to use. Called once as the server comes up,
/// and again whenever it is changed.
pub async fn apply(state: &AppState) -> Result<Status> {
    let stored = state.database().access().await?;
    let mode = Mode::from_word(&stored.mode).unwrap_or_default();
    let tls = state.config().directories.tls();
    let (certificate_path, private_key_path) = (stored.certificate_path, stored.private_key_path);
    let asked = (mode, certificate_path.clone(), private_key_path.clone());
    let loaded = tokio::task::spawn_blocking(move || prepared(asked.0, asked.1, asked.2, &tls))
        .await
        .map_err(|error| AppError::Directory(std::io::Error::other(error)))?;

    let (encryption, certificate, problem) = match loaded {
        Ok(Some((config, certificate))) => (Some(config), Some(certificate), None),
        Ok(None) => (None, None, None),
        Err(problem) => {
            tracing::warn!(mode = mode.as_str(), problem = problem.as_str(), "connections stay in the clear");
            (None, None, Some(problem))
        }
    };
    if encryption.is_some() {
        tracing::info!(mode = mode.as_str(), "connections are encrypted");
    }
    let status = Status { mode, certificate_path, private_key_path, certificate, problem };
    let mut held = state.access().held.write().unwrap_or_else(std::sync::PoisonError::into_inner);
    *held = Held { status: status.clone(), encryption };
    Ok(status)
}

/// What an administrator can be told to put right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// A path left empty, or one that is not absolute.
    PathNeeded,
    /// The certificate and key given cannot be used, for this reason.
    Unusable(Problem),
}

impl Refused {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PathNeeded => "path_needed",
            Self::Unusable(problem) => problem.as_str(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Trouble {
    #[error("refused")]
    Refused(Refused),
    #[error(transparent)]
    Failed(#[from] AppError),
}

/// Chooses how the server is reached. A certificate given is tried before it
/// is kept, so a pair that cannot be used is refused rather than saved.
pub async fn choose(
    state: &AppState,
    mode: Mode,
    certificate_path: Option<&str>,
    private_key_path: Option<&str>,
) -> std::result::Result<Status, Trouble> {
    if mode == Mode::Automatic {
        return Err(Trouble::Refused(Refused::Unusable(Problem::NotYetOffered)));
    }
    let absolute = |path: Option<&str>| {
        path.map(str::trim)
            .filter(|path| Path::new(path).is_absolute())
            .map(str::to_string)
    };
    let (certificate_path, private_key_path) = match mode {
        Mode::Provided => {
            let (Some(certificate), Some(key)) = (absolute(certificate_path), absolute(private_key_path)) else {
                return Err(Trouble::Refused(Refused::PathNeeded));
            };
            let tried = (certificate.clone(), key.clone());
            tokio::task::spawn_blocking(move || encryption_from(Path::new(&tried.0), Path::new(&tried.1)))
                .await
                .map_err(|error| AppError::Directory(std::io::Error::other(error)))?
                .map_err(|problem| Trouble::Refused(Refused::Unusable(problem)))?;
            (Some(certificate), Some(key))
        }
        // Kept for when the administrator comes back to it.
        _ => (absolute(certificate_path), absolute(private_key_path)),
    };
    state
        .database()
        .set_access(&Stored {
            mode: mode.as_str().to_string(),
            certificate_path,
            private_key_path,
        })
        .await
        .map_err(AppError::from)?;
    Ok(apply(state).await?)
}

type Prepared = Option<(Arc<ServerConfig>, Certificate)>;

/// What encrypts connections for this mode, if anything does.
fn prepared(
    mode: Mode,
    certificate_path: Option<String>,
    private_key_path: Option<String>,
    tls: &Path,
) -> std::result::Result<Prepared, Problem> {
    match mode {
        Mode::Proxy => Ok(None),
        Mode::Automatic => Err(Problem::NotYetOffered),
        Mode::Provided => {
            let (Some(certificate), Some(key)) = (certificate_path, private_key_path) else {
                return Err(Problem::CertificateUnreadable);
            };
            encryption_from(Path::new(&certificate), Path::new(&key)).map(Some)
        }
        Mode::SelfSigned => {
            let (certificate, key) = (tls.join(SELF_SIGNED_CERTIFICATE), tls.join(SELF_SIGNED_KEY));
            if !certificate.is_file() || !key.is_file() {
                sign_one(tls, &certificate, &key).map_err(|error| {
                    tracing::warn!(%error, "the certificate this server signs itself could not be made");
                    Problem::NotMade
                })?;
            }
            encryption_from(&certificate, &key).map(Some)
        }
    }
}

/// Makes a certificate for this machine, signed by itself.
fn sign_one(tls: &Path, certificate: &Path, key: &Path) -> std::io::Result<()> {
    let mut names = vec!["localhost".to_string(), "127.0.0.1".to_string()];
    if let Ok(host) = std::fs::read_to_string("/proc/sys/kernel/hostname") {
        let host = host.trim();
        if !host.is_empty() && host != "localhost" {
            names.push(host.to_string());
        }
    }
    let mut parameters = rcgen::CertificateParams::new(names).map_err(std::io::Error::other)?;
    let mut name = rcgen::DistinguishedName::new();
    name.push(rcgen::DnType::CommonName, "Melyxar");
    parameters.distinguished_name = name;
    let now = time::OffsetDateTime::now_utc();
    parameters.not_before = now - time::Duration::days(1);
    parameters.not_after = now + SELF_SIGNED_FOR;
    let pair = rcgen::KeyPair::generate().map_err(std::io::Error::other)?;
    let signed = parameters.self_signed(&pair).map_err(std::io::Error::other)?;

    std::fs::create_dir_all(tls)?;
    write_private(key, pair.serialize_pem().as_bytes())?;
    std::fs::write(certificate, signed.pem())?;
    tracing::info!(path = %certificate.display(), "a certificate signed by this server was made");
    Ok(())
}

/// Writes a key readable by the server's own account only.
fn write_private(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(contents)
}

/// Reads a certificate and its key, and checks they belong together.
pub fn encryption_from(
    certificate: &Path,
    key: &Path,
) -> std::result::Result<(Arc<ServerConfig>, Certificate), Problem> {
    let chain: Vec<CertificateDer<'static>> = CertificateDer::pem_file_iter(certificate)
        .map_err(|_| Problem::CertificateUnreadable)?
        .collect::<std::result::Result<_, _>>()
        .map_err(|_| Problem::CertificateUnreadable)?;
    let first = chain.first().ok_or(Problem::CertificateUnreadable)?;
    let about = described(first).ok_or(Problem::CertificateUnreadable)?;
    let key = PrivateKeyDer::from_pem_file(key).map_err(|_| Problem::KeyUnreadable)?;

    let mut config = ServerConfig::builder_with_provider(Arc::new(rustls::crypto::aws_lc_rs::default_provider()))
        .with_safe_default_protocol_versions()
        .map_err(|_| Problem::NotAPair)?
        .with_no_client_auth()
        .with_single_cert(chain, key)
        .map_err(|_| Problem::NotAPair)?;
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok((Arc::new(config), about))
}

/// What a certificate says of itself.
fn described(der: &CertificateDer<'_>) -> Option<Certificate> {
    let (_, parsed) = x509_parser::parse_x509_certificate(der.as_ref()).ok()?;
    let mut names: Vec<String> = parsed
        .subject_alternative_name()
        .ok()
        .flatten()
        .map(|extension| {
            extension
                .value
                .general_names
                .iter()
                .filter_map(|name| match name {
                    x509_parser::extensions::GeneralName::DNSName(dns) => Some((*dns).to_string()),
                    x509_parser::extensions::GeneralName::IPAddress(bytes) => ip_of(bytes),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    if names.is_empty() {
        names.extend(
            parsed
                .subject()
                .iter_common_name()
                .filter_map(|name| name.as_str().ok().map(str::to_string)),
        );
    }
    let at = |time: x509_parser::time::ASN1Time| Timestamp::from_unix_timestamp(time.timestamp()).ok();
    Some(Certificate {
        names,
        issuer: parsed.issuer().to_string(),
        not_before: at(parsed.validity().not_before)?,
        not_after: at(parsed.validity().not_after)?,
    })
}

fn ip_of(bytes: &[u8]) -> Option<String> {
    match bytes.len() {
        4 => Some(std::net::Ipv4Addr::from(<[u8; 4]>::try_from(bytes).ok()?).to_string()),
        16 => Some(std::net::Ipv6Addr::from(<[u8; 16]>::try_from(bytes).ok()?).to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Where the certificate this server signs itself is kept.
    fn self_signed_in(tls: &Path) -> (PathBuf, PathBuf) {
        (tls.join(SELF_SIGNED_CERTIFICATE), tls.join(SELF_SIGNED_KEY))
    }

    #[test]
    fn a_certificate_signed_here_is_made_once_and_read_back() {
        let directory = tempfile::tempdir().expect("directory");
        let tls = directory.path().join("tls");
        let (_, about) = prepared(Mode::SelfSigned, None, None, &tls)
            .expect("made")
            .expect("encrypted");
        assert!(about.names.contains(&"localhost".to_string()));
        assert!(about.not_after > about.not_before);

        let (certificate, _) = self_signed_in(&tls);
        let first = std::fs::read(&certificate).expect("written");
        prepared(Mode::SelfSigned, None, None, &tls).expect("read").expect("encrypted");
        assert_eq!(std::fs::read(&certificate).expect("kept"), first, "not made again");
    }

    #[test]
    fn a_key_that_is_not_the_certificate_s_own_is_refused() {
        let one = tempfile::tempdir().expect("directory");
        let other = tempfile::tempdir().expect("directory");
        prepared(Mode::SelfSigned, None, None, one.path()).expect("made");
        prepared(Mode::SelfSigned, None, None, other.path()).expect("made");
        let (certificate, _) = self_signed_in(one.path());
        let (_, key) = self_signed_in(other.path());
        assert_eq!(encryption_from(&certificate, &key).err(), Some(Problem::NotAPair));
        assert_eq!(
            encryption_from(&one.path().join("nothing"), &key).err(),
            Some(Problem::CertificateUnreadable)
        );
        assert_eq!(
            encryption_from(&certificate, &one.path().join("nothing")).err(),
            Some(Problem::KeyUnreadable)
        );
    }

    async fn a_server() -> (tempfile::TempDir, AppState) {
        let directory = tempfile::tempdir().expect("temporary directory");
        let config = melyxar_config::Config {
            directories: melyxar_config::Directories {
                data: directory.path().join("data"),
                cache: directory.path().join("cache"),
                transcodes: directory.path().join("cache/transcodes"),
                ..Default::default()
            },
            ..melyxar_config::Config::default()
        };
        crate::startup::prepare_directories(&config).expect("directories prepared");
        let database = melyxar_database::Database::open_in_memory().await.expect("database opens");
        (directory, AppState::new(config, database, None, None))
    }

    #[tokio::test]
    async fn a_pair_that_cannot_be_used_is_refused_and_nothing_changes() {
        let (directory, state) = a_server().await;
        apply(&state).await.expect("applied");
        let missing = directory.path().join("missing.pem");
        let missing = missing.to_str().expect("text");
        assert!(matches!(
            choose(&state, Mode::Provided, Some(missing), Some(missing)).await,
            Err(Trouble::Refused(Refused::Unusable(Problem::CertificateUnreadable)))
        ));
        assert!(matches!(
            choose(&state, Mode::Provided, Some("relative.pem"), Some(missing)).await,
            Err(Trouble::Refused(Refused::PathNeeded))
        ));
        assert_eq!(status(&state).mode, Mode::Proxy);
        assert!(encryption(&state).is_none());
    }

    #[tokio::test]
    async fn signing_its_own_encrypts_at_once_and_a_proxy_stops_it() {
        let (_directory, state) = a_server().await;
        let chosen = choose(&state, Mode::SelfSigned, None, None).await.expect("chosen");
        assert!(chosen.certificate.is_some());
        assert!(encryption(&state).is_some());
        assert!(sends_plain_to_encrypted(&state));

        choose(&state, Mode::Proxy, None, None).await.expect("chosen");
        assert!(encryption(&state).is_none());
        assert!(!sends_plain_to_encrypted(&state));
    }

    #[test]
    fn behind_a_proxy_nothing_is_encrypted() {
        let directory = tempfile::tempdir().expect("directory");
        assert!(prepared(Mode::Proxy, None, None, directory.path()).expect("fine").is_none());
        assert!(!directory.path().join(SELF_SIGNED_CERTIFICATE).exists());
    }
}
