//! `SeaORM` Entity, @generated by sea-orm-codegen 1.1.1

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "file_in_zip")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub name: String,
    pub path: String,
    pub status: String,
    pub log: Json,
    pub json_id: Option<i32>,
    pub file_type: String,
    pub takeout_zip_id: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::takeout_zip::Entity",
        from = "Column::TakeoutZipId",
        to = "super::takeout_zip::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    TakeoutZip,
}

impl Related<super::takeout_zip::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TakeoutZip.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
