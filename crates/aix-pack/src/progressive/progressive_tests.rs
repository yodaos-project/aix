use super::*;

fn fixture() -> Vec<InputFile> {
    let video = include_bytes!("../../tests/data/pattern-tone-flat.mp4");
    vec![
        InputFile::new("app.json", br#"{"pages":["pages/index"]}"#),
        InputFile::new("pages/index.ink", br#"<script def>{}</script>
<script setup>
import clip from '../assets/nested/clip.mp4' with { type: 'video', progressive: 'prefer' };
export default { data: { src: clip } };
</script>
<page><video src="{{src}}" /></page>"#),
        InputFile::new("app.js", br#"
        // import unused from './missing.mp4' with { type: 'video', progressive: 'prefer' };
        const first = () => import('./assets/nested/clip.mp4', { with: { progressive: 'prefer', type: 'video' } });
        export { default as second } from './assets/nested/clip.mp4' with { type: 'video', progressive: 'prefer' };
        export default { first };
        "#),
        InputFile::new("assets/nested/clip.mp4", video),
    ]
}

fn receipt(plan: &ProgressiveAssetPlan) -> ProgressiveAssetUploadReceipt {
    ProgressiveAssetUploadReceipt {
        upload_id: "upload".into(),
        asset_version: plan.asset_version.clone(),
        objects: plan
            .remote_objects
            .iter()
            .map(|object| UploadedObject {
                id: object.id.clone(),
                size: object.size,
                sha256: object.sha256.clone(),
                url: format!("https://cdn.example/assets/{}", object.id),
            })
            .collect(),
    }
}

#[test]
fn lowers_all_declarations_and_packages_only_metadata_after_verified_receipt() {
    let mut builder = ProgressivePack::discover(fixture()).unwrap();
    let candidates = builder.candidates();
    assert_eq!(candidates.len(), 1);
    let candidate = candidates[0];
    let id = candidate.id.clone();
    assert_eq!(candidate.references.len(), 3);
    assert_eq!(candidate.references[2].line, 3);
    assert!(candidate.dependencies.is_empty());
    let plan = builder.prepare(&id).unwrap();
    assert!(plan.remote_objects.len() >= 2);
    let version = plan.asset_version.clone();
    let plan_id = plan.id.clone();
    let receipt = receipt(plan);
    assert_eq!(builder.prepare(&id).unwrap().id, plan_id);
    let key = aix::crypto::PrivateKey::from_seed([82; 32]);
    let output = builder
        .finalize(
            BTreeMap::from([(id.clone(), Decision::Progressive { plan_id, receipt })]),
            PackOptions {
                signing_key: Some(&key),
                ..PackOptions::new("build")
            },
            |_| Ok(()),
        )
        .unwrap();
    assert_eq!(output.progressive_assets[0].retained_source_bytes, 0);
    let reader = aix::AixReader::new(output.data.clone()).unwrap();
    reader.verify_signature(&key.public_key()).unwrap();
    let manifest = reader.get_manifest().unwrap().unwrap();
    let index = reader.read_file(INDEX_PATH).unwrap();
    assert_eq!(
        manifest.progressive_assets.unwrap().index_sha256,
        sha256(&index)
    );
    let init = reader
        .read_file(&format!("{PREFIX}{version}/init.mp4"))
        .unwrap();
    assert!(reader.read_file("assets/nested/clip.mp4").is_err());
    let app = String::from_utf8(reader.read_file("app.js").unwrap()).unwrap();
    assert!(!app.contains("progressive"));
    assert!(app.contains(&id));
    let page = String::from_utf8(reader.read_file("pages/index.ink").unwrap()).unwrap();
    assert!(page.contains(&format!("../{REF_DIR}{id}.js")));
    let module =
        String::from_utf8(reader.read_file(&format!("{REF_DIR}{id}.js")).unwrap()).unwrap();
    assert!(module.contains(&format!("aix-asset:{id}")));

    let optimized =
        crate::optimize_package(&output.data, &crate::OptimizeOptions::default()).unwrap();
    let reader = aix::AixReader::new(optimized.data).unwrap();
    assert_eq!(reader.read_file(INDEX_PATH).unwrap(), index);
    assert_eq!(
        reader
            .read_file(&format!("{PREFIX}{version}/init.mp4"))
            .unwrap(),
        init
    );
}

#[test]
fn embeds_the_complete_source_and_rejects_incomplete_or_modified_receipts() {
    let output = crate::pack(fixture(), PackOptions::new("embedded")).unwrap();
    let reader = aix::AixReader::new(output.data).unwrap();
    assert_eq!(
        reader.read_file("assets/nested/clip.mp4").unwrap(),
        include_bytes!("../../tests/data/pattern-tone-flat.mp4")
    );
    assert_eq!(output.progressive_assets[0].delivery, "embedded");
    for mutate in [
        (|r: &mut ProgressiveAssetUploadReceipt| r.objects.clear())
            as fn(&mut ProgressiveAssetUploadReceipt),
        |r| r.objects[0].size += 1,
        |r| r.objects[0].sha256.clear(),
        |r| r.asset_version.clear(),
        |r| r.objects[0].id = "unknown".into(),
        |r| r.objects[0].url = "not a URL".into(),
        |r| r.objects[0].url = "https://cdn.example/asset?secret=token".into(),
        |r| {
            let object = &r.objects[0];
            r.objects.push(UploadedObject {
                id: object.id.clone(),
                size: object.size,
                sha256: object.sha256.clone(),
                url: object.url.clone(),
            });
        },
    ] {
        let mut builder = ProgressivePack::discover(fixture()).unwrap();
        let id = builder.candidates()[0].id.clone();
        let plan = builder.prepare(&id).unwrap();
        let plan_id = plan.id.clone();
        let mut receipt = receipt(plan);
        mutate(&mut receipt);
        assert!(builder
            .finalize(
                BTreeMap::from([(id, Decision::Progressive { plan_id, receipt })]),
                PackOptions::new("invalid"),
                |_| Ok(())
            )
            .is_err());
    }
}

#[test]
fn computed_progressive_path_is_diagnosed_with_location() {
    let files = vec![InputFile::new(
        "app.js",
        b"\nimport(source, { with: { type: 'video', progressive: 'prefer' } });",
    )];
    let error = ProgressivePack::discover(files).err().unwrap();
    assert!(format!("{error:#}").contains("app.js:2:1: progressive import requires a literal path"));
}

#[test]
fn rejects_malformed_imports_and_escaping_paths_before_pack() {
    for (script, expected) in [
        (
            "import x from './clip' with { type: 'video', progressive: 'always' };",
            "expected progressive: 'prefer'",
        ),
        (
            "import x from './clip' with { type: 'image', progressive: 'prefer' };",
            "requires type 'audio' or 'video'",
        ),
        (
            "import x from './clip' with { type: 'video', progressive: 'prefer', extra: 'x' };",
            "expected only type and progressive",
        ),
        (
            "import('./clip', { ...options, with: { type: 'video', progressive: 'prefer' } });",
            "options must be a literal with object",
        ),
        (
            "import('./clip', { with: { ...attrs, progressive: 'prefer' } });",
            "options must be a literal with object",
        ),
        (
            "import('./clip', { with: { type: 'video', progressive: 'prefer', extra: enabled } });",
            "options must be a literal with object",
        ),
        (
            "import('./clip', { with: { type: 'video', progressive: enabled } });",
            "expected progressive: 'prefer'",
        ),
        (
            "import x from 'clip' with { type: 'video', progressive: 'prefer' };",
            "require a relative package path",
        ),
        (
            "import x from '../clip' with { type: 'video', progressive: 'prefer' };",
            "escapes the package root",
        ),
        (
            "import x from './media//clip' with { type: 'video', progressive: 'prefer' };",
            "empty resource path segment",
        ),
        (
            "import x from './.' with { type: 'video', progressive: 'prefer' };",
            "resource path is empty",
        ),
        (
            "import x from './media%5Cclip' with { type: 'video', progressive: 'prefer' };",
            "unsupported local resource URI",
        ),
        (
            "import x from './META-INF/clip' with { type: 'video', progressive: 'prefer' };",
            "points to generated metadata",
        ),
        (
            "import x from './missing' with { type: 'video', progressive: 'prefer' };",
            "missing progressive source 'missing'",
        ),
    ] {
        let error = ProgressivePack::discover(vec![InputFile::new("app.js", script.as_bytes())])
            .err()
            .unwrap();
        let message = format!("{error:#}");
        assert!(message.contains("app.js:1:"), "{script}: {message}");
        assert!(message.contains(expected), "{script}: {message}");
    }
    for (path, script, expected) in [
        (
            "app.ink",
            "<script setup>const progressive = ;</script>",
            "app.ink:",
        ),
        (
            "app.js",
            "export const broken = ;",
            "Failed to minify app.js",
        ),
    ] {
        let error = ProgressivePack::discover(vec![InputFile::new(path, script.as_bytes())])
            .err()
            .unwrap();
        assert!(format!("{error:#}").contains(expected));
    }
}

#[test]
fn preserves_ordinary_and_type_only_imports_while_lowering_progressive_exports() {
    let mut files = fixture();
    files.push(InputFile::new(
        "types.ts",
        br#"
export type { Clip } from './missing' with { type: 'video', progressive: 'prefer' };
import { type Clip as Other } from './also-missing' with { type: 'video', progressive: 'prefer' };
export * from './assets/nested/clip.mp4' with { "type": 'video', "progressive": 'prefer' };
export const ordinary = () => import('./assets/nested/clip.mp4', { with: 'asset', other: true });
"#,
    ));
    let mut pack = ProgressivePack::discover(files).unwrap();
    let candidate = pack.candidates()[0];
    assert_eq!(candidate.references.len(), 4);
    let id = candidate.id.clone();
    let plan = pack.prepare(&id).unwrap();
    let plan_id = plan.id.clone();
    let receipt = receipt(plan);
    let output = pack
        .finalize(
            BTreeMap::from([(id, Decision::Progressive { plan_id, receipt })]),
            PackOptions::new("retained"),
            |_| Ok(()),
        )
        .unwrap();
    let source = include_bytes!("../../tests/data/pattern-tone-flat.mp4");
    assert_eq!(
        output.progressive_assets[0].retained_source_bytes,
        source.len() as u64
    );
    let reader = aix::AixReader::new(output.data).unwrap();
    assert_eq!(reader.read_file("assets/nested/clip.mp4").unwrap(), source);
}

#[test]
fn requires_a_known_candidate_and_a_prepared_plan_for_remote_delivery() {
    let mut pack = ProgressivePack::discover(fixture()).unwrap();
    let id = pack.candidates()[0].id.clone();
    assert!(pack
        .prepare("unknown")
        .err()
        .unwrap()
        .to_string()
        .contains("unknown asset candidate"));
    let plan = pack.prepare(&id).unwrap();
    let plan_id = plan.id.clone();
    let receipt = receipt(plan);
    let error = ProgressivePack::discover(fixture())
        .unwrap()
        .finalize(
            BTreeMap::from([(id, Decision::Progressive { plan_id, receipt })]),
            PackOptions::new("unprepared"),
            |_| Ok(()),
        )
        .err()
        .unwrap();
    assert!(error.to_string().contains("no prepared plan"));
    let error = pack
        .finalize(
            BTreeMap::from([("unknown".into(), Decision::Embedded)]),
            PackOptions::new("unknown"),
            |_| Ok(()),
        )
        .err()
        .unwrap();
    assert!(error.to_string().contains("unknown asset"));
}

#[test]
fn unsupported_media_stays_embedded_and_cannot_be_prepared() {
    let mut files = fixture();
    files.last_mut().unwrap().data = b"not mp4".to_vec();
    let mut pack = ProgressivePack::discover(files).unwrap();
    let candidate = pack.candidates()[0];
    assert!(!candidate.progressive_support.supported);
    let id = candidate.id.clone();
    assert!(pack
        .prepare(&id)
        .err()
        .unwrap()
        .to_string()
        .contains("clip.mp4"));
    let output = pack
        .finalize(BTreeMap::new(), PackOptions::new("fallback"), |_| Ok(()))
        .unwrap();
    let reader = aix::AixReader::new(output.data).unwrap();
    assert_eq!(
        reader.read_file("assets/nested/clip.mp4").unwrap(),
        b"not mp4"
    );
}

fn iso_box(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let size = u32::try_from(8 + payload.len()).unwrap();
    [size.to_be_bytes().as_slice(), kind, payload].concat()
}

fn box_words(kind: &[u8; 4], words: &[u32]) -> Vec<u8> {
    let payload: Vec<_> = words.iter().flat_map(|word| word.to_be_bytes()).collect();
    iso_box(kind, &payload)
}

fn initialization(track_defaults: &[Vec<u8>]) -> Vec<u8> {
    [
        iso_box(b"ftyp", b"isom\0\0\0\0"),
        iso_box(b"moov", &iso_box(b"mvex", &track_defaults.concat())),
    ]
    .concat()
}

#[test]
fn box_reader_bounds_sizes_and_box_count() {
    use fragment_boxes::boxes;

    let extended = [b"\0\0\0\x01free".as_slice(), &16u64.to_be_bytes()].concat();
    let parsed = boxes(&extended).unwrap();
    assert_eq!(parsed[0].kind, *b"free");
    assert!(parsed[0].payload.is_empty());
    for (data, error) in [
        (extended[..7].to_vec(), "Truncated ISO BMFF header"),
        (extended[..8].to_vec(), "Truncated ISO BMFF extended size"),
        (b"\0\0\0\0free".to_vec(), "Open ended ISO BMFF boxes"),
        (b"\0\0\0\x10free".to_vec(), "Invalid ISO BMFF box length"),
        (
            iso_box(b"free", &[]).repeat(8193),
            "Too many ISO BMFF boxes",
        ),
    ] {
        assert!(boxes(&data).err().unwrap().contains(error));
    }
}

#[test]
fn initialization_requires_unique_supported_track_defaults() {
    use fragment_boxes::defaults;

    let good = box_words(b"trex", &[0, 1, 1, 1024, 2, 0]);
    assert_eq!(
        defaults(&initialization(std::slice::from_ref(&good)))
            .unwrap()
            .len(),
        1
    );
    for (init, error) in [
        (iso_box(b"moov", &[]), "must contain ftyp and moov"),
        (
            initialization(&[iso_box(b"free", &[])]),
            "Expected one or two",
        ),
        (
            initialization(&[box_words(b"trex", &[1, 1, 1, 1024, 2, 0])]),
            "Invalid trex version",
        ),
        (
            initialization(&[box_words(b"trex", &[0, 1, 2, 1024, 2, 0])]),
            "Unsupported sample description",
        ),
        (
            initialization(&[good.clone(), good]),
            "Invalid duplicate fragment track defaults",
        ),
    ] {
        assert!(defaults(&init).err().unwrap().contains(error));
    }
}

#[test]
fn fragmented_samples_require_valid_addressing_timestamps_and_payload() {
    use fragment_boxes::{defaults, fragment};

    let init = initialization(&[box_words(b"trex", &[0, 1, 1, 1024, 2, 0])]);
    let defaults = defaults(&init).unwrap();
    let header = box_words(b"tfhd", &[0x0002_0000, 1]);
    let decode_time = box_words(b"tfdt", &[0, 256]);
    let offset = u32::try_from(8 + 8 + header.len() + decode_time.len() + 20 + 8).unwrap();
    let run = box_words(b"trun", &[1, 1, offset]);
    let frame = |boxes: &[&[u8]], payload: &[u8]| {
        [
            iso_box(b"moof", &iso_box(b"traf", &boxes.concat())),
            iso_box(b"mdat", payload),
        ]
        .concat()
    };
    let valid = frame(&[&header, &decode_time, &run], &[0x21, 0x10]);
    let sample = fragment(&valid, &defaults).unwrap().pop().unwrap();
    assert_eq!(sample.track, 1);
    assert_eq!(sample.dts, 256);
    assert_eq!(sample.duration, 1024);
    assert!(sample.key);
    assert_eq!(&valid[sample.data], &[0x21, 0x10]);

    for (data, error) in [
        (iso_box(b"moof", &[]), "must contain one moof and one mdat"),
        (
            frame(
                &[&header, &decode_time, &run, &iso_box(b"senc", &[])],
                &[0; 2],
            ),
            "Unsupported fragment track box",
        ),
        (
            frame(&[&box_words(b"tfhd", &[0, 1]), &decode_time, &run], &[0; 2]),
            "default-base-is-moof addressing",
        ),
        (
            frame(
                &[&box_words(b"tfhd", &[0x0002_0000, 2]), &decode_time, &run],
                &[0; 2],
            ),
            "Unknown fragment track",
        ),
        (
            frame(
                &[&header, &box_words(b"tfdt", &[0x0200_0000, 0]), &run],
                &[0; 2],
            ),
            "Unsupported tfdt version",
        ),
        (
            frame(&[&header, &decode_time], &[0; 2]),
            "Incomplete fragment track",
        ),
        (
            frame(
                &[
                    &header,
                    &decode_time,
                    &box_words(b"trun", &[0x0200_0001, 1, offset]),
                ],
                &[0; 2],
            ),
            "Unsupported fragment sample run",
        ),
        (
            frame(
                &[
                    &header,
                    &decode_time,
                    &box_words(b"trun", &[1, 8193, offset]),
                ],
                &[0; 2],
            ),
            "sample budget",
        ),
        (
            frame(
                &[
                    &header,
                    &decode_time,
                    &box_words(b"trun", &[1, 1, u32::MAX]),
                ],
                &[0; 2],
            ),
            "Invalid fragment data offset",
        ),
        (
            frame(&[&header, &decode_time, &run], &[0; 1]),
            "outside the media payload",
        ),
        (
            frame(&[&header, &decode_time, &run], &[0; 3]),
            "unreferenced media bytes",
        ),
    ] {
        assert!(
            fragment(&data, &defaults).err().unwrap().contains(error),
            "{error}"
        );
    }
}

#[test]
fn prepares_muxed_and_audio_fragments_without_embedding_segment_zero() {
    let video = include_bytes!("../../tests/data/blue-tone-fragmented.mp4").as_slice();
    let audio = include_bytes!("../../tests/data/tone-fragmented.m4a").as_slice();
    for (kind, path, bytes, track_count) in [
        ("video", "clip.mp4", video, 2),
        (
            "video",
            "ordinary.mp4",
            include_bytes!("../../tests/data/pattern-tone-flat.mp4").as_slice(),
            2,
        ),
        (
            "audio",
            "ordinary.m4a",
            include_bytes!("../../tests/data/tone-flat.m4a").as_slice(),
            1,
        ),
        ("audio", "clip.m4a", audio, 1),
    ] {
        let files = vec![
            InputFile::new("app.json", br#"{"pages":["index"]}"#),
            InputFile::new("app.js", format!("import clip from './{path}' with {{ type: '{kind}', progressive: 'prefer' }}; export default {{ clip }};").into_bytes()),
            InputFile::new(path, bytes),
        ];
        let mut pack = ProgressivePack::discover(files).unwrap();
        let candidate = pack.candidates()[0];
        assert!(
            candidate.progressive_support.supported,
            "{:?}",
            candidate.progressive_support.reason
        );
        let id = candidate.id.clone();
        let plan = pack.prepare(&id).unwrap();
        let layout = &plan.layout;
        assert_eq!(layout.tracks.len(), track_count);
        assert!(layout.fragments.len() >= 2);
        assert_eq!(layout.fragments.len(), plan.remote_objects.len());
        let receipt = receipt(plan);
        let version = plan.asset_version.clone();
        let plan_id = plan.id.clone();
        let output = pack
            .finalize(
                BTreeMap::from([(id, Decision::Progressive { plan_id, receipt })]),
                PackOptions::new("media"),
                |_| Ok(()),
            )
            .unwrap();
        let reader = aix::AixReader::new(output.data).unwrap();
        let descriptor_bytes = reader
            .read_file(&format!("{PREFIX}{version}/asset.json"))
            .unwrap();
        let descriptor: Descriptor = serde_json::from_slice(&descriptor_bytes).unwrap();
        assert_eq!(
            descriptor.asset_version,
            sha256(&descriptor.profile.canonical_bytes().unwrap())
        );
        let init = reader
            .read_file(&format!("{PREFIX}{version}/init.mp4"))
            .unwrap();
        let boxes = fragment_boxes::boxes(&init).unwrap();
        assert_eq!(
            boxes.iter().map(|b| &b.kind).collect::<Vec<_>>(),
            [b"ftyp", b"moov"]
        );
        assert!(reader.read_file(path).is_err());
        assert!(reader
            .list()
            .iter()
            .all(|entry| !entry.name.ends_with(".m4s")));
    }
}
