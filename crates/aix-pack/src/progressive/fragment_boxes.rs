//! Bounded ISO BMFF fragment parser for the AIX fmp4-h264-aac-v1 profile.
//! Kept in sync with aix-pack/src/progressive/fragment_boxes.rs.

use std::collections::BTreeMap;
use std::ops::Range;

#[derive(Clone)]
pub(super) struct IsoBox<'a> {
    pub kind: [u8; 4],
    pub offset: usize,
    pub payload: &'a [u8],
    pub bytes: &'a [u8],
}

pub(super) fn boxes(mut bytes: &[u8]) -> Result<Vec<IsoBox<'_>>, String> {
    let mut offset = 0;
    let mut result = Vec::new();
    while !bytes.is_empty() {
        if result.len() == 8192 {
            return Err("Too many ISO BMFF boxes".into());
        }
        let header = bytes.get(..8).ok_or("Truncated ISO BMFF header")?;
        let size = u32::from_be_bytes(header[..4].try_into().unwrap());
        let (header_size, size) = match size {
            0 => return Err("Open ended ISO BMFF boxes are not supported".into()),
            1 => {
                let extended = bytes.get(8..16).ok_or("Truncated ISO BMFF extended size")?;
                (
                    16,
                    usize::try_from(u64::from_be_bytes(extended.try_into().unwrap()))
                        .map_err(|_| "ISO BMFF box size overflow")?,
                )
            }
            v => (8, v as usize),
        };
        if size < header_size || size > bytes.len() {
            return Err("Invalid ISO BMFF box length".into());
        }
        result.push(IsoBox {
            kind: header[4..8].try_into().unwrap(),
            offset,
            payload: &bytes[header_size..size],
            bytes: &bytes[..size],
        });
        offset += size;
        bytes = &bytes[size..];
    }
    Ok(result)
}

#[derive(Clone, Copy, Default)]
pub(super) struct Defaults {
    duration: u32,
    size: u32,
    flags: u32,
}

pub(super) struct Sample {
    pub track: u32,
    pub dts: u64,
    pub composition_offset: i64,
    pub duration: u32,
    pub key: bool,
    pub data: Range<usize>,
}

pub(super) fn defaults(init: &[u8]) -> Result<BTreeMap<u32, Defaults>, String> {
    let top = boxes(init)?;
    if top.len() != 2 || top[0].kind != *b"ftyp" || top[1].kind != *b"moov" {
        return Err("Fragment initialization must contain ftyp and moov".into());
    }
    let moov = boxes(top[1].payload)?;
    let mvex = moov
        .iter()
        .find(|item| item.kind == *b"mvex")
        .ok_or("MP4 is not fragmented")?;
    let mut defaults = BTreeMap::new();
    for atom in boxes(mvex.payload)? {
        if atom.kind != *b"trex" {
            continue;
        }
        let mut data = Fields(atom.payload);
        if data.u32()? != 0 {
            return Err("Invalid trex version".into());
        }
        let id = data.u32()?;
        if data.u32()? != 1 || id == 0 {
            return Err("Unsupported sample description".into());
        }
        let value = Defaults {
            duration: data.u32()?,
            size: data.u32()?,
            flags: data.u32()?,
        };
        if !data.0.is_empty() || defaults.insert(id, value).is_some() {
            return Err("Invalid duplicate fragment track defaults".into());
        }
    }
    if defaults.is_empty() || defaults.len() > 2 {
        return Err("Expected one or two fragmented tracks".into());
    }
    Ok(defaults)
}

pub(super) fn fragment(
    bytes: &[u8],
    defaults: &BTreeMap<u32, Defaults>,
) -> Result<Vec<Sample>, String> {
    let top = boxes(bytes)?;
    if top.len() != 2 || top[0].kind != *b"moof" || top[1].kind != *b"mdat" {
        return Err("A fragment must contain one moof and one mdat".into());
    }
    let media_start = top[1].offset + top[1].bytes.len() - top[1].payload.len();
    let mut out = Vec::new();
    let mut seen = BTreeMap::new();
    for traf in boxes(top[0].payload)? {
        if traf.kind == *b"mfhd" {
            continue;
        }
        if traf.kind != *b"traf" {
            return Err("Unsupported fragment box".into());
        }
        let items = boxes(traf.payload)?;
        let mut tfhd = None;
        let mut tfdt = None;
        let mut runs = Vec::new();
        for item in items {
            match &item.kind {
                b"tfhd" if tfhd.is_none() => tfhd = Some(item),
                b"tfdt" if tfdt.is_none() => tfdt = Some(item),
                b"trun" => runs.push(item),
                // Encryption, dependency maps and alternate sample-group
                // semantics need separate profiles.
                _ => return Err("Unsupported fragment track box".into()),
            }
        }
        let mut fields = Fields(tfhd.ok_or("Missing fragment track header")?.payload);
        let flags = fields.u32()?;
        if flags & 0x0002_0000 == 0 || flags & !0x0002_003a != 0 {
            return Err("Fragments must use default-base-is-moof addressing".into());
        }
        let track = fields.u32()?;
        let mut default = *defaults.get(&track).ok_or("Unknown fragment track")?;
        if seen.insert(track, ()).is_some() {
            return Err("Duplicate fragment track".into());
        }
        if flags & 2 != 0 && fields.u32()? != 1 {
            return Err("Unsupported sample description".into());
        }
        if flags & 8 != 0 {
            default.duration = fields.u32()?;
        }
        if flags & 16 != 0 {
            default.size = fields.u32()?;
        }
        if flags & 32 != 0 {
            default.flags = fields.u32()?;
        }
        if !fields.0.is_empty() {
            return Err("Unexpected fragment track header data".into());
        }
        let mut fields = Fields(tfdt.ok_or("Missing fragment decode timestamp")?.payload);
        let mut dts = match fields.u32()? {
            0 => u64::from(fields.u32()?),
            0x0100_0000 => fields.u64()?,
            _ => return Err("Unsupported tfdt version".into()),
        };
        if !fields.0.is_empty() || runs.is_empty() {
            return Err("Incomplete fragment track".into());
        }
        let mut cursor = None;
        for run in runs {
            let mut fields = Fields(run.payload);
            let header = fields.u32()?;
            let version = header >> 24;
            let flags = header & 0x00ff_ffff;
            if version > 1 || flags & !0x0000_0f05 != 0 || (flags & 4 != 0 && flags & 0x400 != 0) {
                return Err("Unsupported fragment sample run".into());
            }
            let count = fields.u32()? as usize;
            if count == 0 || count > 8192usize.saturating_sub(out.len()) {
                return Err("Fragment exceeds the sample budget".into());
            }
            if flags & 1 != 0 {
                cursor = Some(
                    usize::try_from(fields.u32()? as i32)
                        .map_err(|_| "Invalid fragment data offset")?,
                );
            }
            let first_flags = if flags & 4 != 0 {
                Some(fields.u32()?)
            } else {
                None
            };
            for i in 0..count {
                let duration = if flags & 0x100 != 0 {
                    fields.u32()?
                } else {
                    default.duration
                };
                let size = if flags & 0x200 != 0 {
                    fields.u32()?
                } else {
                    default.size
                };
                let sample_flags = if flags & 0x400 != 0 {
                    fields.u32()?
                } else if i == 0 {
                    first_flags.unwrap_or(default.flags)
                } else {
                    default.flags
                };
                let composition_offset = if flags & 0x800 != 0 {
                    let offset = fields.u32()?;
                    if version == 1 {
                        i64::from(offset as i32)
                    } else {
                        i64::from(offset)
                    }
                } else {
                    0
                };
                let offset = cursor.ok_or("First run requires a data offset")?;
                let end = offset
                    .checked_add(size as usize)
                    .ok_or("Fragment sample offset overflow")?;
                if duration == 0
                    || size == 0
                    || size > 2 * 1024 * 1024
                    || offset < media_start
                    || end > bytes.len()
                {
                    return Err("Fragment sample falls outside the media payload".into());
                }
                out.push(Sample {
                    track,
                    dts,
                    composition_offset,
                    duration,
                    key: sample_flags & 0x0001_0000 == 0 && (sample_flags >> 24) & 3 != 1,
                    data: offset..end,
                });
                cursor = Some(end);
                dts = dts
                    .checked_add(u64::from(duration))
                    .ok_or("Fragment timestamp overflow")?;
            }
            if !fields.0.is_empty() {
                return Err("Unexpected fragment sample run data".into());
            }
        }
    }
    if out.is_empty() || seen.len() != defaults.len() {
        return Err("Fragment must contain each required track".into());
    }
    out.sort_unstable_by_key(|sample| sample.data.start);
    if out[0].data.start != media_start
        || out.last().unwrap().data.end != bytes.len()
        || out.windows(2).any(|w| w[0].data.end != w[1].data.start)
    {
        return Err("Fragment contains overlapping or unreferenced media bytes".into());
    }
    Ok(out)
}

struct Fields<'a>(&'a [u8]);
impl Fields<'_> {
    fn u32(&mut self) -> Result<u32, String> {
        let (head, tail) = self
            .0
            .split_first_chunk::<4>()
            .ok_or("Truncated fragment field")?;
        self.0 = tail;
        Ok(u32::from_be_bytes(*head))
    }

    fn u64(&mut self) -> Result<u64, String> {
        Ok((u64::from(self.u32()?) << 32) | u64::from(self.u32()?))
    }
}
