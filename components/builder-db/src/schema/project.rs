table! {
    use diesel::sql_types::{Bool, BigInt, Text, Nullable, Timestamptz};

    origin_projects (id) {
        id -> BigInt,
        origin -> Text,
        owner_id -> BigInt,
        package_name -> Text,
        name -> Text,
        plan_path -> Text,
        target -> Text,
        vcs_type -> Text,
        vcs_data -> Text,
        vcs_installation_id -> Nullable<BigInt>,
        auto_build -> Bool,
        created_at -> Nullable<Timestamptz>,
        updated_at -> Nullable<Timestamptz>,
    }
}
