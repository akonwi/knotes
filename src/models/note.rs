use crate::schema::notes;
use diesel::prelude::*;
use diesel::{MysqlConnection, RunQueryDsl};
use validator::Validate;

#[derive(Serialize, Deserialize, Queryable)]
pub struct Note {
    pub id: i32,
    pub user_id: i32,
    pub title: String,
    pub body: String,
}

#[derive(Validate, Insertable)]
#[table_name = "notes"]
struct NewNote<'a> {
    title: &'a str,
    body: &'a str,
    user_id: i32,
}

#[derive(Serialize, Deserialize, Validate)]
pub struct CreateNoteParams<'a> {
    #[validate(length(min = 1, message = "Note title is required"))]
    title: String,
    body: Option<&'a str>,
}

#[derive(Serialize)]
pub enum NoteError {
    NotFound,
    InvalidAttributes,
    DBWrite,
}

impl NoteError {
    pub fn message(self) -> &'static str {
        match self {
            NoteError::InvalidAttributes => "Note attributes are invalid",
            NoteError::DBWrite => "There was an error saving the note",
            NoteError::NotFound => "Note not found",
        }
    }
}

#[derive(Serialize, Deserialize, Validate)]
pub struct UpdateNoteParams<'a> {
    #[validate(length(min = 1, message = "Note title is required"))]
    pub title: Option<&'a str>,
    pub body: Option<&'a str>,
}

impl Note {
    pub fn get(id: i32, db: &MysqlConnection) -> Option<Note> {
        use notes::dsl;

        match dsl::notes.filter(dsl::id.eq(id)).first(db).optional() {
            Ok(optional) => optional,
            Err(e) => {
                println!("Error querying for note {:?}: ", e);
                None
            }
        }
    }

    pub fn find_by_user(id: &i32, db: &MysqlConnection) -> Vec<Note> {
        use notes::dsl;

        match dsl::notes.filter(dsl::user_id.eq(id)).load::<Note>(db) {
            Ok(notes) => notes,
            Err(e) => {
                println!("Error querying for notes: {:?}", e);
                vec![]
            }
        }
    }

    pub fn create_for_user(
        user_id: &i32,
        params: &CreateNoteParams,
        db: &MysqlConnection,
    ) -> Result<Note, NoteError> {
        use notes::dsl;
        if let Err(e) = diesel::insert_into(notes::table)
            .values(NewNote {
                user_id: *user_id,
                title: &params.title,
                body: params.body.unwrap_or("".as_ref()),
            })
            .execute(db)
        {
            println!("Error creating note for user {:?}: {:?}", user_id, e);
            return Err(NoteError::DBWrite);
        }

        match dsl::notes.order(dsl::id.desc()).first(db) {
            Ok(note) => Ok(note),
            Err(e) => {
                println!("Error finding note created for user {:?}: {:?}", user_id, e);
                Err(NoteError::DBWrite)
            }
        }
    }

    pub fn update(
        &mut self,
        title: Option<&str>,
        body: Option<&str>,
        db: &MysqlConnection,
    ) -> Result<Note, NoteError> {
        use notes::dsl;

        let title = title.unwrap_or(&self.title);
        let body = body.unwrap_or(&self.body);

        if let Err(e) = diesel::update(dsl::notes.find(self.id))
            .set((dsl::title.eq(title), dsl::body.eq(body)))
            .execute(db)
        {
            println!("Error updating note {:?}: {:?}", self.id, e);
            return Err(NoteError::DBWrite);
        };

        match dsl::notes.find(self.id).first(db) {
            Ok(result) => {
                self.title = title.to_owned();
                self.body = body.to_owned();
                Ok(result)
            }
            Err(e) => {
                println!("Error finding updated note {:?}: {:?}", self.id, e);
                return Err(NoteError::DBWrite);
            }
        }
    }

    pub fn delete(id: i32, db: &MysqlConnection) -> Result<(), NoteError> {
        use notes::dsl;

        if let Err(e) = diesel::delete(dsl::notes.find(id)).execute(db) {
            println!("Error deleting {:?}: {:?}", id, e);
            return Err(NoteError::DBWrite);
        };

        Ok(())
    }
}
