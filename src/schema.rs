// @generated automatically by Diesel CLI.

diesel::table! {
    guests (id) {
        #[max_length = 12]
        id -> Varchar,
        accepted -> Nullable<Bool>,
        #[max_length = 100]
        guest_name -> Varchar,
        #[max_length = 100]
        guest_dietary_restrictions -> Nullable<Varchar>,
        plus_one_allowed -> Bool,
        #[max_length = 100]
        plus_one_name -> Nullable<Varchar>,
        #[max_length = 100]
        plus_one_dietary_restrictions -> Nullable<Varchar>,
        date_of_rsvp -> Nullable<Timestamp>,
        last_modified -> Nullable<Timestamp>,
    }
}
