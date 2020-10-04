# Discord Mods Bot
A discord bot written in rust.  

### Features
The following commands are currently supported by the bot

#### Tags
Tags are a simple key value store.  

The `create` and `delete` tag commands are limited to users with the WG & Teams
role.  

Create a new tag.
```
?tags create {key} value...
```

Delete a tag. 
```
?tags delete {key}
```

The next two commands can be used to lookup tags.  

Get a specific tag.
```
?tag {key}
```

Get all the tags.
```
?tags
```

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

