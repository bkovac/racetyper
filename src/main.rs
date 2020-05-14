#[macro_use]
extern crate diesel;

use std::time::{Duration, Instant};
use std::iter::Iterator;

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
    points: Option<Vec<MinimumNode>>
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

#[derive(Serialize, Deserialize, Debug)]
struct MinimumNode {
    text: String,
    nomistakes: i64,
    time: i64,
    wpm: i64,
    rel: u64,
}

fn analyze_speed(ics: &Vec<InputChange>, txt: &String, nosplits: usize) -> Result<Vec<MinimumNode>, ()>{
    println!("len: {}", ics.len());
    println!("segment len: {}", ics.len()/nosplits);
    let nspaces : Vec<&str> = txt.split(' ').collect();
    println!("nospaces: {}", nspaces.len());

    if nspaces.len() < nosplits {
        println!("nspaces < nosplits");
        return Err(());
    }
    let divider_f = nspaces.len() as f64 / nosplits as f64;
    let mut remainder = 0;
    if divider_f.fract() != 0.0 {
        remainder = nspaces.len() % nosplits;
    }
    let divider = divider_f.round() as usize;

    println!("text is: {}", txt);

    let mut nodes = Vec::<MinimumNode>::new();

    let mut tmpw = Vec::<&str>::with_capacity(divider);
    let mut splcnt = 0;
    for word in &nspaces {
        if splcnt == divider {
            let x = MinimumNode{text: tmpw.join(" "), time: -1, nomistakes: -1, wpm: -1, rel: 0};
            nodes.push(x);
            tmpw.clear();
            splcnt = 0;
        }
        tmpw.push(word);
        splcnt += 1;
    }
    if remainder > 0 {
        let x = MinimumNode{text: tmpw.join(" "), time: -1, nomistakes: -1, wpm: -1, rel: 0};
        nodes.push(x);
    }

    println!("nodes: {:?}", nodes);

    let mut tmp_cw = String::new();
    let mut tmp_cw_start : i64 = 0;
    let mut fw_iter = nodes.iter_mut();
    let mut fw_iter_c = fw_iter.next().unwrap();
    let mut num_entered = 0;
    for ic in ics {
        match ic.change.as_str() {
            "insertText" => {
                match &ic.data {
                    Some(k) => {
                        num_entered += k.len();
                        if k == " " || k == "\0" {
                            if fw_iter_c.text == tmp_cw {
                                let delta = ic.ts - tmp_cw_start;
                                fw_iter_c.time = delta;
                                fw_iter_c.nomistakes = (num_entered - tmp_cw.len()) as i64;
                                if k == "\0" {
                                    break;
                                }

                                tmp_cw_start = ic.ts;
                                fw_iter_c = fw_iter.next().unwrap();
                                tmp_cw.clear();
                                num_entered = 0;
                            } else {
                                tmp_cw += k;
                            }
                        } else {
                            tmp_cw += k;
                        }
                    },
                    None => {
                        println!("Malformed insertText");
                    },
                };
            },
            "deleteContentBackward" => {
                match ic.data {
                    Some(_) => {
                        println!("Malformed deleteContentBackward");
                    },
                    None => {
                        tmp_cw.pop();
                        num_entered += 1;
                    },
                };
            },
            _ => (),
        };
    }

    let line_height: f64 = 75.0;
    let line_wpm: f64 = 100.0;
    for mut mde in &mut nodes {
        mde.wpm = ((60.0_f64 * (mde.text.len() as f64) / 5.0_f64) / (mde.time as f64 / 1000.0_f64)).round() as i64;
        mde.rel = ((line_height / line_wpm) * mde.wpm as f64).round() as u64;
        println!("wpm for text: '{}' is: {} with {} mistakes at rel: {}", mde.text, mde.wpm, mde.nomistakes, mde.rel);
    }

    Ok(nodes)
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
                    points: None,
                };


                match data.typ.as_ref() {
                    "refresh" => {
                        match db::get_random_typing_text(&self.dbconn) {
                            Ok(nt) => {
                                resp.typ = "text".to_owned();
                                self.text_id = nt.id;
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
                        self.input_buffer.push(InputChange{change: "insertText".to_string(), data: Some("\0".to_string()), ts: self.input_buffer.last().unwrap().ts});
                        let rel_text = match db::get_typing_text_with_id(&self.dbconn, self.text_id) {
                            Ok(txt) => txt,
                            Err(err) => {
                                eprintln!("db get_typing_text_with_id error: {:?}", err);
                                return;
                            },
                        };
                        match analyze_speed(&self.input_buffer, &rel_text.text, 20) {
                            Err(_) => {
                                println!("analyze_speed error");
                                return;
                            },
                            Ok(nodes) => {
                                resp.points = Some(nodes);
                                resp.typ = "graph".to_string();
                            },
                        };

                        let s_session = match serde_json::to_string(&self.input_buffer) {
                            Ok(s) => s,
                            Err(err) => {
                                eprintln!("serde_json to_string error: {}", err);
                                return;
                            }
                        };
                        let session = db::models::NewTypingSession {
                            inputs: s_session,
                            wpm: calculate_wpm(rel_text.text.len() as i32, self.input_buffer.last().unwrap().ts),
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
                        //return;
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
        Self { hb: Instant::now(), dbconn: db::establish_connection(), input_buffer: Vec::new(), text_id: -1 }
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
