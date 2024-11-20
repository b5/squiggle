import { useEffect, useState } from "react";
import { invoke, InvokeArgs } from "@tauri-apps/api/core";

import { User, Program, Schema, Row, Space, Event } from "@/types";

export interface SpaceParam {
  space: string;
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

export const useEventSearch = (space: string, query: string, offset: number, limit: number): ApiEnvelope<Event[]> => {
  const [envelope, setEnvelope] = useState<ApiEnvelope<Event[]>>({
    isLoading: true,
  });

  useEffect(() => {
    if (!query) {
      setEnvelope({ isLoading: false, data: [] });
      return
    }

    invoke("events_search", { space, query, limit, offset }).then((res) => {
      console.log(res);
      setEnvelope({ isLoading: false, data: res as Event[] });
    });
  }, [space, query, limit, offset]);

  return envelope;
}

export const useListSpaces = ApiQueryFactory<Pagination, [Space]>("spaces_list");
export const useQueryUsers = ApiQueryFactory<SpaceParam & Pagination, [User]>("users_list");
export const useQueryPrograms = ApiQueryFactory<SpaceParam & Pagination, [Program]>("programs_list");
export const useQueryProgram = ApiQueryFactory<SpaceParam & { programId: string }, Program>("program_get");
export const useRunProgramMutation = ApiMutationFactory<SpaceParam & { author: string, programId: string, environment: Record<string,string> }, {}>("program_run");
export const useQueryTables = ApiQueryFactory<SpaceParam & Pagination, [Schema]>("tables_list");
export const useQueryTable = ApiQueryFactory<SpaceParam & { table: string }, Schema>("table_get");
export const useQueryRows = ApiQueryFactory<SpaceParam & { table: string } & Pagination, [Row]>("rows_query");
