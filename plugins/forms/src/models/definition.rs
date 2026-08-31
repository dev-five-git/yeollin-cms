use sea_orm::entity::prelude::*;

/// Administrator-defined public forms.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "forms_definitions")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,
    #[sea_orm(unique)]
    pub name: String,
    pub description: String,
    pub fields: Json,
    pub success_message: String,
    #[sea_orm(indexed, default_value = true)]
    pub enabled: bool,
    #[sea_orm(default_value = 100)]
    pub max_submissions_per_hour: i32,
    pub created_by: String,
    #[sea_orm(indexed, default_value = "NOW()")]
    pub created_at: DateTimeWithTimeZone,
    #[sea_orm(default_value = "NOW()")]
    pub updated_at: DateTimeWithTimeZone,
}

vespera::schema_type!(Schema from Model, name = "FormDefinitionsSchema");
impl ActiveModelBehavior for ActiveModel {}
