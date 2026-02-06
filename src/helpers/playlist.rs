use std::{
    fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub enum PlaylistError {
    Io(io::Error),
    Parse(toml::de::Error),
    Serialize(toml::ser::Error),
}

impl std::fmt::Display for PlaylistError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlaylistError::Io(e) => write!(f, "IO error: {}", e),
            PlaylistError::Parse(e) => write!(f, "Parse error: {}", e),
            PlaylistError::Serialize(e) => write!(f, "Serialize error: {}", e),
        }
    }
}

impl std::error::Error for PlaylistError {}

impl From<io::Error> for PlaylistError {
    fn from(e: io::Error) -> Self {
        PlaylistError::Io(e)
    }
}

impl From<toml::de::Error> for PlaylistError {
    fn from(e: toml::de::Error) -> Self {
        PlaylistError::Parse(e)
    }
}

impl From<toml::ser::Error> for PlaylistError {
    fn from(e: toml::ser::Error) -> Self {
        PlaylistError::Serialize(e)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub name: String,
    pub songs: Vec<PathBuf>,
}

impl Playlist {
    pub fn new(name: String, songs: Vec<PathBuf>) -> Self {
        Self { name, songs }
    }

    /// Load a playlist from a TOML file.
    /// Returns the playlist and a list of missing song paths.
    pub fn load(path: &Path) -> Result<(Self, Vec<PathBuf>), PlaylistError> {
        let content = fs::read_to_string(path)?;
        let playlist: Playlist = toml::from_str(&content)?;

        let mut missing = Vec::new();
        let valid_songs: Vec<PathBuf> = playlist
            .songs
            .into_iter()
            .filter(|song| {
                if song.exists() {
                    true
                } else {
                    missing.push(song.clone());
                    false
                }
            })
            .collect();

        Ok((
            Playlist {
                name: playlist.name,
                songs: valid_songs,
            },
            missing,
        ))
    }

    /// Save the playlist to a TOML file.
    /// Creates parent directories if needed.
    /// Canonicalizes song paths to absolute paths.
    pub fn save(&self, path: &Path) -> Result<(), PlaylistError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Canonicalize paths to absolute
        let canonical_songs: Vec<PathBuf> = self
            .songs
            .iter()
            .filter_map(|p| p.canonicalize().ok())
            .collect();

        let playlist = Playlist {
            name: self.name.clone(),
            songs: canonical_songs,
        };

        let content = toml::to_string_pretty(&playlist)?;
        fs::write(path, content)?;
        Ok(())
    }

    /// Returns the default playlist directory: ~/.config/kronos/playlists/
    pub fn default_directory() -> PathBuf {
        home::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config")
            .join("kronos")
            .join("playlists")
    }
}
