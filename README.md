# Discord Mods Bot
A discord bot written in rust.  

### Features
The following commands are currently supported by the bot

#### Tags
Tags are a simple key value store.  

Command | Description
--- | ---
?tags create {key} value...   | Create a tag.  Limited to WG & Teams. 
?tags delete {key}            | Delete a tag.  Limited to WG & Teams.
?tags help                    | This menu.
?tags                         | Get all the tags.
?tag {key}                    | Get a specific tag.

### Crates
Search for a crate on crates.io
```
?crate query...
```
Retreive documentation for a crate
```
?docs query...
```

### Ban
Ban a user
```
?ban {user}

```
### Kick
Kick a user
```
?kick {user}
```
### Slowmode
Set slowmode for a channel.  0 seconds disables slowmode.  
```
?slowmode {channel} {seconds}
```

### Code of conduct welcome message
Sets up the code of conduct message with reaction in the specified channel.
Used for assigning talk roles.  
```
?CoC {channel}
```
# [Getting Started](GETTING_STARTED.md)
# [Commands](COMMANDS.md)

