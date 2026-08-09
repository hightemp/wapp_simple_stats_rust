//! Application bootstrap for the Simple Stats server.

use std::error::Error;

use rocket::fs::FileServer;
use rocket::{catchers, routes, Build, Rocket};
use rocket_dyn_templates::Template;

mod config;
mod database;
mod models;
mod routes;
mod security;
mod utils;

use config::AppConfig;
use routes::counter::get_counter;
use routes::home::get_root;
use routes::statistics::{
    get_statistics, get_statistics_path, get_statistics_recent, get_statistics_self,
    get_statistics_self_full_json,
};
use security::{unauthorized, SecurityHeaders};

pub(crate) type HandlerResult<T> = Result<T, rocket::http::Status>;

fn build_rocket(config: AppConfig) -> Rocket<Build> {
    rocket::build()
        .manage(config)
        .attach(SecurityHeaders)
        .attach(Template::fairing())
        .mount("/assets", FileServer::from("static"))
        .mount(
            "/",
            routes![
                get_counter,
                get_root,
                get_statistics,
                get_statistics_path,
                get_statistics_recent,
                get_statistics_self,
                get_statistics_self_full_json,
            ],
        )
        .register("/", catchers![unauthorized])
}

/// Loads configuration, initializes SQLite and starts the HTTP server.
///
/// # Errors
///
/// Returns an error when configuration, database initialization or Rocket startup fails.
pub async fn run() -> Result<(), Box<dyn Error>> {
    let config = config::load()?;
    database::initialize(&config)?;
    build_rocket(config)
        .launch()
        .await
        .map_err(|error| Box::new(error) as Box<dyn Error>)?;
    Ok(())
}
