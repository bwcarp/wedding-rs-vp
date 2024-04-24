#[macro_use] extern crate rocket;
extern crate diesel;
use chrono::prelude::*;
use models::Guest;
use rocket::form::Form;
use rocket::response::Redirect;
use rocket::http::CookieJar;
use std::net::IpAddr;
use std::env;
use rand::{distributions::Alphanumeric, Rng};
use rocket_db_pools::deadpool_redis::redis::AsyncCommands;
use rocket_dyn_templates::{Template, context};
use rocket_db_pools::{Database, Connection, deadpool_redis};
use rocket_db_pools::diesel::MysqlPool;
use rocket_db_pools::diesel::prelude::*;
use rocket_db_pools::diesel::{dsl::*, RunQueryDsl};
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use lettre::message::Mailboxes;
use lettre::message::header;

pub mod models;
pub mod schema;

#[derive(Database)]
#[database("rsvp")]
struct Db(MysqlPool);

#[derive(Database)]
#[database("redis")]
struct Redis(deadpool_redis::Pool);

#[derive(FromForm)]
struct Invitation {
    code: String,
}

#[derive(FromForm)]
struct Rsvp {
    accepted: String,
    guest_dietary_restrictions: String,
    plus_one_name: Option<String>,
    plus_one_dietary_restrictions: Option<String>,
}

#[derive(FromForm)]
struct Add {
    guest_name: String,
    accepted: String,
    guest_dietary_restrictions: String,
    plus_one_allowed: String,
    plus_one_name: Option<String>,
    plus_one_dietary_restrictions: Option<String>,
}

fn get_invite_cookie(cookies: &CookieJar<'_>) -> String {
    let invite_code = cookies.get_private("invite_code");
    match invite_code {
        Some(c) => c.value().to_string(),
        None => String::from(""),
    }
}

fn parse_ip(client_ip: Option<IpAddr>) -> String {
    let ip: String;

    match client_ip.unwrap() {
        IpAddr::V4(ip4) => {
            ip = ip4.to_string();
        },
        IpAddr::V6(ip6) => {
            let ipb = ip6.segments();
            ip = format!("{:04x}:{:04x}:{:04x}:{:04x}::/64", ipb[0], ipb[1], ipb[2], ipb[3]);
        },
    }
    ip
}

async fn check_ip_ban(r: &mut Connection<Redis>, client_ip: Option<IpAddr>) -> Option<Template> {

    let ip = parse_ip(client_ip);
    let failures = r.get::<&String, i32>(&ip).await;
    if failures.is_ok() {
        let f = failures.unwrap();
        if f >= 5 {
            Some(Template::render("fuckyou", context! {
                ip_addr: ip,
                failures: f,
            }))
        } else {
            None
        }
    } else {
        None
    }
}

async fn check_invite_ban(r: &mut Connection<Redis>, invite_code: &String) -> Option<Template> {

    let submissions = r.get::<&String, i32>(&invite_code).await;
    if submissions.is_ok() {
        let s = submissions.unwrap();
        if s >= 5 {
            Some(Template::render("slowdown", context! {}))
        } else {
            None
        }
    } else {
        None
    }
}

async fn set_ip_failure(r: &mut Connection<Redis>, client_ip: Option<IpAddr>) {
    let ip = parse_ip(client_ip);

    let failures = r.get::<&String, i32>(&ip).await;
    if failures.is_ok() {
        let error_msg: Result<bool, deadpool_redis::redis::RedisError> = r.incr(&ip, 1).await;
        println!("{}", error_msg.unwrap());
    } else {
        let error_msg: Result<String, deadpool_redis::redis::RedisError> = r.set::<&String, i32, String>(&ip, 1).await;
        println!("{}", error_msg.unwrap());
    }
    _ = r.expire::<&String, i64>(&ip, 86400).await;
}

async fn set_invite_counter(r: &mut Connection<Redis>, invite_code: &String) {

    let submissions = r.get::<&String, i32>(&invite_code).await;
    if submissions.is_ok() {
        let error_msg: Result<bool, deadpool_redis::redis::RedisError> = r.incr(&invite_code, 1).await;
        println!("{}", error_msg.unwrap());
    } else {
        let error_msg: Result<String, deadpool_redis::redis::RedisError> = r.set::<&String, i32, String>(&invite_code, 1).await;
        println!("{}", error_msg.unwrap());
    }
    _ = r.expire::<&String, i64>(&invite_code, 86400).await;
}

fn email_us(guest: &Guest) {

    let smtp_password = env::var("SMTP_PASSWORD").unwrap();
    let response: &str = match guest.accepted {
        Some(resp) => {
            if resp {
                "accepted"
            } else {
                "declined"
            }
        },
        None => {
            "BROKEN THE FORM OF"
        }
    };

    let diet: String = match &guest.guest_dietary_restrictions {
        Some(diet) => diet.to_string(),
        None => String::from("N/A"),
    };

    let creds = Credentials::new("noreply@blakeandmellie.wedding".to_owned(), smtp_password.to_string());

    let email_subject = format!("{} has {} your invitation.", guest.guest_name, response);

    let mut email_body = format!(r#"
        {} has {} your invitation.
        Dietary restrictions: {}
    "#, guest.guest_name, response, diet);

    if guest.plus_one_allowed {
        let name: String = match &guest.plus_one_name {
            Some(name) => name.to_string(),
            None => String::from("NO GUEST")
        };
        let diet: String = match &guest.plus_one_dietary_restrictions {
            Some(diet) => diet.to_string(),
            None => String::from("N/A"),
        };
        let plus_one: String = format!(r#"
            Plus one name: {}
            Plus one dietary restrictions: {}
        "#, name, diet);
        email_body.push_str(&plus_one);
    }

    let to_address = "Blake Hartshorn <redacted@forgithub>, Melissa  <redacted@forgithub>";
    let mailboxes: Mailboxes = to_address.parse().unwrap();
    let to_header: header::To = mailboxes.into();

    let email = Message::builder()
        .mailbox(to_header)
        .from("Wedding RSVP Mailer <noreply@blakeandmellie.wedding>".parse().unwrap())
        .reply_to("Blake Hartshorn <redacted@forgithub>".parse().unwrap())
        .subject(email_subject)
        .header(ContentType::TEXT_PLAIN)
        .body(email_body)
        .unwrap();

    let mailer = SmtpTransport::starttls_relay("redacted.forgithub")
        .unwrap()
        .credentials(creds)
        .build();

    match mailer.send(&email) {
        Ok(_) => println!("Email sent successfully!"),
        Err(e) => println!("Could not send email: {e:?}"),
    };
}

#[get("/rsvp")]
async fn index(mut r: Connection<Redis>, client_ip: Option<IpAddr>) -> Template {

    let is_banned = check_ip_ban(&mut r, client_ip).await;
    if is_banned.is_some() {
        return is_banned.unwrap();
    }
    Template::render("index", context! {})
}

#[get("/rsvp/fuckyou")]
async fn fuck_you(mut r: Connection<Redis>, client_ip: Option<IpAddr>) -> Template {
    let ip = parse_ip(client_ip);
    let failures = r.get::<&String, i32>(&ip).await;

    Template::render("fuckyou", context! {
        ip_addr: ip,
        failures: failures.unwrap(),
    })
}

#[post("/rsvp/authenticate", data = "<invitation>")]
async fn login(invitation: Form<Invitation>, cookies: &CookieJar<'_>, mut db: Connection<Db>, mut r: Connection<Redis>, client_ip: Option<IpAddr>) -> Redirect {

    let is_banned = check_ip_ban(&mut r, client_ip).await;
    if is_banned.is_some() {
        return Redirect::to(uri!("/rsvp/fuckyou"));
    }

    let code = invitation.code.clone().replace("-","");

    use self::schema::guests::dsl::*;

    let on_guest_list = select(exists(guests.filter(id.eq(&code)))).get_result(&mut db).await;

    match on_guest_list {
        Ok(true) => {
            cookies.add_private(("invite_code", code));
            return Redirect::to(uri!("/rsvp/form"));
        },
        Ok(false) => {
            set_ip_failure(&mut r, client_ip).await;
            return Redirect::to(uri!("/rsvp/fuckyou"));
        },
        Err(_) => Redirect::to(uri!("/rsvp")),
    }

}

#[get("/rsvp/form")]
async fn rsvp_form(cookies: &CookieJar<'_>, mut db: Connection<Db>, mut r: Connection<Redis>, client_ip: Option<IpAddr>) -> Template {

    let is_banned = check_ip_ban(&mut r, client_ip).await;
    if is_banned.is_some() {
        return is_banned.unwrap();
    }
    let code = get_invite_cookie(cookies);
    set_invite_counter(&mut r, &code).await;
    let submission_spam = check_invite_ban(&mut r, &code).await;
    if submission_spam.is_some() {
        return submission_spam.unwrap();
    }
    use self::schema::guests::dsl::guests;
    let guest = guests
        .find(&code)
        .select(Guest::as_select())
        .first(&mut db).await.unwrap();

    let diet: String = match guest.guest_dietary_restrictions {
        Some(diet) => diet,
        None => "".to_string(),
    };

    if guest.plus_one_allowed {

        let plus_one_name: String = match guest.plus_one_name {
            Some(pone_name) => pone_name,
            None => "".to_string(),
        };
        let plus_one_dietary_restrictions: String = match guest.plus_one_dietary_restrictions {
            Some(pone_diet) => pone_diet,
            None => "".to_string(),
        };

        let ctx = context! {
            invite_code: &code,
            name: &guest.guest_name,
            guest_dietary_restrictions: diet,
            plus_one_allowed: true,
            plus_one_name: plus_one_name,
            plus_one_dietary_restrictions: plus_one_dietary_restrictions,
        };
        Template::render("form", ctx)
    } else {
        let ctx = context! {
            invite_code: &code,
            name: &guest.guest_name,
            guest_dietary_restrictions: diet,
            plus_one_allowed: false,
        };
        Template::render("form", ctx)
    }
}

#[get("/rsvp/submit")]
async fn login_redir() -> Redirect {
    Redirect::temporary("/")
}

#[post("/rsvp/submit", data = "<rsvp>")]
async fn rsvp_submit(rsvp: Form<Rsvp>, cookies: &CookieJar<'_>, mut r: Connection<Redis>, mut db: Connection<Db>) -> Template {
    let current_time = Utc::now().naive_utc();
    let code = get_invite_cookie(cookies);

    let submission_spam = check_invite_ban(&mut r, &code).await;
    if submission_spam.is_some() {
        return submission_spam.unwrap();
    }

    // read prior record
    if code.len() == 12 {
        use self::models::Guest;
        use self::schema::guests::dsl::*;
        let mut guest = guests
            .find(&code)
            .select(Guest::as_select())
            .first(&mut db).await.unwrap();
        let first_accepted = match guest.date_of_rsvp {
            Some(dt) => dt,
            None => current_time
        };

        let accept = match rsvp.accepted.as_str() {
            "yes" => true,
            _ => false,
        };

        diesel::update(guests.find(&code))
            .set((
                accepted.eq(accept),
                guest_dietary_restrictions.eq(&rsvp.guest_dietary_restrictions),
                date_of_rsvp.eq(first_accepted),
                last_modified.eq(now),
            ))
            .execute(&mut db)
            .await.unwrap();
        if guest.plus_one_allowed {
            diesel::update(guests.find(&code))
                .set((
                    plus_one_dietary_restrictions.eq(&rsvp.plus_one_dietary_restrictions),
                    plus_one_name.eq(&rsvp.plus_one_name)
                ))
                .execute(&mut db)
                .await.unwrap();            
        }

        guest = guests
            .find(&code)
            .select(Guest::as_select())
            .first(&mut db).await.unwrap();

        email_us(&guest);

        Template::render("thankyou", context! {
            name: guest.guest_name,
            accepted: guest.accepted,
            dietary: guest.guest_dietary_restrictions,
            plus_one_allowed: guest.plus_one_allowed,
            pone_name: guest.plus_one_name,
            pone_dietary: guest.plus_one_dietary_restrictions,
        })
    } else {
        Template::render("thankyou", context! {
            name: "",
            accepted: "",
            dietary: "",
            plus_one_allowed: false,
            pone_name: "",
            pone_dietary: "",
        })  
    }
}

#[get("/rsvp/admin")]
async fn admin(mut db: Connection<Db>) -> Template {

    use self::schema::guests::dsl::*;
    let attending: Vec<Guest> = guests
        .filter(accepted.eq(true))
        .order(guest_name)
        .load(&mut db)
        .await.unwrap();

    let declined: Vec<Guest> = guests
        .filter(accepted.eq(false))
        .order(guest_name)
        .load(&mut db)
        .await.unwrap();

    let noreply: Vec<Guest> = guests
        .filter(accepted.is_null())
        .order(guest_name)
        .load(&mut db)
        .await.unwrap();

    let plus_one_count: i64 = guests
        .filter(accepted.eq(true))
        .filter(plus_one_allowed.eq(true))
        .filter(plus_one_name.is_not_null())
        .filter(plus_one_name.ne(""))
        .count()
        .get_result(&mut db)
        .await.unwrap();

    Template::render("admin", context! { 
        attending: attending, 
        declined: declined,
        noreply: noreply,
        plus_one_count: plus_one_count,
    })
}

#[get("/rsvp/admin/edit/<invite_code>")]
async fn edit_invitation(invite_code: &str, mut db: Connection<Db>) -> Template {
    use self::schema::guests::dsl::guests;
    let guest = guests
        .find(invite_code)
        .select(Guest::as_select())
        .first(&mut db).await.unwrap();

    let diet: String = match guest.guest_dietary_restrictions {
        Some(diet) => diet,
        None => "".to_string(),
    };
    let plus_one_name: String = match guest.plus_one_name {
        Some(pone_name) => pone_name,
        None => "".to_string(),
    };
    let plus_one_dietary_restrictions: String = match guest.plus_one_dietary_restrictions {
        Some(pone_diet) => pone_diet,
        None => "".to_string(),
    };
    let ctx = context! {
        invite_code: &invite_code,
        name: &guest.guest_name,
        guest_dietary_restrictions: diet,
        plus_one_allowed: &guest.plus_one_allowed,
        plus_one_name: plus_one_name,
        plus_one_dietary_restrictions: plus_one_dietary_restrictions,
    };

    Template::render("edit", ctx)
}

#[post("/rsvp/admin/edit/<invite_code>", data = "<rsvp>")]
async fn submit_edit(invite_code: &str, rsvp: Form<Add>, mut db: Connection<Db>) -> Template {

    let current_time = Utc::now().naive_utc();

    use self::models::Guest;
    use self::schema::guests::dsl::*;
    let mut guest = guests
        .find(&invite_code)
        .select(Guest::as_select())
        .first(&mut db).await.unwrap();
    let first_accepted = match guest.date_of_rsvp {
        Some(dt) => dt,
        None => current_time
    };

    let accept = match rsvp.accepted.as_str() {
        "yes" => Some(true),
        "no" => Some(false),
        _ => None,
    };

    let plus_one_allow = match rsvp.plus_one_allowed.as_str() {
        "yes" => true,
        _ => false,
    };

    diesel::update(guests.find(&invite_code))
        .set((
            accepted.eq(accept),
            guest_name.eq(&rsvp.guest_name),
            guest_dietary_restrictions.eq(&rsvp.guest_dietary_restrictions),
            plus_one_allowed.eq(plus_one_allow),
            date_of_rsvp.eq(first_accepted),
            last_modified.eq(now),
        ))
        .execute(&mut db)
        .await.unwrap();
    if guest.plus_one_allowed {
        diesel::update(guests.find(&invite_code))
            .set((
                plus_one_dietary_restrictions.eq(&rsvp.plus_one_dietary_restrictions),
                plus_one_name.eq(&rsvp.plus_one_name)
            ))
            .execute(&mut db)
            .await.unwrap();            
    }

    guest = guests
        .find(&invite_code)
        .select(Guest::as_select())
        .first(&mut db).await.unwrap();

    Template::render("submit_edit", context! {
        invite_code: &invite_code,
        name: guest.guest_name,
        accepted: &rsvp.accepted,
        dietary: guest.guest_dietary_restrictions,
        plus_one_allowed: guest.plus_one_allowed,
        pone_name: guest.plus_one_name,
        pone_dietary: guest.plus_one_dietary_restrictions,
    })
 
}

#[get("/rsvp/admin/add")]
async fn add_guest_form() -> Template {    
    Template::render("add", context! {})
}

#[post("/rsvp/admin/add", data = "<rsvp>")]
async fn add_guest_submit(rsvp: Form<Add>, mut db: Connection<Db>) -> Template {    
    use self::schema::guests::dsl::*;
    let mut invite_code: String;

    loop {
        invite_code = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(12)
            .map(char::from)
            .collect();

        invite_code = invite_code.to_ascii_uppercase();

        let code_exists: bool = select(exists(guests.filter(id.eq(&invite_code)))).get_result(&mut db).await.unwrap();
        if !code_exists {
            break;
        }
    }

    let accept = match rsvp.accepted.as_str() {
        "yes" => Some(true),
        "no" => Some(false),
        _ => None,
    };

    let allow_plus_one = match rsvp.plus_one_allowed.as_str() {
        "true" => true,
        _ => false,
    };

    let pone_name: Option<String> = match &rsvp.plus_one_name {
        Some(s) => {
            if s == "" {
                None
            } else {
                Some(s.to_string())
            }
        }
        None => None
    };

    let pone_diet: Option<String> = match &rsvp.plus_one_dietary_restrictions {
        Some(s) => {
            if s == "" {
                None
            } else {
                Some(s.to_string())
            }
        }
        None => None
    };

    let new_guest = Guest{
        id: invite_code.to_string(),
        accepted: accept,
        guest_name: rsvp.guest_name.to_string(),
        guest_dietary_restrictions: Some(rsvp.guest_dietary_restrictions.to_string()),
        plus_one_allowed: allow_plus_one,
        plus_one_name: pone_name,
        plus_one_dietary_restrictions: pone_diet,
        date_of_rsvp: None,
        last_modified: None,
    };

    diesel::insert_into(guests)
        .values(new_guest)
        .execute(&mut db)
        .await.unwrap();

    Template::render("submit_edit", context! {
        invite_code: &invite_code,
        name: &rsvp.guest_name,
        accepted: &rsvp.accepted,
        dietary: &rsvp.guest_dietary_restrictions,
        plus_one_allowed: &rsvp.plus_one_allowed,
        pone_name: &rsvp.plus_one_name,
        pone_dietary: &rsvp.plus_one_dietary_restrictions,
    })
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/", routes![index, login, login_redir, rsvp_form, 
                            rsvp_submit, fuck_you, admin,
                            edit_invitation, submit_edit,
                            add_guest_form, add_guest_submit])
        .attach(Template::fairing())
        .attach(Db::init())
        .attach(Redis::init())
}