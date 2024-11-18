const { print, sleep: timeout, event_create, event_mutate, event_query, schema_load_or_create } = Host.getFunctions();

export function log(s: string) {
  const mem = Memory.fromString(s);
  print(mem.offset);
  mem.free();
}

export function sleep(milliseconds: number) {
  timeout(milliseconds);
}

export interface Schema {
  title: string;
  hash: string;
  content: string | {
    hash: string;
    value?: any;
  };
  data: any;
}

export function loadOrCreateSchema(schema: any): Schema {
  const s = Memory.fromJsonObject(schema);
  let offset = schema_load_or_create(s.offset);
  s.free();
  return Memory.find(offset).readJsonObject();
}

export function query(schema: Schema, query: string): Entry[] {
  let hash = (typeof schema.content === 'string') ? schema.content : schema.content.hash;
  const s = Memory.fromString(hash);
  const q = Memory.fromString(query);
  const offset = event_query(s.offset, q.offset);
  s.free();
  q.free();
  return Memory.find(offset).readJsonObject();
}

export interface Entry {
  id: string;
  schema: string;
  data: any;
}

export function addEntry(schema: Schema, entry: any): Entry {
  let hash = (typeof schema.content === 'string') ? schema.content : schema.content.hash;
  const s = Memory.fromString(hash);
  const d = Memory.fromJsonObject(entry);
  const offset = event_create(s.offset, d.offset);
  s.free();
  d.free();
  return Memory.find(offset).readJsonObject();
}

export function updateEntry(schema: Schema, id: string, entry: any): Entry {
  let hash = (typeof schema.content === 'string') ? schema.content : schema.content.hash;
  const s = Memory.fromString(hash);
  const i = Memory.fromString(id);
  const d = Memory.fromJsonObject(entry);
  const offset = event_mutate(s.offset, i.offset, d.offset);
  s.free();
  i.free();
  d.free();
  return Memory.find(offset).readJsonObject();
}