use super::db_id_format;
use chrono::NaiveDateTime;

use diesel::{self,
             dsl::count,
             pg::PgConnection,
             prelude::*,
             result::{Error,
                      QueryResult},
             ExpressionMethods,
             QueryDsl,
             RunQueryDsl};

use crate::{models::{channel::{Channel,
                               CreateChannel},
                     package::PackageVisibility},
            protocol::originsrv};

use crate::schema::{channel::origin_channels,
                    integration::origin_integrations,
                    invitation::origin_invitations,
                    key::{origin_private_encryption_keys,
                          origin_public_encryption_keys,
                          origin_public_keys,
                          origin_secret_keys},
                    member::origin_members,
                    origin::{origins,
                             origins_with_secret_key,
                             origins_with_stats},
                    package::origin_packages,
                    project::origin_projects,
                    project_integration::origin_project_integrations,
                    secrets::origin_secrets,
                    settings::origin_package_settings};

use crate::{bldr_core::metrics::CounterMetric,
            hab_core::ChannelIdent,
            metrics::Counter};

#[derive(Debug, Serialize, Deserialize, QueryableByName, Queryable)]
#[table_name = "origins"]
pub struct Origin {
    #[serde(with = "db_id_format")]
    pub owner_id: i64,
    pub name: String,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
    pub default_package_visibility: PackageVisibility,
}

#[derive(Debug, Serialize, Deserialize, Queryable)]
pub struct OriginWithSecretKey {
    #[serde(with = "db_id_format")]
    pub owner_id: i64,
    pub name: String,
    pub private_key_name: Option<String>,
    pub default_package_visibility: PackageVisibility,
    pub owner_account: String,
}

#[derive(Debug, Serialize, Deserialize, Queryable)]
pub struct OriginWithStats {
    #[serde(with = "db_id_format")]
    pub owner_id: i64,
    pub name: String,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
    pub default_package_visibility: PackageVisibility,
    pub package_count: i64,
}

#[derive(Debug, Serialize, Deserialize, Queryable, QueryableByName, Insertable)]
#[table_name = "origin_members"]
pub struct OriginMember {
    #[serde(with = "db_id_format")]
    pub account_id: i64,
    pub origin: String,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}

#[derive(Insertable)]
#[table_name = "origins"]
pub struct NewOrigin<'a> {
    pub name: &'a str,
    pub owner_id: i64,
    pub default_package_visibility: &'a PackageVisibility,
}

impl Origin {
    pub fn get(origin: &str, conn: &PgConnection) -> QueryResult<OriginWithSecretKey> {
        Counter::DBCall.increment();
        origins_with_secret_key::table.find(origin)
                                      .limit(1)
                                      .get_result(conn)
    }

    pub fn list(owner_id: i64, conn: &PgConnection) -> QueryResult<Vec<OriginWithStats>> {
        Counter::DBCall.increment();
        origins_with_stats::table.inner_join(origin_members::table)
                                 .select(origins_with_stats::table::all_columns())
                                 .filter(origin_members::account_id.eq(owner_id))
                                 .order(origins_with_stats::name.asc())
                                 .get_results(conn)
    }

    pub fn create(req: &NewOrigin, conn: &PgConnection) -> QueryResult<Origin> {
        Counter::DBCall.increment();
        let new_origin = diesel::insert_into(origins::table).values(req)
                                                            .get_result(conn)?;

        OriginMember::add(req.name, req.owner_id, conn)?;
        Channel::create(&CreateChannel { name:     ChannelIdent::unstable().as_str(),
                                         owner_id: req.owner_id,
                                         origin:   req.name, },
                        conn)?;
        Channel::create(&CreateChannel { name:     ChannelIdent::stable().as_str(),
                                         owner_id: req.owner_id,
                                         origin:   req.name, },
                        conn)?;

        Ok(new_origin)
    }

    pub fn update(name: &str, dpv: PackageVisibility, conn: &PgConnection) -> QueryResult<usize> {
        Counter::DBCall.increment();
        diesel::update(origins::table.find(name)).set(origins::default_package_visibility.eq(dpv))
                                                 .execute(conn)
    }

    pub fn delete(origin: &str, conn: &PgConnection) -> QueryResult<()> {
        // By this point, most of the associated origin data has already been manually deleted
        // by the user. We ensure this by double checking the most critical tables are already empty
        // via builder_api::server::resources::origins::origin_delete_preflight
        // The transaction entries below explicitly enumerate deletion of all table data associated
        // with the origin to ensure no vestigial data remains.
        //
        // TODO: Add origin deletion request into an origin audit table.

        Counter::DBCall.increment();
        conn.transaction::<_, Error, _>(|| {
            diesel::delete(origin_channels::table.filter(origin_channels::origin.eq(origin)))
                .execute(conn)?;
            diesel::delete(origin_secret_keys::table.filter(origin_secret_keys::origin.eq(origin)))
                .execute(conn)?;
            diesel::delete(origin_public_keys::table.filter(origin_public_keys::origin.eq(origin)))
                .execute(conn)?;
            diesel::delete(origin_members::table.filter(origin_members::origin.eq(origin)))
                .execute(conn)?;
            diesel::delete(origin_package_settings::table.filter(origin_package_settings::origin.eq(origin)))
                .execute(conn)?;
            diesel::delete(
                origin_integrations::table.filter(origin_integrations::origin.eq(origin)),
            )
            .execute(conn)?;
            diesel::delete(origin_invitations::table.filter(origin_invitations::origin.eq(origin)))
                .execute(conn)?;
            diesel::delete(origin_project_integrations::table.filter(origin_project_integrations::origin.eq(origin)))
                .execute(conn)?;
            diesel::delete(origin_projects::table.filter(origin_projects::origin.eq(origin)))
                .execute(conn)?;
            diesel::delete(origin_secrets::table.filter(origin_secrets::origin.eq(origin)))
                .execute(conn)?;
            diesel::delete(origin_private_encryption_keys::table.filter(origin_private_encryption_keys::origin.eq(origin)))
                .execute(conn)?;
            diesel::delete(origin_public_encryption_keys::table.filter(origin_public_encryption_keys::origin.eq(origin)))
                .execute(conn)?;
            diesel::delete(origin_packages::table.filter(origin_packages::origin.eq(origin)))
                .execute(conn)?;
            diesel::delete(origins::table.filter(origins::name.eq(origin))).execute(conn)?;
            Ok(())
        })
    }

    pub fn transfer(origin: &str, account_id: i64, conn: &PgConnection) -> QueryResult<usize> {
        Counter::DBCall.increment();
        diesel::update(origins::table.find(origin)).set(origins::owner_id.eq(account_id))
                                                   .execute(conn)
    }

    pub fn depart(origin: &str, account_id: i64, conn: &PgConnection) -> QueryResult<usize> {
        Counter::DBCall.increment();
        diesel::delete(origin_members::table
                .filter(origin_members::account_id.eq(account_id))
                .filter(origin_members::origin.eq(origin)))
                .execute(conn)
    }

    pub fn check_membership(origin: &str,
                            account_id: i64,
                            conn: &PgConnection)
                            -> QueryResult<bool> {
        Counter::DBCall.increment();
        origin_members::table.filter(origin_members::origin.eq(origin))
                             .filter(origin_members::account_id.eq(account_id))
                             .execute(conn)
                             .and_then(|s| Ok(s > 0))
    }
}

impl OriginMember {
    pub fn list(origin: &str, conn: &PgConnection) -> QueryResult<Vec<String>> {
        use crate::schema::account::accounts;

        Counter::DBCall.increment();
        origin_members::table.inner_join(accounts::table)
                             .select(accounts::name)
                             .filter(origin_members::origin.eq(origin))
                             .order(accounts::name.asc())
                             .get_results(conn)
    }

    pub fn delete(origin: &str, account_name: &str, conn: &PgConnection) -> QueryResult<usize> {
        use crate::schema::account::accounts;

        Counter::DBCall.increment();
        diesel::delete(
            origin_members::table
                .filter(origin_members::origin.eq(origin))
                .filter(
                    origin_members::account_id.nullable().eq(accounts::table
                        .select(accounts::id)
                        .filter(accounts::name.eq(account_name))
                        .single_value()),
                ),
        )
        .execute(conn)
    }

    pub fn add(origin: &str, account_id: i64, conn: &PgConnection) -> QueryResult<usize> {
        diesel::insert_into(origin_members::table)
            .values((
                origin_members::origin.eq(origin),
                origin_members::account_id.eq(account_id),
            ))
            .execute(conn)
    }

    pub fn count_origin_members(origin: &str, conn: &PgConnection) -> QueryResult<i64> {
        Counter::DBCall.increment();
        origin_members::table.select(count(origin_members::account_id))
                             .filter(origin_members::origin.eq(&origin))
                             .first(conn)
    }
}

impl Into<originsrv::Origin> for Origin {
    fn into(self) -> originsrv::Origin {
        let mut orig = originsrv::Origin::new();
        orig.set_owner_id(self.owner_id as u64);
        orig.set_name(self.name);
        orig.set_default_package_visibility(self.default_package_visibility.into());
        orig
    }
}

impl From<originsrv::Origin> for Origin {
    fn from(origin: originsrv::Origin) -> Origin {
        Origin { owner_id: origin.get_owner_id() as i64,
                 name: origin.get_name().to_string(),
                 default_package_visibility:
                     PackageVisibility::from(origin.get_default_package_visibility()),
                 created_at: None,
                 updated_at: None, }
    }
}
