use crate::KnotesDBConnection;
use mongodb::coll::options::{FindOneAndUpdateOptions, ReturnDocument};
use mongodb::db::ThreadedDatabase;
use mongodb::oid::ObjectId;
use mongodb::Document;
use validator::Validate;

#[derive(Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub body: String,
}

#[derive(Serialize, Deserialize, Validate)]
pub struct UpdateNoteParams {
    #[validate(length(min = 1, message = "Note title is required"))]
    title: Option<String>,
    body: Option<String>,
}

impl Note {
    pub fn from(doc: Document) -> Self {
        Note {
            id: doc.get_object_id("_id").unwrap().to_string(),
            title: doc.get_str("title").unwrap().to_string(),
            body: doc.get_str("body").unwrap().to_string(),
        }
    }

    pub fn update(id: &str, params: UpdateNoteParams, db: &KnotesDBConnection) -> Result<Self, ()> {
        let coll = db.collection("notes");

        let oid = match ObjectId::with_string(id) {
            Ok(o) => o,
            Err(_) => return Err(()),
        };

        let mut update = doc! {};

        if let Some(body) = params.body {
            update.insert("body", body);
        }
        if let Some(title) = params.title {
            update.insert("title", title);
        }

        match coll.find_one_and_update(
            doc! {"_id": oid},
            doc! {"$set": update},
            Some(FindOneAndUpdateOptions {
                return_document: Some(ReturnDocument::After),
                max_time_ms: None,
                projection: None,
                sort: None,
                upsert: None,
                write_concern: None,
            }),
        ) {
            Err(_) => Err(()),
            Ok(doc_option) => match doc_option {
                None => Err(()),
                Some(doc) => Ok(Note::from(doc)),
            },
        }
    }

    pub fn delete(id: &str, db: &KnotesDBConnection) -> Result<(), ()> {
        let coll = db.collection("notes");

        let oid = match ObjectId::with_string(id) {
            Ok(o) => o,
            Err(_) => return Err(()),
        };

        match coll.find_one_and_delete(doc! {"_id": oid}, None) {
            Ok(_) => Ok(()),
            Err(_) => Err(()),
        }
    }
}

pub fn get(id: &ObjectId, db: &KnotesDBConnection) -> Option<Note> {
    let coll = db.collection("notes");

    match coll.find_one(Some(doc! {"_id": id.clone()}), None) {
        Ok(doc_option) => match doc_option {
            None => {
                println!("found no note for {}", id);
                None
            }
            Some(doc) => Some(Note::from(doc)),
        },
        Err(_) => None,
    }
}

pub fn get_with_string_id(id: &str, db: &KnotesDBConnection) -> Option<Note> {
    match ObjectId::with_string(id) {
        Ok(oid) => get(&oid, db),
        Err(_) => return None,
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

    let note_id = match coll
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
        None => {
            println!("There was an error saving note");
            return Err(());
        }
    };

    match get(&note_id.as_object_id().unwrap(), db) {
        Some(note) => Ok(note),
        None => {
            println!("There was an error fething the note");
            Err(())
        }
    }
}
