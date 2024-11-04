


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
  kind: 10000 // nostr "structured data" kind
  schema: Blake3Hash // HashSequence of schema, the "table" this data belongs to
  content: Blake3Hash // HashSequence of content, the "row"
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
    meta: [ ]
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