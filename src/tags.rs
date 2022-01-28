use crate::{api, commands::Args, Error};

use std::sync::Arc;

/// Remove a key value pair from the tags.  
pub async fn delete(args: Arc<Args>) -> Result<(), Error> {
    let key = args
        .params
        .get("key")
        .ok_or("Unable to retrieve param: key")?;

    let query = sqlx::query("delete from tags where key = $1")
        .bind(key)
        .execute(&*args.clone().db)
        .await?;

    match query.rows_affected() {
        0 => {
            api::send_reply(
                args.clone(),
                "A database error occurred when deleting the tag.",
            )
            .await?;
        }
        _ => {
            args.msg.react(&args.cx, '✅').await?;
        }
    }

    Ok(())
}

/// Add a key value pair to the tags.  
pub async fn post(args: Arc<Args>) -> Result<(), Error> {
    let key = args
        .params
        .get("key")
        .ok_or("Unable to retrieve param: key")?;

    let value = args
        .params
        .get("value")
        .ok_or("Unable to retrieve param: value")?;

    let query = sqlx::query("insert into tags(key, value) values ($1, $2)")
        .bind(key)
        .bind(value)
        .execute(&*args.clone().db)
        .await?;

    match query.rows_affected() {
        0 => {
            api::send_reply(
                args.clone(),
                "A database error occurred when creating the tag.",
            )
            .await?
        }
        _ => {
            args.msg.react(&args.cx, '✅').await?;
        }
    }
    Ok(())
}

/// Update an existing tag.
pub async fn update(args: Arc<Args>) -> Result<(), Error> {
    let key = args
        .params
        .get("key")
        .ok_or("Unable to retrieve param: key")?;

    let value = args
        .params
        .get("value")
        .ok_or("Unable to retrieve param: value")?;

    let query = sqlx::query("update tags set value = $1 where key = $2")
        .bind(value)
        .bind(key)
        .execute(&*args.clone().db)
        .await?;

    match query.rows_affected() {
        0 => {
            api::send_reply(
                args.clone(),
                "A database error occurred when updating the tag.",
            )
            .await?
        }
        _ => {
            args.msg.react(&args.cx, '✅').await?;
        }
    }

    Ok(())
}

/// Retrieve a value by key from the tags.
pub async fn get(args: Arc<Args>) -> Result<(), Error> {
    let key = args.params.get("key").ok_or("unable to read params")?;

    let results: Option<(i32, String, String)> =
        sqlx::query_as("select * from tags where key = $1 limit 1")
            .bind(key)
            .fetch_optional(&*args.db)
            .await?;

    if let Some(query_result) = results {
        api::send_reply(args.clone(), &query_result.2).await?;
    } else {
        api::send_reply(args.clone(), &format!("Tag not found for `{}`", key)).await?;
    }

    Ok(())
}

/// Retrieve all tags
pub async fn get_all(args: Arc<Args>) -> Result<(), Error> {
    let results: Vec<(i32, String, String)> = sqlx::query_as("select * from tags")
        .fetch_all(&*args.db)
        .await?;

    if results.is_empty() {
        api::send_reply(args.clone(), "No tags found").await?;
    } else {
        let tags = &results.iter().fold(String::new(), |prev, row| {
            if prev.len() < 1980 {
                prev + &row.1 + "\n"
            } else {
                prev
            }
        });

        api::send_reply(args.clone(), &format!("All tags: ```\n{}```", &tags)).await?;
    }

    Ok(())
}

/// Print the help message
pub async fn help(args: Arc<Args>) -> Result<(), Error> {
    let help_string = "```
?tags create {key} value...     Create a tag.  Limited to WG & Teams.
?tags update {key} value...     Update a tag.  Limited to WG & Teams.
?tags delete {key}              Delete a tag.  Limited to WG & Teams.
?tags help                      This menu.
?tags                           Get all the tags.
?tag {key}                      Get a specific tag.
```";
    api::send_reply(args.clone(), &help_string).await?;
    Ok(())
}
