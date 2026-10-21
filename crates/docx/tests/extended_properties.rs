//! Public typed API acceptance; imports intentionally fail on the prior API.
#[path = "support/extended_fixture.rs"]
mod fixture;
use wordcraft_doc::StoryRef;
use wordcraft_docx::{ExtendedProperties, ExtendedPropertyPatch as P, extended_properties, patch_extended_properties};

#[test]
fn typed_extended_properties_roundtrip_all_fields_and_touch_only_the_app_part() {
    let mut document = fixture::base();
    assert_eq!(extended_properties(&document).unwrap(), ExtendedProperties::default());
    let mut control = document.clone();
    let original_bytes = wordcraft_docx::write(&document).unwrap();
    let original = document.clone();
    patch_extended_properties(
        &mut document,
        &[
            P::Template(Some("Template🌿<&>".into())),
            P::Manager(Some("".into())),
            P::Company(Some(" Company Keep ".into())),
            P::HyperlinkBase(Some("https://example.test/base?x=1&y=2".into())),
            P::ScaleCrop(Some(true)),
            P::LinksUpToDate(Some(false)),
            P::SharedDoc(Some(true)),
            P::HyperlinksChanged(Some(false)),
        ],
    )
    .unwrap();
    let expected = ExtendedProperties {
        template: Some("Template🌿<&>".into()),
        manager: Some("".into()),
        company: Some(" Company Keep ".into()),
        hyperlink_base: Some("https://example.test/base?x=1&y=2".into()),
        scale_crop: Some(true),
        links_up_to_date: Some(false),
        shared_doc: Some(true),
        hyperlinks_changed: Some(false),
    };
    assert_eq!(extended_properties(&document).unwrap(), expected);
    let mut unchanged = document.clone();
    unchanged.passthrough = original.passthrough.clone();
    assert_eq!(unchanged, original);
    let clear = [
        P::Template(None),
        P::Manager(None),
        P::Company(None),
        P::HyperlinkBase(None),
        P::ScaleCrop(None),
        P::LinksUpToDate(None),
        P::SharedDoc(None),
        P::HyperlinksChanged(None),
    ];
    let mut immediate_clear = document.clone();
    patch_extended_properties(&mut immediate_clear, &clear).unwrap();
    assert_eq!(immediate_clear, original);
    assert_eq!(wordcraft_docx::write(&immediate_clear).unwrap(), original_bytes);
    for _ in 0..3 {
        let control_bytes = wordcraft_docx::write(&control).unwrap();
        let before = fixture::files(&control_bytes);
        let bytes = wordcraft_docx::write(&document).unwrap();
        let after = fixture::files(&bytes);
        assert_eq!(before.keys().collect::<Vec<_>>(), after.keys().collect::<Vec<_>>());
        assert_eq!(
            before.iter().filter(|(key, value)| after.get(*key) != Some(*value)).map(|(key, _)| key.as_str()).collect::<Vec<_>>(),
            vec!["docProps/app.xml"]
        );
        let xml = String::from_utf8(after.get("docProps/app.xml").unwrap().clone()).unwrap();
        for text in ["<Manager></Manager>", "<ScaleCrop>true</ScaleCrop>", "<LinksUpToDate>false</LinksUpToDate>", "Template🌿&lt;&amp;&gt;"] {
            assert!(xml.contains(text), "{xml}");
        }
        assert_eq!(document.core, control.core);
        assert_eq!(document.body, control.body);
        assert_eq!(document.styles, control.styles);
        let reopened = wordcraft_docx::read(&bytes).unwrap();
        assert_eq!(extended_properties(&reopened).unwrap(), expected);
        assert_eq!(reopened.plain_text(StoryRef::Body), "MetadataKeep");
        assert_eq!(
            reopened.para(StoryRef::Body, &wordcraft_doc::Path::top(0)).unwrap().runs[0].props.link.as_deref(),
            Some("https://example.test/metadata-keep")
        );
        let flat = wordcraft_docx::write_flat(&reopened).unwrap();
        assert_eq!(extended_properties(&wordcraft_docx::read_xml(&flat).unwrap()).unwrap(), expected);
        document = reopened;
        control = wordcraft_docx::read(&control_bytes).unwrap();
    }
    patch_extended_properties(&mut document, &clear).unwrap();
    assert_eq!(extended_properties(&document).unwrap(), ExtendedProperties::default());
    assert_eq!(wordcraft_docx::write(&document).unwrap(), wordcraft_docx::write(&control).unwrap());
}

#[test]
fn imported_lexical_values_noops_copies_and_privacy_remain_native_policies() {
    let mut document = fixture::imported("<Company>Keep</Company><ScaleCrop>1</ScaleCrop><Manager></Manager><SharedDoc>0</SharedDoc>");
    let snapshot = extended_properties(&document).unwrap();
    assert_eq!(snapshot.company.as_deref(), Some("Keep"));
    assert_eq!(snapshot.manager.as_deref(), Some(""));
    assert_eq!(snapshot.scale_crop, Some(true));
    assert_eq!(snapshot.shared_doc, Some(false));
    let before = wordcraft_docx::write(&document).unwrap();
    let copy = document.clone();
    for changes in [vec![], vec![P::ScaleCrop(Some(true)), P::SharedDoc(Some(false)), P::Company(Some("Keep".into())), P::Template(None)]] {
        patch_extended_properties(&mut document, &changes).unwrap();
        assert_eq!(wordcraft_docx::write(&document).unwrap(), before);
    }
    patch_extended_properties(&mut document, &[P::Template(Some("New🌿".into())), P::ScaleCrop(Some(false))]).unwrap();
    assert_eq!(wordcraft_docx::write(&copy).unwrap(), before);
    assert_eq!(extended_properties(&copy).unwrap(), snapshot);
    let app = fixture::app(&document);
    assert!(app.contains("<ScaleCrop>false</ScaleCrop>"));
    assert!(app.contains("<SharedDoc>0</SharedDoc>"));
    assert!(app.find("<Company>").unwrap() < app.find("<ScaleCrop>").unwrap());
    assert!(app.find("<SharedDoc>").unwrap() < app.find("<Template>").unwrap());
    document.settings.remove_personal_info = true;
    assert_eq!(extended_properties(&document).unwrap().company.as_deref(), Some("Keep"));
    let bytes = wordcraft_docx::write(&document).unwrap();
    let app = fixture::app(&document);
    assert!(!app.contains("Company") && !app.contains("Manager"));
    assert!(
        app.contains("<Application>WordCraft</Application>")
            && app.contains("<Words>1</Words>")
            && app.contains("<Paragraphs>1</Paragraphs>")
            && app.contains("<DocSecurity>0</DocSecurity>")
    );
    let reopened = wordcraft_docx::read(&bytes).unwrap();
    let read = extended_properties(&reopened).unwrap();
    assert_eq!(read.company, None);
    assert_eq!(read.manager, None);
    assert_eq!(read.template.as_deref(), Some("New🌿"));
    assert_eq!(reopened.plain_text(StoryRef::Body), "MetadataKeep");
}
