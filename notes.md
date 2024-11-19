
HashSequences form a really nice substrate for places where we'd normally use merkle-backpointers
* using public key infra maintains that trust derives from trust in the signer
* we know merkle-backpointers end up leading to 
* just like "everything needs a version number field", a 2-element hash sequence has the advantage of binding metadata to the principle object
* making statements about things that are "historically related" is as simple as expanding the collection
* mutation doesn't affect principle data, which means we can re-order and omit history without affecting hashes
* because we can omit things, we can give different collections to different users based on histories
* we should be able to get 


```rust
struct Event {
  id: Sha512Hash,
  pubkey: Ed25519PublicKey
  kind: 10000 // nostr "write structured data" kind
  created_at: DateTime,
  content: [
    schema: Blake3Hash, // HashSequence of schema, the "table" this data belongs to
    id: Uuid,           // identifier for this datum. Not editable in userland beyond create / delete
    value: Blake3Hash,  // Hash of serialized content. History is formed by the event stream itself, last writer wins, delete writes form tombstones
  ]
  tags: Vec<(String,String,Option<String>) // relations
  sig: Ed25519Signature
}
```

```rust
struct Event {
  // meta
  id: Sha512Hash,
  pubkey: Ed25519PublicKey
  created_at: u32 // timestamp
  kind: 10000 // nostr "structured data" kind
  sig: Ed25519Signature

  // links
  schema: Blake3Hash // HashSequence of schema, the "table" this data belongs to
  content: Blake3Hash // HashSequence of content, the "row"
  references: Blake3Hash // HashSequence of references
}
```

```
Events [
  meta: [[ timestamp, author, kind, pubkey, created_at, sig ]]
  Event... [
    meta: [ id ]
    Schema [
      meta: 
    ]
    Content [

    ]
    References [

    ]
  ]
]
```


```json
{
  "id": "4376c65d2f232afbe9b882a35baa4f6fe8667c4e684749af565f981833ed6a65",
  "pubkey": "6e468422dfb74a5738702a8823b9b28168abab8655faacb6853cd0ee15deee93",
  "created_at": 1673347337,
  "kind": 1,
  "content": "Walled gardens became prisons, and nostr is the first step towards tearing down the prison walls.",
  "tags": [
    ["e", "3da979448d9ba263864c4d6f14984c423a3838364ec255f03c7904b1ae77f206"],
    ["p", "bf2376e17ba4ec269d10fcc996a4746b451152be9031fa48e74553dde5526bce"]
  ],
  "sig": "908a15e46fb4d8675bab026fc230a0e3542bfade63da02d542fb78b2a8513fcd0092619a2c8c1221e581946e0191f2af505dfdf8657a414dbca329186f009262"
}
```

* The Nostr Protocol event definition is 90% metadata, so move most of it into the "meta" section of the collection
* Bots write to schemas
* App data is the union of all schemas in the the event stream
  * a context is a single author, plus a set of remote authors



```
bot [
  meta: [[ name, version, url ]]
  binaries... []
]
```


## Data Table
* We need identifiers for datums, so we can track histories, these need to go into the event stream

Here's what a structured data event should look like:

```rust
struct Event {
  id: 
}

```

## Programs
* programs can be loaded from URLs that respond to an HTTP GET request with a blob ticket that points to an iroh collection
* program collections must contain the following files:
```
datalayer.json -- manifest of info about the program, and test

```
* a program MAY contain an `index.html`, which is interpreted to be the entry point for the program
* the resulting program will have

* response string of an HTTP get to `/datalayer` should be a blob ticket for the program
* 

## Capabilities

https://github.com/ucan-wg/spec?tab=readme-ov-file#command

* Capabilities table determines what programs are installed
  * installation = any capabilitiy issued for the program identifier

* Command Space:
```js
"/"                  // 
"/evt/"
"/evt/read"
"/evt/write"
"/evt/schema/"
"/evt/schema/read"
"/evt/schema/write"
"/evt/schema/list"

"/exe" // execute 
// "/exe/html/"
// "/exe/evt/"
// "/exe/evt/read"
// "/exe/evt/write"
// "/exe/evt/schema/"
// "/exe/evt/schema/read"
// "/exe/evt/schema/write"
// "/exe/evt/schema/list"

// "/prg/"
// "/exe/evt/"
// "/exe/evt/read"
// "/exe/evt/write"
```

* `/exe` - execute command policy options


* `HTTP` make HTTP requests
  * refinement by verb set: {`*`, `GET`, `PUT`, `POST`, `DELETE`, etc. }
* execute in the background
  * refine by resource characteristics, memory, timeouts, etc.
* display UI
* read / write / list events
  * usually scoped to a schema, or set of schemas
* self-update


* read user profile information
* run a program
  * make HTTP calls within a program
    * make HTTP {POST,GET,PUT,POST} calls within a program
  * read (and list) events
  * write events

A user Alice adding a program "TicTacToe" will look like this:

| field              | value                |
| ------------------ | -------------------- |
| `iss`: issuer      | `AliceKey`           |
| `aud`: audience    | `TicTacToeKey`       |
| `sub`: subject     | `TicTacToeProgramID` |
| `cmd`: command     | `"/exe"`         |
| `pol`: policy      | `[]` |


# Standard Schemas:
Datalayer comes with a few *required* schemas that allow it to operate

* Programs:

```
{
  "type": "array",
  "prefixItems": [

  ]
}
```

* Configurations
* 