
export interface Space {
  name: string;
}

export interface User {

}

export interface ProgramManifest {
  name: string,
  version: string,
  description?: string,
  homepage?: string,
  repository?: string,
  license?: string,
  main?: string,
}

export interface Program {
  id: string,
  createdAt: number,
  author: string,
  content: HashLink,
  manifest: ProgramManifest,
  html_index?: string,
  program_entry?: string,
}

export interface HashLink {
  hash: string;
  value?: any;
}

export interface Table {
  title: string;
  description: string;
  content: HashLink;
}

export interface Row {
  content: HashLink;
}

export type Tag = [string, string, string?];

export enum EventKind {
  MutateUser = 100000,
  DeleteUser = 100001,
  MutateSpace = 100002,
  DeleteSpace = 100003,
  MutateProgram = 100004,
  DeleteProgram = 100005,
  MutateSchema = 100006,
  DeleteSchema = 100007,
  MutateRow = 100008,
  DeleteRow = 100009,
}

export interface Event {
  id: string,
  pubkey: string,
  createdAt: number,
  kind: EventKind,
  tags: [Tag],
  content: HashLink,
}

export function schemaId(event: Event): string | undefined {
  const tag = event.tags.find(([tag]) => tag === SCHEMA_TAG);
  return tag && tag[1]
}

export function dataId(event: Event): string | undefined {
  const tag = event.tags.find(([tag]) => tag === ID_TAG);
  return tag && tag[1]
}

const SCHEMA_TAG = "sch";
const ID_TAG = "id";