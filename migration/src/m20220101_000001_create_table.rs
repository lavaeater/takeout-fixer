use sea_orm_migration::{prelude::*, schema::*};
use sea_orm_migration::sea_orm::Iterable;
use crate::sea_orm::{DeriveActiveEnum, EnumIter};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(EnumIter, DeriveActiveEnum, Iden)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)", rename_all = "UPPERCASE")]
pub enum Status {
    Pending,
    Downloading,
    Downloaded,
    Processing,
    Completed,
    Failed,
}

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
                    .col(string(TakeoutZip::DriveId))
                    .col(string(TakeoutZip::Name))
                    .col(string(TakeoutZip::LocalPath))
                    .col(enumeration(TakeoutZip::Status, StatusEnum, Status::iter()))
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
