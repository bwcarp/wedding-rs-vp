use super::schema::guests;
use diesel::prelude::*;
use chrono::NaiveDateTime;
use serde::{Serialize, Deserialize};

#[derive(Insertable, Selectable, Queryable, Serialize, Deserialize, AsChangeset)]
#[diesel(table_name = guests)]
pub struct Guest {
    pub id: String,
    pub accepted: Option<bool>,
    pub guest_name: String,
    pub guest_dietary_restrictions: Option<String>,
    pub plus_one_allowed: bool,
    pub plus_one_name: Option<String>,
    pub plus_one_dietary_restrictions: Option<String>,
    pub date_of_rsvp: Option<NaiveDateTime>,
    pub last_modified: Option<NaiveDateTime>,
}
