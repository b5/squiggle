
# Squiggle
A hackable event model with an airtable-like app

There are only two types of data within the system:

## Events
All mutations in the system are modeled as events. Events are organized into related streams.

### Capabilities
Permissions are given out through 


## Notable Technologies used

* "Accounts" - Public Key
* Capabilities - permissions
* Spaces - local replica gets an sqlite db
  * Users - Metadata on a public Key
  * Programs - a WASM executable that creates events
    `content` - `HashSeq`
  * Tables - JSON Schemas
  * Rows - Datums
* VM - run a program in a space


```
[[...], [...], [...]]
```

```
[{}, {}, {}]
```