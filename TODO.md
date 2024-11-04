
* [x] Sqlite Events Table
* [x] VM Compiles 
* [x] list accounts on frontend
* [ ] Automation 1
  * [x] Hello-world WASM Execution
  * [x] Tasks & Jobs
  * [x] Wire up job kickoff to the frontend
  * [x] interface to write blobs (re-using fog uploads dir)
  * [ ] write a task that creates one or more blobs
* App Detour 1:
  * [ ] build out project scores database:
    * [ ] encode existing notion table as jsonschema:
      * can we just use notion's schema for this?
      * let's start with some simple illustration that adheres to jsonschema
    * [ ] add schema as a collection: 1th blob is schema, prior schemas are 2-Nth blobs
    * [ ] add a single event for every entry
* [ ] Events 1
  * [ ] Jobs are associated with accounts
  * [ ] create event on job run
  * [ ] interface to write events
  * [ ] written from a job get a collection
* [ ] Data Modeling 1
  * [ ] URL-as-anchor datums
  * [ ] Mechanism for Grouping Datums
  * [ ] People-Specific Data Modeling
    * [ ] Github User Data Example
    * [ ] Twitter User Data Example
  * [ ] Reduce function that constructs account context?
* [ ] Sync 1
  * Connect & Sync All events
* [ ] Search Index
* [ ] Automation 2
  * [ ] List Bots on Frontend
  * [ ] List Jobs on Frontend
  * [ ] Run user-selectable Job from Frontend
* [ ] Multi-tenancy
  * [ ] Account Creation
  * [ ] 
* [ ] Access Control 1
  * [ ] 
* [ ] Blob Persistence
* [ ] Secrets for Bots