//! Identical baseline/candidate diagnostics, not fixed default-reader/writer behavior.
#[path = "support/extended_fixture.rs"]
mod fixture;

#[test]
fn default_writer_retains_legacy_duplicate_first_eight_and_scalar_omission_semantics() {
    let duplicate = "<Template>A</Template><Template>B</Template><Manager>M</Manager><HyperlinkBase>H</HyperlinkBase><ScaleCrop>1</ScaleCrop><LinksUpToDate>0</LinksUpToDate><SharedDoc>true</SharedDoc><HyperlinksChanged>false</HyperlinksChanged><Company>NinthKeep</Company>";
    let invalid = "<ScaleCrop>not-a-bool</ScaleCrop><LinksUpToDate> true </LinksUpToDate><Company></Company><Pages>500</Pages><Application>OldProducer</Application>";
    let omitted = format!(
        "<Company>{}</Company><Manager>has&#9;tab</Manager><Template>has&#10;line</Template><Unknown>Ignore</Unknown><HyperlinkBase>Keep</HyperlinkBase>",
        "x".repeat(2049)
    );
    for (label, source) in [("duplicate", duplicate), ("invalid", invalid), ("omitted", omitted.as_str())] {
        let document = fixture::imported(source);
        let xml = fixture::app(&document);
        assert_eq!(document.plain_text(wordcraft_doc::StoryRef::Body), "MetadataKeep");
        match label {
            "duplicate" => {
                assert_eq!(xml.matches("<Template>").count(), 2);
                assert!(!xml.contains("NinthKeep") && !xml.contains("<Company>"));
                assert!(xml.contains("<HyperlinksChanged>false</HyperlinksChanged>"));
            }
            "invalid" => {
                assert!(xml.contains("<ScaleCrop>not-a-bool</ScaleCrop>") && xml.contains("<LinksUpToDate> true </LinksUpToDate>"));
                assert!(!xml.contains("<Pages>") && !xml.contains("OldProducer"));
            }
            _ => {
                assert!(!xml.contains("<Company>") && !xml.contains("<Manager>") && !xml.contains("<Template>") && !xml.contains("<Unknown>"));
                assert!(xml.contains("<HyperlinkBase>Keep</HyperlinkBase>"));
            }
        }
        if let Ok(directory) = std::env::var("EXTENDED_LEGACY_EVIDENCE_DIR") {
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(std::path::Path::new(&directory).join(format!("{label}.docx")), wordcraft_docx::write(&document).unwrap()).unwrap();
            std::fs::write(std::path::Path::new(&directory).join(format!("{label}.xml")), xml).unwrap();
        }
    }
}
