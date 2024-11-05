declare module 'main' {
    export function main(): I32;
}

declare module 'extism:host' {
    interface user {
      event_create(ptr: I64, ptr: I64): I64;
      event_mutate(ptr: I64, ptr: I64, ptr: I64): I64;
    }
  }