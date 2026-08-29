//! Metalink XML parsing
//!
//! ### Architectural Overview
//! - **What it does**: Parses Metalink 3.0 and Metalink 4.0 XML manifests to extract download file definitions, target filenames, byte sizes, expected SHA256 integrity hashes, and prioritized mirror URLs.
//! - **How it does**: Employs `quick_xml::Reader` to stream XML tokens, maintaining a parser state stack to extract `<file name="...">`, `<hash type="sha-256">`, `<size>`, and `<url>` elements safely.
//! - **Where it comes from**: Called by `rpc::handlers::add_metalink` when Base64 Metalink manifests are submitted.
//! - **Where it leads to**: Generates `Vec<MetalinkFile>` models used by `manager::TaskSpawner` to register multi-source downloads.

use quick_xml::events::Event;
use quick_xml::Reader;

/// Parsed file record extracted from a Metalink XML document.
#[derive(Debug, Clone)]
pub struct MetalinkFile {
    /// Inferred or manifest-specified destination filename.
    pub name: Option<String>,
    /// Expected file length in bytes if specified in `<size>`.
    pub size: Option<u64>,
    /// Expected SHA256 checksum string if specified in `<hash type="sha-256">`.
    pub hash_sha256: Option<String>,
    /// Ordered list of mirror download URLs.
    pub urls: Vec<String>,
}

/// Parses an XML string containing a Metalink 3.0 or 4.0 manifest.
///
/// Uses an event-based streaming parser (`quick_xml::Reader`) to avoid loading the entire XML
/// DOM into memory, extracting file metadata, expected hashes, and mirror links.
pub fn parse_metalink(xml: &str) -> Result<Vec<MetalinkFile>, String> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);

    let mut buf = Vec::new();
    let mut files = Vec::new();

    let mut current_file: Option<MetalinkFile> = None;
    let mut current_tag = String::new();
    let mut current_hash_type = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            // Start of XML element tag
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                current_tag = name.clone();

                if name == "file" {
                    // New file record: extract name attribute if present
                    let mut file_name = None;
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"name" {
                            file_name = Some(String::from_utf8_lossy(&attr.value).to_string());
                        }
                    }
                    current_file = Some(MetalinkFile {
                        name: file_name,
                        size: None,
                        hash_sha256: None,
                        urls: Vec::new(),
                    });
                } else if name == "hash" {
                    // Hash element: check type attribute for sha-256 / sha256
                    current_hash_type.clear();
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"type" {
                            current_hash_type = String::from_utf8_lossy(&attr.value).to_lowercase();
                        }
                    }
                }
            }

            // Text content inside an element
            Ok(Event::Text(ref e)) => {
                let text = e.unescape().map_err(|err| err.to_string())?.to_string();

                if let Some(ref mut file) = current_file {
                    if current_tag == "url" && !text.trim().is_empty() {
                        file.urls.push(text.trim().to_string());
                    } else if current_tag == "size" {
                        file.size = text.trim().parse::<u64>().ok();
                    } else if current_tag == "hash"
                        && (current_hash_type == "sha-256" || current_hash_type == "sha256")
                    {
                        file.hash_sha256 = Some(text.trim().to_string());
                    } else if current_tag == "identity" && file.name.is_none() {
                        file.name = Some(text.trim().to_string());
                    }
                }
            }

            // End of XML element tag
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "file" {
                    if let Some(file) = current_file.take() {
                        if !file.urls.is_empty() {
                            files.push(file);
                        }
                    }
                }
                current_tag.clear();
            }

            // End of XML document stream
            Ok(Event::Eof) => break,

            // XML parsing error encountered
            Err(e) => {
                return Err(format!(
                    "XML parse error at position {}: {:?}",
                    reader.buffer_position(),
                    e
                ))
            }

            _ => {}
        }
        buf.clear();
    }

    Ok(files)
}
