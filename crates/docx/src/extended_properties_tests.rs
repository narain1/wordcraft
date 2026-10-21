use super::*;

fn document(bytes: &[u8]) -> Document {
    let mut document = Document::from_text("RetainedKeep");
    document.passthrough.insert(APP_KEPT.into(), Arc::new(bytes.to_vec()));
    document.passthrough.insert("other-kept".into(), Arc::new(vec![1, 2, 3]));
    document
}

fn rejects_atomically(mut document: Document, patches: &[ExtendedPropertyPatch], expected: ExtendedPropertiesError) {
    let before = document.clone();
    let shared = document.passthrough.get(APP_KEPT).cloned();
    assert_eq!(patch_extended_properties(&mut document, patches), Err(expected));
    assert_eq!(document.clone(), before);
    if let Some(shared) = shared {
        assert!(Arc::ptr_eq(&shared, document.passthrough.get(APP_KEPT).unwrap()));
    }
}

#[test]
fn invalid_retained_profile_is_refused_even_for_empty_noop_and_clear_patches() {
    use ExtendedPropertiesError as E;
    let cases: Vec<(Vec<u8>, E)> = vec![
        (vec![0xff], E::MalformedState),
        (b"Company".to_vec(), E::MalformedState),
        (b"\n".to_vec(), E::MalformedState),
        (b"Company\tx\n\n".to_vec(), E::MalformedState),
        (b"Unknown\tx\n".to_vec(), E::UnsupportedProperty),
        (b"Company\tx\nCompany\ty\n".to_vec(), E::DuplicateProperty("Company")),
        (b"Company\tx\ty\n".to_vec(), E::InvalidValue("Company")),
        (b"Company\tx\r\n".to_vec(), E::InvalidValue("Company")),
        (b"ScaleCrop\t\n".to_vec(), E::InvalidValue("ScaleCrop")),
        (b"ScaleCrop\t true \n".to_vec(), E::InvalidValue("ScaleCrop")),
        (b"ScaleCrop\tTRUE\n".to_vec(), E::InvalidValue("ScaleCrop")),
        (b"ScaleCrop\t2\n".to_vec(), E::InvalidValue("ScaleCrop")),
        ("Company\t\u{fffe}\n".as_bytes().to_vec(), E::InvalidValue("Company")),
        ("Company\t\u{ffff}\n".as_bytes().to_vec(), E::InvalidValue("Company")),
    ];
    for (bytes, error) in cases {
        let document = document(&bytes);
        assert_eq!(extended_properties(&document), Err(error.clone()));
        for patches in [vec![], vec![ExtendedPropertyPatch::Template(None)], vec![ExtendedPropertyPatch::Company(None)]] {
            rejects_atomically(document.clone(), &patches, error.clone());
        }
    }
    for bad in ["\0", "\t", "\n", "\r", "\u{85}", "\u{fffe}", "\u{ffff}"] {
        rejects_atomically(document(b"Company\tKeep\n"), &[ExtendedPropertyPatch::Company(Some(bad.into()))], E::InvalidValue("Company"));
    }
}

#[test]
fn exact_value_record_patch_and_coarse_raw_bounds_precede_allocation() {
    use ExtendedPropertiesError as E;
    let text = "🌿".repeat(MAX_VALUE_BYTES / 4);
    let mut document = document(format!("Template\t{text}\nManager\t\nCompany\tKeep\nHyperlinkBase\thttps://example.test/\nScaleCrop\t1\nLinksUpToDate\t0\nSharedDoc\ttrue\nHyperlinksChanged\tfalse\n").as_bytes());
    let snapshot = extended_properties(&document).unwrap();
    assert_eq!(snapshot.template.as_deref(), Some(text.as_str()));
    assert_eq!(snapshot.manager.as_deref(), Some(""));
    assert_eq!(
        (snapshot.scale_crop, snapshot.links_up_to_date, snapshot.shared_doc, snapshot.hyperlinks_changed),
        (Some(true), Some(false), Some(true), Some(false))
    );
    let exact_patches = [
        ExtendedPropertyPatch::Template(Some(text.clone())),
        ExtendedPropertyPatch::Manager(None),
        ExtendedPropertyPatch::Company(None),
        ExtendedPropertyPatch::HyperlinkBase(None),
        ExtendedPropertyPatch::ScaleCrop(None),
        ExtendedPropertyPatch::LinksUpToDate(None),
        ExtendedPropertyPatch::SharedDoc(None),
        ExtendedPropertyPatch::HyperlinksChanged(None),
    ];
    patch_extended_properties(&mut document, &exact_patches).unwrap();
    assert_eq!(extended_properties(&document).unwrap().template.as_deref(), Some(text.as_str()));
    let oversized = format!("{text}x");
    rejects_atomically(document.clone(), &[ExtendedPropertyPatch::Template(Some(oversized.clone()))], E::Limit);
    assert_eq!(extended_properties(&self::document(format!("Template\t{oversized}\n").as_bytes())), Err(E::Limit));
    let all = b"Template\tx\nManager\tx\nCompany\tx\nHyperlinkBase\tx\nScaleCrop\t0\nLinksUpToDate\t0\nSharedDoc\t0\nHyperlinksChanged\t0\n";
    let mut ninth = all.to_vec();
    ninth.extend_from_slice(b"Company\ttail\n");
    rejects_atomically(self::document(&ninth), &[], E::Limit);
    let mut nine_patches = exact_patches.to_vec();
    nine_patches.push(ExtendedPropertyPatch::Company(None));
    rejects_atomically(document.clone(), &nine_patches, E::Limit);
    rejects_atomically(document, &[ExtendedPropertyPatch::Company(None), ExtendedPropertyPatch::Company(None)], E::DuplicatePatch("Company"));
    // The coarse raw cap is separate from stricter scalar/record rules: exact cap
    // reaches structural validation; cap+1 is rejected before UTF8/line parsing.
    let cap = max_retained_bytes().unwrap();
    assert_eq!(extended_properties(&self::document(&vec![b'x'; cap])), Err(E::MalformedState));
    assert_eq!(extended_properties(&self::document(&vec![0xff; cap + 1])), Err(E::Limit));
}

#[test]
fn semantic_noops_preserve_bytes_order_arcs_and_edits_are_copy_isolated() {
    let raw = b"Company\t Keep \nScaleCrop\t1\nManager\t\nSharedDoc\t0";
    let mut document = document(raw);
    let original = document.passthrough.get(APP_KEPT).unwrap().clone();
    let other = document.passthrough.get("other-kept").unwrap().clone();
    let copy = document.clone();
    let model = document.clone();
    for patches in [
        vec![],
        vec![ExtendedPropertyPatch::ScaleCrop(Some(true))],
        vec![ExtendedPropertyPatch::Company(Some(" Keep ".into()))],
        vec![ExtendedPropertyPatch::Template(None)],
    ] {
        patch_extended_properties(&mut document, &patches).unwrap();
        assert!(Arc::ptr_eq(&original, document.passthrough.get(APP_KEPT).unwrap()));
        assert_eq!(document.clone(), model);
    }
    patch_extended_properties(
        &mut document,
        &[ExtendedPropertyPatch::Company(Some("New🌿".into())), ExtendedPropertyPatch::HyperlinksChanged(Some(true))],
    )
    .unwrap();
    assert_eq!(
        document.passthrough.get(APP_KEPT).unwrap().as_slice(),
        "Company\tNew🌿\nScaleCrop\t1\nManager\t\nSharedDoc\t0\nHyperlinksChanged\ttrue\n".as_bytes()
    );
    assert_eq!(copy.passthrough.get(APP_KEPT).unwrap().as_slice(), raw);
    assert!(Arc::ptr_eq(&other, document.passthrough.get("other-kept").unwrap()));
    let mut stripped = document.clone();
    stripped.passthrough = copy.passthrough.clone();
    assert_eq!(stripped, model);
    let clear = [
        ExtendedPropertyPatch::Template(None),
        ExtendedPropertyPatch::Manager(None),
        ExtendedPropertyPatch::Company(None),
        ExtendedPropertyPatch::HyperlinkBase(None),
        ExtendedPropertyPatch::ScaleCrop(None),
        ExtendedPropertyPatch::LinksUpToDate(None),
        ExtendedPropertyPatch::SharedDoc(None),
        ExtendedPropertyPatch::HyperlinksChanged(None),
    ];
    patch_extended_properties(&mut document, &clear).unwrap();
    assert!(!document.passthrough.contains_key(APP_KEPT));
    assert_eq!(extended_properties(&document).unwrap(), ExtendedProperties::default());
    assert!(Arc::ptr_eq(&other, document.passthrough.get("other-kept").unwrap()));
    for raw in [b"".as_slice(), b"Company\t\n"] {
        let mut empty = self::document(raw);
        let previous = empty.passthrough.get(APP_KEPT).unwrap().clone();
        patch_extended_properties(&mut empty, &[]).unwrap();
        assert!(Arc::ptr_eq(&previous, empty.passthrough.get(APP_KEPT).unwrap()));
    }
}
