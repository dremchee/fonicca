//! Track tags and cover art, read straight from the file when a sample is shown.
//! Not stored: most samples carry none, and the file is the source of truth.

use std::path::Path;

use lofty::config::WriteOptions;
use lofty::picture::{Picture, PictureType};
use lofty::prelude::*;
use lofty::tag::Tag;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackTags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<u16>,
    pub genre: Option<String>,
    pub has_cover: bool,
}

/// The file's main tag, or the first one it has. Unreadable files and files without
/// tags both give `None`: neither is worth an error in the player.
fn with_tag<T>(path: &Path, f: impl FnOnce(&Tag) -> T) -> Option<T> {
    let file = lofty::read_from_path(path).ok()?;
    file.primary_tag().or_else(|| file.first_tag()).map(f)
}

fn text(v: Option<std::borrow::Cow<'_, str>>) -> Option<String> {
    v.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

pub fn read(path: &Path) -> Option<TrackTags> {
    let tags = with_tag(path, |tag| TrackTags {
        title: text(tag.title()),
        artist: text(tag.artist()),
        album: text(tag.album()),
        year: tag.date().map(|d| d.year).filter(|&y| y > 0),
        genre: text(tag.genre()),
        has_cover: !tag.pictures().is_empty(),
    })?;
    (tags != TrackTags::default()).then_some(tags)
}

/// Image bytes of the front cover, or of the first picture. The webview tells the
/// format from the bytes, so no MIME type is passed along.
pub fn cover(path: &Path) -> Option<Vec<u8>> {
    with_tag(path, |tag| {
        let pic = tag.get_picture_type(PictureType::CoverFront).or_else(|| tag.pictures().first())?;
        Some(pic.data().to_vec())
    })
    .flatten()
}

/// Reads an image file picked as a new cover, refusing anything lofty would not embed.
pub fn image_file(path: &Path) -> Result<Picture> {
    Picture::from_reader(&mut std::fs::File::open(path)?).map_err(|e| Error::Other(format!("not a supported image: {e}")))
}

/// New values for a file's tags; an empty or missing value removes that field.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagEdit {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<u16>,
    pub genre: Option<String>,
    #[serde(default)]
    pub cover: CoverEdit,
}

#[derive(Debug, Default, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CoverEdit {
    #[default]
    Keep,
    Remove,
    /// Replace the front cover with this image file.
    File { path: String },
}

fn set_text(tag: &mut Tag, value: Option<String>, set: fn(&mut Tag, String), remove: fn(&mut Tag)) {
    match value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty()) {
        Some(v) => set(tag, v),
        None => remove(tag),
    }
}

/// Writes `edit` into the file's main tag, creating one of the file's own kind (ID3v2
/// for MP3 and WAV, Vorbis comments for FLAC and Ogg, MP4 atoms…) if it has none.
pub fn write(path: &Path, edit: TagEdit) -> Result<()> {
    let cover = match &edit.cover {
        CoverEdit::File { path } => {
            let mut pic = image_file(Path::new(path))?;
            pic.set_pic_type(PictureType::CoverFront);
            Some(pic)
        }
        _ => None,
    };
    let mut file = lofty::read_from_path(path).map_err(|e| Error::Audio(e.to_string()))?;
    if file.primary_tag().is_none() && file.first_tag().is_none() {
        file.insert_tag(Tag::new(file.primary_tag_type()));
    }
    let tag = match file.primary_tag_mut() {
        Some(tag) => tag,
        None => file.first_tag_mut().expect("a tag was just inserted"),
    };
    set_text(tag, edit.title, Tag::set_title, Tag::remove_title);
    set_text(tag, edit.artist, Tag::set_artist, Tag::remove_artist);
    set_text(tag, edit.album, Tag::set_album, Tag::remove_album);
    set_text(tag, edit.genre, Tag::set_genre, Tag::remove_genre);
    tag.remove_key(ItemKey::Year);
    tag.remove_key(ItemKey::RecordingDate);
    if let Some(year) = edit.year.filter(|&y| y > 0) {
        tag.insert_text(ItemKey::RecordingDate, year.to_string());
    }
    match edit.cover {
        CoverEdit::Keep => {}
        CoverEdit::Remove => {
            while !tag.pictures().is_empty() {
                tag.remove_picture(0);
            }
        }
        CoverEdit::File { .. } => {
            tag.remove_picture_type(PictureType::CoverFront);
            tag.push_picture(cover.expect("read above"));
        }
    }
    tag.save_to_path(path, WriteOptions::default()).map_err(|e| Error::Audio(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lofty::picture::MimeType;
    use lofty::tag::TagType;

    fn silent_wav(path: &Path) {
        let spec = hound::WavSpec { channels: 1, sample_rate: 8000, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
        let mut w = hound::WavWriter::create(path, spec).unwrap();
        for _ in 0..800 {
            w.write_sample(0i16).unwrap();
        }
        w.finalize().unwrap();
    }

    #[test]
    fn untagged_file_has_no_tags_or_cover() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kick.wav");
        silent_wav(&path);
        assert_eq!(read(&path), None);
        assert!(cover(&path).is_none());
        assert_eq!(read(&dir.path().join("missing.wav")), None);
    }

    #[test]
    fn reads_tags_and_front_cover() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("song.wav");
        silent_wav(&path);
        let mut tag = Tag::new(TagType::Id3v2);
        tag.set_title("New Age".into());
        tag.set_artist("The Velvet Underground".into());
        tag.set_album("  ".into());
        // ID3v2.4 keeps the year as the recording date (TDRC); v2.3 TYER is read as that too.
        tag.insert_text(ItemKey::RecordingDate, "1970".into());
        let png = b"\x89PNG\r\n\x1a\nfake".to_vec();
        tag.push_picture(Picture::unchecked(png.clone()).pic_type(PictureType::CoverFront).mime_type(MimeType::Png).into());
        tag.save_to_path(&path, WriteOptions::default()).unwrap();

        let tags = read(&path).unwrap();
        assert_eq!(tags.title.as_deref(), Some("New Age"));
        assert_eq!(tags.artist.as_deref(), Some("The Velvet Underground"));
        assert_eq!(tags.album, None, "blank values are dropped");
        assert_eq!(tags.year, Some(1970));
        assert!(tags.has_cover);
        assert_eq!(cover(&path), Some(png));
    }

    #[test]
    fn writes_tags_into_an_untagged_file_and_clears_them() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("loop.wav");
        silent_wav(&path);
        let png = dir.path().join("cover.png");
        // Smallest valid PNG: 1x1, so lofty can tell the format.
        std::fs::write(&png, b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0\x1f\x15\xc4\x89\0\0\0\rIDATx\x9cc\xf8\x0f\0\x01\x01\x01\0\x18\xdd\x8d\xb4\0\0\0\0IEND\xaeB`\x82").unwrap();

        let edit = TagEdit {
            title: Some(" Groove ".into()),
            artist: Some("Me".into()),
            year: Some(2024),
            cover: CoverEdit::File { path: png.to_string_lossy().into() },
            ..Default::default()
        };
        write(&path, edit).unwrap();
        let tags = read(&path).unwrap();
        assert_eq!((tags.title.as_deref(), tags.artist.as_deref(), tags.year), (Some("Groove"), Some("Me"), Some(2024)));
        assert!(tags.has_cover);
        // The audio is still readable after the tag was added.
        assert!(crate::audio::probe(&path).is_ok());

        write(&path, TagEdit { title: Some("Groove".into()), cover: CoverEdit::Remove, ..Default::default() }).unwrap();
        let tags = read(&path).unwrap();
        assert_eq!((tags.artist, tags.year, tags.has_cover), (None, None, false));
    }
}
