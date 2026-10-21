#![allow(dead_code)]
use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};
use wordcraft_doc::{Document, Paragraph, para_block, props::CharProps};

pub fn base() -> Document {
    let mut d = Document::new();
    d.core.title = "TitleKeep".into();
    d.core.language = "fr-CA".into();
    d.core.created = "2020-01-02T03:04:05Z".into();
    d.core.modified = "2021-02-03T04:05:06Z".into();
    d.body = vec![para_block(Paragraph::with_text(
        "MetadataKeep",
        CharProps { link: Some("https://example.test/metadata-keep".into()), lang: Some("de-DE".into()), ..Default::default() },
    ))];
    d
}

pub fn files(bytes: &[u8]) -> BTreeMap<String, Vec<u8>> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    (0..zip.len())
        .map(|i| {
            let mut file = zip.by_index(i).unwrap();
            let mut data = Vec::new();
            file.read_to_end(&mut data).unwrap();
            (file.name().to_owned(), data)
        })
        .collect()
}

pub fn imported(inner: &str) -> Document {
    let mut entries = files(&wordcraft_docx::write(&base()).unwrap());
    entries.insert(
        "docProps/app.xml".into(),
        format!(r#"<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">{inner}</Properties>"#).into_bytes(),
    );
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in entries {
        zip.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(&data).unwrap();
    }
    wordcraft_docx::read(&zip.finish().unwrap().into_inner()).unwrap()
}

pub fn app(document: &Document) -> String {
    String::from_utf8(files(&wordcraft_docx::write(document).unwrap()).remove("docProps/app.xml").unwrap()).unwrap()
}
