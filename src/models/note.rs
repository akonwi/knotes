use crate::KnotesDBConnection;
use mongodb::db::ThreadedDatabase;
use mongodb::Document;

#[derive(Serialize)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub body: String,
}

impl Note {
    pub fn from(doc: Document) -> Self {
        Note {
            id: doc.get_object_id("_id").unwrap().to_string(),
            title: doc.get_str("title").unwrap().to_string(),
            body: doc.get_str("body").unwrap().to_string(),
        }
    }
}

pub fn find_by_user(id: &str, db: &KnotesDBConnection) -> Vec<Note> {
    let coll = db.collection("notes");

    let cursor = match coll.find(Some(doc!{"userId": id}), None).ok() {
        None => return vec![],
        Some(c) => c
    };

    cursor.filter_map(|v| {
        match v.ok() {
            None => None,
            Some(d) => Some(Note::from(d))
        }
    }).collect()
}
