use sea_orm::entity::prelude::*;

/// Validated values submitted to a public form.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "forms_submissions")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,
    #[sea_orm(indexed)]
    pub form_id: String,
    pub form_name: String,
    pub form_fields: Json,
    pub values: Json,
    #[sea_orm(indexed, default_value = "NOW()")]
    pub created_at: DateTimeWithTimeZone,
}

vespera::schema_type!(Schema from Model, name = "FormSubmissionsSchema");
impl ActiveModelBehavior for ActiveModel {}
