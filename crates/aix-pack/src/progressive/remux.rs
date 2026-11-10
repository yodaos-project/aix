//! Converts ordinary ISO BMFF sample tables to independently addressable
//! fragments. Encoded sample bytes and codec configuration are preserved.

use super::{fragment_boxes::boxes, AssetKind, OBJECT_LIMIT};
use anyhow::{anyhow, bail, ensure, Result};
use mp4::{Mp4Reader, Mp4Track};
use std::{io::Cursor, ops::Range};

const MAX_SAMPLES: usize = 1_000_000;
const MAX_FRAGMENTS: usize = 4095;
const TARGET_FRAGMENT_MS: u128 = 2000;

struct Sample {
    data: Range<usize>,
    dts: u64,
    duration: u32,
    composition_offset: i32,
    sync: bool,
}

struct Track {
    id: u32,
    timescale: u32,
    samples: Vec<Sample>,
}

pub(super) fn is_flat(bytes: &[u8]) -> Result<bool> {
    let top = boxes(bytes).map_err(|err| anyhow!(err))?;
    let moov = top
        .iter()
        .find(|item| item.kind == *b"moov")
        .ok_or_else(|| anyhow!("Missing MP4 moov"))?;
    let children = boxes(moov.payload).map_err(|err| anyhow!(err))?;
    Ok(!children.iter().any(|item| item.kind == *b"mvex")
        && !top.iter().any(|item| item.kind == *b"moof"))
}

pub(super) fn inspect_flat(bytes: &[u8], kind: AssetKind, emit: bool) -> Result<Option<Vec<u8>>> {
    let top = boxes(bytes).map_err(|err| anyhow!(err))?;
    let ftyp = top
        .first()
        .filter(|item| item.kind == *b"ftyp")
        .ok_or_else(|| anyhow!("MP4 must begin with ftyp"))?;
    let moov = top
        .iter()
        .find(|item| item.kind == *b"moov")
        .ok_or_else(|| anyhow!("Missing MP4 moov"))?;
    ensure!(
        moov.bytes.len() <= 16 * 1024 * 1024 && ftyp.bytes.len() <= 4096,
        "MP4 metadata exceeds the remux budget"
    );
    let media: Vec<_> = top
        .iter()
        .filter(|item| item.kind == *b"mdat")
        .map(|item| {
            item.offset + item.bytes.len() - item.payload.len()..item.offset + item.bytes.len()
        })
        .collect();
    ensure!(!media.is_empty(), "MP4 has no media data");

    // Track metadata and sample tables can be read from ftyp + moov alone.
    // Chunk offsets continue to refer to the original file.
    let header = [ftyp.bytes, moov.bytes].concat();
    let reader = Mp4Reader::read_header(Cursor::new(&header), header.len() as u64)?;
    let (tracks, video_id) = super::media::classify_tracks(&reader, kind)?;
    ensure!(
        reader.moov.traks.len() == tracks.len(),
        "Duplicate MP4 track ID"
    );
    let mut output_tracks = Vec::new();
    for (&id, track) in &tracks {
        let info = &reader.tracks()[&id];
        let samples = samples(info, &media)?;
        ensure!(
            samples.first().is_some_and(|sample| sample.sync),
            "MP4 must begin with a random-access sample"
        );
        output_tracks.push(Track {
            id,
            timescale: track.timescale,
            samples,
        });
    }
    // Reject overlapping sample ranges so malformed tables cannot amplify
    // the input into a much larger remuxed file.
    let mut ranges: Vec<_> = output_tracks
        .iter()
        .flat_map(|track| track.samples.iter().map(|sample| &sample.data))
        .collect();
    ranges.sort_unstable_by_key(|range| range.start);
    ensure!(
        ranges.windows(2).all(|pair| pair[0].end <= pair[1].start),
        "Overlapping MP4 sample data"
    );
    let anchor = output_tracks
        .iter()
        .position(|track| Some(track.id) == video_id)
        .unwrap_or(0);
    let anchor_track = &output_tracks[anchor];
    let mut starts = vec![0];
    for (i, sample) in anchor_track.samples.iter().enumerate().skip(1) {
        let previous = &anchor_track.samples[*starts.last().unwrap()];
        if sample.sync
            && u128::from(sample.dts - previous.dts) * 1000
                >= u128::from(anchor_track.timescale) * TARGET_FRAGMENT_MS
        {
            starts.push(i);
        }
    }
    starts.push(anchor_track.samples.len());
    ensure!(
        starts.len() <= MAX_FRAGMENTS + 1,
        "MP4 exceeds the fragment count budget"
    );

    let mut init = ftyp.bytes.to_vec();
    let mut moov_payload = rewrite_init(moov.kind, moov.payload)?;
    let mut mvex = Vec::new();
    for track in &output_tracks {
        append_box(&mut mvex, b"trex", &fields(&[0, track.id, 1, 0, 0, 0]))?;
    }
    append_box(&mut moov_payload, b"mvex", &mvex)?;
    append_box(&mut init, b"moov", &moov_payload)?;
    ensure!(
        moov_payload.len() + 8 <= 1024 * 1024,
        "MP4 initialization exceeds its size budget"
    );
    super::media::validate_edits(&reader)?;

    let mut track_starts = vec![0; output_tracks.len()];
    for (fragment, boundary) in starts.windows(2).enumerate() {
        let next_anchor = anchor_track.samples.get(boundary[1]);
        let mut runs = Vec::new();
        for (index, track) in output_tracks.iter().enumerate() {
            let start = track_starts[index];
            let end = if let Some(next) = next_anchor {
                track.samples.partition_point(|sample| {
                    u128::from(sample.dts) * u128::from(anchor_track.timescale)
                        < u128::from(next.dts) * u128::from(track.timescale)
                })
            } else {
                track.samples.len()
            };
            let run = track
                .samples
                .get(start..end)
                .filter(|run| !run.is_empty())
                .ok_or_else(|| anyhow!("MP4 tracks do not overlap each fragment"))?;
            let first = &run[0];
            let last = run.last().unwrap();
            ensure!(
                last.dts + u64::from(last.duration) - first.dts <= u64::from(track.timescale) * 10,
                "MP4 requires a random-access sample at least every 10 seconds"
            );
            let scale = i128::from(track.timescale);
            let start_pts = run.iter().map(presentation).min().unwrap();
            let end_pts = run
                .iter()
                .map(|sample| presentation(sample) + i128::from(sample.duration))
                .max()
                .unwrap();
            ensure!(
                (start_pts - i128::from(first.dts)).abs() <= scale * 10
                    && end_pts - start_pts <= scale * 12,
                "MP4 presentation interval exceeds the fragment budget"
            );
            track_starts[index] = end;
            runs.push((track.id, run));
        }
        let count: usize = runs.iter().map(|(_, run)| run.len()).sum();
        let payload: usize = runs
            .iter()
            .flat_map(|(_, run)| run.iter())
            .map(|sample| sample.data.len())
            .sum();
        ensure!(
            count <= 8192 && payload + count * 16 + 512 <= OBJECT_LIMIT,
            "MP4 fragment exceeds the remux budget"
        );
        if emit {
            append_fragment(&mut init, bytes, (fragment + 1) as u32, &runs)?;
        }
    }
    Ok(emit.then_some(init))
}

fn presentation(sample: &Sample) -> i128 {
    i128::from(sample.dts) + i128::from(sample.composition_offset)
}

fn samples(track: &Mp4Track, media: &[Range<usize>]) -> Result<Vec<Sample>> {
    let table = &track.trak.mdia.minf.stbl;
    let count = table.stsz.sample_count as usize;
    ensure!(
        count > 0 && count <= MAX_SAMPLES,
        "Invalid MP4 sample count"
    );
    ensure!(
        table.stsz.sample_size != 0 || table.stsz.sample_sizes.len() == count,
        "Invalid MP4 sample sizes"
    );
    let runs = &table.stsc.entries;
    let chunks: Vec<u64> = match (&table.stco, &table.co64) {
        (Some(stco), None) => stco
            .entries
            .iter()
            .map(|&offset| u64::from(offset))
            .collect(),
        (None, Some(co64)) => co64.entries.clone(),
        _ => bail!("Invalid MP4 chunk offsets"),
    };
    ensure!(
        runs.first().is_some_and(|run| run.first_chunk == 1)
            && runs.windows(2).all(|w| w[0].first_chunk < w[1].first_chunk)
            && runs.iter().all(|run| run.samples_per_chunk > 0
                && run.sample_description_index == 1
                && run.first_chunk as usize <= chunks.len()),
        "Unsupported MP4 sample-to-chunk table"
    );

    if let Some(ctts) = &table.ctts {
        ensure!(
            ctts.version <= 1
                && (ctts.version == 1 || ctts.entries.iter().all(|e| e.sample_offset >= 0)),
            "Unsupported MP4 composition offsets"
        );
    }
    let mut durations = table
        .stts
        .entries
        .iter()
        .flat_map(|entry| std::iter::repeat_n(entry.sample_delta, entry.sample_count as usize));
    let mut offsets = table.ctts.as_ref().map(|ctts| {
        ctts.entries
            .iter()
            .flat_map(|entry| std::iter::repeat_n(entry.sample_offset, entry.sample_count as usize))
    });
    if let Some(stss) = &table.stss {
        ensure!(
            stss.entries.windows(2).all(|w| w[0] < w[1])
                && stss
                    .entries
                    .iter()
                    .all(|&index| index > 0 && index as usize <= count),
            "Invalid MP4 sync table"
        );
    }
    let mut out = Vec::with_capacity(count);
    let mut dts = 0u64;
    let mut run_index = 0;
    for (chunk_index, chunk) in chunks.into_iter().enumerate() {
        if runs
            .get(run_index + 1)
            .is_some_and(|run| run.first_chunk as usize == chunk_index + 1)
        {
            run_index += 1;
        }
        let mut start = usize::try_from(chunk)?;
        let index = media.partition_point(|range| range.end <= start);
        let container = media
            .get(index)
            .filter(|range| range.contains(&start))
            .ok_or_else(|| anyhow!("MP4 chunk is outside media data"))?;
        for _ in 0..runs[run_index].samples_per_chunk {
            let index = out.len();
            ensure!(index < count, "MP4 chunk contains too many samples");
            let size = if table.stsz.sample_size > 0 {
                table.stsz.sample_size
            } else {
                table.stsz.sample_sizes[index]
            } as usize;
            ensure!(
                (1..=2 * 1024 * 1024).contains(&size),
                "Invalid MP4 sample size"
            );
            let end = start
                .checked_add(size)
                .filter(|&end| end <= container.end)
                .ok_or_else(|| anyhow!("MP4 sample is outside media data"))?;
            let duration = durations
                .next()
                .filter(|&duration| duration > 0)
                .ok_or_else(|| anyhow!("Invalid MP4 decode times"))?;
            let composition_offset = match offsets.as_mut() {
                Some(offsets) => offsets
                    .next()
                    .ok_or_else(|| anyhow!("Truncated MP4 composition offsets"))?,
                None => 0,
            };
            let sync = table
                .stss
                .as_ref()
                .is_none_or(|stss| stss.entries.binary_search(&((index + 1) as u32)).is_ok());
            out.push(Sample {
                data: start..end,
                dts,
                duration,
                composition_offset,
                sync,
            });
            dts += u64::from(duration);
            start = end;
        }
    }
    ensure!(
        out.len() == count
            && durations.next().is_none()
            && offsets.is_none_or(|mut offsets| offsets.next().is_none()),
        "MP4 sample tables do not agree"
    );
    Ok(out)
}

fn rewrite_init(parent: [u8; 4], input: &[u8]) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    for child in boxes(input).map_err(|err| anyhow!(err))? {
        match (&parent, &child.kind) {
            (b"moov", b"trak") | (b"trak", b"mdia") | (b"mdia", b"minf") | (b"minf", b"stbl") => {
                let payload = rewrite_init(child.kind, child.payload)?;
                append_box(&mut output, &child.kind, &payload)?;
            }
            (b"stbl", b"stsd") => {
                ensure!(
                    child.payload.get(4..8) == Some(&1u32.to_be_bytes()),
                    "MP4 requires one sample description per track"
                );
                output.extend_from_slice(child.bytes);
            }
            (b"stbl", b"stts" | b"stsc" | b"stco" | b"co64") => {
                append_box(&mut output, &child.kind, &[0; 8])?;
            }
            (b"stbl", b"stsz") => append_box(&mut output, b"stsz", &[0; 12])?,
            (b"stbl", b"stss" | b"ctts" | b"sdtp") => (),
            // FFmpeg uses a roll recovery group on AAC. Preserve the priming
            // samples and edit, without carrying ordinary sample numbers.
            (b"stbl", b"sbgp" | b"sgpd") if child.payload.get(4..8) == Some(b"roll") => (),
            (b"stbl", _) => bail!("Unsupported MP4 sample table box"),
            (b"moov", b"udta" | b"meta") => (),
            _ => output.extend_from_slice(child.bytes),
        }
    }
    Ok(output)
}

fn append_fragment(
    out: &mut Vec<u8>,
    source: &[u8],
    number: u32,
    runs: &[(u32, &[Sample])],
) -> Result<()> {
    // Each traf has fixed overhead plus 16 bytes per sample. Offsets are
    // relative to the start of moof and point to the first sample of each run.
    let moof_size = 8
        + 16
        + runs
            .iter()
            .map(|(_, run)| 64 + run.len() * 16)
            .sum::<usize>();
    let mut offset = moof_size + 8;
    let mut moof = Vec::with_capacity(moof_size);
    append_box(&mut moof, b"mfhd", &fields(&[0, number]))?;
    for &(id, run) in runs {
        let mut traf = Vec::new();
        append_box(&mut traf, b"tfhd", &fields(&[0x0002_0000, id]))?;
        let mut tfdt = fields(&[0x0100_0000]);
        tfdt.extend_from_slice(&run[0].dts.to_be_bytes());
        append_box(&mut traf, b"tfdt", &tfdt)?;
        let mut trun = fields(&[0x0100_0f01, run.len() as u32, offset as u32]);
        for sample in run {
            let flags = if sample.sync {
                0x0200_0000
            } else {
                0x0101_0000
            };
            trun.extend(fields(&[
                sample.duration,
                sample.data.len() as u32,
                flags,
                sample.composition_offset as u32,
            ]));
            offset += sample.data.len();
        }
        append_box(&mut traf, b"trun", &trun)?;
        append_box(&mut moof, b"traf", &traf)?;
    }
    ensure!(
        moof.len() + 8 == moof_size,
        "Invalid generated moof offsets"
    );
    append_box(out, b"moof", &moof)?;
    out.extend_from_slice(&u32::try_from(offset - moof_size)?.to_be_bytes());
    out.extend_from_slice(b"mdat");
    for (_, run) in runs {
        for sample in *run {
            out.extend_from_slice(&source[sample.data.clone()]);
        }
    }
    Ok(())
}

fn fields(values: &[u32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_be_bytes())
        .collect()
}

fn append_box(out: &mut Vec<u8>, kind: &[u8; 4], payload: &[u8]) -> Result<()> {
    out.extend_from_slice(&u32::try_from(payload.len() + 8)?.to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(payload);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::progressive::fragment_boxes::{defaults, fragment};

    #[test]
    fn maps_constant_sample_sizes_across_chunks() {
        let original = include_bytes!("../../tests/data/tone-flat.m4a");
        let reader = Mp4Reader::read_header(Cursor::new(original), original.len() as u64).unwrap();
        let mut track = Mp4Track {
            trak: reader.moov.traks[0].clone(),
            trafs: Vec::new(),
            default_sample_duration: 0,
        };
        let table = &mut track.trak.mdia.minf.stbl;
        let sizes = &mut table.stsz;
        sizes.sample_size = 3;
        sizes.sample_count = 4;
        sizes.sample_sizes.clear();
        table.stts.entries.truncate(1);
        table.stts.entries[0].sample_count = 4;
        table.stsc.entries[0].samples_per_chunk = 2;
        table.stco.as_mut().unwrap().entries = vec![44, 58];

        let payload = 44..64;
        let samples = samples(&track, std::slice::from_ref(&payload)).unwrap();
        let ranges: Vec<_> = samples.into_iter().map(|sample| sample.data).collect();
        assert_eq!(ranges, [44..47, 47..50, 58..61, 61..64]);
    }

    #[test]
    fn flat_remux_keeps_encoded_samples_and_decode_and_presentation_times() {
        let original = include_bytes!("../../tests/data/pattern-tone-flat.mp4");
        let reader = Mp4Reader::read_header(Cursor::new(original), original.len() as u64).unwrap();
        let payloads: Vec<_> = boxes(original)
            .unwrap()
            .into_iter()
            .filter(|item| item.kind == *b"mdat")
            .map(|item| {
                item.offset + item.bytes.len() - item.payload.len()..item.offset + item.bytes.len()
            })
            .collect();
        let expected: std::collections::BTreeMap<_, _> = reader
            .tracks()
            .iter()
            .map(|(&id, track)| (id, samples(track, &payloads).unwrap()))
            .collect();
        assert!(expected
            .values()
            .flatten()
            .any(|sample| sample.composition_offset > 0));
        let output = inspect_flat(original, AssetKind::Video, true)
            .unwrap()
            .unwrap();
        let atoms = boxes(&output).unwrap();
        let init = [atoms[0].bytes, atoms[1].bytes].concat();
        let remuxed = Mp4Reader::read_header(Cursor::new(&init), init.len() as u64).unwrap();
        for (&id, track) in reader.tracks() {
            let edit = |track: &Mp4Track| {
                let edit = &track
                    .trak
                    .edts
                    .as_ref()
                    .unwrap()
                    .elst
                    .as_ref()
                    .unwrap()
                    .entries[0];
                (edit.media_time, edit.segment_duration)
            };
            assert_eq!(edit(track), edit(&remuxed.tracks()[&id]));
        }
        let defaults = defaults(&init).unwrap();
        let mut next = std::collections::BTreeMap::<u32, usize>::new();
        for pair in atoms[2..].as_chunks::<2>().0 {
            let bytes = &output[pair[0].offset..pair[1].offset + pair[1].bytes.len()];
            for sample in fragment(bytes, &defaults).unwrap() {
                let index = next.entry(sample.track).or_default();
                let source = &expected[&sample.track][*index];
                assert_eq!(sample.dts, source.dts);
                assert_eq!(
                    sample.composition_offset,
                    i64::from(source.composition_offset)
                );
                assert_eq!(sample.duration, source.duration);
                assert_eq!(sample.key, source.sync);
                assert_eq!(bytes[sample.data], original[source.data.clone()]);
                *index += 1;
            }
        }
        assert!(expected
            .iter()
            .all(|(track, samples)| next[track] == samples.len()));
    }
}
