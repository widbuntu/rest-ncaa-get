use actix_web::{web, App, HttpServer};
use actix_cors::Cors;
use std::sync::Arc;
use crate::repository::ddb::{SSHRepository, get_team_history, get_teams, get_team_season, get_current_teams_view, get_current_season};

mod repository {
    pub mod ddb;
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let repo = Arc::new(SSHRepository::new(
        String::from("192.168.1.252"),
        22,
        String::from("widbuntu"),
        String::from("Office366!")
    ));

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(repo.clone()))
            .wrap(Cors::permissive())
            .route("/team-history", web::get().to(get_team_history))
            .route("/teams", web::get().to(get_teams))
            .route("/current-teams", web::get().to(get_current_teams_view))
            .route("/team-season", web::get().to(get_team_season))
            .route("/current-season", web::get().to(get_current_season))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}

// example http://127.0.0.1:8080/teams
// example http://127.0.0.1:8080/current-teams
// example http://127.0.0.1:8080/current-season
// example http://127.0.0.1:8080/teams-history
// example http://127.0.0.1:8080/team-history?team_id=414
// example http://127.0.0.1:8080/team-season?team_id=2&season=2022
