use std::io::{Read, Seek};
use epub_builder::ReferenceType;

pub trait InputData: Read + Seek + Send {}

impl<T: Read + Seek + Send> InputData for T {}

pub enum EpubPart {
    Content {
        filename: String,
        title: String,
        content: String,
        reftype: Option<ReferenceType>,
    },
    Resource {
        filename: String,
        content: Box<dyn InputData>,
        mime_type: String,
    },
}

pub struct CompletionMessage {
    pub sequence_id: usize,
    pub parts: Vec<EpubPart>,
}