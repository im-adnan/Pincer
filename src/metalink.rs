use quick_xml::events::Event;
use quick_xml::Reader;

#[derive(Debug, Clone)]
pub struct MetalinkFile {
    pub name: Option<String>,
    pub size: Option<u64>,
    pub hash_sha256: Option<String>,
    pub urls: Vec<String>,
}

pub fn parse_metalink(xml: &str) -> Result<Vec<MetalinkFile>, String> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);

    let mut buf = Vec::new();
    let mut files = Vec::new();

    let mut current_file = None;
    let mut in_file = false;
    let mut current_tag = String::new();
    let mut current_hash_type = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                current_tag = tag_name.clone();

                if tag_name == "file" {
                    in_file = true;
                    let mut file = MetalinkFile {
                        name: None,
                        size: None,
                        hash_sha256: None,
                        urls: Vec::new(),
                    };
                    // Extract name attribute if available
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"name" {
                            file.name = Some(String::from_utf8_lossy(&attr.value).to_string());
                        }
                    }
                    current_file = Some(file);
                } else if tag_name == "hash" {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"type" {
                            current_hash_type = String::from_utf8_lossy(&attr.value).to_lowercase();
                        }
                    }
                }
            }
            #[allow(clippy::collapsible_match)]
            Ok(Event::Text(e)) => {
                if in_file {
                    if let Some(ref mut f) = current_file {
                        let text = e.unescape().unwrap_or_default().to_string();
                        match current_tag.as_str() {
                            "size" => {
                                f.size = text.parse::<u64>().ok();
                            }
                            "hash" => {
                                if current_hash_type == "sha-256" || current_hash_type == "sha256" {
                                    f.hash_sha256 = Some(text);
                                }
                            }
                            "url" => {
                                f.urls.push(text);
                            }
                            _ => {}
                        }
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                current_tag.clear();
                if tag_name == "file" {
                    in_file = false;
                    if let Some(f) = current_file.take() {
                        if !f.urls.is_empty() {
                            files.push(f);
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(format!(
                    "XML parsing error at position {}: {:?}",
                    reader.buffer_position(),
                    e
                ))
            }
            _ => (),
        }
        buf.clear();
    }

    Ok(files)
}
