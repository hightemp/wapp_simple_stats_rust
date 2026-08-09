use rocket::{get, State};
use rocket_dyn_templates::Template;
use serde_json::json;

use crate::config::AppConfig;

#[get("/")]
pub(crate) fn get_root(config: &State<AppConfig>) -> Template {
    Template::render(
        "landing",
        json!({
            "site_title": config.site_title(),
            "page_title": "Home",
            "active_page": "home",
            "theme_auto": config.theme_auto(),
        }),
    )
}
