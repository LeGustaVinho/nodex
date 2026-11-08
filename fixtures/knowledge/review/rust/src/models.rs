use diesel::prelude::*;

diesel::table! {
    gadgets (id) {
        id -> Integer,
        name -> Text,
    }
}

#[derive(Queryable, Insertable)]
#[diesel(table_name = gadgets)]
pub struct Gadget {
    pub id: i32,
    pub name: String,
}

pub fn add(conn: &mut SqliteConnection, gadget: &Gadget) {
    insert_into(gadgets::table).values(gadget).execute(conn).unwrap();
}

pub fn rename(conn: &mut SqliteConnection, id: i32, name: &str) {
    update(gadgets::table.find(id)).set(gadgets::name.eq(name)).execute(conn).unwrap();
}
