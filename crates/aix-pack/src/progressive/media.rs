use super::{
    fragment_boxes, remux, AssetKind, Fragment, Support, TimeRange, TimelineLayout, TimelineTrack,
    OBJECT_LIMIT,
};
use anyhow::{anyhow, bail, Result};
use mp4::{AudioObjectType, MediaType, Mp4Reader, TrackType};
use std::{collections::BTreeMap, io::Cursor, sync::Arc};

pub(super) const PROFILE: &str = "fmp4-h264-aac-v1";

pub(super) type Preparation = (String, Vec<u8>, TimelineLayout, BTreeMap<String, Arc<[u8]>>);

pub(super) fn probe(bytes: &[u8], kind: AssetKind) -> Support {
    let result = remux::is_flat(bytes).and_then(|flat| {
        if flat {
            remux::inspect_flat(bytes, kind, false).map(|_| ())
        } else {
            inspect(bytes, kind, false).map(|_| ())
        }
    });
    let supported = result.is_ok();
    Support {
        supported,
        profile: supported.then_some(PROFILE),
        required_capabilities: vec![PROFILE],
        features: if supported {
            vec!["random-access-fragments"]
        } else {
            Vec::new()
        },
        reason: result.err().map(|e| e.to_string()),
    }
}

pub(super) fn prepare(bytes: &[u8], kind: AssetKind) -> Result<Preparation> {
    let remuxed = if remux::is_flat(bytes)? {
        remux::inspect_flat(bytes, kind, true)?
    } else {
        None
    };
    let (init, timeline, objects) = inspect(remuxed.as_deref().unwrap_or(bytes), kind, true)?;
    Ok(("init.mp4".into(), init, timeline, objects))
}

type Inspection = (Vec<u8>, TimelineLayout, BTreeMap<String, Arc<[u8]>>);

fn inspect(bytes: &[u8], kind: AssetKind, collect: bool) -> Result<Inspection> {
    let top = fragment_boxes::boxes(bytes).map_err(|e| anyhow!(e))?;
    let mut init = Vec::new();
    if let Some(ftyp) = top.first().filter(|item| item.kind == *b"ftyp") {
        init.extend_from_slice(ftyp.bytes);
    } else {
        bail!("fMP4 source must begin with ftyp");
    }
    let moov = top
        .iter()
        .position(|item| item.kind == *b"moov")
        .ok_or_else(|| anyhow!("Missing MP4 initialization"))?;
    if top[moov].bytes.len() > 1024 * 1024 {
        bail!("fMP4 initialization exceeds its size budget");
    }
    if top[1..moov]
        .iter()
        .any(|item| item.kind == *b"mdat" || item.kind == *b"moof")
    {
        bail!("Media payload precedes MP4 initialization");
    }
    init.extend_from_slice(top[moov].bytes);
    let defaults = fragment_boxes::defaults(&init).map_err(|e| anyhow!(e))?;
    let reader = Mp4Reader::read_header(Cursor::new(&init), init.len() as u64)?;
    let (tracks, video_track) = classify_tracks(&reader, kind)?;
    validate_edits(&reader)?;
    if defaults.len() != tracks.len()
        || reader
            .tracks()
            .iter()
            .any(|(id, track)| !defaults.contains_key(id) || track.sample_count() != 0)
    {
        bail!("fMP4 profile requires empty sample tables and defaults for each track");
    }
    let mut timeline = TimelineLayout {
        init: "init.mp4".into(),
        tracks,
        fragments: Vec::new(),
    };
    let mut objects = BTreeMap::new();
    let mut i = moov + 1;
    while let Some(item) = top.get(i) {
        match &item.kind {
            b"sidx" | b"styp" | b"free" | b"mfra" => {
                i += 1;
                continue;
            }
            b"moof" => {}
            _ => bail!("Unsupported top-level fMP4 box"),
        }
        let mdat = top
            .get(i + 1)
            .filter(|item| item.kind == *b"mdat")
            .ok_or_else(|| anyhow!("Fragment is missing mdat"))?;
        let data = &bytes[item.offset..mdat.offset + mdat.bytes.len()];
        if data.len() > OBJECT_LIMIT {
            bail!("fMP4 fragment exceeds its object budget");
        }
        let samples = fragment_boxes::fragment(data, &defaults).map_err(|e| anyhow!(e))?;
        let mut ranges: BTreeMap<u32, TimeRange> = BTreeMap::new();
        for sample in samples {
            let end = sample.dts + u64::from(sample.duration);
            let pts = i64::try_from(i128::from(sample.dts) + i128::from(sample.composition_offset))
                .map_err(|_| anyhow!("Invalid fMP4 presentation timestamp"))?;
            let pts_end = pts
                .checked_add(i64::from(sample.duration))
                .ok_or_else(|| anyhow!("Invalid fMP4 presentation end"))?;
            if let Some(range) = ranges.get_mut(&sample.track) {
                if sample.dts != range.end {
                    bail!("Discontinuous fragment samples");
                }
                range.end = end;
                range.presentation_start = range.presentation_start.min(pts);
                range.presentation_end = range.presentation_end.max(pts_end);
            } else {
                if Some(sample.track) == video_track && !sample.key {
                    bail!("Each fMP4 fragment must start with a video random-access sample");
                }
                ranges.insert(
                    sample.track,
                    TimeRange {
                        start: sample.dts,
                        end,
                        presentation_start: pts,
                        presentation_end: pts_end,
                    },
                );
            }
            // The fragment parser has already checked all sample payload
            // ranges, including nonoverlap. Also reject timestamp overflow.
            if sample.data.is_empty() {
                bail!("Invalid fMP4 presentation timestamp or sample size");
            }
        }
        if ranges.len() != timeline.tracks.len() || timeline.fragments.len() >= 4096 {
            bail!("Invalid fragment track coverage or fragment count");
        }
        for (&id, range) in &ranges {
            let expected = timeline
                .fragments
                .last()
                .map_or(0, |previous| previous.ranges[&id].end);
            let scale = u64::from(timeline.tracks[&id].timescale);
            if range.start != expected
                || range.end - range.start > scale * 10
                || range.end > i64::MAX as u64
                || range.presentation_end.abs_diff(range.presentation_start) > scale * 12
                || range.presentation_start.abs_diff(range.start as i64) > scale * 10
            {
                bail!("Expected contiguous fMP4 fragments at most 10 seconds long");
            }
        }
        let id = if collect {
            super::identity(data).sha256
        } else {
            String::new()
        };
        if collect {
            objects.entry(id.clone()).or_insert_with(|| Arc::from(data));
        }
        timeline.fragments.push(Fragment { object: id, ranges });
        i += 2;
    }
    if timeline.fragments.is_empty() {
        bail!("No prepared fMP4 fragments; remux to the supported profile first");
    }
    Ok((init, timeline, objects))
}

pub(super) fn classify_tracks<R: std::io::Read + std::io::Seek>(
    reader: &Mp4Reader<R>,
    kind: AssetKind,
) -> Result<(BTreeMap<u32, TimelineTrack>, Option<u32>)> {
    let mut tracks = BTreeMap::new();
    let mut video_track = None;
    let mut audio_track = None;
    for (&id, track) in reader.tracks() {
        if id == 0 || track.timescale() == 0 {
            bail!("Invalid MP4 track");
        }
        let kind = match (track.track_type()?, track.media_type()?) {
            (TrackType::Video, MediaType::H264) if video_track.is_none() => {
                video_track = Some(id);
                AssetKind::Video
            }
            (TrackType::Audio, MediaType::AAC)
                if audio_track.is_none()
                    && track.audio_profile()? == AudioObjectType::AacLowComplexity =>
            {
                audio_track = Some(id);
                AssetKind::Audio
            }
            _ => bail!("Expected H.264 and/or AAC-LC in fMP4"),
        };
        tracks.insert(
            id,
            TimelineTrack {
                kind,
                timescale: track.timescale(),
            },
        );
    }
    match kind {
        AssetKind::Audio if audio_track.is_some() && video_track.is_none() => {}
        AssetKind::Video if video_track.is_some() => {}
        _ => bail!("fMP4 tracks do not match the declared asset kind"),
    }
    Ok((tracks, video_track))
}

pub(super) fn validate_edits<R: std::io::Read + std::io::Seek>(
    reader: &Mp4Reader<R>,
) -> Result<()> {
    if reader.timescale() == 0 {
        bail!("Invalid MP4 movie timescale");
    }
    for track in reader.tracks().values() {
        if let Some(edts) = &track.trak.edts {
            let edit = edts
                .elst
                .as_ref()
                .filter(|elst| elst.version <= 1 && elst.entries.len() == 1)
                .map(|elst| (&elst.entries[0], elst.version))
                .ok_or_else(|| anyhow!("MP4 requires one continuous edit per track"))?;
            let max = if edit.1 == 0 {
                i32::MAX as u64
            } else {
                i64::MAX as u64
            };
            if edit.0.media_rate != 1 || edit.0.media_rate_fraction != 0 || edit.0.media_time > max
            {
                bail!("MP4 only supports nonnegative, rate-one edits");
            }
        }
    }
    Ok(())
}
