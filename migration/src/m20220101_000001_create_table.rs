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
                    .table(TakeoutZip::Table)
                    .if_not_exists()
                    .col(pk_auto(TakeoutZip::Id))
                    .col(string_uniq(TakeoutZip::DriveId))
                    .col(string(TakeoutZip::Name))
                    .col(string(TakeoutZip::LocalPath))
                    .col(string(TakeoutZip::Status))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        
        manager
            .drop_table(Table::drop().table(TakeoutZip::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum TakeoutZip {
    Table,
    Id,
    DriveId,
    Name,
    LocalPath,
    Status,
}
