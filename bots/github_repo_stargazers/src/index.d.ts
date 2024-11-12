declare module 'main' {
    export function main(): PTR;
}

declare module 'extism:host' {
    interface user {
      print(ptr: I64);
      sleep(milliseconds: I64);
      schema_load_or_create(ptr: I64): I64;
      event_create(ptr: I64, ptr: I64): I64;
      event_mutate(ptr: I64, ptr: I64, ptr: I64): I64;
      event_query(ptr: I64, ptr: I64): I64;
    }
  }