//! Track tags and cover art, read straight from the file when a sample is shown.
//! Not stored: most samples carry none, and the file is the source of truth.

use std::path::Path;

use lofty::picture::PictureType;
use lofty::prelude::*;
use lofty::tag::Tag;
use serde::Serialize;

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

#[cfg(test)]
mod tests {
    use super::*;
    use lofty::config::WriteOptions;
    use lofty::picture::{MimeType, Picture};
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
}
