import { useEffect, useState } from "react";
import { invoke, InvokeArgs } from "@tauri-apps/api/core";

import { User, Program, Table, Row, SpaceDetails, Event, Uuid } from "@/types";
import { string } from "zod";

export interface SpaceParam {
  spaceId: Uuid;
}

export interface Pagination {
  offset?: number;
  limit?: number;
}

export interface ApiEnvelope<O> {
  isLoading: boolean;
  data?: O;
}

function ApiQueryFactory<I, O>(method_name: string): ((i: I) => ApiEnvelope<O>) {
  return function(input: I) {
    const [envelope, setEnvelope] = useState<ApiEnvelope<O>>({
      isLoading: true,
    });

    useEffect(() => {
      console.log(method_name, input);
      invoke<O>(method_name, input as InvokeArgs).then((res) => {
        setEnvelope({ isLoading: false, data: res });
      });
    }, []);

    return envelope;
  }
}

function ApiMutationFactory<I, O>(method_name: string): (() => (i: I) => Promise<O>) {
  return () => { 
    return function(input: I) {
        return invoke<O>(method_name, input as InvokeArgs)
    }
  }
}

export const useEventSearch = (spaceId: Uuid, query: string, offset: number, limit: number): ApiEnvelope<Event[]> => {
  const [envelope, setEnvelope] = useState<ApiEnvelope<Event[]>>({
    isLoading: true,
  });

  useEffect(() => {
    if (!query) {
      setEnvelope({ isLoading: false, data: [] });
      return
    }

    invoke("events_search", { spaceId, query, limit, offset }).then((res) => {
      setEnvelope({ isLoading: false, data: res as Event[] });
    });
  }, [spaceId, query, limit, offset]);

  return envelope;
}

export const useQuerySpace = ApiQueryFactory<SpaceParam, SpaceDetails>("current_space");
export const useQueryListSpaces = ApiQueryFactory<Pagination, [SpaceDetails]>("spaces_list");
export const useQueryUsers = ApiQueryFactory<SpaceParam & Pagination, [User]>("users_list");
export const useQueryPrograms = ApiQueryFactory<SpaceParam & Pagination, [Program]>("programs_list");
export const useQueryProgram = ApiQueryFactory<SpaceParam & { programId: Uuid }, Program>("program_get");
export const useQuerySecrets = ApiQueryFactory<SpaceParam & { programId: Uuid }, Record<string,string>>("secrets_get");
export const useMutationSetSecrets = ApiMutationFactory<SpaceParam & { programId: Uuid, secrets: Record<string, string> }, {}>("secrets_set");
export const useMutationRunProgram = ApiMutationFactory<SpaceParam & { author: string, programId: string, environment: Record<string,string> }, {}>("program_run");
export const useQueryTables = ApiQueryFactory<SpaceParam & Pagination, [Table]>("tables_list");
export const useQueryTable = ApiQueryFactory<SpaceParam & { table: string }, Table>("table_get");
export const useQueryRows = ApiQueryFactory<SpaceParam & { table: string } & Pagination, [Row]>("rows_query");
