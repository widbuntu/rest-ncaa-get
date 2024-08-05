use actix_web::{web, HttpResponse, Responder};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::io::Read;
use ssh2::Session;
use std::net::TcpStream;

pub struct SSHRepository {
    host: String,
    port: u16,
    username: String,
    password: String,
}

impl SSHRepository {
    pub fn new(host: String, port: u16, username: String, password: String) -> SSHRepository {
        SSHRepository {
            host,
            port,
            username,
            password,
        }
    }

    pub fn execute_query(&self, sql: &str) -> Result<String, Box<dyn std::error::Error>> {
        let tcp = TcpStream::connect(format!("{}:{}", self.host, self.port))?;
        let mut sess = Session::new()?;
        sess.set_tcp_stream(tcp);
        sess.handshake()?;
        sess.userauth_password(&self.username, &self.password)?;

        let command = format!("sqlite3 ncaa.db \"{}\"", sql);
        let mut channel = sess.channel_session()?;
        channel.exec(&command)?;
        
        let mut output = String::new();
        channel.read_to_string(&mut output)?;
        channel.wait_close()?;

        Ok(output)
    }

    pub fn get_teams(&self) -> Result<String, Box<dyn std::error::Error>> {
        let sql = "SELECT * FROM ncaa_teams where team_name != 'Institution';";
        self.execute_query(sql)
    }

    pub fn get_current_teams_view(&self) -> Result<String, Box<dyn std::error::Error>> {
        let sql = "select distinct 
                            ncaa_team_hist.team_id,
                            ncaa_team_hist.conference,
                            ncaa_team_hist.division,
                            ncaa_teams.team_name,
                            ncaa_seas.team_img_url
                            from ncaa_team_hist 
                            left join ncaa_teams ON ncaa_team_hist.team_id = ncaa_teams.team_id 
                            left join ncaa_seas ON ncaa_team_hist.team_id = ncaa_seas.team_id
                            where ncaa_teams.team_name != '-' 
                            and ncaa_team_hist.conference != '-' 
                            and ncaa_team_hist.Year == '2024-25' 
                            order by conference;";
        self.execute_query(sql)
    }

    pub fn get_team_history(&self, team_id: Option<i32>) -> Result<String, Box<dyn std::error::Error>> {
        let sql = if let Some(id) = team_id {
            format!(
                "SELECT nt.team_name, nth.year, nth.head_coaches, nth.division, nth.conference, nth.wins, nth.losses
                 FROM ncaa_team_hist nth
                 JOIN ncaa_teams nt ON nth.team_id = nt.team_id
                 WHERE nth.team_id = {}
                 ORDER BY nth.year DESC",
                id
            )
        } else {
            String::from(
                "SELECT nt.team_name, nth.year, nth.head_coaches, nth.division, nth.conference, nth.wins, nth.losses
                 FROM ncaa_team_hist nth
                 JOIN ncaa_teams nt ON nth.team_id = nt.team_id
                 ORDER BY nt.team_name, nth.year DESC"
            )
        };

        self.execute_query(&sql)
    }
    
    pub fn get_team_season(&self, team_id: Option<i32>, season: Option<String>) -> Result<String, Box<dyn std::error::Error>> {
        let mut sql = "SELECT nt.team_name, nth.date, nth.opponent, nth.result, nth.attendance
                        FROM ncaa_seas nth
                        JOIN ncaa_teams nt ON nth.team_id = nt.team_id".to_string();
        
        let mut conditions = Vec::new();
        
        if let Some(id) = team_id {
            conditions.push(format!("nth.team_id = {}", id));
        }
        
        if let Some(season) = season {
            conditions.push(format!("SUBSTR(nth.date, -4) = '{}'", season));
        }
        
        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }
        
        sql.push_str(" ORDER BY nth.date DESC");
    
        // Debugging output
        println!("Generated SQL Query: {}", sql);
        
        self.execute_query(&sql)
    }

    pub fn get_current_season(&self, team_id: Option<i32>, conference: Option<String>) -> Result<String, Box<dyn std::error::Error>> {
        let mut sql = "WITH match_data AS (
        SELECT
            mi.date,
            mi.match_id,
            cs.team_id,
            cs.opponent,
            CASE
                WHEN INSTR(cs.opponent, '@') > 0 THEN SUBSTR(cs.opponent, 1, INSTR(cs.opponent, '@') - 1)
                ELSE cs.opponent
            END AS away_team,
            CASE
                WHEN INSTR(cs.opponent, '@') > 0 THEN SUBSTR(cs.opponent, INSTR(cs.opponent, '@') + 1)
                ELSE ''
            END AS home_team,
            cs.team_img_url,
            tm.team_name,
            th.conference,
            CASE
                WHEN INSTR(cs.opponent, '@') > 0 THEN 'away'
                ELSE 'home'
            END AS team_type
        FROM current_ncaa_season cs
        LEFT JOIN match_id_by_season mi ON cs.date = mi.date AND cs.game_by_game_url = mi.game_by_game_url
        LEFT JOIN ncaa_teams tm ON cs.team_id = tm.team_id
        LEFT JOIN ncaa_team_hist th ON cs.team_id = th.team_id
        WHERE th.year = '2024-25' AND th.conference != ''
    )
    SELECT
        m1.date,
        m1.match_id,
        m1.team_id AS home_team_id,
        m2.team_id AS away_team_id,
        REPLACE(COALESCE(m2.team_name, m1.team_name), 'A&amp;', 'A&M') AS home_team,
        REPLACE(COALESCE(m1.team_name, m2.team_name), 'A&amp;', 'A&M') AS away_team,
        COALESCE(m2.team_img_url, m1.team_img_url) AS home_team_img_url,
        COALESCE(m1.team_img_url, m2.team_img_url) AS away_team_img_url,
        COALESCE(m2.conference, m1.conference) AS home_team_conference,
        COALESCE(m1.conference, m2.conference) AS away_team_conference,
        CASE
            WHEN m1.home_team != '' AND m1.away_team != '' THEN 'neutral'
            ELSE REPLACE(COALESCE(m2.team_name, m1.team_name), 'A&amp;', 'A&M')
        END AS location
    FROM match_data m1
    LEFT JOIN match_data m2 ON m1.match_id = m2.match_id AND m1.team_id != m2.team_id
    WHERE (m1.team_type = 'away' OR (m1.team_type = 'home' AND m2.team_id IS NULL))".to_string();
    
        if let Some(id) = team_id {
            sql.push_str(&format!(" AND (m1.team_id = {} OR m2.team_id = {})", id, id));
        }
    
        if let Some(conf) = conference {
            sql.push_str(&format!(" AND (m1.conference = '{}' OR m2.conference = '{}')", conf, conf));
        }
    
        sql.push_str(" GROUP BY m1.match_id ORDER BY m1.date");
    
        self.execute_query(&sql)
    }
}


pub async fn get_teams(repo: web::Data<Arc<SSHRepository>>) -> impl Responder {
    match repo.get_ref().get_teams() {
        Ok(result) => {
            let lines: Vec<&str> = result.trim().split('\n').collect();
            let headers = vec!["team_id", "team"];
            
            let data: Vec<Value> = lines.iter().map(|line| {
                let values: Vec<&str> = line.split('|').collect();
                let mut obj = json!({});
                for (i, &header) in headers.iter().enumerate() {
                    if i < values.len() {
                        obj[header] = json!(values[i].trim());
                    }
                }
                obj
            }).collect();

            HttpResponse::Ok().json(json!({ "data": data }))
        },
        Err(e) => HttpResponse::InternalServerError().json(json!({
            "error": e.to_string()
        })),
    }
}

pub async fn get_current_teams_view(repo: web::Data<Arc<SSHRepository>>) -> impl Responder {
    match repo.get_ref().get_current_teams_view() {
        Ok(result) => {
            let lines: Vec<&str> = result.trim().split('\n').collect();
            let headers = vec!["team_id", "conference", "division", "team", "team_img"];
            
            let data: Vec<Value> = lines.iter().map(|line| {
                let values: Vec<&str> = line.split('|').collect();
                let mut obj = json!({});
                for (i, &header) in headers.iter().enumerate() {
                    if i < values.len() {
                        obj[header] = json!(values[i].trim());
                    }
                }
                obj
            }).collect();

            HttpResponse::Ok().json(json!({ "data": data }))
        },
        Err(e) => HttpResponse::InternalServerError().json(json!({
            "error": e.to_string()
        })),
    }
}

pub async fn get_team_history(repo: web::Data<Arc<SSHRepository>>, query: web::Query<HashMap<String, String>>) -> impl Responder {
    let team_id = query.get("team_id").and_then(|id| id.parse().ok());
    
    match repo.get_team_history(team_id) {
        Ok(result) => {
            let lines: Vec<&str> = result.trim().split('\n').collect();
            let headers = vec!["TeamName", "Year", "HeadCoaches", "Division", "Conference", "Wins", "Losses"];
            
            let data: Vec<Value> = lines.iter().map(|line| {
                let values: Vec<&str> = line.split('|').collect();
                let mut obj = json!({});
                for (i, &header) in headers.iter().enumerate() {
                    if i < values.len() {
                        obj[header] = json!(values[i].trim());
                    }
                }
                obj
            }).collect();

            HttpResponse::Ok().json(json!({ "data": data }))
        },
        Err(e) => HttpResponse::InternalServerError().json(json!({
            "error": e.to_string()
        })),
    }
}

pub async fn get_team_season(repo: web::Data<Arc<SSHRepository>>, query: web::Query<HashMap<String, String>>) -> impl Responder {
    // Extract `team_id` and `season` from the query parameters
    let team_id: Option<i32> = query.get("team_id").and_then(|id| id.parse().ok());
    let season: Option<String> = query.get("season").cloned();

    // Match on `team_id` and `season` to construct the appropriate query
    match repo.get_team_season(team_id, season) {
        Ok(result) => {
            let lines: Vec<&str> = result.trim().split('\n').collect();
            let headers = vec!["TeamName", "Date", "Opponent", "Result", "Attendance"];
            
            let data: Vec<Value> = lines.iter().map(|line| {
                let values: Vec<&str> = line.split('|').collect();
                let mut obj = json!({});
                for (i, &header) in headers.iter().enumerate() {
                    if i < values.len() {
                        obj[header] = json!(values[i].trim());
                    }
                }
                obj
            }).collect();

            HttpResponse::Ok().json(json!({ "data": data }))
        },
        Err(e) => HttpResponse::InternalServerError().json(json!({
            "error": e.to_string()
        })),
    }
}

pub async fn get_current_season(repo: web::Data<Arc<SSHRepository>>, query: web::Query<HashMap<String, String>>) -> impl Responder {
    let team_id = query.get("team_id").and_then(|id| id.parse().ok());
    let conference = query.get("conference").cloned();

    match repo.get_current_season(team_id, conference) {
        Ok(result) => {
            let lines: Vec<&str> = result.trim().split('\n').collect();
            let headers = vec!["Date", "MatchID", "HomeTeamID", "AwayTeamID", "HomeTeam", "AwayTeam", "HomeTeamImgUrl", "AwayTeamImgUrl", "HomeTeamConference", "AwayTeamConference", "Location"];

            let data: Vec<Value> = lines.iter().map(|line| {
                let values: Vec<&str> = line.split('|').collect();
                let mut obj = json!({});
                for (i, &header) in headers.iter().enumerate() {
                    if i < values.len() {
                        obj[header] = json!(values[i].trim());
                    }
                }
                obj
            }).collect();
            HttpResponse::Ok().json(json!({ "data": data }))
        },
        Err(e) => HttpResponse::InternalServerError().json(json!({
            "error": e.to_string()
        })),
    }
}