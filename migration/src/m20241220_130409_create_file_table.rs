use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        manager
            .create_table(
                Table::create()
                    .table(FileInZip::Table)
                    .if_not_exists()
                    .col(pk_auto(FileInZip::Id))
                    .col(string(FileInZip::Name))
                    .col(string(FileInZip::Path))
                    .col(string(FileInZip::Status))
                    .col(json(FileInZip::Log))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        manager
            .drop_table(Table::drop().table(FileInZip::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum FileInZip {
    Table,
    Id,
    Name,
    Path,
    Status,
    Log
}
