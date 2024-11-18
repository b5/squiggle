import { useEffect, useState } from "react";
import { invoke, InvokeArgs } from "@tauri-apps/api/core";

import { User, Program, Schema, Row } from "@/types";

export interface Pagination {
  offset?: number;
  limit?: number;
}

export interface ApiEnvelope<O> {
  isLoading: boolean;
  data?: O;
}

function ApiFactory<I, O>(method_name: string): ((i: I) => ApiEnvelope<O>) {
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

export const useQueryUser = ApiFactory<Pagination, [User]>("users_list");
export const useQueryPrograms = ApiFactory<Pagination, [Program]>("programs_list");
export const useRunProgram = ApiFactory<{ id: string }, Program>("programs_run");
export const useQuerySchemas = ApiFactory<Pagination, [Schema]>("schemas_list");
export const useQuerySchema = ApiFactory<{ schema: string }, Schema>("schemas_get");
export const useQueryRows = ApiFactory<{ schema: string } & Pagination, [Row]>("rows_query");
