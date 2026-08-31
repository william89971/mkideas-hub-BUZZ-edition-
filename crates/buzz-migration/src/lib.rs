#![deny(unsafe_code)]

//! Deterministic, offline-first primitives for migrating the legacy MK Ideas
//! Command Center into MK Ideas Buzz.
//!
//! This crate deliberately contains no database or network client. Source
//! extraction and relay submission are integration seams so the pure mapping,
//! planning, checkpoint, and reconciliation contracts can be reviewed and
//! tested before any credentialed operation exists.

pub mod bundle;
pub mod canonical;
pub mod checkpoint;
pub mod dry_run;
mod dry_run_content;
mod dry_run_model;
mod dry_run_report;
mod dry_run_validation;
pub mod error;
pub mod ids;
pub mod manifest;
pub mod mapping;
pub mod plan;
pub mod receipt;
pub mod report;
pub mod secrets;
pub mod status;

pub use error::{MigrationError, Result};
