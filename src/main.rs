#![feature(custom_attribute)]

extern crate actix_web;
extern crate actix_session;
extern crate futures;
extern crate json;

use futures::{Future, Stream};
use std::sync::{Arc, Mutex};

use actix_web::{web, App, HttpResponse, HttpServer, Error as WError, Result as WResult};
use actix_web::http::StatusCode;
use actix_session::{CookieSession, Session};

struct Source {
    label: String,
    website: String,
}

struct Manga {
    id: u64,
    name: String,
    sources: Vec<Source>,
}

struct User {
    id: String,
    mangas: Vec<Manga>,
}

// FIXME: This string is only used for debug and will be removed once a database will be added.
#[derive(Clone, Debug)]
struct UserInfo {
    login: String,
    password: String,
    id: String,
}

impl UserInfo {
    fn new(login: &str, password: &str, id: &str) -> UserInfo {
        UserInfo {
            login: login.into(),
            password: password.into(),
            id: id.into(),
        }
    }
}

struct Server {
    // FIXME: Should be removed once a database is plugged!
    users: Vec<UserInfo>,
    port: u16,
    master_login: String,
    master_password: String,
}

fn init_server() -> Server {
    Server {
        users: vec![
            UserInfo::new("a", "a", "a"),
            UserInfo::new("b", "b", "b"),
            UserInfo::new("c", "c", "c"),
            UserInfo::new("d", "d", "d"),
            UserInfo::new("e", "e", "e"),
        ],
        port: 2345, // FIXME: should be loaded from json
        // For those two, if not provided by the JSON settings file, quit the program.
        master_login: "admin".into(), // FIXME: should be loaded from json
        master_password: "admin".into(), // FIXME: should be loaded from json
    }
}

fn get_id_from_session(session: &Session) -> Result<String, HttpResponse> {
    if let Some(id) = session.get::<String>("id")? {
        Ok(id)
    } else {
        // not logged in!
        Err(HttpResponse::Unauthorized().body("You need to log in!"))
    }
}

fn get_user_entry<'a>(
    session: &Session,
    server: &'a Server,
) -> Result<&'a UserInfo, HttpResponse> {
    match get_id_from_session(session) {
        Ok(id) => {
            for user in server.users.iter() {
                if user.id == id {
                    return Ok(user);
                }
            }
            Err(HttpResponse::NotFound().body("Unknown user"))
        }
        Err(e) => Err(e),
    }
}

/// Update a manga entry.
fn update_manga(state: web::Data<Arc<Mutex<Server>>>, session: Session, pl: web::Payload) -> impl Future<Item = HttpResponse, Error = WError> {
    pl.concat2().from_err().and_then(move |body| {
        // FIXME: this part should happen *before* receiving the JSON data in case the user
        //        isn't logged in.
        //
        // Before going any further, we need to check if the user is logged or not.
        match state.lock() {
            Ok(guard) => {
                match get_user_entry(&session, &*guard) {
                    Ok(x) => {
                        println!("update from {}", x.login);
                    }
                    Err(e) => return Err(e.into()),
                }
            }
            Err(e) => {
                return Err(HttpResponse::InternalServerError().body(&format!("cannot get server info: {}", e)).into())
            }
        }

        // body is loaded, now we can deserialize json-rust
        match json::parse(std::str::from_utf8(&body).unwrap()) {
            Ok(json::JsonValue::Object(j)) => {
                let mut manga: Option<String> = None;
                let mut source: Option<String> = None;
                let mut chapter: Option<u32> = None;
                let mut page: Option<u32> = None;
                for (key, entry) in j.iter() {
                    match key {
                        "manga" => {
                            if !entry.is_string() {
                                return Err(HttpResponse::BadRequest()
                                                        .body("manga should be a string").into())
                            }
                            manga = entry.as_str().map(|x| x.to_owned());
                        }
                        "source" => {
                            if !entry.is_string() {
                                return Err(HttpResponse::BadRequest()
                                                        .body("source should be a string").into())
                            }
                            source = entry.as_str().map(|x| x.to_owned());
                        }
                        "chapter" => {
                            if !entry.is_number() {
                                return Err(HttpResponse::BadRequest()
                                                        .body("chapter should be a number").into())
                            }
                            chapter = entry.as_u32();
                        }
                        "page" => {
                            if !entry.is_number() {
                                return Err(HttpResponse::BadRequest()
                                                        .body("page should be a number").into())
                            }
                            page = entry.as_u32();
                        }
                        _ => {
                            // we just ignore all other keys...
                        }
                    }
                }
                if manga.is_none() || source.is_none() || chapter.is_none() {
                    return Err(HttpResponse::BadRequest()
                                            .body("'manga', 'source' and 'chapter' are mandatory")
                                            .into())
                }
                println!("=> {}/{} [{}:{}]",
                         manga.unwrap(), source.unwrap(), chapter.unwrap(), page.unwrap_or_else(|| 0));
            }
            Ok(_) => return Err(HttpResponse::BadRequest().body("Expected JSON object").into()),
            Err(e) => return Err(HttpResponse::BadRequest().body(&format!("Expected JSON: {}", e)).into()),
        }

        Ok(HttpResponse::Ok().body("OK"))
    })
}

fn main() -> std::io::Result<()> {
    let server = init_server();
    let port = server.port;
    let server = Arc::new(Mutex::new(server));

    // TODO: Add an endpoint to register a new user.
    // TODO: Add an endpoint to login a user.
    //
    // TODO: Add an admin web page.
    println!("starting on 0.0.0.0:{}", port);
    HttpServer::new(move || {
        App::new()
            .data(Arc::clone(&server))
            .wrap(CookieSession::signed(&[0; 32]).secure(false))
            .data(web::JsonConfig::default().limit(4096)) // limit max message size
            .service(
                web::resource("/update").route(web::post().to_async(update_manga)),
            )
    })
    .bind(&format!("0.0.0.0:{}", port))?
    .run()
}
