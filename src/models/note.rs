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

    let cursor = match coll.find(Some(doc! {"userId": id}), None).ok() {
        None => return vec![],
        Some(c) => c,
    };

    cursor
        .filter_map(|v| match v.ok() {
            None => None,
            Some(d) => Some(Note::from(d)),
        })
        .collect()
}

pub fn create_for_user(
    id: &str,
    title: &str,
    body: Option<&String>,
    db: &KnotesDBConnection,
) -> Result<Note, ()> {
    let coll = db.collection("notes");

    let id = match coll
        .insert_one(
            doc! {"userId": id, "title": title, "body": body.unwrap_or(&"".to_owned())},
            None,
        )
        .ok()
    {
        Some(res) => {
            if res.acknowledged == false || res.inserted_id.is_none() {
                return Err(());
            }
            res.inserted_id.unwrap()
        }
        None => return Err(()),
    };

    match coll.find_one(Some(doc! {"_id": id}), None) {
        Ok(doc_option) => match doc_option {
            None => Err(()),
            Some(doc) => Ok(Note::from(doc)),
        },
        Err(_) => Err(()),
    }
}
