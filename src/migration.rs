//! Interface for schema migrations.
//!
//! As your system evolves over time, you may find yourself needing to evolve the data in your
//! databases along with your models. An [`IntervalMigration`](./struct.IntervalMigration.html)
//! is a great place to start & should be defined in your `Model` implementation.
//!
//! ```rust
//! // snip ...
//!
//! // Define any migrations which your model needs in this method.
//! // You should never have to remove these from your source code.
//! fn migrations() -> Vec<Box<wither::Migration>> {
//!     return vec![
//!         Box::new(wither::IntervalMigration{
//!             name: String::from("remove-oldfield"),
//!             // NOTE: use a logical time here. A day after your deployment date, or the like.
//!             threshold: chrono::Utc.ymd(2100, 1, 1).and_hms(1, 0, 0),
//!             filter: doc!{"oldfield": doc!{"$exists": true}},
//!             set: None,
//!             unset: Some(doc!{"oldfield": ""}),
//!         }),
//!     ];
//! }
//!
//! // snip ...
//! ```
//!
//! **Remember, MongoDB is not a SQL based system.** There is no true database level schema
//! enforcement. `IntervalMigration`s bridge this gap quite nicely.
//!
//! `Model`s defined in this system use [serde](https://serde.rs/), and as such, it is quite
//! likely that no explicit schema migration is needed for changes to your model. Often times,
//! [field defaults](https://serde.rs/field-attrs.html#serdedefault) can be used and no additional
//! overhead would be required.
//!
//! With that said, schema migrations in this system:
//!
//! - are defined in Rust code. Allowing them to live as child elements of your data models.
//! - are executed per model, whenever [`Model::sync`](../model/trait.Model.html#method.sync)
//!   is called — which should be once per system life cycle, early on at boottime. When dealing
//!   with an API service, this should occur before the API begins handling traffic.
//! - require no downtime to perform.
//! - require minimal configuration. The logic you use directly in your model for connecting to
//!   the backend is used for the migrations system as well.
//! - require no imperative logic. Simply declare your `filter`, `$set` & `$unset` documents, and
//!   the rest will be taken care of.
//!
//! An important question which you should be asking at this point is _"Well, how is this going to
//! work at scale?"._ This is an excellent question, of course. The answer is that it depends on
//! how you write your migrations. Here are a few pointers & a few notes to help you succeed.
//!
//! - be sure that the queries used by your migrations are covered. Just add some new indexes to
//!   your [`Model::indexes`](../model/trait.Model.html#method.indexes) implementation to be sure.
//!   Indexes will always be synced by [`Model::sync`](../model/trait.Model.html#method.sync)
//!   before migrations are executed for this reason.
//! - when you are dealing with massive amounts of data, and every document needs to be touched,
//!   **indexing still matters!** Especially when using an `IntervalMigration`, as you may be
//!   under heavy write load, and new documents will potentially be introduced having the old
//!   schema after the first service performs the migration. Schema convergence will only take
//!   place after all service instances have been updated & have executed their migrations.
//!
//! Currently, the following migration types are available. If there is a new migration "type"
//! which you find yourself in need of, [please open an issue](https://github.com/thedodd/wither)!
//!
//! - [IntervalMigration](./struct.IntervalMigration.html)

use std::error::Error;

use bson::{Bson, Document};
use chrono;
use mongodb::coll::Collection;
use mongodb::coll::options::UpdateOptions;
use mongodb::common::WriteConcern;
use mongodb::error::Error::{DefaultError, WriteError};
use mongodb::error::Result;

/// A trait definition for objects which can be used to manage schema migrations.
pub trait Migration {
    /// The function which is to execute this migration.
    fn execute<'c>(&self, coll: &'c Collection) -> Result<()>;
}

/// A migration type which allows execution until the specifed `threshold` date. Then will no-op.
///
/// This migration type works nicely in environments where multiple instances of the system — in
/// which this migration is defined — are continuously running, even during deployment cycles.
/// Highly available systems. With an `IntervalMigration`, each instance will execute the
/// migration at boottime, until the `threshold` date is passed. This will compensate for
/// write-heavy workloads, as the final instance to be updated will ensure schema convergence.
/// As long as you ensure your migrations are idempotent — **WHICH YOU ALWAYS SHOULD** — this
/// will work quite nicely.
pub struct IntervalMigration {
    /// The name for this migration. Must be unique per collection.
    pub name: String,

    /// The UTC datetime when this migration should no longer execute.
    ///
    /// Use something like: `chrono::Utc.ymd(2017, 11, 20).and_hms(22, 37, 34)`.
    pub threshold: chrono::DateTime<chrono::Utc>,

    /// The filter to be used for selecting the documents to update.
    pub filter: Document,

    /// The document to be used for the `$set` operation of the update.
    pub set: Option<Document>,

    /// The document to be used for the `$unset` operation of the update.
    pub unset: Option<Document>,
}

impl Migration for IntervalMigration {
    fn execute<'c>(&self, coll: &'c Collection) -> Result<()> {
        info!("Executing migration '{}' against '{}'.", &self.name, coll.namespace);
        // If the migrations threshold has been passed, then no-op.
        if chrono::Utc::now() > self.threshold {
            info!("Successfully executed migration '{}' against '{}'. No-op.", &self.name, coll.namespace);
            return Ok(());
        };

        // Build update document.
        let mut update = doc!{};
        if self.set.clone().is_none() && self.unset.clone().is_none() {
            return Err(DefaultError(String::from("One of '$set' or '$unset' must be specified.")));
        };
        if let Some(set) = self.set.clone() {
            update.insert_bson(String::from("$set"), Bson::from(set));
        }
        if let Some(unset) = self.unset.clone() {
            update.insert_bson(String::from("$unset"), Bson::from(unset));
        }

        // Build up & execute the migration.
        let options = UpdateOptions{upsert: Some(false), write_concern: Some(WriteConcern{w: 1, w_timeout: 0, j: true, fsync: false})};
        let res = coll.update_many(self.filter.clone(), update, Some(options))?;

        // Handle nested error condition.
        if let Some(err) = res.write_exception {
            error!("Error executing migration: {:?}", err.description());
            return Err(WriteError(err));
        }
        info!("Successfully executed migration '{}' against '{}'. {} matched. {} modified.", &self.name, coll.namespace, res.matched_count, res.modified_count);
        Ok(())
    }
}
