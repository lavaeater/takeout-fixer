use sea_orm_migration::{prelude::*, schema::*};
use std::fmt;
use std::fmt::Display;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        manager
            .create_table(
                Table::create()
                    .table(TakeoutZip::Table)
                    .if_not_exists()
                    .col(pk_auto(TakeoutZip::Id))
                    .col(string_uniq(TakeoutZip::DriveId))
                    .col(string(TakeoutZip::Name))
                    .col(string(TakeoutZip::LocalPath))
                    .col(string(TakeoutZip::Status))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(foreign_key_auto(
                &mut Table::create()
                    .table(FileInZip::Table)
                    .if_not_exists()
                    .col(pk_auto(FileInZip::Id))
                    .col(string(FileInZip::Name))
                    .col(string(FileInZip::Path))
                    .col(string(FileInZip::Status))
                    .col(json(FileInZip::Log))
                    .col(integer_null(FileInZip::RelatedId))
                    .col(string(FileInZip::FileType))
                    .col(string(FileInZip::Extension))
                    .to_owned(),
                FileInZip::Table,
                FileInZip::TakeoutZipId,
                TakeoutZip::Table,
                TakeoutZip::Id,
            ))
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts

        manager
            .drop_table(Table::drop().table(TakeoutZip::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(FileInZip::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden, Copy, Clone, Debug)]
enum TakeoutZip {
    Table,
    Id,
    DriveId,
    Name,
    LocalPath,
    Status,
}
impl Display for TakeoutZip {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TakeoutZip::Table => write!(f, "takeout_zip"),
            TakeoutZip::Id => write!(f, "id"),
            TakeoutZip::DriveId => write!(f, "drive_id"),
            TakeoutZip::Name => write!(f, "name"),
            TakeoutZip::LocalPath => write!(f, "local_path"),
            TakeoutZip::Status => write!(f, "status"),
        }
    }
}

pub fn foreign_key_auto<T, U>(
    table_create_statement: &mut TableCreateStatement,
    from_table: T,
    fk_column: T,
    to_table: U,
    to_id_column: U,
) -> TableCreateStatement
where
    T: IntoIden + Copy + Display + 'static,
    U: IntoIden + Copy + Display + 'static,
{
    table_create_statement.col(integer(fk_column).not_null());
    table_create_statement.foreign_key(&mut fk_auto(from_table, fk_column, to_table, to_id_column));
    table_create_statement.to_owned()
}

pub fn fk_auto<T, U>(
    from_table: T,
    fk_column: T,
    to_table: U,
    to_id_column: U,
) -> ForeignKeyCreateStatement
where
    T: IntoIden + Copy + Display + 'static,
    U: IntoIden + Copy + Display + 'static,
{
    ForeignKey::create()
        .name(format!("fk_{}_{}", from_table, to_table))
        .from(from_table, fk_column)
        .to(to_table, to_id_column)
        .on_delete(ForeignKeyAction::Cascade)
        .on_update(ForeignKeyAction::Cascade)
        .take()
}

#[derive(DeriveIden, Copy, Clone, Debug)]
enum FileInZip {
    Table,
    Id,
    TakeoutZipId,
    Name,
    Path,
    Status,
    Log,
    RelatedId,
    FileType,
    Extension
}
impl Display for FileInZip {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileInZip::Table => write!(f, "file_in_zip"),
            FileInZip::Id => write!(f, "id"),
            FileInZip::TakeoutZipId => write!(f, "takeout_zip_id"),
            FileInZip::Name => write!(f, "name"),
            FileInZip::Path => write!(f, "path"),
            FileInZip::Status => write!(f, "status"),
            FileInZip::Log => write!(f, "log"),
            FileInZip::RelatedId => write!(f, "related_id"),
            FileInZip::FileType => write!(f, "type"),
            FileInZip::Extension => write!(f, "extension"),
        }
    }
}
