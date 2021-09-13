use crate::{api, commands::Args, db::DB, schema::tags, Error};

use diesel::prelude::*;

use std::sync::Arc;

/// Remove a key value pair from the tags.  
pub async fn delete(args: Arc<Args>) -> Result<(), Error> {
    let conn = DB.get()?;
    let key = args
        .params
        .get("key")
        .ok_or("Unable to retrieve param: key")?;

    match diesel::delete(tags::table.filter(tags::key.eq(key))).execute(&conn) {
        Ok(_) => {
            args.msg.react(&args.cx, '✅').await?;
        }
        Err(_) => {
            api::send_reply(
                args.clone(),
                "A database error occurred when deleting the tag.",
            )
            .await?
        }
    }
    Ok(())
}

/// Add a key value pair to the tags.  
pub async fn post(args: Arc<Args>) -> Result<(), Error> {
    let conn = DB.get()?;

    let key = args
        .params
        .get("key")
        .ok_or("Unable to retrieve param: key")?;

    let value = args
        .params
        .get("value")
        .ok_or("Unable to retrieve param: value")?;

    match diesel::insert_into(tags::table)
        .values((tags::key.eq(key), tags::value.eq(value)))
        .execute(&conn)
    {
        Ok(_) => {
            args.msg.react(&args.cx, '✅').await?;
        }
        Err(_) => {
            api::send_reply(
                args.clone(),
                "A database error occurred when creating the tag.",
            )
            .await?
        }
    }
    Ok(())
}

/// Update an existing tag.
pub async fn update(args: Arc<Args>) -> Result<(), Error> {
    let conn = DB.get()?;

    let key = args
        .params
        .get("key")
        .ok_or("Unable to retrieve param: key")?;

    let value = args
        .params
        .get("value")
        .ok_or("Unable to retrieve param: value")?;

    match diesel::update(tags::table.filter(tags::key.eq(key)))
        .set(tags::value.eq(value))
        .execute(&conn)
    {
        Ok(_) => {
            args.msg.react(&args.cx, '✅').await?;
        }
        Err(_) => {
            api::send_reply(
                args.clone(),
                "A database error occurred when updating the tag.",
            )
            .await?
        }
    }

    Ok(())
}

/// Retrieve a value by key from the tags.  
pub async fn get(args: Arc<Args>) -> Result<(), Error> {
    let conn = DB.get()?;

    let key = args.params.get("key").ok_or("unable to read params")?;

    let results = tags::table
        .filter(tags::key.eq(key))
        .load::<(i32, String, String)>(&conn)?;

    if results.is_empty() {
        api::send_reply(args.clone(), &format!("Tag not found for `{}`", key)).await?;
    } else {
        api::send_reply(args.clone(), &results[0].2).await?;
    }

    Ok(())
}

/// Retrieve all tags
pub async fn get_all(args: Arc<Args>) -> Result<(), Error> {
    let conn = DB.get()?;

    let results = tags::table.load::<(i32, String, String)>(&conn)?;

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
