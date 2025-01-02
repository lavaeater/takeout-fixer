use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(MediaFile::Table)
                    .if_not_exists()
                    .col(pk_auto(MediaFile::Id))
                    .col(string(MediaFile::FileName))
                    .col(string(MediaFile::Path))
                    .col(json(MediaFile::JsonMeta))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(MediaFile::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum MediaFile {
    Table,
    Id,
    FileName,
    Path,
    JsonMeta,
}
