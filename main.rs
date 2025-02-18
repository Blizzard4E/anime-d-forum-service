#[macro_use]
extern crate rocket;

use mysql::*;
use mysql::prelude::*;
use rocket::serde::{ json::Json };
use rocket::form::{ Form };
use serde::Serialize;
use uuid::Uuid;

#[derive(FromForm)]
struct NewUser {
    username: String,
    password: String,
    profile_url: Option<String>,
}

#[derive(Serialize)]
struct SessionResponse {
    session_token: String,
}

fn get_connection() -> Result<PooledConn> {
    let url = "mysql://root:root@localhost:3306/anime_d_verse_forum";
    let pool = Pool::new(url)?;
    pool.get_conn()
}
#[derive(Serialize)]
struct Thread {
    id: i32,
    title: String,
    author_id: i32,
    anime_id: i32,
    created_at: String,
    username: String, // New field for username
    profile_url: Option<String>, // New field for profile URL
    user_created_at: String,
    num_posts: i64, // New field for the count of posts
}

#[get("/threads")]
fn get_threads() -> Json<Vec<Thread>> {
    match get_connection() {
        Ok(mut conn) => {
            let result: Vec<Thread> = conn
                .query_map(
                    // Use JOIN to include username and profile_url from users table
                    r"
                    SELECT 
                        threads.id, 
                        threads.title, 
                        threads.author_id, 
                        threads.anime_id, 
                        threads.created_at, 
                        users.username, 
                        users.profile_url, 
                        users.created_at,
                        COUNT(posts.id) AS num_posts
                    FROM threads
                    JOIN users ON threads.author_id = users.id
                    LEFT JOIN posts ON posts.thread_id = threads.id
                    GROUP BY threads.id, users.id;
                    ",
                    |(
                        id,
                        title,
                        author_id,
                        anime_id,
                        created_at,
                        username,
                        profile_url,
                        user_created_at,
                        num_posts,
                    )| Thread {
                        id,
                        title,
                        author_id,
                        anime_id,
                        created_at,
                        username, // Map username from users table
                        profile_url, // Map profile_url from users table
                        user_created_at,
                        num_posts, // Map the post count
                    }
                )
                .unwrap_or_else(|_| vec![]);
            Json(result)
        }
        Err(_) => Json(vec![]),
    }
}

#[get("/threads/anime/<anime_id>")]
fn get_threads_by_anime(anime_id: i32) -> Json<Vec<Thread>> {
    match get_connection() {
        Ok(mut conn) => {
            let result: Vec<Thread> = conn
                .query_map(
                    // Use JOIN to include username and profile_url from users table
                    format!(r"
                    SELECT 
                        threads.id, 
                        threads.title, 
                        threads.author_id, 
                        threads.anime_id, 
                        threads.created_at, 
                        users.username, 
                        users.profile_url, 
                        users.created_at,
                        COUNT(posts.id) AS num_posts
                    FROM threads
                    JOIN users ON threads.author_id = users.id WHERE threads.anime_id = {}
                    LEFT JOIN posts ON posts.thread_id = threads.id
                    GROUP BY threads.id, users.id;
                    ", anime_id),
                    |(
                        id,
                        title,
                        author_id,
                        anime_id,
                        created_at,
                        username,
                        profile_url,
                        user_created_at,
                        num_posts,
                    )| Thread {
                        id,
                        title,
                        author_id,
                        anime_id,
                        created_at,
                        username, // Map username from users table
                        profile_url, // Map profile_url from users table
                        user_created_at,
                        num_posts, // Map the post count
                    }
                )
                .unwrap_or_else(|_| vec![]);
            Json(result)
        }
        Err(_) => Json(vec![]),
    }
}

#[derive(Serialize)]
struct Post {
    id: i32,
    content: String,
    author_id: i32,
    created_at: String,
    thread_id: i32,
    username: String, // New field for username
    profile_url: Option<String>, // New field for profile URL
}

#[get("/posts/<thread_id>")]
fn get_posts(thread_id: i32) -> Json<Vec<Post>> {
    match get_connection() {
        Ok(mut conn) => {
            let result: Vec<Post> = conn
                .query_map(
                    // Using JOIN to include data from the users table
                    format!(r"
                        SELECT posts.id, posts.content, posts.author_id, posts.created_at, posts.thread_id, users.username, users.profile_url
                        FROM posts
                        JOIN users ON posts.author_id = users.id
                        WHERE posts.thread_id = {}
                        ", thread_id),
                    |(id, content, author_id, created_at, thread_id, username, profile_url)| Post {
                        id,
                        content,
                        author_id,
                        created_at,
                        thread_id,
                        username, // Map the username
                        profile_url, // Map the profile URL
                    }
                )
                .unwrap_or_else(|_| vec![]);
            Json(result)
        }
        Err(_) => Json(vec![]),
    }
}

#[post("/register", data = "<user>")]
async fn register(user: Form<NewUser>) -> Json<SessionResponse> {
    let mut conn = get_connection().unwrap();

    // Insert the new user into the database
    let query =
        r"INSERT INTO users (username, password, profile_url) VALUES (:username, :password, :profile_url)";
    conn.exec_drop(
        query,
        params! {
        "username" => &user.username,
        "password" => &user.password, // Make sure to hash the password in a real-world app
        "profile_url" => user.profile_url.as_ref().unwrap_or(&String::new()),
    }
    ).unwrap();

    // Generate a session token
    let session_token = Uuid::new_v4().to_string();

    // Insert session for the user
    let insert_session_query =
        r"INSERT INTO sessions (user_id, session_token) VALUES ((SELECT id FROM users WHERE username = :username), :session_token)";
    conn.exec_drop(
        insert_session_query,
        params! {
        "username" => &user.username,
        "session_token" => &session_token,
    }
    ).unwrap();

    // Return the session token
    Json(SessionResponse { session_token })
}

#[derive(FromForm)]
struct LoginForm {
    username: String,
    password: String,
}

#[post("/login", data = "<login_form>")]
fn login(login_form: Form<LoginForm>) -> Json<Option<SessionResponse>> {
    match get_connection() {
        Ok(mut conn) => {
            let query = r"SELECT id FROM users WHERE username = :username AND password = :password";
            let user_id: Option<i32> = conn
                .exec_first(
                    query,
                    params! { "username" => &login_form.username, "password" => &login_form.password }
                )
                .unwrap_or(None);

            if let Some(user_id) = user_id {
                let session_token = Uuid::new_v4().to_string();
                let insert_session_query =
                    r"INSERT INTO sessions (user_id, session_token) VALUES (:user_id, :session_token)";
                conn.exec_drop(
                    insert_session_query,
                    params! { "user_id" => user_id, "session_token" => &session_token }
                ).unwrap();
                return Json(Some(SessionResponse { session_token }));
            }
            Json(None)
        }
        Err(_) => Json(None),
    }
}

#[derive(Serialize)]
struct User {
    id: i32,
    username: String,
    profile_url: Option<String>,
}

#[derive(FromForm)]
struct SessionForm {
    session_token: String,
}

#[post("/user", data = "<session>")]
fn get_user_by_session(session: Form<SessionForm>) -> Json<Option<User>> {
    match get_connection() {
        Ok(mut conn) => {
            let query =
                r"
                SELECT users.id, users.username, users.profile_url 
                FROM users 
                JOIN sessions ON users.id = sessions.user_id 
                WHERE sessions.session_token = :session_token
            ";
            let result: Vec<User> = conn
                .exec_map(
                    query,
                    params! { "session_token" => &session.session_token },
                    |(id, username, profile_url)| User { id, username, profile_url }
                )
                .unwrap_or_else(|_| vec![]);

            return Json(result.into_iter().next());
        }
        Err(_) => {
            return Json(None);
        }
    }
}

#[get("/")]
fn index() -> &'static str {
    match get_connection() {
        Ok(mut conn) => {
            // Test the connection with a simple query
            match conn.query_first::<String, _>("SELECT 'Connection successful!'") {
                Ok(_) => "Hello, world! Database connection successful!",
                Err(_) => "Hello, world! Query failed!",
            }
        }
        Err(_) => "Hello, world! Database connection failed!",
    }
}

#[derive(FromForm)]
struct NewThread {
    title: String,
    author_id: i32,
    anime_id: i32,
}

#[post("/thread", data = "<new_thread>")]
fn create_thread(new_thread: Form<NewThread>) -> Json<Option<i32>> {
    match get_connection() {
        Ok(mut conn) => {
            let insert_query =
                r"
                    INSERT INTO threads (title, author_id, anime_id, created_at) 
                    VALUES (:title, :author_id, :anime_id, NOW())
                ";
            conn.exec_drop(
                insert_query,
                params! {
                        "title" => &new_thread.title,
                        "author_id" => new_thread.author_id,
                        "anime_id" => &new_thread.anime_id,
                    }
            ).unwrap();

            let thread_id = conn.last_insert_id() as i32;
            return Json(Some(thread_id));
        }
        Err(_) => {
            return Json(None);
        }
    }
}

#[derive(FromForm)]
struct NewPost {
    content: String,
    author_id: i32,
    thread_id: i32,
}

#[post("/post", data = "<new_post>")]
fn create_post(new_post: Form<NewPost>) -> Json<Option<i32>> {
    match get_connection() {
        Ok(mut conn) => {
            let insert_query =
                r"
                    INSERT INTO posts (thread_id, author_id, content, created_at) 
                    VALUES (:thread_id, :author_id, :content, NOW())
                ";
            conn.exec_drop(
                insert_query,
                params! {
                        "content" => &new_post.content,
                        "author_id" => new_post.author_id,
                        "thread_id" => &new_post.thread_id,
                    }
            ).unwrap();

            let post_id = conn.last_insert_id() as i32;
            return Json(Some(post_id));
        }
        Err(_) => {
            return Json(None);
        }
    }
}

#[launch]
fn rocket() -> _ {
    rocket
        ::build()
        .configure(rocket::Config::figment().merge(("port", 25566)))
        .mount(
            "/",
            routes![
                index,
                login,
                get_threads,
                register,
                get_user_by_session,
                create_thread,
                create_post,
                get_posts,
                get_threads_by_anime
            ]
        )
}
