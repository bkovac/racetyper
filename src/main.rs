#[macro_use]
extern crate diesel;

use std::time::{Duration, Instant};

use actix::prelude::*;
use actix::{Actor, StreamHandler};
use actix_files as fs;
use actix_web::{middleware, web, App, Error, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws;

use serde::{Deserialize, Serialize};

use diesel::sqlite::SqliteConnection;
mod db;

mod cmd;


const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

async fn ws_index(r: HttpRequest, stream: web::Payload) -> Result<HttpResponse, Error> {
    println!("{:?}", r);
    let res = ws::start(MyWebSocket::new(), &r, stream);
    res
}

struct MyWebSocket {
    hb: Instant,

    dbconn: SqliteConnection,
    input_buffer: Vec<InputChange>,
    text_id: i32,
    text_len: i32,
}

impl Actor for MyWebSocket {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct WsData {
    #[serde(rename = "type")]
    typ: String,

    text: Option<String>,
    data: Option<String>,
    change: Option<String>,
    ts: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug)]
struct InputChange {
    data: Option<String>,
    change: String,
    ts: i64,
}

fn calculate_wpm(chars: i32, millis: i64) -> i32 {
    let calc_wpm = ((chars as f64) / 5.0_f64) * (60.0_f64 / ((millis as f64 / 1000.0_f64)));
    return calc_wpm.round() as i32
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for MyWebSocket {
    fn handle(
        &mut self,
        msg: Result<ws::Message, ws::ProtocolError>,
        ctx: &mut Self::Context,
    ) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                self.hb = Instant::now();
            }
            Ok(ws::Message::Text(text)) => {
                let data : WsData = match serde_json::from_str(&text) {
                    Ok(data) => data,
                    Err(err) => {
                        eprintln!("serde_json from_str error: {}", err);
                        return;
                    }
                };

                let mut resp = WsData {
                    typ: "Unknown".to_owned(), 
                    text: None,
                    data: None,
                    change: None,
                    ts: None,
                };


                match data.typ.as_ref() {
                    "refresh" => {
                        match db::get_random_typing_text(&self.dbconn) {
                            Ok(nt) => {
                                resp.typ = "text".to_owned();
                                self.text_id = nt.id;
                                self.text_len = nt.text.len() as i32;
                                resp.text = Some(nt.text);
                                self.input_buffer.clear();
                            },
                            Err(e) => {
                                eprintln!("get_random_typing_text error occured: {:?}", e);
                                return;
                            }
                        };
                    },
                    "change" => {
                        let change = match data.change {
                            Some(c) => c,
                            None => {
                                println!("Change object missing change!");
                                return;
                            },
                        };
                        
                        let ts = match data.ts {
                            Some(t) => t,
                            None => {
                                println!("Timestamp object missing change!");
                                return;
                            },
                        };

                        let inputchg = InputChange {
                            data: data.data,
                            change: change,
                            ts: ts,
                        };
                        self.input_buffer.push(inputchg);

                        return;
                    },
                    "done" => {
                        println!("Done!\nTook {:?} tm units.", data.ts);
                        println!("Serializing...");
                        let s_session = match serde_json::to_string(&self.input_buffer) {
                            Ok(s) => s,
                            Err(err) => {
                                eprintln!("serde_json to_string error: {}", err);
                                return;
                            }
                        };
                        let session = db::models::NewTypingSession {
                            inputs: s_session,
                            wpm: calculate_wpm(self.text_len, self.input_buffer.last().unwrap().ts),
                            wpm80: 0,
                            parent: self.text_id,
                        };
                        match db::create_typing_session(&self.dbconn, &session) {
                            Ok(x) => {
                                println!("Session save OK, id: {}", x.id);
                            },
                            Err(e) => {
                                eprintln!("Session save ERR: {:?}", e);
                            }
                        };
                        return;
                    }
                    _ => {
                        println!("Handler for type {} not yet implemented!", data.typ);
                        return;
                    }
                };

                match serde_json::to_string(&resp) {
                    Ok(sresp) => ctx.text(sresp),
                    Err(err) => {
                        eprintln!("serde_json to_string error: {}", err);
                        return;
                    }
                };
            },
            Ok(ws::Message::Binary(_bin)) => {
                println!("Binary messages not supported!");
            },
            Ok(ws::Message::Close(_)) => {
                ctx.stop();
            }
            _ => ctx.stop(),
        }
    }
}

impl MyWebSocket {
    fn new() -> Self {
        Self { hb: Instant::now(), dbconn: db::establish_connection(), input_buffer: Vec::new(), text_id: -1, text_len: -1 }
    }

    fn hb(&self, ctx: &mut <Self as Actor>::Context) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                println!("Websocket Client heartbeat failed, disconnecting!");
                ctx.stop();
                return;
            }

            ctx.ping(b"");
        });
    }
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "actix_server=info,actix_web=info");
    env_logger::init();

    match cmd::parse_cmdline().await {
        Ok(flag) => {
            if flag {
                return Ok(());
            }
            //else just continue
        },
        Err(e) => {
            return Err(e);
        },
    };

    HttpServer::new(|| {
        App::new().wrap(middleware::Logger::default())
                  .service(web::resource("/ws/").route(web::get().to(ws_index)))
                  .service(fs::Files::new("/", "static/").index_file("index.html"))
    }).bind("0.0.0.0:8080")?.run().await
}
