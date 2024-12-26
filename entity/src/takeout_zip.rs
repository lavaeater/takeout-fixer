//! `SeaORM` Entity, @generated by sea-orm-codegen 1.1.3

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "takeout_zip")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub drive_id: String,
    pub name: String,
    pub local_path: String,
    pub status: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::file_in_zip::Entity")]
    FileInZip,
}

impl Related<super::file_in_zip::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::FileInZip.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
