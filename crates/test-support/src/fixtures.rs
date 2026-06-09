use camino::Utf8PathBuf;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

// ── PDF builders ──

struct PdfBuilder {
    buf: Vec<u8>,
    offsets: Vec<usize>,
    obj_count: u32,
}

impl PdfBuilder {
    fn new() -> Self {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"%PDF-1.4\n");
        Self {
            offsets: vec![0],
            buf,
            obj_count: 0,
        }
    }

    fn next_obj(&mut self) -> u32 {
        self.obj_count += 1;
        self.obj_count
    }

    fn write_obj(&mut self, num: u32, data: &str) {
        self.offsets.push(self.buf.len());
        write!(self.buf, "{num} 0 obj\n{data}\nendobj\n")
            .expect("write to Vec should not fail");
    }

    fn write_stream(&mut self, data: &[u8]) {
        let len = data.len();
        write!(self.buf, "<< /Length {len} >>\nstream\n")
            .expect("write to Vec should not fail");
        self.buf.extend_from_slice(data);
        self.buf.extend_from_slice(b"\nendstream\n");
    }

    fn finish(mut self) -> Vec<u8> {
        let xref_offset = self.buf.len();
        writeln!(self.buf, "xref\n0 {}", self.offsets.len())
            .expect("write to Vec should not fail");
        writeln!(self.buf, "{:010} {:05} f ", 0, 65535)
            .expect("write to Vec should not fail");
        for &offset in &self.offsets[1..] {
            writeln!(self.buf, "{:010} {:05} n ", offset, 0)
                .expect("write to Vec should not fail");
        }
        write!(
            self.buf,
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            self.offsets.len(),
            xref_offset
        )
        .expect("write to Vec should not fail");
        self.buf
    }
}

fn escape_pdf_string(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '(' => escaped.push_str("\\("),
            ')' => escaped.push_str("\\)"),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            c if (c as u32) < 32 => {
                escaped.push_str(&format!("\\{:03o}", c as u32));
            }
            c => escaped.push(c),
        }
    }
    escaped
}

/// Write a minimal valid PDF with the given text content.
pub fn write_minimal_pdf(path: &Path, text: &str) -> io::Result<()> {
    let mut pdf = PdfBuilder::new();

    let catalog = pdf.next_obj();
    let pages_obj = pdf.next_obj();
    let page_obj = pdf.next_obj();
    let _stream_obj = pdf.next_obj();
    let font_obj = pdf.next_obj();

    pdf.write_obj(catalog, "<< /Type /Catalog /Pages 2 0 R >>");
    pdf.write_obj(pages_obj, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>");
    pdf.write_obj(
        page_obj,
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
         /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>",
    );

    let escaped = escape_pdf_string(text);
    let stream_data = format!("BT /F1 12 Tf 100 700 Td ({escaped}) Tj ET");
    pdf.write_stream(stream_data.as_bytes());

    pdf.write_obj(font_obj, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>");

    fs::write(path, pdf.finish())
}

/// Write random bytes to a `.pdf` path (not parseable as PDF).
pub fn write_malformed_pdf(path: &Path) -> io::Result<()> {
    fs::write(path, b"not a pdf file content")
}

/// Write a valid PDF whose page streams are corrupt (parseable structure, unreadable content).
pub fn write_corrupt_stream_pdf(path: &Path) -> io::Result<()> {
    let mut pdf = PdfBuilder::new();

    let _catalog = pdf.next_obj();
    let _pages = pdf.next_obj();
    let _page = pdf.next_obj();
    let _stream_obj = pdf.next_obj();

    pdf.write_obj(1, "<< /Type /Catalog /Pages 2 0 R >>");
    pdf.write_obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>");
    pdf.write_obj(
        3,
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
         /Contents 4 0 R /Resources << >> >>",
    );

    // Stream data that is not valid PDF content stream operators
    pdf.write_stream(b"not a valid content stream \xff\xfe garbage");

    fs::write(path, pdf.finish())
}

/// Write a valid PDF with no selectable text (image-only / blank pages).
pub fn write_image_only_pdf(path: &Path) -> io::Result<()> {
    let mut pdf = PdfBuilder::new();

    pdf.write_obj(1, "<< /Type /Catalog /Pages 2 0 R >>");
    pdf.write_obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>");
    pdf.write_obj(
        3,
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
         /Contents 4 0 R /Resources << /XObject << /Im0 5 0 R >> >> >>",
    );

    // Minimal empty content stream (does nothing)
    pdf.write_stream(b"q Q");

    // Dummy image XObject (minimal JPEG-like stream header)
    let dummy_jpeg: Vec<u8> = vec![
        0xff, 0xd8, 0xff, 0xe0, 0x00, 0x10, 0x4a, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x00,
        0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0xff, 0xdb, 0x00, 0x43, 0x00, 0x03, 0x02, 0x02,
        0x02, 0x02, 0x02, 0x03, 0x02, 0x02, 0x02, 0x03, 0x03, 0x03, 0x03, 0x04, 0x06, 0x04,
        0x04, 0x04, 0x04, 0x04, 0x08, 0x06, 0x06, 0x05, 0x06, 0x09, 0x08, 0x0a, 0x0a, 0x09,
        0x08, 0x09, 0x09, 0x0a, 0x0c, 0x0f, 0x0c, 0x0a, 0x0b, 0x0e, 0x0b, 0x09, 0x09, 0x0d,
        0x11, 0x0d, 0x0e, 0x0f, 0x10, 0x10, 0x11, 0x10, 0x0a, 0x0c, 0x12, 0x13, 0x12, 0x10,
        0x13, 0x0f, 0x10, 0x10, 0x10, 0xff, 0xc9, 0x00, 0x0b, 0x08, 0x00, 0x01, 0x00, 0x01,
        0x01, 0x01, 0x11, 0x00, 0xff, 0xcc, 0x00, 0x06, 0x00, 0x10, 0x10, 0x05, 0xff, 0xda,
        0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3f,
    ];
    let len = dummy_jpeg.len();
    write!(
        pdf.buf,
        "5 0 obj\n<< /Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace \
         /DeviceGray /BitsPerComponent 8 /Length {len} >>\nstream\n"
    )
    .expect("write to Vec should not fail");
    pdf.buf.extend_from_slice(&dummy_jpeg);
    pdf.buf.extend_from_slice(b"\nendstream\nendobj\n");

    fs::write(path, pdf.finish())
}

// ── DOCX builders ──

fn make_docx_xml(text: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r>
        <w:t>{}</w:t>
      </w:r>
    </w:p>
  </w:body>
</w:document>"#,
        text
    )
}

fn write_docx_zip(path: &Path, xml_content: &str) -> io::Result<()> {
    let file = fs::File::create(path)?;
    let mut zip = zip::ZipWriter::new(file);

    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    zip.add_directory("word/", options)?;
    zip.add_directory("_rels/", options)?;
    zip.add_directory("word/_rels/", options)?;

    zip.start_file(
        "[Content_Types].xml",
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
    )?;
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#,
    )?;

    zip.start_file(
        "_rels/.rels",
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
    )?;
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#,
    )?;

    zip.start_file(
        "word/_rels/document.xml.rels",
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
    )?;
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
</Relationships>"#,
    )?;

    zip.start_file("word/document.xml", options)?;
    zip.write_all(xml_content.as_bytes())?;

    zip.finish()?;
    Ok(())
}

/// Write a minimal valid DOCX with the given text content.
pub fn write_minimal_docx(path: &Path, text: &str) -> io::Result<()> {
    write_docx_zip(path, &make_docx_xml(text))
}

/// Write random bytes to a `.docx` path (not a valid zip).
pub fn write_malformed_docx(path: &Path) -> io::Result<()> {
    fs::write(path, b"not a zip file content")
}

/// Write a valid zip that is missing `word/document.xml`.
pub fn write_docx_missing_xml(path: &Path) -> io::Result<()> {
    let file = fs::File::create(path)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    zip.add_directory("word/", options)?;
    zip.start_file(
        "some_other_file.xml",
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
    )?;
    zip.write_all(b"<root />")?;
    zip.finish()?;
    Ok(())
}

/// Write a valid zip with malformed XML as the document entry.
pub fn write_docx_malformed_xml(path: &Path) -> io::Result<()> {
    write_docx_zip(path, "<w:document><w:body><w:p><w:r><w:t>unclosed")
}

// ── ODT builders ──

fn make_odt_xml(text: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content
    xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
    xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
    office:version="1.2">
  <office:body>
    <office:text>
      <text:p>{}</text:p>
    </office:text>
  </office:body>
</office:document-content>"#,
        text
    )
}

fn make_odt_span_xml(text: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content
    xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
    xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
    office:version="1.2">
  <office:body>
    <office:text>
      <text:p><text:span>{}</text:span></text:p>
    </office:text>
  </office:body>
</office:document-content>"#,
        text
    )
}

fn write_odt_zip(path: &Path, xml_content: &str) -> io::Result<()> {
    let file = fs::File::create(path)?;
    let mut zip = zip::ZipWriter::new(file);

    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    zip.add_directory("META-INF/", options)?;

    zip.start_file(
        "mimetype",
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
    )?;
    zip.write_all(b"application/vnd.oasis.opendocument.text")?;

    zip.start_file(
        "META-INF/manifest.xml",
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
    )?;
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0"
                   manifest:version="1.2">
  <manifest:file-entry manifest:full-path="/" manifest:version="1.2"
                       manifest:media-type="application/vnd.oasis.opendocument.text"/>
  <manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/>
</manifest:manifest>"#,
    )?;

    zip.start_file("content.xml", options)?;
    zip.write_all(xml_content.as_bytes())?;

    zip.finish()?;
    Ok(())
}

/// Write a minimal valid ODT with the given text content.
pub fn write_minimal_odt(path: &Path, text: &str) -> io::Result<()> {
    write_odt_zip(path, &make_odt_xml(text))
}

/// Write random bytes to a `.odt` path (not a valid zip).
pub fn write_malformed_odt(path: &Path) -> io::Result<()> {
    fs::write(path, b"not a zip file content")
}

/// Write a valid zip that is missing `content.xml`.
pub fn write_odt_missing_xml(path: &Path) -> io::Result<()> {
    let file = fs::File::create(path)?;
    let mut zip = zip::ZipWriter::new(file);

    zip.start_file(
        "some_other_file.xml",
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
    )?;
    zip.write_all(b"<root />")?;
    zip.finish()?;
    Ok(())
}

/// Write a valid zip with malformed XML as the content entry.
pub fn write_odt_malformed_xml(path: &Path) -> io::Result<()> {
    write_odt_zip(path, "<office:document-content><office:body><office:text><text:p>unclosed")
}

/// Write ODT where text is inside `<text:span>` (may be missed by naive parsers).
pub fn write_odt_span_text(path: &Path, text: &str) -> io::Result<()> {
    write_odt_zip(path, &make_odt_span_xml(text))
}

// ── Text file builders ──

/// Write a text file exceeding a size threshold (filled with repeating pattern).
pub fn write_oversized_text(path: &Path, target_bytes: u64) -> io::Result<()> {
    let line = b"This is a line of text that repeats to fill space.\n";
    let mut file = fs::File::create(path)?;
    let mut written: u64 = 0;
    while written < target_bytes {
        file.write_all(line)?;
        written += line.len() as u64;
    }
    Ok(())
}

/// Write a text file containing NUL bytes.
pub fn write_nul_text(path: &Path) -> io::Result<()> {
    fs::write(path, b"hello\0world\0some\x00content")
}

/// Write binary content to a text-like extension.
pub fn write_binary_as_text(path: &Path) -> io::Result<()> {
    fs::write(path, b"\x00\x01\x02\xff\xfe\xfd\xfc\x00\xffbinary")
}

// ── Edge-case layout helpers ──

/// Create hidden files (dot-prefixed) under `dir` and return their paths.
pub fn write_hidden_files(dir: &Path) -> io::Result<Vec<Utf8PathBuf>> {
    fs::create_dir_all(dir)?;
    let paths = vec![
        dir.join(".hidden"),
        dir.join(".config"),
        dir.join("..dotfile"),
    ];
    for p in &paths {
        fs::write(p, b"hidden content that should be excluded by default")?;
    }
    let parent = Utf8PathBuf::from_path_buf(dir.to_path_buf()).map_err(|_| {
        io::Error::new(io::ErrorKind::InvalidInput, "non-utf8 path")
    })?;
    Ok(paths
        .into_iter()
        .map(|p| {
            let name = p.file_name().expect("path has file name");
            parent.join(name.to_str().expect("file name is valid utf-8"))
        })
        .collect())
}

/// Create files with identical content (duplicate aliases) under `dir` and return their paths.
pub fn write_duplicate_aliases(dir: &Path) -> io::Result<Vec<Utf8PathBuf>> {
    fs::create_dir_all(dir)?;
    let content = b"identical content for alias detection";
    let names = ["original.txt", "copy_same.txt", "another_copy.txt"];
    let mut paths = Vec::new();
    for name in &names {
        let p = dir.join(name);
        fs::write(&p, content)?;
        paths.push(p);
    }
    let parent = Utf8PathBuf::from_path_buf(dir.to_path_buf()).map_err(|_| {
        io::Error::new(io::ErrorKind::InvalidInput, "non-utf8 path")
    })?;
    Ok(paths
        .into_iter()
        .map(|p| {
            let name = p.file_name().expect("path has file name");
            parent.join(name.to_str().expect("file name is valid utf-8"))
        })
        .collect())
}

// ── Fixture packs ──

/// Statistics about fixtures in a pack.
#[derive(Debug, Clone, Copy)]
pub struct FixtureCounts {
    pub total: usize,
    pub malformed: usize,
    pub valid: usize,
    pub hidden: usize,
    pub aliases: usize,
}

/// A directory of fixture files ready for use in tests or benchmarks.
#[derive(Debug)]
pub struct FixturePack {
    pub root: Utf8PathBuf,
    pub files: Vec<Utf8PathBuf>,
    pub counts: FixtureCounts,
}

/// Creates a full fixture directory under `root` with all fixture types.
pub fn full_fixture_pack(root: &Utf8PathBuf) -> io::Result<FixturePack> {
    fs::create_dir_all(root.as_std_path())?;

    let mut files = Vec::new();
    let mut counts = FixtureCounts {
        total: 0,
        malformed: 0,
        valid: 0,
        hidden: 0,
        aliases: 0,
    };

    let malformed_dir = root.join("malformed");
    fs::create_dir_all(malformed_dir.as_std_path())?;

    let p = malformed_dir.join("not_a_pdf.pdf");
    write_malformed_pdf(p.as_std_path())?;
    files.push(p);
    counts.malformed += 1;

    let p = malformed_dir.join("corrupt_pdf.pdf");
    write_corrupt_stream_pdf(p.as_std_path())?;
    files.push(p);
    counts.malformed += 1;

    let p = malformed_dir.join("not_a_docx.docx");
    write_malformed_docx(p.as_std_path())?;
    files.push(p);
    counts.malformed += 1;

    let p = malformed_dir.join("docx_missing_xml.docx");
    write_docx_missing_xml(p.as_std_path())?;
    files.push(p);
    counts.malformed += 1;

    let p = malformed_dir.join("docx_bad_xml.docx");
    write_docx_malformed_xml(p.as_std_path())?;
    files.push(p);
    counts.malformed += 1;

    let p = malformed_dir.join("not_a_odt.odt");
    write_malformed_odt(p.as_std_path())?;
    files.push(p);
    counts.malformed += 1;

    let p = malformed_dir.join("odt_missing_xml.odt");
    write_odt_missing_xml(p.as_std_path())?;
    files.push(p);
    counts.malformed += 1;

    let p = malformed_dir.join("odt_bad_xml.odt");
    write_odt_malformed_xml(p.as_std_path())?;
    files.push(p);
    counts.malformed += 1;

    let formats_dir = root.join("formats");
    fs::create_dir_all(formats_dir.as_std_path())?;

    let p = formats_dir.join("sample.pdf");
    write_minimal_pdf(p.as_std_path(), "Hello from PDF fixture")?;
    files.push(p);
    counts.valid += 1;

    let p = formats_dir.join("image_only.pdf");
    write_image_only_pdf(p.as_std_path())?;
    files.push(p);
    counts.valid += 1;

    let p = formats_dir.join("sample.docx");
    write_minimal_docx(p.as_std_path(), "Hello from DOCX fixture")?;
    files.push(p);
    counts.valid += 1;

    let p = formats_dir.join("sample.odt");
    write_minimal_odt(p.as_std_path(), "Hello from ODT fixture")?;
    files.push(p);
    counts.valid += 1;

    let p = formats_dir.join("odt_span_text.odt");
    write_odt_span_text(p.as_std_path(), "nested span text")?;
    files.push(p);
    counts.valid += 1;

    let edge_dir = root.join("edge");
    fs::create_dir_all(edge_dir.as_std_path())?;

    let hidden = write_hidden_files(edge_dir.as_std_path())?;
    counts.hidden += hidden.len();
    files.extend(hidden.into_iter().map(|p| {
        let rel = p
            .strip_prefix(edge_dir.as_path())
            .expect("hidden path should be under edge dir");
        edge_dir.join(rel)
    }));

    let aliases = write_duplicate_aliases(edge_dir.as_std_path())?;
    counts.aliases += aliases.len();
    files.extend(aliases.into_iter().map(|p| {
        let rel = p
            .strip_prefix(edge_dir.as_path())
            .expect("alias path should be under edge dir");
        edge_dir.join(rel)
    }));

    counts.total = files.len();

    Ok(FixturePack {
        root: root.clone(),
        files,
        counts,
    })
}

/// Creates a fixture directory with only malformed documents.
pub fn malformed_fixture_pack(root: &Utf8PathBuf) -> io::Result<FixturePack> {
    fs::create_dir_all(root.as_std_path())?;
    let mut files = Vec::new();
    let mut count = 0;

    for (name, writer_fn) in [
        ("not_a_pdf.pdf", write_malformed_pdf as fn(&Path) -> io::Result<()>),
        ("corrupt_pdf.pdf", write_corrupt_stream_pdf),
        ("not_a_docx.docx", write_malformed_docx),
        ("docx_missing_xml.docx", write_docx_missing_xml),
        ("docx_bad_xml.docx", write_docx_malformed_xml),
        ("not_a_odt.odt", write_malformed_odt),
        ("odt_missing_xml.odt", write_odt_missing_xml),
        ("odt_bad_xml.odt", write_odt_malformed_xml),
    ] {
        let p = root.join(name);
        writer_fn(p.as_std_path())?;
        files.push(p);
        count += 1;
    }

    Ok(FixturePack {
        root: root.clone(),
        files,
        counts: FixtureCounts {
            total: count,
            malformed: count,
            valid: 0,
            hidden: 0,
            aliases: 0,
        },
    })
}

/// Creates a fixture directory with valid format samples.
pub fn format_fixture_pack(root: &Utf8PathBuf) -> io::Result<FixturePack> {
    fs::create_dir_all(root.as_std_path())?;
    let mut files = Vec::new();
    let mut count = 0;

    for (name, build) in [
        ("sample.pdf", Box::new(|p: &Path| write_minimal_pdf(p, "Hello from PDF fixture")) as Box<dyn FnOnce(&Path) -> io::Result<()>>),
        ("image_only.pdf", Box::new(|p: &Path| write_image_only_pdf(p))),
        ("sample.docx", Box::new(|p: &Path| write_minimal_docx(p, "Hello from DOCX fixture"))),
        ("sample.odt", Box::new(|p: &Path| write_minimal_odt(p, "Hello from ODT fixture"))),
    ] {
        let p = root.join(name);
        build(p.as_std_path())?;
        files.push(p);
        count += 1;
    }

    let p = root.join("odt_span_text.odt");
    write_odt_span_text(p.as_std_path(), "nested span text")?;
    files.push(p);
    count += 1;

    Ok(FixturePack {
        root: root.clone(),
        files,
        counts: FixtureCounts {
            total: count,
            malformed: 0,
            valid: count,
            hidden: 0,
            aliases: 0,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Read;

    // ── PDF fixtures ──

    #[test]
    fn minimal_pdf_is_valid_and_contains_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.pdf");
        write_minimal_pdf(&path, "expected text").unwrap();

        let bytes = fs::read(&path).unwrap();
        let content = String::from_utf8_lossy(&bytes);
        assert!(content.starts_with("%PDF-1.4"), "should have PDF header");
        assert!(content.contains("expected text"), "should contain the text");
        assert!(content.contains("xref"), "should have xref table");
        assert!(content.contains("%%EOF"), "should have EOF marker");
    }

    #[test]
    fn malformed_pdf_is_not_valid() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.pdf");
        write_malformed_pdf(&path).unwrap();
        let bytes = fs::read(&path).unwrap();
        assert_eq!(bytes, b"not a pdf file content");
    }

    #[test]
    fn corrupt_stream_pdf_has_valid_structure() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("corrupt.pdf");
        write_corrupt_stream_pdf(&path).unwrap();
        let bytes = fs::read(&path).unwrap();
        let content = String::from_utf8_lossy(&bytes);
        assert!(content.starts_with("%PDF-1.4"));
        assert!(content.contains("%%EOF"));
    }

    #[test]
    fn image_only_pdf_has_no_selectable_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("image.pdf");
        write_image_only_pdf(&path).unwrap();
        let bytes = fs::read(&path).unwrap();
        let content = String::from_utf8_lossy(&bytes);
        assert!(content.starts_with("%PDF-1.4"));
        assert!(content.contains("/XObject"));
    }

    // ── DOCX fixtures ──

    #[test]
    fn minimal_docx_contains_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.docx");
        write_minimal_docx(&path, "hello docx").unwrap();

        let bytes = fs::read(&path).unwrap();
        // valid zip starts with PK\x03\x04
        assert_eq!(&bytes[..2], b"PK", "should be a valid zip");
    }

    #[test]
    fn malformed_docx_is_not_a_zip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.docx");
        write_malformed_docx(&path).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"not a zip file content");
    }

    #[test]
    fn docx_missing_xml_is_valid_zip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("no_xml.docx");
        write_docx_missing_xml(&path).unwrap();
        let bytes = fs::read(&path).unwrap();
        assert_eq!(&bytes[..2], b"PK");
    }

    #[test]
    fn docx_malformed_xml_is_valid_zip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad_xml.docx");
        write_docx_malformed_xml(&path).unwrap();
        let bytes = fs::read(&path).unwrap();
        assert_eq!(&bytes[..2], b"PK");
    }

    // ── ODT fixtures ──

    #[test]
    fn minimal_odt_contains_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.odt");
        write_minimal_odt(&path, "hello odt").unwrap();
        let bytes = fs::read(&path).unwrap();
        assert_eq!(&bytes[..2], b"PK");
    }

    #[test]
    fn malformed_odt_is_not_a_zip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.odt");
        write_malformed_odt(&path).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"not a zip file content");
    }

    #[test]
    fn odt_missing_xml_is_valid_zip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("no_xml.odt");
        write_odt_missing_xml(&path).unwrap();
        let bytes = fs::read(&path).unwrap();
        assert_eq!(&bytes[..2], b"PK");
    }

    #[test]
    fn odt_span_text_contains_span_xml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("span.odt");
        write_odt_span_text(&path, "span content").unwrap();
        // re-read zip and check content.xml
        let file = fs::File::open(&path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut content_xml = String::new();
        archive
            .by_name("content.xml")
            .unwrap()
            .read_to_string(&mut content_xml)
            .unwrap();
        assert!(content_xml.contains("text:span"));
    }

    // ── Text fixtures ──

    #[test]
    fn oversized_text_reaches_target() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.txt");
        write_oversized_text(&path, 10_000).unwrap();
        let meta = fs::metadata(&path).unwrap();
        assert!(meta.len() >= 10_000);
    }

    #[test]
    fn nul_text_contains_nul_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nul.txt");
        write_nul_text(&path).unwrap();
        let bytes = fs::read(&path).unwrap();
        assert!(bytes.contains(&0x00));
    }

    // ── Edge-case fixtures ──

    #[test]
    fn hidden_files_are_dot_prefixed() {
        let dir = tempfile::tempdir().unwrap();
        let files = write_hidden_files(dir.path()).unwrap();
        for f in &files {
            let name = f.file_name().expect("should have name");
            assert!(name.starts_with('.'), "hidden file should start with dot: {name}");
        }
    }

    #[test]
    fn duplicate_aliases_have_identical_content() {
        let dir = tempfile::tempdir().unwrap();
        let files = write_duplicate_aliases(dir.path()).unwrap();
        assert_eq!(files.len(), 3);
        let contents: Vec<_> = files
            .iter()
            .map(|f| fs::read_to_string(f.as_std_path()).unwrap())
            .collect();
        assert_eq!(contents[0], contents[1]);
        assert_eq!(contents[1], contents[2]);
    }

    // ── Fixture packs ──

    #[test]
    fn full_fixture_pack_creates_all_categories() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let pack = full_fixture_pack(&root).unwrap();

        assert!(pack.counts.total > 0);
        assert!(pack.counts.malformed > 0);
        assert!(pack.counts.valid > 0);
        assert!(pack.counts.hidden > 0);
        assert!(pack.counts.aliases > 0);

        // all files exist on disk
        for f in &pack.files {
            assert!(f.as_std_path().exists(), "fixture file should exist: {f}");
        }
    }

    #[test]
    fn malformed_fixture_pack_contains_only_malformed() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let pack = malformed_fixture_pack(&root).unwrap();

        assert!(pack.counts.malformed > 0);
        assert_eq!(pack.counts.valid, 0);
        assert_eq!(pack.counts.hidden, 0);
        assert_eq!(pack.counts.total, pack.counts.malformed);
    }

    #[test]
    fn format_fixture_pack_contains_only_valid() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let pack = format_fixture_pack(&root).unwrap();

        assert!(pack.counts.valid > 0);
        assert_eq!(pack.counts.malformed, 0);
        assert_eq!(pack.counts.total, pack.counts.valid);
    }
}
