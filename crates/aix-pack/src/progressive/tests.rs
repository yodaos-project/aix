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
        |r| r.objects[0].url = "https://cdn.example/asset?secret=token".into(),
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
