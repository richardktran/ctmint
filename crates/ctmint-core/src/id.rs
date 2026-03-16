/// Stable, deterministic IDs for graph entities.
///
/// Convention: `{type}:{scope}::{name}`
///   - service:auth-service
///   - module:auth-service::auth.login
///   - func:auth-service::login_user
///   - endpoint:auth-service::POST /login
///   - db:project::main_db
///   - table:main_db::public.users
///   - column:public.users::email

pub fn service_id(name: &str) -> String {
    format!("service:{name}")
}

pub fn module_id(service: &str, module_path: &str) -> String {
    format!("module:{service}::{module_path}")
}

pub fn function_id(service: &str, qualified_name: &str) -> String {
    format!("func:{service}::{qualified_name}")
}

pub fn endpoint_id(service: &str, method: &str, path: &str) -> String {
    format!("endpoint:{service}::{method} {path}")
}

pub fn database_id(project: &str, db_name: &str) -> String {
    format!("db:{project}::{db_name}")
}

pub fn table_id(db_name: &str, schema_table: &str) -> String {
    format!("table:{db_name}::{schema_table}")
}

pub fn column_id(schema_table: &str, column_name: &str) -> String {
    format!("column:{schema_table}::{column_name}")
}

pub fn index_id(schema_table: &str, index_name: &str) -> String {
    format!("index:{schema_table}::{index_name}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_id_formats() {
        assert_eq!(service_id("auth-service"), "service:auth-service");
        assert_eq!(
            function_id("auth-service", "login_user"),
            "func:auth-service::login_user"
        );
        assert_eq!(
            endpoint_id("auth-service", "POST", "/login"),
            "endpoint:auth-service::POST /login"
        );
        assert_eq!(
            table_id("main_db", "public.users"),
            "table:main_db::public.users"
        );
    }
}
